//! Play mode: rolling snowballs out of the ground pile, stacking them,
//! building snowmen, throwing and crumbling them.

use crate::flakes::{Flake, Flakes};
use crate::pile::COL_W;
use crate::prim::{Prim, Rgba};
use crate::rng::Rng;
use crate::surfaces::{Surfaces, DEPOSIT_AREA};
use crate::world::{Pointer, SurfaceKind};

/// Seconds without mouse input before play mode asks to be switched off.
pub const PLAY_TIMEOUT: f32 = 30.0;
/// A rolled ball smaller than this falls apart when let go.
pub const BALL_MIN_R: f32 = 6.0;
pub const BALL_MAX_R: f32 = 60.0;
/// Pointer speed at release that turns a drop into a throw, px/s.
pub const THROW_SPEED: f32 = 600.0;
pub const GRAVITY: f32 = 1200.0;
/// A stack this tall is a snowman.
pub const SNOWMAN_BALLS: usize = 3;
/// How far above the snow a press still starts a ball, px.
const REACH: f32 = 30.0;
/// Most flakes one bursting ball turns into.
const BURST_MAX: usize = 200;
/// Window over which pointer speed is measured, s.
const SPEED_WINDOW: f32 = 0.1;

const SNOW: Rgba = Rgba::new(0.96, 0.98, 1.0, 1.0);
const SHADE: Rgba = Rgba::new(0.74, 0.82, 0.93, 1.0);
const COAL: Rgba = Rgba::new(0.1, 0.1, 0.12, 1.0);
const CARROT: Rgba = Rgba::new(1.0, 0.5, 0.1, 1.0);
const TWIG: Rgba = Rgba::new(0.36, 0.24, 0.14, 1.0);
const CLOTH: Rgba = Rgba::new(0.8, 0.12, 0.14, 1.0);
const FROST: Rgba = Rgba::new(0.7, 0.9, 1.0, 0.8);

#[derive(Clone, Debug)]
pub struct Ball {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    /// Snow in the ball, px². The radius follows from it.
    pub area: f32,
}

impl Ball {
    fn contains(&self, x: f32, y: f32, slack: f32) -> bool {
        (x - self.x).powi(2) + (y - self.y).powi(2) <= (self.r + slack).powi(2)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    Hat,
    Scarf,
}

/// One to three balls standing on each other, bottom first.
pub struct Stack {
    pub balls: Vec<Ball>,
    /// Set once the stack became a snowman.
    pub look: Option<Look>,
}

pub struct Flying {
    pub ball: Ball,
    pub vx: f32,
    pub vy: f32,
}

#[derive(PartialEq)]
enum HeldMode {
    /// Being rolled along the ground, growing.
    Rolling,
    /// Picked up and following the pointer.
    Carrying,
}

struct Held {
    ball: Ball,
    mode: HeldMode,
}

#[derive(Default)]
pub struct Play {
    pub stacks: Vec<Stack>,
    pub flying: Vec<Flying>,
    held: Option<Held>,
    prev: Option<Pointer>,
    still_for: f32,
    /// f64: the app may run for weeks.
    clock: f64,
    /// Recent pointer positions: (time, x, y).
    trail: Vec<(f64, f32, f32)>,
    active: bool,
}

/// Put every ball of a stack in its place above ground line `gy`.
fn seat(stack: &mut Stack, gy: f32) {
    let x = stack.balls[0].x;
    let mut below: Option<(f32, f32)> = None;
    for ball in &mut stack.balls {
        ball.x = x;
        ball.y = match below {
            None => gy - ball.r,
            // Sink a little into the ball below, like packed snow.
            Some((y, r)) => y - r - ball.r * 0.8,
        };
        below = Some((ball.y, ball.r));
    }
}

impl Play {
    #[cfg(test)]
    pub(crate) fn set_clock(&mut self, t: f64) {
        self.clock = t;
    }

    /// Free balls: standing, held or in the air. Snowmen are not counted.
    pub fn ball_count(&self) -> usize {
        self.stacks.iter().filter(|s| s.look.is_none()).map(|s| s.balls.len()).sum::<usize>()
            + self.held.is_some() as usize
            + self.flying.len()
    }

