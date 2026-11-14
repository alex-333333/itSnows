//! Notification-area icon and its menu. The icon belongs to a hidden helper
//! window, so opening the menu never touches the overlay's no-focus state.

use windows::core::{w, Result, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::{AppSettings, LIGHTS_MODES, PRESETS};
use crate::overlay::wndproc;

/// Message the shell sends to the helper window for clicks on the icon.
pub const WM_TRAY: u32 = WM_APP + 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    SetIntensity(f32),
    LightsMode(&'static str),
    NextPattern,
    CallBlower,
    ClearSnow,
    TogglePlay,
    ToggleCapture,
    ToggleAutostart,
    TogglePause,
    Exit,
}

const ID_PRESET: usize = 100;
const ID_LIGHTS: usize = 110;
const ID_PATTERN: usize = 115;
const ID_BLOWER: usize = 120;
const ID_CLEAR: usize = 121;
const ID_PLAY: usize = 122;
const ID_CAPTURE: usize = 123;
const ID_AUTOSTART: usize = 124;
const ID_PAUSE: usize = 125;
const ID_EXIT: usize = 126;

pub struct MenuState<'a> {
    pub settings: &'a AppSettings,
    pub play: bool,
    pub autostart: bool,
}

pub struct Tray {
    pub hwnd: HWND,
    icon: HICON,
    /// Broadcast by Explorer when the taskbar is (re)created; the icon must be added again.
    pub taskbar_created: u32,
}

/// 32×32 BGRA pixels of a white six-armed snowflake on transparent.
fn snowflake_pixels() -> Vec<u8> {
    const N: usize = 32;
    let mut px = vec![0u8; N * N * 4];
    let c = (N as f32 - 1.0) / 2.0;
    let dist_to_segment = |x: f32, y: f32, ax: f32, ay: f32, bx: f32, by: f32| {
        let (dx, dy) = (bx - ax, by - ay);
        let t = (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        ((x - ax - t * dx).powi(2) + (y - ay - t * dy).powi(2)).sqrt()
    };
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32, y as f32);
            let mut d = f32::MAX;
            for arm in 0..6 {
                let a = arm as f32 * std::f32::consts::FRAC_PI_3 + std::f32::consts::FRAC_PI_2;
                let (ux, uy) = (a.cos(), a.sin());
                d = d.min(dist_to_segment(fx, fy, c, c, c + ux * 14.0, c + uy * 14.0));
                // Two short barbs two thirds of the way out along each arm.
                let (bx, by) = (c + ux * 9.0, c + uy * 9.0);
                for side in [-1.0f32, 1.0] {
                    let b = a + side * std::f32::consts::FRAC_PI_3;
                    d = d.min(dist_to_segment(fx, fy, bx, by, bx + b.cos() * 4.5, by + b.sin() * 4.5));
                }
            }
            let alpha = ((1.9 - d).clamp(0.0, 1.0) * 255.0) as u8;
            let i = (y * N + x) * 4;
            // Premultiplied white.
            px[i..i + 4].copy_from_slice(&[alpha, alpha, alpha, alpha]);
        }
    }
    px
}

unsafe fn make_icon() -> Result<HICON> {
    let pixels = snowflake_pixels();
    let color = CreateBitmap(32, 32, 1, 32, Some(pixels.as_ptr() as *const _));
    let mask = CreateBitmap(32, 32, 1, 1, None);
    let info = ICONINFO { fIcon: true.into(), xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
    let icon = CreateIconIndirect(&info);
    let _ = DeleteObject(color.into());
    let _ = DeleteObject(mask.into());
    icon
}

impl Tray {
    pub fn create() -> Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = w!("itSnowsTray");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: class,
                ..Default::default()
            };
            RegisterClassExW(&wc);
            // A real (never shown) top-level window: message-only windows do
            // not receive the TaskbarCreated broadcast.
            let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, class, w!("itSnows"), WS_POPUP, 0, 0, 0, 0, None, None, Some(instance.into()), None)?;
            let tray = Self { hwnd, icon: make_icon()?, taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")) };
            tray.add();
            Ok(tray)
        }
    }

    fn data(&self) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: self.icon,
            ..Default::default()
        };
        for (dst, src) in data.szTip.iter_mut().zip("itSnows".encode_utf16()) {
            *dst = src;
        }
        data
    }

    pub fn add(&self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &self.data());
        }
    }

    pub fn remove(&self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.data());
        }
    }

    /// Show the menu at the cursor and return what was picked, if anything.
    /// Blocks while the menu is open.
    pub fn show_menu(&self, state: &MenuState) -> Option<Command> {
        unsafe {
            let menu = CreatePopupMenu().ok()?;
            let check = |on: bool| if on { MF_CHECKED } else { MF_UNCHECKED };
            let item = |m: HMENU, id: usize, text: &str, checked: bool| {
                let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = AppendMenuW(m, MF_STRING | check(checked), id, PCWSTR(wide.as_ptr()));
            };
            let separator = |m: HMENU| {
                let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
            };

            let s = state.settings;
            if let Ok(sub) = CreatePopupMenu() {
                for (i, (name, value)) in PRESETS.iter().enumerate() {
                    item(sub, ID_PRESET + i, name, (s.intensity - value).abs() < 0.001);
                }
                let _ = AppendMenuW(menu, MF_POPUP, sub.0 as usize, w!("Intensity"));
            }
            if let Ok(sub) = CreatePopupMenu() {
                for (i, (name, key)) in LIGHTS_MODES.iter().enumerate() {
                    item(sub, ID_LIGHTS + i, name, s.lights_mode == *key);
                }
                separator(sub);
                item(sub, ID_PATTERN, &format!("Pattern: {} (click to change)", s.lights_pattern), false);
                let _ = AppendMenuW(menu, MF_POPUP, sub.0 as usize, w!("Lights"));
            }
            separator(menu);
            item(menu, ID_BLOWER, "Call the snow blower", false);
            item(menu, ID_CLEAR, "Clear all snow", false);
            item(menu, ID_PLAY, "Play mode\tCtrl+Alt+S", state.play);
            separator(menu);
            item(menu, ID_CAPTURE, "Show in screen sharing", s.show_in_capture);
            item(menu, ID_AUTOSTART, "Start with Windows", state.autostart);
            item(menu, ID_PAUSE, "Pause\tCtrl+Alt+P", s.paused);
            separator(menu);
            item(menu, ID_EXIT, "Exit", false);

            let mut at = POINT::default();
            let _ = GetCursorPos(&mut at);
            // Required so the menu closes when the user clicks elsewhere.
            let _ = SetForegroundWindow(self.hwnd);
            let picked = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON, at.x, at.y, None, self.hwnd, None).0 as usize;
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(menu);

            Some(match picked {
                id if (ID_PRESET..ID_PRESET + PRESETS.len()).contains(&id) => Command::SetIntensity(PRESETS[id - ID_PRESET].1),
                id if (ID_LIGHTS..ID_LIGHTS + LIGHTS_MODES.len()).contains(&id) => Command::LightsMode(LIGHTS_MODES[id - ID_LIGHTS].1),
                ID_PATTERN => Command::NextPattern,
                ID_BLOWER => Command::CallBlower,
                ID_CLEAR => Command::ClearSnow,
                ID_PLAY => Command::TogglePlay,
                ID_CAPTURE => Command::ToggleCapture,
                ID_AUTOSTART => Command::ToggleAutostart,
                ID_PAUSE => Command::TogglePause,
                ID_EXIT => Command::Exit,
                _ => return None,
            })
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        self.remove();
        unsafe {
            let _ = DestroyIcon(self.icon);
        }
    }
}
