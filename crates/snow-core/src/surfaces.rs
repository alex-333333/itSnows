//! Piles of snow on the surfaces the app reports, kept in step with windows
//! appearing, moving, getting covered and closing.

use crate::pile::HeightMap;
use crate::world::{SurfaceIn, SurfaceKind};

/// Area one landed flake adds to a pile, px².
pub const DEPOSIT_AREA: f32 = 6.0;
pub const WINDOW_CAP: f32 = 24.0;
/// Deepest the ground pile gets. Kept low so the snow never hides much of
/// what sits just above the taskbar.
pub const GROUND_CAP: f32 = 40.0;
/// Share of near-layer flakes crossing a window top that land on it.
pub const WINDOW_LAND_CHANCE: f32 = 0.5;
/// How fast piles shrink while snowfall is off, px/s.
pub const MELT_RATE: f32 = 0.2;
/// Tallest a window-top pile may stand at the end of a visible stretch;
/// anything above slides off, which gives drifts their rounded ends.
pub const WINDOW_EDGE_MAX: f32 = 3.0;
/// Most flakes one surface may shed in one step.
pub const SHED_MAX: usize = 400;

pub struct Pile {
    pub id: u64,
    pub kind: SurfaceKind,
    pub y: f32,
    pub map: HeightMap,
    pub visible: Vec<(f32, f32)>,
    /// Loose snow not yet turned into a whole flake, px².
    pub loose: f32,
}

impl Pile {
    pub fn is_visible_at(&self, x: f32) -> bool {
        self.visible.iter().any(|(a, b)| x >= *a && x < *b)
    }

    /// Y of the snow surface at `x`.
    pub fn line_at(&self, x: f32) -> f32 {
        self.y - self.map.height_at(x)
    }
}

/// A flake to spawn where snow came loose.
pub struct Shed {
    pub x: f32,
    pub y: f32,
    /// The surface it fell from, so it does not land straight back on it.
    pub from: u64,
}

#[derive(Default)]
pub struct Surfaces {
    /// Window tops from highest on screen to lowest, then the ground.
    pub piles: Vec<Pile>,
}

/// Lower column `i` to `keep`, turning what was removed into shed flakes.
fn shed_column(pile: &mut Pile, i: usize, keep: f32, budget: &mut usize, out: &mut Vec<Shed>) {
    let h = pile.map.col_height(i);
    if h <= keep {
        return;
    }
    // Carry fractions over, so a slow trickle sheds exactly as many flakes
    // as the snow it loses is worth.
    pile.loose += (h - keep) * crate::pile::COL_W;
    let whole = (pile.loose / DEPOSIT_AREA).floor();
    let n = (whole as usize).min(*budget);
    pile.loose = if n < whole as usize { 0.0 } else { pile.loose - whole * DEPOSIT_AREA };
    *budget -= n;
    let x = pile.map.col_x(i);
    pile.map.set_col(i, keep);
    for k in 0..n {
        // Spread the flakes through the height of the removed snow.
        let frac = (k as f32 + 0.5) / n as f32;
        out.push(Shed { x, y: pile.y - keep - (h - keep) * frac, from: pile.id });
    }
}

fn shed_all(pile: &mut Pile, out: &mut Vec<Shed>) {
    let mut budget = SHED_MAX;
    for i in 0..pile.map.len() {
        shed_column(pile, i, 0.0, &mut budget, out);
    }
}

fn moved(a: f32, b: f32) -> bool {
    (a - b).abs() > 0.5
}

impl Surfaces {
    pub fn ground(&self) -> Option<&Pile> {
        self.piles.iter().find(|p| p.kind == SurfaceKind::Ground)
    }

    pub fn ground_mut(&mut self) -> Option<&mut Pile> {
        self.piles.iter_mut().find(|p| p.kind == SurfaceKind::Ground)
    }

    /// Match stored piles to this frame's surfaces. Snow on anything that
    /// moved or vanished comes loose.
    pub fn reconcile(&mut self, input: &[SurfaceIn], out: &mut Vec<Shed>) {
        let mut old = std::mem::take(&mut self.piles);
        let mut ground_seen = false;
        for s in input {
            if !(s.x1 > s.x0) || !s.y.is_finite() || !s.x0.is_finite() || !s.x1.is_finite() {
                continue;
            }
            if s.kind == SurfaceKind::Ground {
                if ground_seen {
                    continue;
                }
                ground_seen = true;
            }
            if self.piles.iter().any(|p| p.id == s.id) {
                continue;
            }
            let existing = old.iter().position(|p| p.id == s.id && p.kind == s.kind).map(|i| old.swap_remove(i));
            let pile = match (existing, s.kind) {
                (Some(mut p), SurfaceKind::Ground) => {
                    // The ground keeps its snow through resolution changes.
                    if moved(p.map.x0(), s.x0) || moved(p.map.x1(), s.x1) {
                        p.map.resample(s.x0, s.x1);
                    }
                    p.y = s.y;
                    p.visible = s.visible.clone();
                    p
                }
                (Some(mut p), SurfaceKind::WindowTop) => {
                    if moved(p.map.x0(), s.x0) || moved(p.map.x1(), s.x1) || moved(p.y, s.y) {
                        shed_all(&mut p, out);
                        p.map = HeightMap::new(s.x0, s.x1, WINDOW_CAP);
                        p.y = s.y;
                    }
                    p.visible = s.visible.clone();
                    p
                }
                (None, kind) => {
                    let cap = if kind == SurfaceKind::Ground { GROUND_CAP } else { WINDOW_CAP };
                    Pile { id: s.id, kind, y: s.y, map: HeightMap::new(s.x0, s.x1, cap), visible: s.visible.clone(), loose: 0.0 }
                }
            };
            self.piles.push(pile);
        }
        for mut gone in old {
            shed_all(&mut gone, out);
        }
        self.piles.sort_by(|a, b| {
            let rank = |p: &Pile| p.kind == SurfaceKind::Ground;
            rank(a).cmp(&rank(b)).then(a.y.total_cmp(&b.y))
        });
    }

    /// Let piles slump, drop snow from covered or overfull edge columns, and
    /// melt when snowfall is off.
    ///
    /// `hold_ground`: leave the ground pile as it is. Set while the blower is
    /// cutting through it, so the uncleared snow ahead does not slump back
    /// into the cleared strip behind.
    pub fn settle(&mut self, dt: f32, melting: bool, hold_ground: bool, out: &mut Vec<Shed>) {
        for pile in &mut self.piles {
            if !(hold_ground && pile.kind == SurfaceKind::Ground) {
                pile.map.relax(dt);
            }
            if pile.kind == SurfaceKind::WindowTop {
                let mut budget = SHED_MAX;
                let n = pile.map.len();
                for i in 0..n {
                    let x = pile.map.col_x(i);
                    if !pile.is_visible_at(x) {
                        shed_column(pile, i, 0.0, &mut budget, out);
                        continue;
                    }
                    let at_edge = i == 0
                        || i == n - 1
                        || !pile.is_visible_at(pile.map.col_x(i - 1))
                        || !pile.is_visible_at(pile.map.col_x(i + 1));
                    if at_edge {
                        shed_column(pile, i, WINDOW_EDGE_MAX, &mut budget, out);
                    }
                }
            }
            if melting {
                pile.map.melt(dt, MELT_RATE);
            }
        }
    }

    pub fn all_empty(&self) -> bool {
        self.piles.iter().all(|p| p.map.is_empty())
    }

    pub fn clear(&mut self) {
        for p in &mut self.piles {
            p.map.fill(0.0);
        }
    }
}
