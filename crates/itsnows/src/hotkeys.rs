//! Global hotkeys. They are registered to the thread (no window), so they
//! arrive as `WM_HOTKEY` in the message pump. A combination another app
//! already owns simply fails to register and is skipped.

use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub const PLAY: i32 = 1;
pub const UP: i32 = 2;
pub const DOWN: i32 = 3;
pub const PAUSE: i32 = 4;
pub const ESCAPE: i32 = 5;

pub fn register_all() {
    let mods = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;
    unsafe {
        let _ = RegisterHotKey(None, PLAY, mods, 'S' as u32);
        let _ = RegisterHotKey(None, UP, mods, VK_UP.0 as u32);
        let _ = RegisterHotKey(None, DOWN, mods, VK_DOWN.0 as u32);
        let _ = RegisterHotKey(None, PAUSE, mods, 'P' as u32);
    }
}

/// Esc is only claimed while play mode is on: the overlay never has keyboard
/// focus, so a hotkey is the only way it can see the key.
pub fn set_escape(on: bool) {
    unsafe {
        if on {
            let _ = RegisterHotKey(None, ESCAPE, MOD_NOREPEAT, VK_ESCAPE.0 as u32);
        } else {
            let _ = UnregisterHotKey(None, ESCAPE);
        }
    }
}

pub fn unregister_all() {
    unsafe {
        for id in [PLAY, UP, DOWN, PAUSE, ESCAPE] {
            let _ = UnregisterHotKey(None, id);
        }
    }
}
