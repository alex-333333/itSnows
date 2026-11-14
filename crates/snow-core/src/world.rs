//! The world: owns all simulation state, steps it, and emits a draw list.

use crate::flakes::{Flakes, Layer};
use crate::prim::{Prim, Rgba};
use crate::rng::Rng;

/// Longest time a single step may advance. A stalled frame or a resume from
/// sleep must not turn into a burst of simulation.
pub const MAX_DT: f32 = 0.1;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LightsMode {
    Off,
    ScreenTop,
    Windows,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LightsPattern {
    Steady,
    Twinkle,
    Chase,
}

#[derive(Clone, Debug)]
pub struct Settings {
    /// 0..=1
    pub intensity: f32,
    pub lights_mode: LightsMode,
    pub lights_pattern: LightsPattern,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SurfaceKind {
    Ground,
    WindowTop,
}

/// A horizontal edge snow can land on.
#[derive(Clone, Debug)]
pub struct SurfaceIn {
    /// Stable across frames for the same window.
    pub id: u64,
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
    pub kind: SurfaceKind,
    /// Absolute x intervals of the edge that are actually visible.
    pub visible: Vec<(f32, f32)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Pointer {
    pub x: f32,
    pub y: f32,
    pub left: bool,
    pub right: bool,
}

pub struct FrameInput<'a> {
    /// Seconds since the previous step.
    pub dt: f32,
    pub width: f32,
    pub height: f32,
    pub surfaces: &'a [SurfaceIn],
    pub settings: &'a Settings,
    /// `Some` while play mode is on.
    pub pointer: Option<Pointer>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StepOutput {
    /// Nothing is moving; the caller may stop rendering.
    pub idle: bool,
    /// Play mode timed out and should be switched off.
    pub exit_play: bool,
}

pub struct World {
    rng: Rng,
    t: f32,
    width: f32,
    height: f32,
    flakes: Flakes,
}

fn sanitize_dt(dt: f32) -> f32 {
    if dt.is_finite() {
        dt.clamp(0.0, MAX_DT)
    } else {
        0.0
    }
}

impl World {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let flakes = Flakes::new(&mut rng);
        Self { rng, t: 0.0, width: 0.0, height: 0.0, flakes }
    }

    pub fn step(&mut self, input: &FrameInput) -> StepOutput {
        let dt = sanitize_dt(input.dt);
        let (w, h) = (input.width, input.height);
        if !(w > 0.0 && h > 0.0) || !w.is_finite() || !h.is_finite() {
            self.width = 0.0;
            self.height = 0.0;
            self.flakes.list.clear();
            return StepOutput { idle: true, exit_play: false };
        }
        self.width = w;
        self.height = h;
        self.t += dt;

        let intensity = if input.settings.intensity.is_finite() { input.settings.intensity.clamp(0.0, 1.0) } else { 0.0 };
        self.flakes.update(dt, self.t, w, h, intensity, &mut self.rng);

        let idle = intensity == 0.0 && self.flakes.list.is_empty();
        StepOutput { idle, exit_play: false }
    }

    /// Append this frame's primitives, back to front.
    pub fn draw(&self, out: &mut Vec<Prim>) {
        if self.width <= 0.0 {
            return;
        }
        self.draw_flakes(out, Layer::Far);
        self.draw_flakes(out, Layer::Mid);
        self.draw_flakes(out, Layer::Near);
    }

    fn draw_flakes(&self, out: &mut Vec<Prim>, layer: Layer) {
        let color = Rgba::WHITE.with_alpha(layer.alpha());
        out.extend(
            self.flakes.list.iter().filter(|f| f.layer == layer).map(|f| Prim::Circle { x: f.x, y: f.y, r: f.r, color }),
        );
    }

    pub fn flake_count(&self) -> usize {
        self.flakes.list.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn settings(intensity: f32) -> Settings {
        Settings { intensity, lights_mode: LightsMode::Off, lights_pattern: LightsPattern::Steady }
    }

    fn step(world: &mut World, dt: f32, intensity: f32, w: f32, h: f32) -> StepOutput {
        let s = settings(intensity);
        world.step(&FrameInput { dt, width: w, height: h, surfaces: &[], settings: &s, pointer: None })
    }

    fn run(world: &mut World, secs: f32, intensity: f32, w: f32, h: f32) {
        for _ in 0..(secs / DT).round() as usize {
            step(world, DT, intensity, w, h);
        }
    }

    #[test]
    fn intensity_zero_spawns_nothing() {
        let mut w = World::new(1);
        run(&mut w, 10.0, 0.0, 1920.0, 1080.0);
        assert_eq!(w.flake_count(), 0);
    }

    #[test]
    fn half_intensity_settles_near_750() {
        let mut w = World::new(1);
        run(&mut w, 40.0, 0.5, 1920.0, 1080.0);
        let n = w.flake_count();
        assert!((600..=760).contains(&n), "{n}");
    }

    #[test]
    fn never_exceeds_cap() {
        let mut w = World::new(2);
        for _ in 0..3600 {
            step(&mut w, DT, 1.0, 1920.0, 1080.0);
            assert!(w.flake_count() <= 1500, "{}", w.flake_count());
        }
        assert!(w.flake_count() > 1000);
    }

    #[test]
    fn cap_scales_with_area() {
        let mut w = World::new(3);
        for _ in 0..3600 {
            step(&mut w, DT, 1.0, 960.0, 540.0);
            assert!(w.flake_count() <= 375, "{}", w.flake_count());
        }
    }

    #[test]
    fn huge_dt_is_clamped() {
        let mut w = World::new(4);
        step(&mut w, 30.0, 1.0, 1920.0, 1080.0);
        assert!(w.flake_count() as f32 <= 1500.0 * MAX_DT / 4.0 + 1.0, "{}", w.flake_count());
        let mut out = Vec::new();
        w.draw(&mut out);
        for p in &out {
            if let Prim::Circle { x, y, r, .. } = p {
                assert!(x.is_finite() && y.is_finite() && r.is_finite());
            }
        }
    }

    #[test]
    fn nan_dt_is_harmless() {
        let mut w = World::new(4);
        run(&mut w, 2.0, 1.0, 800.0, 600.0);
        step(&mut w, f32::NAN, 1.0, 800.0, 600.0);
        step(&mut w, -1.0, 1.0, 800.0, 600.0);
        let mut out = Vec::new();
        w.draw(&mut out);
        assert!(out.iter().all(|p| matches!(p, Prim::Circle { x, y, .. } if x.is_finite() && y.is_finite())));
    }

    #[test]
    fn zero_size_screen_is_noop() {
        let mut w = World::new(5);
        for _ in 0..100 {
            step(&mut w, DT, 1.0, 0.0, 0.0);
        }
        assert_eq!(w.flake_count(), 0);
        let mut out = Vec::new();
        w.draw(&mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn same_seed_same_world() {
        let mut a = World::new(9);
        let mut b = World::new(9);
        run(&mut a, 5.0, 0.7, 1280.0, 720.0);
        run(&mut b, 5.0, 0.7, 1280.0, 720.0);
        let (mut da, mut db) = (Vec::new(), Vec::new());
        a.draw(&mut da);
        b.draw(&mut db);
        assert!(!da.is_empty());
        assert_eq!(da, db);
    }

    #[test]
    fn idle_when_off_and_empty() {
        let mut w = World::new(6);
        assert!(step(&mut w, DT, 0.0, 1920.0, 1080.0).idle);
        assert!(!step(&mut w, DT, 1.0, 1920.0, 1080.0).idle);
    }

    #[test]
    fn not_idle_while_flakes_remain_after_off() {
        let mut w = World::new(6);
        run(&mut w, 3.0, 1.0, 1920.0, 1080.0);
        assert!(!step(&mut w, DT, 0.0, 1920.0, 1080.0).idle);
        run(&mut w, 60.0, 0.0, 1920.0, 1080.0);
        assert_eq!(w.flake_count(), 0);
        assert!(step(&mut w, DT, 0.0, 1920.0, 1080.0).idle);
    }

    #[test]
    fn draw_emits_one_circle_per_flake() {
        let mut w = World::new(7);
        run(&mut w, 5.0, 0.5, 1920.0, 1080.0);
        let mut out = Vec::new();
        w.draw(&mut out);
        let circles = out.iter().filter(|p| matches!(p, Prim::Circle { .. })).count();
        assert_eq!(circles, w.flake_count());
        assert!(circles > 0);
    }

    #[test]
    fn flakes_come_in_three_sizes() {
        let mut w = World::new(8);
        run(&mut w, 10.0, 1.0, 1920.0, 1080.0);
        let mut out = Vec::new();
        w.draw(&mut out);
        let radii: Vec<f32> = out.iter().filter_map(|p| match p { Prim::Circle { r, .. } => Some(*r), _ => None }).collect();
        assert!(radii.iter().all(|r| (1.0..=4.0).contains(r)));
        assert!(radii.iter().any(|r| *r < 1.5));
        assert!(radii.iter().any(|r| *r > 2.5));
    }
}
