//! The frame loop: pump messages, react to tray and hotkeys, step the world, draw.

use std::time::{Duration, Instant};

use snow_core::{FrameInput, Prim, SurfaceIn, World};
use windows::core::w;
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::AppSettings;
use crate::gfx::Gfx;
use crate::overlay::{self, Event, Overlay};
use crate::tray::{Command, MenuState, Tray};
use crate::{hotkeys, pacing, system, tracker};

const FULLSCREEN_POLL: Duration = Duration::from_secs(1);
const BATTERY_POLL: Duration = Duration::from_secs(10);
const TOPMOST_POLL: Duration = Duration::from_secs(2);
const TRACKER_POLL: Duration = Duration::from_millis(250);
/// How long to sleep between checks while nothing needs drawing.
const IDLE_WAIT_MS: u32 = 250;

fn seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
}

fn load_settings() -> AppSettings {
    system::settings_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| AppSettings::parse(&t)).unwrap_or_default()
}

fn save_settings(settings: &AppSettings) {
    if let Some(path) = system::settings_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, settings.to_json());
    }
}

/// Sleep until a message arrives or `ms` pass.
fn wait(ms: u32) {
    unsafe {
        MsgWaitForMultipleObjects(None, false, ms, QS_ALLINPUT);
    }
}

struct App {
    overlay: Overlay,
    tray: Tray,
    gfx: Option<Gfx>,
    world: World,
    settings: AppSettings,
    play: bool,
    locked: bool,
    fullscreen: bool,
    on_battery: bool,
    quit: bool,
}

impl App {
    /// The overlay is hidden and the simulation frozen.
    fn hidden(&self) -> bool {
        self.settings.paused || self.locked || self.fullscreen
    }

    fn apply_visibility(&mut self) {
        let hidden = self.hidden();
        if hidden {
            self.set_play(false);
        }
        self.overlay.set_visible(!hidden);
    }

    fn set_play(&mut self, on: bool) {
        let on = on && !self.hidden();
        if on == self.play {
            return;
        }
        self.play = on;
        if !on {
            // Do not keep the mouse if play mode ended in the middle of a drag.
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
            }
        }
        overlay::reset_pointer();
        self.overlay.set_click_through(!on);
        hotkeys::set_escape(on);
    }

    fn changed(&mut self) {
        save_settings(&self.settings);
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::SetIntensity(v) => {
                self.settings.intensity = v;
                self.changed();
            }
            Command::LightsMode(mode) => {
                self.settings.lights_mode = mode.to_string();
                self.changed();
            }
            Command::NextPattern => {
                self.settings.next_pattern();
                self.changed();
            }
            Command::CallBlower => self.world.call_blower(),
            Command::ClearSnow => self.world.clear_snow(),
            Command::TogglePlay => self.set_play(!self.play),
            Command::ToggleCapture => {
                self.settings.show_in_capture = !self.settings.show_in_capture;
                self.overlay.set_capture_excluded(!self.settings.show_in_capture);
                self.changed();
            }
            Command::ToggleAutostart => system::set_autostart(!system::autostart_enabled()),
            Command::TogglePause => {
                self.settings.paused = !self.settings.paused;
                self.apply_visibility();
                self.changed();
            }
            Command::ToggleHotkeys => {
                self.settings.hotkeys = !self.settings.hotkeys;
                if self.settings.hotkeys {
                    hotkeys::register_all();
                } else {
                    hotkeys::unregister_all();
                }
                self.changed();
            }
            Command::Exit => self.quit = true,
        }
    }

    fn hotkey(&mut self, id: i32) {
        match id {
            hotkeys::PLAY => self.set_play(!self.play),
            hotkeys::ESCAPE => self.set_play(false),
            hotkeys::UP => {
                self.settings.step_intensity(0.1);
                self.changed();
            }
            hotkeys::DOWN => {
                self.settings.step_intensity(-0.1);
                self.changed();
            }
            hotkeys::PAUSE => self.command(Command::TogglePause),
            _ => {}
        }
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::DisplayChange => {
                self.overlay.refit();
                self.gfx = None;
            }
            Event::TrayClick => {
                let state = MenuState { settings: &self.settings, play: self.play, autostart: system::autostart_enabled() };
                if let Some(command) = self.tray.show_menu(&state) {
                    self.command(command);
                }
            }
            Event::TaskbarCreated => self.tray.add(),
            Event::SessionLocked(locked) => {
                self.locked = locked;
                self.apply_visibility();
            }
        }
    }

    /// Dispatch pending window messages. Returns false once the app should quit.
    fn pump(&mut self) -> bool {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                match msg.message {
                    WM_QUIT => return false,
                    WM_HOTKEY => self.hotkey(msg.wParam.0 as i32),
                    _ => {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
        }
        for event in overlay::drain_events() {
            self.event(event);
        }
        !self.quit
    }

    /// Draw one frame. Returns false on failure; the device is then dropped
    /// and rebuilt on the next call.
    fn render(&mut self, prims: &[Prim]) -> bool {
        if self.gfx.is_none() {
            self.gfx = Gfx::new(self.overlay.hwnd, self.overlay.width, self.overlay.height).ok();
        }
        // Frame rate is set by the limiter in the loop, not by the display.
        match self.gfx.as_mut().map(|g| g.render(prims, 1)) {
            Some(Ok(())) => true,
            _ => {
                self.gfx = None;
                wait(500);
                false
            }
        }
    }
}

