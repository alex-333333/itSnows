//! itSnows: a snowy overlay for the Windows desktop.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod gfx;
#[cfg(windows)]
mod overlay;
#[cfg(windows)]
mod app;

#[cfg(windows)]
fn main() {
    app::run();
}

#[cfg(not(windows))]
fn main() {
    println!("itSnows runs on Windows only.");
}
