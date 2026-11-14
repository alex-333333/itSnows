//! The world: owns all simulation state, steps it, and emits a draw list.

use crate::blower::{self, Blower, Particle};
pub use crate::blower::{BLOWER_COOLDOWN, BLOWER_H, BLOWER_REMOVE, BLOWER_SPEED, BLOWER_TRIGGER, BLOWER_W};
use crate::flakes::{Flake, Flakes, Layer};
use crate::pile::HeightMap;
use crate::prim::{Prim, Rgba};
use crate::rng::Rng;
use crate::surfaces::{Shed, Surfaces};
pub use crate::surfaces::{DEPOSIT_AREA, GROUND_CAP, MELT_RATE, SHED_MAX, WINDOW_CAP, WINDOW_EDGE_MAX, WINDOW_LAND_CHANCE};

const SNOW: Rgba = Rgba::new(0.96, 0.98, 1.0, 1.0);

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
    surfaces: Surfaces,
    shed: Vec<Shed>,
    /// Flakes spawned from snow that came loose, in total.
    shed_count: u64,
    blower: Option<Blower>,
    blower_requested: bool,
    blower_cooldown: f32,
    spray: Vec<Particle>,
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
        Self { rng, t: 0.0, width: 0.0, height: 0.0, flakes, surfaces: Surfaces::default(), shed: Vec::new(), shed_count: 0, blower: None, blower_requested: false, blower_cooldown: 0.0, spray: Vec::new() }
    }

    pub fn step(&mut self, input: &FrameInput) -> StepOutput {
        let dt = sanitize_dt(input.dt);
        let (w, h) = (input.width, input.height);
        if !(w > 0.0 && h > 0.0) || !w.is_finite() || !h.is_finite() {
            self.width = 0.0;
            self.height = 0.0;
            self.flakes.list.clear();
            self.shed.clear();
            self.blower = None;
            self.blower_requested = false;
            self.spray.clear();
            return StepOutput { idle: true, exit_play: false };
        }
        self.width = w;
        self.height = h;
        self.t += dt;

        let intensity = if input.settings.intensity.is_finite() { input.settings.intensity.clamp(0.0, 1.0) } else { 0.0 };
        self.flakes.update(dt, self.t, w, h, intensity, &mut self.rng);

        self.surfaces.reconcile(input.surfaces, &mut self.shed);
        self.land_flakes(dt);
        self.surfaces.settle(dt, intensity == 0.0, self.blower.is_some(), &mut self.shed);
        self.spawn_shed();
        self.step_blower(dt);

        let idle = intensity == 0.0
            && self.flakes.list.is_empty()
            && self.surfaces.all_empty()
            && self.blower.is_none()
            && self.spray.is_empty();
        StepOutput { idle, exit_play: false }
    }

    /// Move flakes that reached a pile into it.
    fn land_flakes(&mut self, dt: f32) {
        let piles = &mut self.surfaces.piles;
        let rng = &mut self.rng;
        self.flakes.list.retain_mut(|f| {
            let bottom = f.y + f.r;
            let prev_bottom = bottom - f.vy * dt;
            for pile in piles.iter_mut() {
                let line = pile.line_at(f.x);
                match pile.kind {
                    SurfaceKind::Ground => {
                        // The ground catches everything that gets this far.
                        if bottom >= line {
                            pile.map.deposit(f.x, DEPOSIT_AREA);
                            return false;
                        }
                    }
                    SurfaceKind::WindowTop => {
                        let crossing = prev_bottom <= line && bottom >= line;
                        if f.layer == Layer::Near && crossing && pile.is_visible_at(f.x) && !f.passed.contains(&pile.id) {
                            if rng.chance(WINDOW_LAND_CHANCE) {
                                pile.map.deposit(f.x, DEPOSIT_AREA);
                                return false;
                            }
                            f.passed.push(pile.id);
                        }
                    }
                }
            }
            true
        });
    }

    fn step_blower(&mut self, dt: f32) {
        self.blower_cooldown = (self.blower_cooldown - dt).max(0.0);
        let requested = std::mem::take(&mut self.blower_requested);
        match self.surfaces.ground_mut() {
            None => self.blower = None,
            Some(ground) => {
                if self.blower.is_none() {
                    let deep = self.blower_cooldown <= 0.0 && ground.map.mean_height() >= BLOWER_TRIGGER;
                    if requested || deep {
                        self.blower = Some(Blower::new(&mut self.rng, ground.map.x0(), ground.map.x1()));
                    }
                }
                if let Some(b) = self.blower.as_mut() {
                    if b.update(dt, &mut ground.map, ground.y, &mut self.rng, &mut self.spray) {
                        self.blower = None;
                        self.blower_cooldown = BLOWER_COOLDOWN;
                    }
                }
            }
        }
        blower::update_spray(&mut self.spray, dt, self.width, self.height);
    }

    /// Summon the snow blower now, however little snow there is. Ignored
    /// while one is already out or when there is no ground.
    pub fn call_blower(&mut self) {
        self.blower_requested = true;
    }

    /// The vehicle is on its run.
    pub fn blower_active(&self) -> bool {
        self.blower.is_some()
    }

    /// Turn snow that came loose into falling flakes.
    fn spawn_shed(&mut self) {
        self.shed_count += self.shed.len() as u64;
        for s in self.shed.drain(..) {
            let mut flake = Flake::near(&mut self.rng, s.x, s.y, 0.0, 0.0);
            flake.vx = self.rng.range(-25.0, 25.0);
            flake.vy = self.rng.range(40.0, 120.0);
            flake.passed.push(s.from);
            self.flakes.push_burst(flake, self.width, self.height);
        }
    }

    /// Empty every pile (tray "Clear all snow").
    pub fn clear_snow(&mut self) {
        self.surfaces.clear();
    }

    pub fn pile_mean_height(&self, id: u64) -> Option<f32> {
        self.surfaces.piles.iter().find(|p| p.id == id).map(|p| p.map.mean_height())
    }

    /// The ground surface's pile, if there is a ground.
    pub fn ground(&self) -> Option<&HeightMap> {
        self.surfaces.ground().map(|p| &p.map)
    }

    #[cfg(test)]
    pub(crate) fn fill_ground(&mut self, h: f32) {
        self.surfaces.ground_mut().expect("ground").map.fill(h);
    }

    /// Append this frame's primitives, back to front.
    pub fn draw(&self, out: &mut Vec<Prim>) {
        if self.width <= 0.0 {
            return;
        }
        self.draw_flakes(out, Layer::Far);
        self.draw_flakes(out, Layer::Mid);
        for pile in &self.surfaces.piles {
            let pts = pile.map.outline(pile.y);
            if !pts.is_empty() {
                out.push(Prim::Polygon { pts, color: SNOW });
            }
        }
        if let (Some(b), Some(ground)) = (&self.blower, self.surfaces.ground()) {
            b.draw(ground.y, out);
        }
        blower::draw_spray(&self.spray, out);
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

    // ---- accumulation ----

    const SW: f32 = 1920.0;
    const SH: f32 = 1080.0;
    const GROUND_ID: u64 = 1;

    fn ground(w: f32, y: f32) -> SurfaceIn {
        SurfaceIn { id: GROUND_ID, x0: 0.0, x1: w, y, kind: SurfaceKind::Ground, visible: vec![(0.0, w)] }
    }

    fn window(id: u64, x0: f32, x1: f32, y: f32) -> SurfaceIn {
        SurfaceIn { id, x0, x1, y, kind: SurfaceKind::WindowTop, visible: vec![(x0, x1)] }
    }

    fn step_s(world: &mut World, dt: f32, intensity: f32, surfaces: &[SurfaceIn]) -> StepOutput {
        let s = settings(intensity);
        world.step(&FrameInput { dt, width: SW, height: SH, surfaces, settings: &s, pointer: None })
    }

    fn run_s(world: &mut World, secs: f32, intensity: f32, surfaces: &[SurfaceIn]) {
        let dt = 1.0 / 30.0;
        for _ in 0..(secs / dt).round() as usize {
            step_s(world, dt, intensity, surfaces);
        }
    }

    fn pile_of(world: &World, id: u64) -> &HeightMap {
        &world.surfaces.piles.iter().find(|p| p.id == id).expect("pile").map
    }

    fn max_height(m: &HeightMap) -> f32 {
        (0..m.len()).map(|i| m.col_height(i)).fold(0.0, f32::max)
    }

    #[test]
    fn ground_collects_snow() {
        let mut w = World::new(1);
        run_s(&mut w, 60.0, 1.0, &[ground(SW, 1040.0)]);
        assert!(w.pile_mean_height(GROUND_ID).unwrap() > 1.0);
        assert!(w.ground().is_some());
    }

    #[test]
    fn flakes_do_not_fall_below_ground() {
        let mut w = World::new(1);
        let surfaces = [ground(SW, 1040.0)];
        run_s(&mut w, 30.0, 1.0, &surfaces);
        assert!(w.flakes.list.iter().all(|f| f.y - f.r <= 1040.0));
    }

    #[test]
    fn window_top_collects_less() {
        let mut w = World::new(2);
        let surfaces = [ground(SW, 1040.0), window(5, 400.0, 1200.0, 300.0)];
        run_s(&mut w, 120.0, 1.0, &surfaces);
        let win = w.pile_mean_height(5).unwrap();
        let gr = w.pile_mean_height(GROUND_ID).unwrap();
        assert!(win > 0.0 && gr > 0.0, "{win} {gr}");
        assert!(win < gr);
        assert!(win <= WINDOW_CAP);
    }

    #[test]
    fn only_near_flakes_land_on_windows() {
        let mut w = World::new(3);
        let surfaces = [window(5, 0.0, SW, 500.0)];
        run_s(&mut w, 60.0, 1.0, &surfaces);
        // Far and mid flakes pass the window edge and are still falling below it.
        assert!(w.flakes.list.iter().any(|f| f.layer != Layer::Near && f.y > 520.0));
        assert!(w.pile_mean_height(5).unwrap() > 0.0);
    }

    #[test]
    fn window_top_never_exceeds_cap() {
        let mut w = World::new(4);
        let surfaces = [window(5, 400.0, 1200.0, 300.0)];
        step_s(&mut w, DT, 1.0, &surfaces);
        w.surfaces.piles[0].map.fill(WINDOW_CAP);
        run_s(&mut w, 60.0, 1.0, &surfaces);
        assert!(max_height(pile_of(&w, 5)) <= WINDOW_CAP + 1e-3);
    }

    #[test]
    fn window_snow_slides_off_the_ends() {
        let mut w = World::new(4);
        let surfaces = [window(5, 400.0, 1200.0, 300.0)];
        step_s(&mut w, DT, 0.01, &surfaces);
        w.surfaces.piles[0].map.fill(WINDOW_CAP);
        let before = w.flake_count();
        run_s(&mut w, 5.0, 0.01, &surfaces);
        let m = pile_of(&w, 5);
        assert!(m.col_height(0) <= WINDOW_EDGE_MAX + 1e-3, "{}", m.col_height(0));
        assert!(m.col_height(m.len() - 1) <= WINDOW_EDGE_MAX + 1e-3);
        assert!(m.height_at(800.0) > 20.0, "middle keeps its snow");
        assert!(w.flake_count() > before, "excess leaves as falling flakes");
    }

    #[test]
    fn ground_never_exceeds_cap() {
        let mut w = World::new(5);
        let surfaces = [ground(SW, 1040.0)];
        step_s(&mut w, DT, 1.0, &surfaces);
        w.fill_ground(GROUND_CAP - 0.5);
        run_s(&mut w, 60.0, 1.0, &surfaces);
        assert!(max_height(w.ground().unwrap()) <= GROUND_CAP + 1e-3);
    }

    #[test]
    fn moving_window_shakes_snow_off() {
        let mut w = World::new(6);
        step_s(&mut w, DT, 0.01, &[window(5, 400.0, 1200.0, 300.0)]);
        w.surfaces.piles[0].map.fill(10.0);
        let before = w.flake_count();
        step_s(&mut w, DT, 0.01, &[window(5, 450.0, 1250.0, 300.0)]);
        assert_eq!(w.pile_mean_height(5), Some(0.0));
        assert!(w.flake_count() > before + 50);
    }

    #[test]
    fn closing_window_shakes_snow_off() {
        let mut w = World::new(6);
        step_s(&mut w, DT, 0.01, &[window(5, 400.0, 1200.0, 300.0)]);
        w.surfaces.piles[0].map.fill(10.0);
        let before = w.flake_count();
        step_s(&mut w, DT, 0.01, &[]);
        assert_eq!(w.pile_mean_height(5), None);
        assert!(w.flake_count() > before + 50);
    }

    #[test]
    fn shake_off_is_bounded_per_step() {
        let mut w = World::new(6);
        step_s(&mut w, DT, 0.01, &[window(5, 0.0, SW, 300.0)]);
        w.surfaces.piles[0].map.fill(WINDOW_CAP);
        let before = w.flake_count();
        step_s(&mut w, DT, 0.01, &[]);
        assert!(w.flake_count() <= before + SHED_MAX + 2);
    }

    #[test]
    fn covered_columns_shake_off() {
        let mut w = World::new(7);
        step_s(&mut w, DT, 0.01, &[window(5, 400.0, 1200.0, 300.0)]);
        w.surfaces.piles[0].map.fill(10.0);
        let mut half = window(5, 400.0, 1200.0, 300.0);
        half.visible = vec![(400.0, 800.0)];
        step_s(&mut w, DT, 0.01, &[half]);
        let m = pile_of(&w, 5);
        assert_eq!(m.height_at(1000.0), 0.0);
        assert!(m.height_at(600.0) > 9.0);
    }

    #[test]
    fn hidden_part_gets_no_snow() {
        let mut w = World::new(8);
        let mut half = window(5, 400.0, 1200.0, 300.0);
        half.visible = vec![(400.0, 800.0)];
        run_s(&mut w, 120.0, 1.0, &[half]);
        let m = pile_of(&w, 5);
        assert_eq!(m.height_at(1000.0), 0.0);
        assert!(m.total_area() > 0.0);
    }

    #[test]
    fn no_surfaces_is_fine() {
        let mut w = World::new(9);
        run_s(&mut w, 40.0, 1.0, &[]);
        assert!(w.flake_count() > 0);
        assert!(w.ground().is_none());
        assert!(w.flakes.list.iter().all(|f| f.y <= SH + f.r));
    }

    #[test]
    fn second_ground_is_ignored() {
        let mut w = World::new(9);
        let mut other = ground(SW, 900.0);
        other.id = 77;
        run_s(&mut w, 30.0, 1.0, &[ground(SW, 1040.0), other]);
        assert_eq!(w.pile_mean_height(77), None);
        assert!(w.pile_mean_height(GROUND_ID).unwrap() > 0.0);
    }

    #[test]
    fn resolution_change_keeps_ground() {
        let mut w = World::new(10);
        run_s(&mut w, 10.0, 1.0, &[ground(1920.0, 1040.0)]);
        w.fill_ground(12.0);
        let s = settings(0.5);
        let small = [ground(1280.0, 680.0)];
        w.step(&FrameInput { dt: DT, width: 1280.0, height: 720.0, surfaces: &small, settings: &s, pointer: None });
        let g = w.ground().unwrap();
        assert_eq!(g.x1(), 1280.0);
        assert!((g.height_at(600.0) - 12.0).abs() < 0.5);
        assert!(w.flakes.list.iter().all(|f| f.x <= 1430.0));
    }

    #[test]
    fn melts_only_when_off() {
        let mut w = World::new(11);
        let surfaces = [ground(SW, 1040.0)];
        step_s(&mut w, DT, 0.0, &surfaces);
        w.fill_ground(5.0);
        run_s(&mut w, 10.0, 0.0, &surfaces);
        let melted = w.pile_mean_height(GROUND_ID).unwrap();
        assert!((melted - (5.0 - 10.0 * MELT_RATE)).abs() < 0.1, "{melted}");

        let mut w = World::new(11);
        step_s(&mut w, DT, 0.5, &surfaces);
        w.fill_ground(5.0);
        run_s(&mut w, 10.0, 0.5, &surfaces);
        assert!(w.pile_mean_height(GROUND_ID).unwrap() >= 5.0);
    }

    #[test]
    fn not_idle_while_piles_melt() {
        let mut w = World::new(12);
        let surfaces = [ground(SW, 1040.0)];
        step_s(&mut w, DT, 0.0, &surfaces);
        w.fill_ground(1.0);
        assert!(!step_s(&mut w, DT, 0.0, &surfaces).idle);
        run_s(&mut w, 10.0, 0.0, &surfaces);
        assert!(step_s(&mut w, DT, 0.0, &surfaces).idle);
    }

    #[test]
    fn clear_snow_empties_piles() {
        let mut w = World::new(13);
        let surfaces = [ground(SW, 1040.0), window(5, 400.0, 1200.0, 300.0)];
        step_s(&mut w, DT, 0.5, &surfaces);
        w.fill_ground(20.0);
        w.surfaces.piles[1].map.fill(10.0);
        w.clear_snow();
        assert_eq!(w.pile_mean_height(GROUND_ID), Some(0.0));
        assert_eq!(w.pile_mean_height(5), Some(0.0));
    }

    #[test]
    fn piles_are_drawn_between_mid_and_near_flakes() {
        let mut w = World::new(14);
        let surfaces = [ground(SW, 1040.0)];
        run_s(&mut w, 20.0, 1.0, &surfaces);
        let mut out = Vec::new();
        w.draw(&mut out);
        let poly = out.iter().position(|p| matches!(p, Prim::Polygon { .. })).expect("pile polygon");
        assert!(out[..poly].iter().all(|p| matches!(p, Prim::Circle { r, .. } if *r <= 2.5)));
        assert!(out[poly + 1..].iter().all(|p| matches!(p, Prim::Circle { r, .. } if *r >= 2.5)));
    }

    #[test]
    fn edge_trickle_matches_snow_lost() {
        let mut w = World::new(4);
        let surfaces = [window(5, 400.0, 1200.0, 300.0)];
        step_s(&mut w, DT, 0.0, &surfaces);
        w.surfaces.piles[0].map.fill(WINDOW_CAP);
        let before = pile_of(&w, 5).total_area();
        // Intensity 0 would melt; hold the pile with a whisper of snow that cannot reach it in time.
        for _ in 0..120 {
            step_s(&mut w, DT, 0.001, &surfaces);
        }
        let lost = before - pile_of(&w, 5).total_area();
        assert!(lost > 0.0);
        let shed_area = w.shed_count as f32 * DEPOSIT_AREA;
        assert!(shed_area <= lost + 2.0 * DEPOSIT_AREA, "shed {shed_area} px² of flakes for {lost} px² of snow");
        assert!(shed_area >= lost - 2.0 * DEPOSIT_AREA, "shed {shed_area} px² of flakes for {lost} px² of snow");
    }

    // ---- snow blower ----

    /// A whisper of snowfall: keeps piles from melting without adding much.
    const HOLD: f32 = 0.01;

    fn ground_world(h: f32) -> (World, [SurfaceIn; 1]) {
        let mut w = World::new(21);
        let surfaces = [ground(SW, 1040.0)];
        step_s(&mut w, DT, HOLD, &surfaces);
        w.fill_ground(h);
        (w, surfaces)
    }

    fn run_until_blower_done(w: &mut World, surfaces: &[SurfaceIn], max_secs: f32) -> f32 {
        let mut t = 0.0;
        while w.blower_active() && t < max_secs {
            step_s(w, DT, HOLD, surfaces);
            t += DT;
        }
        t
    }

    #[test]
    fn triggers_at_threshold() {
        let (mut w, surfaces) = ground_world(55.0);
        assert!(!w.blower_active());
        step_s(&mut w, DT, HOLD, &surfaces);
        assert!(w.blower_active());
    }

    #[test]
    fn does_not_trigger_below() {
        let (mut w, surfaces) = ground_world(40.0);
        for _ in 0..600 {
            step_s(&mut w, DT, HOLD, &surfaces);
            assert!(!w.blower_active());
        }
    }

    #[test]
    fn removes_about_80_percent() {
        let (mut w, surfaces) = ground_world(60.0);
        let before = w.ground().unwrap().total_area();
        step_s(&mut w, DT, HOLD, &surfaces);
        assert!(w.blower_active());
        run_until_blower_done(&mut w, &surfaces, 60.0);
        let left = w.ground().unwrap().total_area() / before;
        assert!((0.17..=0.23).contains(&left), "{left}");
    }

    #[test]
    fn manual_call_works_when_low() {
        let (mut w, surfaces) = ground_world(10.0);
        w.call_blower();
        step_s(&mut w, DT, HOLD, &surfaces);
        assert!(w.blower_active());
        run_until_blower_done(&mut w, &surfaces, 60.0);
        assert!(w.pile_mean_height(GROUND_ID).unwrap() < 3.0);
    }

    #[test]
    fn second_call_while_active_ignored() {
        let (mut w, surfaces) = ground_world(10.0);
        w.call_blower();
        step_s(&mut w, DT, HOLD, &surfaces);
        run_s(&mut w, 2.0, HOLD, &surfaces);
        w.call_blower();
        let t = run_until_blower_done(&mut w, &surfaces, 60.0);
        // One pass only: done in well under a full crossing from now.
        assert!(t < (SW + 2.0 * BLOWER_W) / BLOWER_SPEED, "{t}");
        run_s(&mut w, 1.0, HOLD, &surfaces);
        assert!(!w.blower_active());
    }

    #[test]
    fn no_ground_no_blower() {
        let mut w = World::new(22);
        w.call_blower();
        step_s(&mut w, DT, HOLD, &[]);
        assert!(!w.blower_active());
        step_s(&mut w, DT, HOLD, &[ground(SW, 1040.0)]);
        assert!(!w.blower_active(), "a call made with no ground is dropped, not queued");
    }

    #[test]
    fn finishes_and_leaves() {
        let (mut w, surfaces) = ground_world(10.0);
        w.call_blower();
        step_s(&mut w, DT, HOLD, &surfaces);
        let t = run_until_blower_done(&mut w, &surfaces, 60.0);
        assert!(t <= (SW + 2.0 * BLOWER_W) / BLOWER_SPEED + 1.0, "{t}");
        assert!(!w.blower_active());
    }

    #[test]
    fn cooldown_blocks_retrigger() {
        let (mut w, surfaces) = ground_world(60.0);
        step_s(&mut w, DT, HOLD, &surfaces);
        run_until_blower_done(&mut w, &surfaces, 60.0);
        w.fill_ground(60.0);
        run_s(&mut w, 19.0, HOLD, &surfaces);
        assert!(!w.blower_active());
        run_s(&mut w, 2.0, HOLD, &surfaces);
        assert!(w.blower_active());
    }

    #[test]
    fn manual_call_ignores_cooldown() {
        let (mut w, surfaces) = ground_world(60.0);
        step_s(&mut w, DT, HOLD, &surfaces);
        run_until_blower_done(&mut w, &surfaces, 60.0);
        w.call_blower();
        step_s(&mut w, DT, HOLD, &surfaces);
        assert!(w.blower_active());
    }

    #[test]
    fn blower_throws_snow_and_is_drawn() {
        let (mut w, surfaces) = ground_world(60.0);
        step_s(&mut w, DT, HOLD, &surfaces);
        run_s(&mut w, 4.0, HOLD, &surfaces);
        let mut out = Vec::new();
        w.draw(&mut out);
        assert!(out.iter().any(|p| matches!(p, Prim::RoundRect { .. })), "blower body");
        assert!(w.spray.len() > 10, "chute spray");
    }

    #[test]
    fn not_idle_while_blower_runs() {
        let mut w = World::new(23);
        let surfaces = [ground(SW, 1040.0)];
        step_s(&mut w, DT, 0.0, &surfaces);
        w.call_blower();
        assert!(!step_s(&mut w, DT, 0.0, &surfaces).idle);
    }

    #[test]
    fn blower_follows_ground_through_resize() {
        let (mut w, _) = ground_world(60.0);
        step_s(&mut w, DT, HOLD, &[ground(SW, 1040.0)]);
        assert!(w.blower_active());
        let s = settings(HOLD);
        let small = [ground(800.0, 560.0)];
        let mut t = 0.0;
        while w.blower_active() && t < 60.0 {
            w.step(&FrameInput { dt: DT, width: 800.0, height: 600.0, surfaces: &small, settings: &s, pointer: None });
            t += DT;
        }
        assert!(!w.blower_active(), "blower must still leave after the screen shrinks");
    }
}
