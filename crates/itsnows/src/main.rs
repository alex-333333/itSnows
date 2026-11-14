//! itSnows: a snowy overlay for the Windows desktop.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg_attr(not(windows), allow(dead_code))]
mod config;
#[cfg(windows)]
mod gfx;
#[cfg(windows)]
mod overlay;
#[cfg(windows)]
mod app;
#[cfg(windows)]
mod hotkeys;
#[cfg(windows)]
mod system;
#[cfg(windows)]
mod tray;

#[cfg(windows)]
fn main() {
    app::run();
}

#[cfg(not(windows))]
fn main() {
    println!("itSnows runs on Windows only.");
}
