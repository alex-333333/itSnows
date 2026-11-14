//! Christmas lights: garlands of coloured bulbs hanging in shallow swags.

use crate::prim::{Prim, Rgba};
use crate::world::LightsPattern;

/// Narrowest stretch of window top that gets a garland, px.
pub const MIN_GARLAND: f32 = 120.0;
pub const BULB_SPACING: f32 = 28.0;
/// Approximate width of one swag, px.
pub const SWAG_W: f32 = 140.0;
/// How far the wire hangs below its anchors at the middle of a swag, px.
pub const SWAG_SAG: f32 = 10.0;
pub const BULB_COLORS: [Rgba; 4] =
    [Rgba::new(1.0, 0.2, 0.2, 1.0), Rgba::new(0.2, 0.9, 0.3, 1.0), Rgba::new(1.0, 0.8, 0.2, 1.0), Rgba::new(0.3, 0.5, 1.0, 1.0)];
const WIRE: Rgba = Rgba::new(0.08, 0.28, 0.14, 1.0);
/// Bulbs lit at once, and the repeat length, in the chase pattern.
const CHASE_LIT: usize = 3;
const CHASE_PERIOD: usize = 12;
const CHASE_SPEED: f32 = 6.0;

pub struct Bulb {
    pub x: f32,
    pub y: f32,
    pub color_index: usize,
}

/// Lay out one garland between `x0` and `x1`, anchored at height `y`.
/// Returns the wire as a polyline and the bulbs hanging on it.
pub fn garland(x0: f32, x1: f32, y: f32) -> (Vec<(f32, f32)>, Vec<Bulb>) {
    let len = x1 - x0;
    if !(len > 0.0) || !len.is_finite() || !y.is_finite() {
        return (Vec::new(), Vec::new());
    }
    let swags = (len / SWAG_W).round().max(1.0);
    let swag_w = len / swags;
    let wire_y = |x: f32| {
        let u = (((x - x0) / swag_w).fract()).clamp(0.0, 1.0);
        y + SWAG_SAG * 4.0 * u * (1.0 - u)
    };

    const SEGMENTS: usize = 12;
    let mut wire = Vec::with_capacity(swags as usize * SEGMENTS + 1);
    for s in 0..swags as usize {
        for k in 0..SEGMENTS {
            let u = k as f32 / SEGMENTS as f32;
            wire.push((x0 + (s as f32 + u) * swag_w, y + SWAG_SAG * 4.0 * u * (1.0 - u)));
        }
    }
    wire.push((x1, y));

    let count = ((len / BULB_SPACING).floor() as usize).max(1);
    let start = x0 + (len - (count - 1) as f32 * BULB_SPACING) / 2.0;
    let bulbs = (0..count)
        .map(|i| {
            let x = start + i as f32 * BULB_SPACING;
            Bulb { x, y: wire_y(x), color_index: i % BULB_COLORS.len() }
        })
        .collect();
    (wire, bulbs)
}

/// How bright bulb `bulb_index` is at time `t`, in 0.15..=1.0.
pub fn brightness(pattern: LightsPattern, bulb_index: usize, t: f32) -> f32 {
    match pattern {
        LightsPattern::Steady => 1.0,
        LightsPattern::Twinkle => {
            // Golden-angle phase steps keep neighbours out of sync.
            let phase = bulb_index as f32 * 2.399;
            (0.675 + 0.325 * (t * 3.0 + phase).sin()).clamp(0.35, 1.0)
        }
        LightsPattern::Chase => {
            let head = (t.max(0.0) * CHASE_SPEED) as usize % CHASE_PERIOD;
            let behind = (bulb_index % CHASE_PERIOD + CHASE_PERIOD - head) % CHASE_PERIOD;
            if behind < CHASE_LIT {
                1.0
            } else {
                0.15
            }
        }
    }
}

