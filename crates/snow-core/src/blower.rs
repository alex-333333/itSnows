//! The snow blower: a little vehicle that drives along the ground, takes
//! most of the snow it passes and throws it away through its chute.

use crate::pile::HeightMap;
use crate::prim::{Prim, Rgba};
use crate::rng::Rng;
use crate::surfaces::DEPOSIT_AREA;

/// Mean ground height that summons the blower: 60 % of the ground cap.
pub const BLOWER_TRIGGER: f32 = 54.0;
/// Share of the snow it removes from each column it passes.
pub const BLOWER_REMOVE: f32 = 0.8;
pub const BLOWER_SPEED: f32 = 160.0;
pub const BLOWER_W: f32 = 64.0;
pub const BLOWER_H: f32 = 40.0;
/// Seconds after a run before snow depth alone may summon it again.
pub const BLOWER_COOLDOWN: f32 = 20.0;
/// Removed area per spray particle, px².
const SPRAY_AREA: f32 = DEPOSIT_AREA * 4.0;
const SPRAY_GRAVITY: f32 = 600.0;
const SPRAY_LIFE: f32 = 3.0;

pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub age: f32,
    pub r: f32,
}

pub struct Blower {
    /// Left edge of the vehicle.
    pub x: f32,
    /// +1 driving right, -1 driving left.
    pub dir: f32,
    carry: f32,
}

impl Blower {
    /// Start just off-screen on a random side of the ground span.
    pub fn new(rng: &mut Rng, x0: f32, x1: f32) -> Self {
        let dir = if rng.chance(0.5) { 1.0 } else { -1.0 };
        Self { x: if dir > 0.0 { x0 - BLOWER_W } else { x1 }, dir, carry: 0.0 }
    }

    fn front(&self) -> f32 {
        if self.dir > 0.0 {
            self.x + BLOWER_W
        } else {
            self.x
        }
    }

    /// Where snow leaves the chute.
    fn chute_tip(&self, ground_y: f32) -> (f32, f32) {
        (self.mirror(20.0), ground_y - BLOWER_H - 12.0)
    }

    /// X of a point `offset` px behind the front... measured from the rear edge.
    fn mirror(&self, from_rear: f32) -> f32 {
        if self.dir > 0.0 {
            self.x + from_rear
        } else {
            self.x + BLOWER_W - from_rear
        }
    }

    /// Drive on, eat snow, emit spray. Returns true once it has left the span.
    pub fn update(&mut self, dt: f32, ground: &mut HeightMap, ground_y: f32, rng: &mut Rng, spray: &mut Vec<Particle>) -> bool {
        let before = self.front();
        self.x += self.dir * BLOWER_SPEED * dt;
        self.carry += ground.take(before, self.front(), BLOWER_REMOVE);
        let (tx, ty) = self.chute_tip(ground_y);
        while self.carry >= SPRAY_AREA {
            self.carry -= SPRAY_AREA;
            // Up and backwards.
            let speed = rng.range(250.0, 400.0);
            let angle = rng.range(55.0f32, 75.0).to_radians();
            spray.push(Particle { x: tx, y: ty, vx: -self.dir * speed * angle.cos(), vy: -speed * angle.sin(), age: 0.0, r: rng.range(1.5, 3.0) });
        }
        if self.dir > 0.0 {
            self.x > ground.x1()
        } else {
            self.x + BLOWER_W < ground.x0()
        }
    }

    pub fn draw(&self, ground_y: f32, out: &mut Vec<Prim>) {
        let top = ground_y - BLOWER_H;
        // A rectangle given by its rear offset, mirrored for direction.
        let rect = |from_rear: f32, y: f32, w: f32, h: f32, radius: f32, color: Rgba| {
            let x = if self.dir > 0.0 { self.x + from_rear } else { self.x + BLOWER_W - from_rear - w };
            Prim::RoundRect { x, y, w, h, radius, color }
        };
        let red = Rgba::new(0.86, 0.16, 0.14, 1.0);
        let dark_red = Rgba::new(0.55, 0.08, 0.08, 1.0);
        let steel = Rgba::new(0.25, 0.27, 0.32, 1.0);
        let tyre = Rgba::new(0.08, 0.08, 0.1, 1.0);

        // Chute: up from the body, bent backwards at the top.
        let (tx, ty) = self.chute_tip(ground_y);
        let base_x = self.mirror(30.0);
        out.push(Prim::Polyline { pts: vec![(base_x, top + 12.0), (base_x, top - 4.0), (tx, ty)], width: 7.0, color: steel });
        // Handle bar at the rear.
        out.push(Prim::Line { x0: self.mirror(10.0), y0: top + 16.0, x1: self.mirror(0.0), y1: top + 2.0, width: 3.0, color: steel });
        // Body and engine cover.
        out.push(rect(6.0, top + 10.0, 42.0, 22.0, 6.0, red));
        out.push(rect(12.0, top + 6.0, 20.0, 8.0, 3.0, dark_red));
        // Auger housing at the front, with a steel lip.
        out.push(rect(44.0, top + 8.0, 20.0, 32.0, 8.0, dark_red));
        out.push(rect(58.0, top + 12.0, 6.0, 28.0, 3.0, steel));
        // Wheels.
        for from_rear in [16.0, 36.0] {
            out.push(Prim::Circle { x: self.mirror(from_rear), y: ground_y - 7.0, r: 7.0, color: tyre });
            out.push(Prim::Circle { x: self.mirror(from_rear), y: ground_y - 7.0, r: 2.5, color: steel });
        }
        // Headlight.
        let (hx, hy) = (self.mirror(50.0), top + 14.0);
        out.push(Prim::Glow { x: hx, y: hy, r: 16.0, color: Rgba::new(1.0, 0.9, 0.4, 0.7) });
        out.push(Prim::Circle { x: hx, y: hy, r: 2.5, color: Rgba::new(1.0, 0.95, 0.6, 1.0) });
    }
}

/// Advance the thrown snow and drop what has landed out of sight or expired.
pub fn update_spray(spray: &mut Vec<Particle>, dt: f32, w: f32, h: f32) {
    for p in spray.iter_mut() {
        p.vy += SPRAY_GRAVITY * dt;
        p.x += p.vx * dt;
        p.y += p.vy * dt;
        p.age += dt;
    }
    spray.retain(|p| p.age < SPRAY_LIFE && p.x > -50.0 && p.x < w + 50.0 && p.y < h + 50.0);
}

pub fn draw_spray(spray: &[Particle], out: &mut Vec<Prim>) {
    for p in spray {
        // Fade out over the last second.
        let alpha = (SPRAY_LIFE - p.age).clamp(0.0, 1.0);
        out.push(Prim::Circle { x: p.x, y: p.y, r: p.r, color: Rgba::WHITE.with_alpha(alpha) });
    }
}