    pub fn snowman_count(&self) -> usize {
        self.stacks.iter().filter(|s| s.look.is_some()).count()
    }

    /// All snow currently packed into balls, px².
    pub fn total_area(&self) -> f32 {
        self.stacks.iter().flat_map(|s| s.balls.iter()).map(|b| b.area).sum::<f32>()
            + self.held.as_ref().map_or(0.0, |h| h.ball.area)
            + self.flying.iter().map(|f| f.ball.area).sum::<f32>()
    }

    /// Something here needs frames: play mode is on, or a ball is moving.
    pub fn busy(&self) -> bool {
        self.active || self.held.is_some() || !self.flying.is_empty()
    }

    /// Advance one step. Returns true when play mode has been left untouched
    /// for `PLAY_TIMEOUT` and should be switched off.
    #[allow(clippy::too_many_arguments)]
    pub fn step(&mut self, dt: f32, pointer: Option<Pointer>, surfaces: &mut Surfaces, flakes: &mut Flakes, rng: &mut Rng, w: f32, h: f32) -> bool {
        self.clock += dt as f64;
        self.active = pointer.is_some();
        let ground_y = surfaces.ground().map(|g| g.y);
        if let Some(gy) = ground_y {
            for stack in &mut self.stacks {
                seat(stack, gy);
            }
        }

        let mut exit = false;
        match pointer {
            None => {
                // Play mode is off: whatever was in hand is set down.
                if let Some(held) = self.held.take() {
                    self.release(held, 0.0, 0.0, surfaces);
                }
                self.prev = None;
                self.still_for = 0.0;
                self.trail.clear();
            }
            Some(p) => {
                let prev = self.prev.unwrap_or(Pointer { x: p.x, y: p.y, left: false, right: false });
                let moved = (p.x - prev.x).abs() + (p.y - prev.y).abs() > 0.5;
                if moved || p.left != prev.left || p.right != prev.right || self.prev.is_none() {
                    self.still_for = 0.0;
                } else {
                    self.still_for += dt;
                }
                exit = self.still_for >= PLAY_TIMEOUT;

                self.trail.push((self.clock, p.x, p.y));
                let cutoff = self.clock - 2.0 * SPEED_WINDOW as f64;
                self.trail.retain(|e| e.0 >= cutoff);

                if p.left && !prev.left {
                    self.press(p, surfaces);
                }
                if p.left {
                    self.drag(p, surfaces);
                }
                if !p.left && prev.left {
                    if let Some(held) = self.held.take() {
                        let (vx, vy) = self.pointer_velocity();
                        self.release(held, vx, vy, surfaces);
                    }
                }
                if p.right && !prev.right {
                    self.crumble(p, surfaces);
                }
                self.prev = Some(p);
            }
        }

        self.fly(dt, surfaces, flakes, rng, w, h);
        exit
    }

    fn pointer_velocity(&self) -> (f32, f32) {
        let Some(&(now, x, y)) = self.trail.last() else { return (0.0, 0.0) };
        let Some(&(t0, x0, y0)) = self.trail.iter().find(|e| e.0 >= now - SPEED_WINDOW as f64) else { return (0.0, 0.0) };
        let span = (now - t0) as f32;
        if span <= 0.0 {
            (0.0, 0.0)
        } else {
            ((x - x0) / span, (y - y0) / span)
        }
    }

    /// Left button went down: pick up a free ball, or start rolling a new one.
    fn press(&mut self, p: Pointer, surfaces: &Surfaces) {
        if self.held.is_some() {
            return;
        }
        // Only the top ball of a stack that is not yet a snowman can be taken.
        let pick = self.stacks.iter().position(|s| s.look.is_none() && s.balls.last().is_some_and(|b| b.contains(p.x, p.y, 4.0)));
        if let Some(i) = pick {
            let ball = self.stacks[i].balls.pop().expect("stack has a top ball");
            if self.stacks[i].balls.is_empty() {
                self.stacks.remove(i);
            }
            self.held = Some(Held { ball, mode: HeldMode::Carrying });
            return;
        }
        let Some(ground) = surfaces.ground() else { return };
        let on_snow = p.x >= ground.map.x0() && p.x < ground.map.x1() && p.y >= ground.line_at(p.x) - REACH && p.y <= ground.y + 2.0 * REACH;
        let over_stack = self.stacks.iter().any(|s| s.balls.iter().any(|b| b.contains(p.x, p.y, 0.0)));
        if on_snow && !over_stack {
            self.held = Some(Held { ball: Ball { x: p.x, y: ground.y, r: 0.0, area: 0.0 }, mode: HeldMode::Rolling });
        }
    }

