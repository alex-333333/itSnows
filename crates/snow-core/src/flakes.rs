//! Falling snowflakes: spawning, falling, sway and wind gusts.

use crate::rng::Rng;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Layer {
    Far,
    Mid,
    Near,
}

impl Layer {
    /// How strongly sideways motion affects this layer.
    pub fn drift_scale(self) -> f32 {
        match self {
            Layer::Far => 0.4,
            Layer::Mid => 0.7,
            Layer::Near => 1.0,
        }
    }

    pub fn alpha(self) -> f32 {
        match self {
            Layer::Far => 0.5,
            Layer::Mid => 0.75,
            Layer::Near => 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Flake {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub r: f32,
    pub layer: Layer,
    pub phase: f32,
    /// Ids of window-top surfaces this flake already decided to fall past.
    pub passed: Vec<u64>,
}

impl Flake {
    pub fn new(x: f32, y: f32, vx: f32, vy: f32, r: f32, layer: Layer, phase: f32) -> Self {
        Self { x, y, vx, vy, r, layer, phase, passed: Vec::new() }
    }

    /// A near-layer flake with random size, used for bursts and shake-off.
    pub fn near(rng: &mut Rng, x: f32, y: f32, vx: f32, vy: f32) -> Self {
        Self::new(x, y, vx, vy, rng.range(2.5, 4.0), Layer::Near, rng.range(0.0, std::f32::consts::TAU))
    }
}

pub const BASE_FLAKES: f32 = 1500.0;
pub const BASE_AREA: f32 = 1920.0 * 1080.0;
/// Seconds to spawn a full complement of flakes.
pub const FILL_TIME: f32 = 4.0;
pub const SWAY: f32 = 12.0;
pub const GUST_PEAK: f32 = 80.0;
/// Extra flakes allowed above the spawn target for bursts (shake-off, thrown balls).
pub const BURST_HEADROOM: usize = 1500;

pub struct Flakes {
    pub list: Vec<Flake>,
    spawn_acc: f32,
    gust_wait: f32,
    gust_t: f32,
    gust_dur: f32,
    gust_peak: f32,
}

impl Flakes {
    pub fn new(rng: &mut Rng) -> Self {
        Self { list: Vec::new(), spawn_acc: 0.0, gust_wait: rng.range(8.0, 20.0), gust_t: 0.0, gust_dur: 0.0, gust_peak: 0.0 }
    }

    pub fn target(intensity: f32, w: f32, h: f32) -> usize {
        (BASE_FLAKES * intensity.clamp(0.0, 1.0) * (w * h) / BASE_AREA).floor() as usize
    }

    /// Current wind speed for the near layer, px/s.
    pub fn wind(&self) -> f32 {
        if self.gust_dur > 0.0 {
            self.gust_peak * (std::f32::consts::PI * self.gust_t / self.gust_dur).sin()
        } else {
            0.0
        }
    }

    fn update_gust(&mut self, dt: f32, rng: &mut Rng) {
        if self.gust_dur > 0.0 {
            self.gust_t += dt;
            if self.gust_t >= self.gust_dur {
                self.gust_dur = 0.0;
                self.gust_wait = rng.range(8.0, 20.0);
            }
        } else {
            self.gust_wait -= dt;
            if self.gust_wait <= 0.0 {
                self.gust_t = 0.0;
                self.gust_dur = rng.range(3.0, 6.0);
                self.gust_peak = rng.range(0.4, 1.0) * GUST_PEAK * if rng.chance(0.5) { 1.0 } else { -1.0 };
            }
        }
    }

    fn spawn_one(&mut self, w: f32, rng: &mut Rng) {
        let pick = rng.next_f32();
        let (layer, r, vy) = if pick < 0.5 {
            (Layer::Far, rng.range(1.0, 1.5), rng.range(30.0, 50.0))
        } else if pick < 0.8 {
            (Layer::Mid, rng.range(1.5, 2.5), rng.range(50.0, 90.0))
        } else {
            (Layer::Near, rng.range(2.5, 4.0), rng.range(90.0, 140.0))
        };
        let x = rng.range(-100.0, w + 100.0);
        let phase = rng.range(0.0, std::f32::consts::TAU);
        self.list.push(Flake::new(x, -r, rng.range(-5.0, 5.0), vy, r, layer, phase));
    }

    /// Spawn new flakes and move existing ones. `t` is world time in seconds.
    pub fn update(&mut self, dt: f32, t: f32, w: f32, h: f32, intensity: f32, rng: &mut Rng) {
        self.update_gust(dt, rng);

        let target = Self::target(intensity, w, h);
        if self.list.len() < target {
            self.spawn_acc += target as f32 / FILL_TIME * dt;
            while self.spawn_acc >= 1.0 && self.list.len() < target {
                self.spawn_acc -= 1.0;
                self.spawn_one(w, rng);
            }
        }
        self.spawn_acc = self.spawn_acc.min(1.0);

        let wind = self.wind();
        for f in &mut self.list {
            let sway = SWAY * (f.phase + t * 1.3).sin();
            f.x += (f.vx + (sway + wind) * f.layer.drift_scale()) * dt;
            f.y += f.vy * dt;
        }
        self.list.retain(|f| f.y <= h + f.r && f.x >= -150.0 && f.x <= w + 150.0);
    }

    /// Add a burst flake unless the hard ceiling is reached.
    pub fn push_burst(&mut self, flake: Flake, w: f32, h: f32) {
        if self.list.len() < Self::target(1.0, w, h) + BURST_HEADROOM {
            self.list.push(flake);
        }
    }
}
