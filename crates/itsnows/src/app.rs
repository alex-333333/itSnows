//! The frame loop: pump messages, step the world, draw.

use std::time::Instant;

use snow_core::{FrameInput, LightsMode, LightsPattern, Prim, Settings, World};
use windows::core::w;
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::gfx::Gfx;
use crate::overlay::{self, Event, Overlay};

fn seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
}

/// Dispatch pending window messages. Returns false once the app should quit.
fn pump() -> bool {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_QUIT {
                return false;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    true
}

pub fn run() {
    unsafe {
        // One instance only: a second launch exits quietly.
        let _mutex = CreateMutexW(None, true, w!("Local\\itSnows"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let Ok(mut overlay) = Overlay::create() else { return };
        let mut gfx = Gfx::new(overlay.hwnd, overlay.width, overlay.height).ok();

        let mut world = World::new(seed());
        let settings = Settings { intensity: 0.5, lights_mode: LightsMode::Off, lights_pattern: LightsPattern::Twinkle };
        let mut prims: Vec<Prim> = Vec::new();
        let mut last = Instant::now();

        while pump() {
            for event in overlay::drain_events() {
                match event {
                    Event::DisplayChange => {
                        overlay.refit();
                        gfx = None;
                    }
                }
            }

            let now = Instant::now();
            let dt = (now - last).as_secs_f32();
            last = now;

            world.step(&FrameInput {
                dt,
                width: overlay.width as f32,
                height: overlay.height as f32,
                surfaces: &[],
                settings: &settings,
                pointer: None,
            });
            prims.clear();
            world.draw(&mut prims);

            if gfx.is_none() {
                gfx = Gfx::new(overlay.hwnd, overlay.width, overlay.height).ok();
            }
            match gfx.as_mut().map(|g| g.render(&prims, 1)) {
                Some(Ok(())) => {}
                // Device lost, or no device yet: drop it and retry shortly
                // rather than spinning.
                _ => {
                    gfx = None;
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            }
        }
    }
}