    /// Left button is down: the held ball follows the pointer.
    fn drag(&mut self, p: Pointer, surfaces: &mut Surfaces) {
        let Some(held) = self.held.as_mut() else { return };
        match held.mode {
            HeldMode::Carrying => {
                held.ball.x = p.x;
                held.ball.y = p.y;
            }
            HeldMode::Rolling => {
                let Some(ground) = surfaces.ground_mut() else { return };
                let x = p.x.clamp(ground.map.x0(), ground.map.x1());
                let max_area = std::f32::consts::PI * BALL_MAX_R * BALL_MAX_R;
                if held.ball.area < max_area {
                    // Pick up the snow in every column the ball rolled over,
                    // and whatever slumped in under its trailing half.
                    let (from, r) = (held.ball.x, held.ball.r);
                    let (lo, hi) = if x >= from { (from - r, x) } else { (x, from + r) };
                    held.ball.area += ground.map.take(lo, hi, 1.0);
                    if held.ball.area > max_area {
                        // Put back what a full-size ball cannot hold.
                        ground.map.deposit(x, held.ball.area - max_area);
                        held.ball.area = max_area;
                    }
                }
                held.ball.r = (held.ball.area / std::f32::consts::PI).sqrt();
                held.ball.x = x;
                held.ball.y = ground.y - held.ball.r;
            }
        }
    }

    /// The held ball was let go with the pointer moving at (vx, vy).
    fn release(&mut self, held: Held, vx: f32, vy: f32, surfaces: &mut Surfaces) {
        let ball = held.ball;
        match held.mode {
            HeldMode::Rolling => {
                if ball.r < BALL_MIN_R {
                    // Too small to hold together: back into the pile.
                    if let Some(ground) = surfaces.ground_mut() {
                        ground.map.deposit(ball.x, ball.area);
                    }
                } else {
                    self.set_down(ball, surfaces);
                }
            }
            HeldMode::Carrying => {
                if (vx * vx + vy * vy).sqrt() >= THROW_SPEED {
                    self.flying.push(Flying { ball, vx, vy });
                } else {
                    self.set_down(ball, surfaces);
                }
            }
        }
    }

    /// Stack the ball if it was dropped on a larger one, else stand it on the ground.
    fn set_down(&mut self, mut ball: Ball, surfaces: &Surfaces) {
        let ground = surfaces.ground();
        let gy = ground.map_or(ball.y + ball.r, |g| g.y);

        let target = self.stacks.iter().position(|s| {
            let top = s.balls.last().expect("stacks are never empty");
            s.look.is_none() && s.balls.len() < SNOWMAN_BALLS && (ball.x - top.x).abs() <= top.r && ball.r < top.r
        });
        if let Some(i) = target {
            let stack = &mut self.stacks[i];
            stack.balls.push(ball);
            if stack.balls.len() == SNOWMAN_BALLS {
                // The look is fixed by the snowman's own shape, so it never changes.
                let seed = stack.balls.iter().map(|b| b.area).sum::<f32>() as u32;
                stack.look = Some(if seed % 2 == 0 { Look::Hat } else { Look::Scarf });
            }
            seat(stack, gy);
            return;
        }

        // Stand beside anything already there.
        for _ in 0..8 {
            let hit = self.stacks.iter().map(|s| &s.balls[0]).find(|base| (ball.x - base.x).abs() < ball.r + base.r);
            let Some(base) = hit else { break };
            let side = if ball.x < base.x { -1.0 } else { 1.0 };
            ball.x = base.x + side * (ball.r + base.r);
        }
        if let Some(g) = ground {
            ball.x = ball.x.clamp(g.map.x0(), g.map.x1());
        }
        let mut stack = Stack { balls: vec![ball], look: None };
        seat(&mut stack, gy);
        self.stacks.push(stack);
    }

