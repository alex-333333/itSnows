//! Finds the surfaces snow can land on: the taskbar and the visible top
//! edges of open windows.

use snow_core::{window_top_surfaces, SurfaceIn, SurfaceKind, WinRect};
use windows::core::{w, BOOL};
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Id of the ground surface. Window ids are window handles, which are never 1.
pub const GROUND_ID: u64 = 1;

/// Desktop and shell windows that are not app windows.
const SHELL_CLASSES: [&str; 4] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<HWND>);
    list.push(hwnd);
    true.into()
}

unsafe fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 64];
    let n = GetClassNameW(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

unsafe fn frame_rect(hwnd: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    // The DWM bounds leave out the invisible resize border around a window.
    let size = std::mem::size_of::<RECT>() as u32;
    if DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut rect as *mut _ as *mut _, size).is_ok() {
        return Some(rect);
    }
    GetWindowRect(hwnd, &mut rect).ok().map(|_| rect)
}

/// An ordinary app window the user can see on the primary monitor.
unsafe fn app_window(hwnd: HWND, overlay: HWND, screen_w: f32, screen_h: f32) -> Option<WinRect> {
    if hwnd == overlay || !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
        return None;
    }
    let ex = WINDOW_EX_STYLE(GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32);
    // Tool windows are helpers; click-through windows are other overlays.
    if ex.contains(WS_EX_TOOLWINDOW) || ex.contains(WS_EX_TRANSPARENT) {
        return None;
    }
    let mut cloaked = 0u32;
    let _ = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut _ as *mut _, 4);
    if cloaked != 0 {
        return None;
    }
    if SHELL_CLASSES.contains(&class_name(hwnd).as_str()) {
        return None;
    }
    let r = frame_rect(hwnd)?;
    let rect = WinRect { id: hwnd.0 as u64, left: r.left as f32, top: r.top as f32, right: r.right as f32, bottom: r.bottom as f32 };
    let on_screen = rect.right > 0.0 && rect.left < screen_w && rect.bottom > 0.0 && rect.top < screen_h;
    (on_screen && rect.right > rect.left && rect.bottom > rect.top).then_some(rect)
}

/// Y of the line snow settles on: the top of a bottom-docked, showing
/// taskbar, or else the bottom of the screen.
unsafe fn ground_y(screen_w: f32, screen_h: f32) -> f32 {
    let Ok(taskbar) = FindWindowW(w!("Shell_TrayWnd"), None) else { return screen_h };
    let mut r = RECT::default();
    if GetWindowRect(taskbar, &mut r).is_err() {
        return screen_h;
    }
    let (top, bottom, width) = (r.top as f32, r.bottom as f32, (r.right - r.left) as f32);
    let along_bottom = width >= screen_w * 0.9 && bottom >= screen_h - 2.0;
    let showing = screen_h - top >= 8.0;
    if along_bottom && showing && top > 0.0 {
        top
    } else {
        screen_h
    }
}

/// Everything snow can land on right now.
pub fn snapshot(overlay: HWND, screen_w: f32, screen_h: f32) -> Vec<SurfaceIn> {
    unsafe {
        let mut handles: Vec<HWND> = Vec::new();
        // Top-level windows, front to back.
        let _ = EnumWindows(Some(collect), LPARAM(&mut handles as *mut _ as isize));
        let windows: Vec<WinRect> = handles.into_iter().filter_map(|h| app_window(h, overlay, screen_w, screen_h)).collect();

        let ground = ground_y(screen_w, screen_h);
        let mut surfaces =
            vec![SurfaceIn { id: GROUND_ID, x0: 0.0, x1: screen_w, y: ground, kind: SurfaceKind::Ground, visible: vec![(0.0, screen_w)] }];
        surfaces.extend(window_top_surfaces(&windows, screen_w, screen_h, ground));
        surfaces
    }
}
