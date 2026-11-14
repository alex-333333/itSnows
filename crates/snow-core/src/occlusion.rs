//! Which parts of each window's top edge can actually be seen.

use crate::world::{SurfaceIn, SurfaceKind};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WinRect {
    pub id: u64,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl WinRect {
    fn is_sane(&self) -> bool {
        [self.left, self.top, self.right, self.bottom].iter().all(|v| v.is_finite()) && self.right > self.left && self.bottom > self.top
    }
}

/// Visible stretches narrower than this are ignored, px.
pub const MIN_SEGMENT: f32 = 8.0;

/// Remove `[cut_a, cut_b)` from a sorted list of disjoint intervals.
fn subtract(intervals: &mut Vec<(f32, f32)>, cut_a: f32, cut_b: f32) {
    let mut out = Vec::with_capacity(intervals.len() + 1);
    for &(a, b) in intervals.iter() {
        if cut_b <= a || cut_a >= b {
            out.push((a, b));
            continue;
        }
        if cut_a > a {
            out.push((a, cut_a));
        }
        if cut_b < b {
            out.push((cut_b, b));
        }
    }
    *intervals = out;
}

/// `windows` is front-to-back. Returns one `WindowTop` surface per window
/// whose top edge has a visible part: on screen, above the ground line, and
/// not covered by a window in front of it.
pub fn window_top_surfaces(windows: &[WinRect], screen_w: f32, _screen_h: f32, ground_y: f32) -> Vec<SurfaceIn> {
    let mut out = Vec::new();
    if !(screen_w > 0.0) || !(ground_y > 0.0) {
        return out;
    }
    for (i, win) in windows.iter().enumerate() {
        if !win.is_sane() || win.top <= 0.0 || win.top >= ground_y {
            continue;
        }
        let x0 = win.left.max(0.0);
        let x1 = win.right.min(screen_w);
        if x1 - x0 < MIN_SEGMENT {
            continue;
        }
        let mut visible = vec![(x0, x1)];
        for front in windows[..i].iter().filter(|f| f.is_sane()) {
            if front.top <= win.top && win.top < front.bottom {
                subtract(&mut visible, front.left, front.right);
                if visible.is_empty() {
                    break;
                }
            }
        }
        visible.retain(|(a, b)| b - a >= MIN_SEGMENT);
        if !visible.is_empty() {
            out.push(SurfaceIn { id: win.id, x0, x1, y: win.top, kind: SurfaceKind::WindowTop, visible });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    fn win(id: u64, l: f32, t: f32, r: f32, b: f32) -> WinRect {
        WinRect { id, left: l, top: t, right: r, bottom: b }
    }

    const W: f32 = 1920.0;
    const H: f32 = 1080.0;
    const G: f32 = 1040.0;

    #[test]
    fn lone_window_full_top() {
        let s = window_top_surfaces(&[win(7, 100.0, 200.0, 500.0, 600.0)], W, H, G);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].id, s[0].x0, s[0].x1, s[0].y), (7, 100.0, 500.0, 200.0));
        assert_eq!(s[0].kind, SurfaceKind::WindowTop);
        assert_eq!(s[0].visible, vec![(100.0, 500.0)]);
    }

    #[test]
    fn front_window_splits_back() {
        let s = window_top_surfaces(&[win(1, 200.0, 100.0, 300.0, 400.0), win(2, 100.0, 200.0, 500.0, 600.0)], W, H, G);
        let back = s.iter().find(|s| s.id == 2).unwrap();
        assert_eq!(back.visible, vec![(100.0, 200.0), (300.0, 500.0)]);
        let front = s.iter().find(|s| s.id == 1).unwrap();
        assert_eq!(front.visible, vec![(200.0, 300.0)]);
    }

    #[test]
    fn window_behind_does_not_cover_front() {
        let s = window_top_surfaces(&[win(1, 100.0, 200.0, 500.0, 600.0), win(2, 0.0, 0.0, 1920.0, 1000.0)], W, H, G);
        assert_eq!(s.iter().find(|s| s.id == 1).unwrap().visible, vec![(100.0, 500.0)]);
    }

    #[test]
    fn fully_covered_is_omitted() {
        let s = window_top_surfaces(&[win(1, 0.0, 50.0, 1000.0, 900.0), win(2, 100.0, 200.0, 500.0, 600.0)], W, H, G);
        assert!(s.iter().all(|s| s.id != 2));
    }

    #[test]
    fn front_window_above_or_below_edge_does_not_cover() {
        // Front window ends above the back window's top edge.
        let s = window_top_surfaces(&[win(1, 100.0, 50.0, 500.0, 150.0), win(2, 100.0, 200.0, 500.0, 600.0)], W, H, G);
        assert_eq!(s.iter().find(|s| s.id == 2).unwrap().visible, vec![(100.0, 500.0)]);
    }

    #[test]
    fn maximised_has_no_top() {
        assert!(window_top_surfaces(&[win(1, 0.0, 0.0, 1920.0, 1040.0)], W, H, G).is_empty());
        assert!(window_top_surfaces(&[win(1, -8.0, -8.0, 1928.0, 1048.0)], W, H, G).is_empty());
    }

    #[test]
    fn clipped_to_screen() {
        let s = window_top_surfaces(&[win(1, -50.0, 200.0, 300.0, 600.0), win(2, 1800.0, 300.0, 2100.0, 700.0)], W, H, G);
        assert_eq!(s.iter().find(|s| s.id == 1).unwrap().visible, vec![(0.0, 300.0)]);
        assert_eq!(s.iter().find(|s| s.id == 2).unwrap().visible, vec![(1800.0, 1920.0)]);
    }

    #[test]
    fn below_ground_omitted() {
        assert!(window_top_surfaces(&[win(1, 100.0, 1040.0, 500.0, 1080.0)], W, H, G).is_empty());
        assert!(window_top_surfaces(&[win(1, 100.0, 1060.0, 500.0, 1080.0)], W, H, G).is_empty());
    }

    #[test]
    fn slivers_dropped() {
        // 5 px left visible on the right of the back window.
        let s = window_top_surfaces(&[win(1, 0.0, 100.0, 495.0, 400.0), win(2, 100.0, 200.0, 500.0, 600.0)], W, H, G);
        assert!(s.iter().all(|s| s.id != 2));
    }

    #[test]
    fn degenerate_rects_are_safe() {
        let odd = [
            win(1, 100.0, 100.0, 100.0, 300.0),
            win(2, 500.0, 100.0, 400.0, 300.0),
            win(3, f32::NAN, 100.0, 400.0, 300.0),
            win(4, 100.0, f32::INFINITY, 400.0, 300.0),
            win(5, -5000.0, 300.0, -4000.0, 500.0),
        ];
        assert!(window_top_surfaces(&odd, W, H, G).is_empty());
        assert!(window_top_surfaces(&odd, 0.0, 0.0, 0.0).is_empty());
        assert!(window_top_surfaces(&[], W, H, G).is_empty());

        let mut rng = Rng::new(11);
        let many: Vec<WinRect> = (0..500)
            .map(|i| {
                let l = rng.range(-300.0, 2000.0);
                let t = rng.range(-100.0, 1200.0);
                win(i, l, t, l + rng.range(-50.0, 900.0), t + rng.range(-50.0, 700.0))
            })
            .collect();
        for s in window_top_surfaces(&many, W, H, G) {
            assert!(!s.visible.is_empty());
            assert!(s.y > 0.0 && s.y < G);
            let mut prev_end = f32::MIN;
            for (a, b) in &s.visible {
                assert!(b - a >= MIN_SEGMENT, "{a} {b}");
                assert!(*a >= prev_end, "not sorted/disjoint");
                assert!(*a >= 0.0 && *b <= W && *a >= s.x0 && *b <= s.x1);
                prev_end = *b;
            }
        }
    }
}
