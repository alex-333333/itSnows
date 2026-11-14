//! The overlay window: borderless, always on top, never focused, and
//! click-through unless play mode is on.

use std::cell::RefCell;

use windows::core::{w, Result};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Things the window procedure saw that the frame loop needs to act on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    DisplayChange,
}

thread_local! {
    static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
}

pub fn push_event(e: Event) {
    EVENTS.with(|q| q.borrow_mut().push(e));
}

pub fn drain_events() -> Vec<Event> {
    EVENTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

pub struct Overlay {
    pub hwnd: HWND,
    pub width: u32,
    pub height: u32,
}

/// Size of the primary monitor in physical pixels (the process is per-monitor DPI aware).
pub fn primary_size() -> (i32, i32) {
    unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
}

/// The overlay is one pixel shorter than the screen. A top-most window that
/// covers a monitor exactly can be treated by the shell as a full-screen app,
/// which hides the taskbar behind it and silences notifications.
fn overlay_size() -> (i32, i32) {
    let (w, h) = primary_size();
    (w.max(1), (h - 1).max(1))
}

impl Overlay {
    pub fn create() -> Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = w!("itSnowsOverlay");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: class,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                ..Default::default()
            };
            RegisterClassExW(&wc);

            let (w, h) = overlay_size();
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP,
                class,
                w!("itSnows"),
                WS_POPUP,
                0,
                0,
                w,
                h,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            // A layered window is only composed once its attributes are set.
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            Ok(Self { hwnd, width: w as u32, height: h as u32 })
        }
    }

    /// Re-read the primary monitor size and resize the window to match.
    pub fn refit(&mut self) {
        let (w, h) = overlay_size();
        self.width = w as u32;
        self.height = h as u32;
        unsafe {
            let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), 0, 0, w, h, SWP_NOACTIVATE);
        }
    }

    /// `true`: every click passes through to whatever is underneath.
    pub fn set_click_through(&self, on: bool) {
        unsafe {
            let ex = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
            let bit = WS_EX_TRANSPARENT.0 as isize;
            let new = if on { ex | bit } else { ex & !bit };
            if new != ex {
                SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, new);
            }
        }
    }

    pub fn set_visible(&self, on: bool) {
        unsafe {
            let _ = ShowWindow(self.hwnd, if on { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
    }

    /// `true`: the overlay is left out of screenshots, recordings and screen sharing.
    pub fn set_capture_excluded(&self, on: bool) {
        unsafe {
            let _ = SetWindowDisplayAffinity(self.hwnd, if on { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_DISPLAYCHANGE => {
            push_event(Event::DisplayChange);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
