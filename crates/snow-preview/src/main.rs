//! Dev tool: run scripted scenes through the core and save frames as PNG.
//!
//!   cargo run -p snow-preview -- <scene> [out_dir]

mod raster;

use snow_core::{
    window_top_surfaces, FrameInput, LightsMode, LightsPattern, Pointer, Prim, Rgba, Settings, SurfaceIn, SurfaceKind, WinRect, World,
};
use std::path::PathBuf;

const BG: Rgba = Rgba::new(0.06, 0.09, 0.16, 1.0);
const DT: f32 = 1.0 / 60.0;

/// A scripted run of the world plus the fake desktop drawn behind it.
#[allow(dead_code)]
struct Sim {
    world: World,
    w: f32,
    h: f32,
    settings: Settings,
    surfaces: Vec<SurfaceIn>,
    pointer: Option<Pointer>,
    /// Fake windows/taskbar drawn under the overlay: (x, y, w, h, colour).
    backdrop: Vec<(f32, f32, f32, f32, Rgba)>,
    scene: String,
    out: PathBuf,
    shot: usize,
}

#[allow(dead_code)]
impl Sim {
    fn new(scene: &str, out: PathBuf, w: f32, h: f32, intensity: f32) -> Self {
        Self {
            world: World::new(42),
            w,
            h,
            settings: Settings { intensity, lights_mode: LightsMode::Off, lights_pattern: LightsPattern::Twinkle },
            surfaces: Vec::new(),
            pointer: None,
            backdrop: Vec::new(),
            scene: scene.to_string(),
            out,
            shot: 0,
        }
    }

    fn step(&mut self) {
        self.world.step(&FrameInput {
            dt: DT,
            width: self.w,
            height: self.h,
            surfaces: &self.surfaces,
            settings: &self.settings,
            pointer: self.pointer,
        });
    }

    fn run(&mut self, secs: f32) {
        for _ in 0..(secs / DT).round() as usize {
            self.step();
        }
    }

    fn shoot(&mut self) {
        let mut prims: Vec<Prim> =
            self.backdrop.iter().map(|&(x, y, w, h, color)| Prim::RoundRect { x, y, w, h, radius: 0.0, color }).collect();
        self.world.draw(&mut prims);
        let pm = raster::render(&prims, self.w as u32, self.h as u32, BG);
        std::fs::create_dir_all(&self.out).expect("create output dir");
        let path = self.out.join(format!("{}-{:02}.png", self.scene, self.shot));
        pm.save_png(&path).expect("write png");
        println!("{}", path.display());
        self.shot += 1;
    }
}

const TASKBAR_H: f32 = 40.0;
const WINDOW_COLORS: [Rgba; 3] = [Rgba::new(0.20, 0.24, 0.32, 1.0), Rgba::new(0.26, 0.22, 0.30, 1.0), Rgba::new(0.18, 0.28, 0.28, 1.0)];

#[allow(dead_code)]
impl Sim {
    /// Lay out a fake desktop: a taskbar along the bottom and `windows`
    /// (front to back). Sets both the backdrop and the surfaces.
    fn desktop(&mut self, windows: &[WinRect]) {
        let ground_y = self.h - TASKBAR_H;
        self.backdrop.clear();
        for (i, win) in windows.iter().enumerate().rev() {
            self.backdrop.push((win.left, win.top, win.right - win.left, win.bottom - win.top, WINDOW_COLORS[i % WINDOW_COLORS.len()]));
            // Title bar.
            self.backdrop.push((win.left, win.top, win.right - win.left, 28.0, Rgba::new(0.12, 0.14, 0.2, 1.0)));
        }
        self.backdrop.push((0.0, ground_y, self.w, TASKBAR_H, Rgba::new(0.1, 0.1, 0.13, 1.0)));
        self.surfaces = vec![SurfaceIn { id: 1, x0: 0.0, x1: self.w, y: ground_y, kind: SurfaceKind::Ground, visible: vec![(0.0, self.w)] }];
        self.surfaces.extend(window_top_surfaces(windows, self.w, self.h, ground_y));
    }
}

fn win(id: u64, left: f32, top: f32, right: f32, bottom: f32) -> WinRect {
    WinRect { id, left, top, right, bottom }
}

fn scene_piles(out: PathBuf) {
    let mut s = Sim::new("piles", out, 1280.0, 720.0, 1.0);
    let front = win(10, 520.0, 300.0, 1000.0, 620.0);
    let back = win(11, 120.0, 180.0, 760.0, 560.0);
    s.desktop(&[front, back]);
    for secs in [30.0, 90.0, 180.0, 600.0] {
        s.run(secs);
        s.shoot();
    }
    // Drag the front window: its snow should fall off.
    s.desktop(&[win(10, 640.0, 340.0, 1120.0, 660.0), back]);
    s.run(0.25);
    s.shoot();
}

fn scene_snow(out: PathBuf) {
    let mut s = Sim::new("snow", out, 1280.0, 720.0, 0.5);
    for secs in [5.0, 10.0, 15.0] {
        s.run(secs);
        s.shoot();
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let scene = args.next().unwrap_or_default();
    let out = PathBuf::from(args.next().unwrap_or_else(|| "preview-out".into()));
    match scene.as_str() {
        "snow" => scene_snow(out),
        "piles" => scene_piles(out),
        _ => {
            eprintln!("usage: snow-preview <scene> [out_dir]\nscenes: snow piles");
            std::process::exit(2);
        }
    }
}
