//! Dev tool: run scripted scenes through the core and save frames as PNG.
//!
//!   cargo run -p snow-preview -- <scene> [out_dir]

mod raster;

use snow_core::{FrameInput, LightsMode, LightsPattern, Pointer, Prim, Rgba, Settings, SurfaceIn, World};
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
        _ => {
            eprintln!("usage: snow-preview <scene> [out_dir]\nscenes: snow");
            std::process::exit(2);
        }
    }
}