pub fn run() {
    unsafe {
        // One instance only: a second launch exits quietly.
        let _mutex = CreateMutexW(None, true, w!("Local\\itSnows"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let Ok(overlay) = Overlay::create() else {
        system::fatal("itSnows could not create its overlay window.");
        return;
    };
    let Ok(tray) = Tray::create() else {
        system::fatal("itSnows could not create its notification-area icon.");
        return;
    };
    overlay::set_taskbar_created_message(tray.taskbar_created);
    system::watch_session(tray.hwnd);
    let settings = load_settings();
    if settings.hotkeys {
        hotkeys::register_all();
    }

    let mut app = App {
        overlay,
        tray,
        gfx: None,
        world: World::new(seed()),
        settings,
        play: false,
        locked: false,
        fullscreen: system::should_hide(),
        on_battery: system::on_battery(),
        quit: false,
    };
    app.overlay.set_capture_excluded(!app.settings.show_in_capture);
    app.apply_visibility();

    let mut prims: Vec<Prim> = Vec::new();
    // What is on screen now, so an unchanged idle frame is not drawn again.
    let mut shown: Vec<Prim> = Vec::new();
    let mut shown_valid = false;
    let mut last = Instant::now();
    let mut fullscreen_checked = last;
    let mut battery_checked = last;
    let mut topmost_checked = last;
    let mut tracker_checked = last - TRACKER_POLL;
    let mut surfaces: Vec<SurfaceIn> = Vec::new();

    while app.pump() {
        let now = Instant::now();
        if now - fullscreen_checked >= FULLSCREEN_POLL {
            fullscreen_checked = now;
            let fullscreen = system::should_hide();
            if fullscreen != app.fullscreen {
                app.fullscreen = fullscreen;
                app.apply_visibility();
            }
        }
        if now - battery_checked >= BATTERY_POLL {
            battery_checked = now;
            app.on_battery = system::on_battery();
        }

        if app.hidden() {
            last = now;
            shown_valid = false;
            wait(IDLE_WAIT_MS);
            continue;
        }

        if now - topmost_checked >= TOPMOST_POLL {
            topmost_checked = now;
            // Stay above the taskbar, which is top-most too.
            unsafe {
                let _ = SetWindowPos(app.overlay.hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
        }

        if now - tracker_checked >= TRACKER_POLL {
            tracker_checked = now;
            let (screen_w, screen_h) = overlay::primary_size();
            surfaces = tracker::snapshot(app.overlay.hwnd, screen_w as f32, screen_h as f32);
        }

        let dt = (now - last).as_secs_f32();
        last = now;
        let core_settings = app.settings.to_core();
        let out = app.world.step(&FrameInput {
            dt,
            width: app.overlay.width as f32,
            height: app.overlay.height as f32,
            surfaces: &surfaces,
            settings: &core_settings,
            pointer: app.play.then(overlay::pointer),
        });
        if out.exit_play {
            app.set_play(false);
        }

        prims.clear();
        app.world.draw(&mut prims);
        if app.gfx.is_none() {
            shown_valid = false;
        }
        if !shown_valid || pacing::should_present(out.idle, &prims, &shown) {
            shown_valid = app.render(&prims);
            if out.idle {
                shown.clone_from(&prims);
            }
        }
        if out.idle {
            // Nothing is moving: look again a few times a second, and draw
            // only if standing things (lights, snowmen) moved with the desktop.
            wait(IDLE_WAIT_MS);
        } else {
            shown_valid = false;
            // Cap the frame rate whatever the display's refresh rate is, and
            // never spin if presenting returns at once (display off).
            let pause = pacing::frame_wait_ms(now.elapsed().as_secs_f32(), app.on_battery);
            if pause > 0 {
                wait(pause);
            }
        }
    }

    hotkeys::unregister_all();
    hotkeys::set_escape(false);
}