    /// Right button went down: whatever is under the pointer goes back to snow.
    fn crumble(&mut self, p: Pointer, surfaces: &mut Surfaces) {
        let Some(i) = self.stacks.iter().position(|s| s.balls.iter().any(|b| b.contains(p.x, p.y, 2.0))) else { return };
        let stack = self.stacks.remove(i);
        let Some(ground) = surfaces.ground_mut() else { return };
        let base = &stack.balls[0];
        let area: f32 = stack.balls.iter().map(|b| b.area).sum();
        let columns = ((2.0 * base.r / COL_W).round() as usize).max(1);
        for k in 0..columns {
            let x = base.x - base.r + (k as f32 + 0.5) * COL_W;
            ground.map.deposit(x, area / columns as f32);
        }
    }

    /// Move thrown balls; burst the ones that hit something.
    #[allow(clippy::too_many_arguments)]
    fn fly(&mut self, dt: f32, surfaces: &Surfaces, flakes: &mut Flakes, rng: &mut Rng, w: f32, h: f32) {
        let mut i = 0;
        while i < self.flying.len() {
            let f = &mut self.flying[i];
            let prev_bottom = f.ball.y + f.ball.r;
            f.vy += GRAVITY * dt;
            f.ball.x += f.vx * dt;
            f.ball.y += f.vy * dt;
            let bottom = f.ball.y + f.ball.r;
            let (x, r) = (f.ball.x, f.ball.r);

            let hit_surface = surfaces.piles.iter().any(|pile| {
                let line = pile.line_at(x);
                match pile.kind {
                    SurfaceKind::Ground => bottom >= line,
                    SurfaceKind::WindowTop => prev_bottom <= line && bottom >= line && pile.is_visible_at(x),
                }
            });
            let gone = x < -r || x > w + r || f.ball.y > h + r;
            if hit_surface || gone {
                let ball = self.flying.swap_remove(i).ball;
                burst(&ball, flakes, rng, w, h);
            } else {
                i += 1;
            }
        }
    }

    /// Balls and snowmen. Drawn over the piles, under the nearest flakes.
    pub fn draw(&self, out: &mut Vec<Prim>) {
        for stack in &self.stacks {
            for ball in &stack.balls {
                draw_ball(ball, out);
            }
            if let Some(look) = stack.look {
                draw_snowman_details(stack, look, out);
            }
        }
        for f in &self.flying {
            draw_ball(&f.ball, out);
        }
        if let Some(held) = &self.held {
            if held.ball.r > 0.5 {
                draw_ball(&held.ball, out);
            }
        }
    }

