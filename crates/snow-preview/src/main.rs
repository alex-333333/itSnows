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

fn scene_blower(out: PathBuf) {
    let mut s = Sim::new("blower", out, 1280.0, 720.0, 1.0);
    s.desktop(&[win(11, 200.0, 180.0, 760.0, 500.0)]);
    // Snow until the blower comes out by itself.
    let mut waited = 0.0;
    while !s.world.blower_active() && waited < 1200.0 {
        s.run(1.0);
        waited += 1.0;
    }
    println!("blower appeared after {waited} s of blizzard");
    s.shoot();
    s.run(3.5);
    s.shoot();
    s.run(8.0);
    s.shoot();
}

fn scene_lights(out: PathBuf) {
    let mut s = Sim::new("lights", out, 1280.0, 720.0, 0.5);
    s.desktop(&[win(10, 520.0, 300.0, 1000.0, 620.0), win(11, 120.0, 180.0, 760.0, 560.0)]);
    s.settings.lights_mode = LightsMode::Windows;
    s.run(60.0);
    s.shoot();
    s.run(0.5);
    s.shoot();
    s.settings.lights_mode = LightsMode::ScreenTop;
    s.settings.lights_pattern = LightsPattern::Chase;
    s.run(0.5);
    s.shoot();
}

#[allow(dead_code)]
impl Sim {
    /// Hold the pointer at a spot for one frame.
    fn point(&mut self, x: f32, y: f32, left: bool) {
        self.pointer = Some(Pointer { x, y, left, right: false });
        self.step();
    }

    /// Move the pointer in a straight line, `speed` px per frame.
    fn glide(&mut self, from: (f32, f32), to: (f32, f32), speed: f32, left: bool) {
        let dist = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt();
        let steps = (dist / speed).ceil().max(1.0) as usize;
        for k in 1..=steps {
            let u = k as f32 / steps as f32;
            self.point(from.0 + (to.0 - from.0) * u, from.1 + (to.1 - from.1) * u, left);
        }
    }

    fn roll(&mut self, x0: f32, x1: f32) {
        let y = self.h - TASKBAR_H - 5.0;
        self.point(x0, y, false);
        self.point(x0, y, true);
        self.glide((x0, y), (x1, y), 4.0, true);
        self.point(x1, y, false);
    }

    fn carry(&mut self, from_x: f32, to_x: f32) {
        let gy = self.h - TASKBAR_H;
        let (grab, up, over) = ((from_x, gy - 8.0), (from_x, gy - 250.0), (to_x, gy - 250.0));
        self.point(grab.0, grab.1, false);
        self.point(grab.0, grab.1, true);
        self.glide(grab, up, 8.0, true);
        self.glide(up, over, 8.0, true);
        for _ in 0..15 {
            self.point(over.0, over.1, true);
        }
        self.point(over.0, over.1, false);
    }
}

fn scene_snowman(out: PathBuf) {
    let mut s = Sim::new("snowman", out, 1280.0, 720.0, 0.6);
    s.desktop(&[win(11, 700.0, 160.0, 1180.0, 480.0)]);
    // Let a decent layer build up first.
    s.run(420.0);
    s.shoot();
    // First snowman: three balls, stacked.
    s.roll(120.0, 330.0);
    s.shoot();
    s.roll(420.0, 520.0);
    s.roll(600.0, 640.0);
    s.carry(520.0, 330.0);
    s.carry(640.0, 330.0);
    // A second, with a ball left lying about.
    s.roll(1150.0, 960.0);
    s.roll(900.0, 800.0);
    s.roll(760.0, 720.0);
    s.carry(800.0, 960.0);
    s.carry(720.0, 960.0);
    s.roll(540.0, 600.0);
    s.shoot();
    s.pointer = None;
    s.run(30.0);
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
        "blower" => scene_blower(out),
        "lights" => scene_lights(out),
        "snowman" => scene_snowman(out),
        _ => {
            eprintln!("usage: snow-preview <scene> [out_dir]\nscenes: snow piles blower lights snowman");
            std::process::exit(2);
        }
    }
}