/// Append one garland's wire and bulbs to the draw list.
pub fn draw_garland(x0: f32, x1: f32, y: f32, pattern: LightsPattern, t: f32, out: &mut Vec<Prim>) {
    let (wire, bulbs) = garland(x0, x1, y);
    if wire.len() >= 2 {
        out.push(Prim::Polyline { pts: wire, width: 1.5, color: WIRE });
    }
    for (i, bulb) in bulbs.iter().enumerate() {
        let b = brightness(pattern, i, t);
        let color = BULB_COLORS[bulb.color_index];
        out.push(Prim::Glow { x: bulb.x, y: bulb.y + 2.0, r: 9.0, color: color.with_alpha(0.45 * b) });
        out.push(Prim::Circle { x: bulb.x, y: bulb.y + 2.0, r: 3.0, color: color.with_alpha(0.4 + 0.6 * b) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulbs_evenly_spaced_and_inside() {
        let (_, bulbs) = garland(100.0, 500.0, 50.0);
        assert!(bulbs.len() == 14 || bulbs.len() == 15, "{}", bulbs.len());
        assert!(bulbs.iter().all(|b| b.x >= 100.0 && b.x <= 500.0));
        for pair in bulbs.windows(2) {
            assert!((pair[1].x - pair[0].x - BULB_SPACING).abs() < 0.01);
        }
        // Centred in the span.
        let left_gap = bulbs[0].x - 100.0;
        let right_gap = 500.0 - bulbs[bulbs.len() - 1].x;
        assert!((left_gap - right_gap).abs() < 0.01);
    }

    #[test]
    fn bulb_colours_cycle() {
        let (_, bulbs) = garland(0.0, 400.0, 0.0);
        for (i, b) in bulbs.iter().enumerate() {
            assert_eq!(b.color_index, i % BULB_COLORS.len());
        }
    }

    #[test]
    fn wire_sags_between_anchors() {
        let (wire, _) = garland(100.0, 500.0, 50.0);
        let first = wire.first().unwrap();
        let last = wire.last().unwrap();
        assert!((first.0 - 100.0).abs() < 0.01 && (first.1 - 50.0).abs() < 0.01);
        assert!((last.0 - 500.0).abs() < 0.01 && (last.1 - 50.0).abs() < 0.01);
        let lowest = wire.iter().map(|p| p.1).fold(f32::MIN, f32::max);
        assert!((lowest - (50.0 + SWAG_SAG)).abs() < 0.5, "{lowest}");
        assert!(wire.iter().all(|p| p.1 >= 50.0 - 0.01));
    }

    #[test]
    fn bulbs_hang_on_the_wire() {
        let (_, bulbs) = garland(100.0, 500.0, 50.0);
        assert!(bulbs.iter().all(|b| b.y >= 50.0 && b.y <= 50.0 + SWAG_SAG + 0.01));
        assert!(bulbs.iter().any(|b| b.y > 55.0));
    }

    #[test]
    fn short_or_bad_span_still_valid() {
        let (wire, bulbs) = garland(0.0, 30.0, 0.0);
        assert!(bulbs.len() <= 2);
        assert!(wire.len() >= 2);
        for (a, b) in [(10.0, 10.0), (10.0, 0.0), (f32::NAN, 5.0)] {
            let (wire, bulbs) = garland(a, b, 0.0);
            assert!(wire.is_empty() && bulbs.is_empty());
        }
    }

    #[test]
    fn brightness_in_range() {
        for pattern in [LightsPattern::Steady, LightsPattern::Twinkle, LightsPattern::Chase] {
            for i in 0..200 {
                for step in 0..200 {
                    let b = brightness(pattern, i, step as f32 * 0.1);
                    assert!((0.15..=1.0).contains(&b), "{pattern:?} {i} {b}");
                }
            }
        }
    }

    #[test]
    fn steady_is_constant() {
        assert_eq!(brightness(LightsPattern::Steady, 3, 0.0), 1.0);
        assert_eq!(brightness(LightsPattern::Steady, 9, 7.3), 1.0);
    }

    #[test]
    fn twinkle_varies_per_bulb_and_time() {
        let a = brightness(LightsPattern::Twinkle, 0, 0.0);
        assert!((a - brightness(LightsPattern::Twinkle, 0, 0.5)).abs() > 0.01);
        assert!((a - brightness(LightsPattern::Twinkle, 1, 0.0)).abs() > 0.01);
        assert!((0..50).all(|i| brightness(LightsPattern::Twinkle, i, 1.0) >= 0.35));
    }

    #[test]
    fn chase_lights_a_moving_window_of_three() {
        let lit = |t: f32| (0..12).filter(|i| brightness(LightsPattern::Chase, *i, t) > 0.9).collect::<Vec<_>>();
        let now = lit(0.0);
        assert_eq!(now.len(), 3);
        assert_ne!(now, lit(0.5));
        assert!((0..12).any(|i| brightness(LightsPattern::Chase, i, 0.0) == 0.15));
    }
}