    /// The frosty frame that says play mode is on. Drawn last.
    pub fn draw_border(&self, w: f32, h: f32, out: &mut Vec<Prim>) {
        if !self.active {
            return;
        }
        const T: f32 = 3.0;
        out.push(Prim::RoundRect { x: 0.0, y: 0.0, w, h: T, radius: 0.0, color: FROST });
        out.push(Prim::RoundRect { x: 0.0, y: h - T, w, h: T, radius: 0.0, color: FROST });
        out.push(Prim::RoundRect { x: 0.0, y: T, w: T, h: h - 2.0 * T, radius: 0.0, color: FROST });
        out.push(Prim::RoundRect { x: w - T, y: T, w: T, h: h - 2.0 * T, radius: 0.0, color: FROST });
    }
}

fn burst(ball: &Ball, flakes: &mut Flakes, rng: &mut Rng, w: f32, h: f32) {
    let n = ((ball.area / DEPOSIT_AREA).ceil() as usize).min(BURST_MAX);
    for _ in 0..n {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let dist = rng.range(0.0, ball.r);
        let speed = rng.range(40.0, 220.0);
        let flake = Flake::near(rng, ball.x + angle.cos() * dist, ball.y + angle.sin() * dist, angle.cos() * speed, angle.sin() * speed * 0.6 + 30.0);
        flakes.push_burst(flake, w, h);
    }
}

fn draw_ball(ball: &Ball, out: &mut Vec<Prim>) {
    // A cool rim low and to the right reads as shading on a white ball.
    out.push(Prim::Circle { x: ball.x, y: ball.y, r: ball.r, color: SHADE });
    let inset = (ball.r * 0.08).max(1.0);
    out.push(Prim::Circle { x: ball.x - inset * 0.6, y: ball.y - inset * 0.8, r: (ball.r - inset).max(0.5), color: SNOW });
}

fn draw_snowman_details(stack: &Stack, look: Look, out: &mut Vec<Prim>) {
    let (body, chest, head) = (&stack.balls[0], &stack.balls[1], &stack.balls[2]);
    let hr = head.r;

    // Twig arms from the chest, each with a small fork.
    for side in [-1.0f32, 1.0] {
        let (sx, sy) = (chest.x + side * chest.r * 0.8, chest.y - chest.r * 0.2);
        let (ex, ey) = (sx + side * chest.r * 1.3, sy - chest.r * 0.7);
        let width = (chest.r * 0.1).clamp(1.5, 4.0);
        out.push(Prim::Line { x0: sx, y0: sy, x1: ex, y1: ey, width, color: TWIG });
        let (fx, fy) = (sx + (ex - sx) * 0.7, sy + (ey - sy) * 0.7);
        out.push(Prim::Line { x0: fx, y0: fy, x1: fx + side * chest.r * 0.15, y1: fy - chest.r * 0.4, width: width * 0.8, color: TWIG });
    }

    // Coal buttons down the chest and belly.
    let button = (chest.r * 0.1).max(1.5);
    for (ball, dy) in [(chest, -0.35), (chest, 0.25), (body, -0.45)] {
        out.push(Prim::Circle { x: ball.x, y: ball.y + ball.r * dy, r: button, color: COAL });
    }

    match look {
        Look::Scarf => {
            let (w, h) = (hr * 2.0, (hr * 0.4).max(4.0));
            let y = head.y + hr * 0.7;
            out.push(Prim::RoundRect { x: head.x - w / 2.0, y, w, h, radius: h / 2.0, color: CLOTH });
            out.push(Prim::RoundRect { x: head.x + w * 0.2, y: y + h * 0.5, w: h, h: hr * 1.1, radius: h / 3.0, color: CLOTH });
        }
        Look::Hat => {
            let brim_h = (hr * 0.2).max(3.0);
            let brim_y = head.y - hr * 0.75 - brim_h;
            out.push(Prim::RoundRect { x: head.x - hr * 1.15, y: brim_y, w: hr * 2.3, h: brim_h, radius: brim_h / 2.0, color: COAL });
            out.push(Prim::RoundRect { x: head.x - hr * 0.7, y: brim_y - hr * 1.1, w: hr * 1.4, h: hr * 1.1 + 1.0, radius: 2.0, color: COAL });
            out.push(Prim::RoundRect { x: head.x - hr * 0.7, y: brim_y - hr * 0.3, w: hr * 1.4, h: hr * 0.22, radius: 0.0, color: CLOTH });
        }
    }

    // Face: coal eyes and mouth, carrot nose pointing right.
    let eye = (hr * 0.12).max(1.5);
    for side in [-1.0f32, 1.0] {
        out.push(Prim::Circle { x: head.x + side * hr * 0.35, y: head.y - hr * 0.2, r: eye, color: COAL });
    }
    for k in -2i32..=2 {
        let u = k as f32 / 2.0;
        out.push(Prim::Circle { x: head.x + u * hr * 0.4, y: head.y + hr * 0.45 - u * u * hr * 0.14, r: eye * 0.55, color: COAL });
    }
    out.push(Prim::Polygon {
        pts: vec![(head.x, head.y - hr * 0.08), (head.x + hr * 0.95, head.y + hr * 0.1), (head.x, head.y + hr * 0.22)],
        color: CARROT,
    });
}
