//! Software replay of the core's draw list, for looking at frames on any OS.

use snow_core::{Prim, Rgba};
use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Point, RadialGradient, Rect, SpreadMode,
    Stroke, Transform,
};

fn color(c: Rgba) -> Color {
    Color::from_rgba(c.r.clamp(0.0, 1.0), c.g.clamp(0.0, 1.0), c.b.clamp(0.0, 1.0), c.a.clamp(0.0, 1.0)).unwrap_or(Color::TRANSPARENT)
}

fn solid(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color(c));
    p.anti_alias = true;
    p
}

fn stroke(width: f32) -> Stroke {
    Stroke { width, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() }
}

fn poly_path(pts: &[(f32, f32)], close: bool) -> Option<tiny_skia::Path> {
    let (first, rest) = pts.split_first()?;
    let mut pb = PathBuilder::new();
    pb.move_to(first.0, first.1);
    for p in rest {
        pb.line_to(p.0, p.1);
    }
    if close {
        pb.close();
    }
    pb.finish()
}

fn round_rect_path(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Option<tiny_skia::Path> {
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
    let r = radius.max(0.0).min(w / 2.0).min(h / 2.0);
    if r <= 0.0 {
        return Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?));
    }
    // Quadratic corners are close enough for a preview.
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

pub fn render(prims: &[Prim], w: u32, h: u32, background: Rgba) -> Pixmap {
    let mut pm = Pixmap::new(w.max(1), h.max(1)).expect("pixmap size");
    pm.fill(color(background));
    let id = Transform::identity();
    for prim in prims {
        match prim {
            Prim::Circle { x, y, r, color: c } => {
                if let Some(path) = PathBuilder::from_circle(*x, *y, *r) {
                    pm.fill_path(&path, &solid(*c), FillRule::Winding, id, None);
                }
            }
            Prim::Glow { x, y, r, color: c } => {
                let Some(path) = PathBuilder::from_circle(*x, *y, *r) else { continue };
                let centre = Point::from_xy(*x, *y);
                let stops = vec![GradientStop::new(0.0, color(*c)), GradientStop::new(1.0, color(c.with_alpha(0.0)))];
                let Some(shader) = RadialGradient::new(centre, 0.0, centre, *r, stops, SpreadMode::Pad, id) else { continue };
                let paint = Paint { shader, anti_alias: true, ..Paint::default() };
                pm.fill_path(&path, &paint, FillRule::Winding, id, None);
            }
            Prim::Line { x0, y0, x1, y1, width, color: c } => {
                if *width > 0.0 {
                    if let Some(path) = poly_path(&[(*x0, *y0), (*x1, *y1)], false) {
                        pm.stroke_path(&path, &solid(*c), &stroke(*width), id, None);
                    }
                }
            }
            Prim::Polyline { pts, width, color: c } => {
                if *width > 0.0 && pts.len() >= 2 {
                    if let Some(path) = poly_path(pts, false) {
                        pm.stroke_path(&path, &solid(*c), &stroke(*width), id, None);
                    }
                }
            }
            Prim::Polygon { pts, color: c } => {
                if pts.len() >= 3 {
                    if let Some(path) = poly_path(pts, true) {
                        pm.fill_path(&path, &solid(*c), FillRule::Winding, id, None);
                    }
                }
            }
            Prim::RoundRect { x, y, w, h, radius, color: c } => {
                if let Some(path) = round_rect_path(*x, *y, *w, *h, *radius) {
                    pm.fill_path(&path, &solid(*c), FillRule::Winding, id, None);
                }
            }
        }
    }
    pm
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: Rgba = Rgba::new(0.0, 0.0, 0.0, 1.0);

    fn px(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let c = p.pixel(x, y).unwrap();
        (c.red(), c.green(), c.blue())
    }

    #[test]
    fn circle_colours_its_centre() {
        let p = render(&[Prim::Circle { x: 10.0, y: 10.0, r: 5.0, color: Rgba::WHITE }], 20, 20, BLACK);
        assert_eq!(px(&p, 10, 10), (255, 255, 255));
        assert_eq!(px(&p, 0, 0), (0, 0, 0));
    }

    #[test]
    fn glow_fades_outward() {
        let p = render(&[Prim::Glow { x: 20.0, y: 20.0, r: 10.0, color: Rgba::WHITE }], 40, 40, BLACK);
        let centre = px(&p, 20, 20).0;
        let mid = px(&p, 27, 20).0;
        assert!(centre > mid && mid > 0, "{centre} {mid}");
        assert_eq!(px(&p, 31, 20), (0, 0, 0));
    }

    #[test]
    fn polygon_fills_interior() {
        let pts = vec![(5.0, 5.0), (15.0, 5.0), (15.0, 15.0), (5.0, 15.0)];
        let p = render(&[Prim::Polygon { pts, color: Rgba::new(1.0, 0.0, 0.0, 1.0) }], 20, 20, BLACK);
        assert_eq!(px(&p, 10, 10), (255, 0, 0));
        assert_eq!(px(&p, 2, 2), (0, 0, 0));
    }

    #[test]
    fn line_and_polyline_draw_along_path() {
        let green = Rgba::new(0.0, 1.0, 0.0, 1.0);
        let p = render(
            &[
                Prim::Line { x0: 2.0, y0: 5.5, x1: 18.0, y1: 5.5, width: 3.0, color: green },
                Prim::Polyline { pts: vec![(2.0, 14.5), (10.0, 14.5), (18.0, 14.5)], width: 3.0, color: green },
            ],
            20,
            20,
            BLACK,
        );
        assert_eq!(px(&p, 10, 5), (0, 255, 0));
        assert_eq!(px(&p, 10, 14), (0, 255, 0));
        assert_eq!(px(&p, 10, 10), (0, 0, 0));
    }

    #[test]
    fn round_rect_fills_centre_not_corner() {
        let p = render(&[Prim::RoundRect { x: 2.0, y: 2.0, w: 16.0, h: 16.0, radius: 6.0, color: Rgba::WHITE }], 20, 20, BLACK);
        assert_eq!(px(&p, 10, 10), (255, 255, 255));
        assert_eq!(px(&p, 2, 2), (0, 0, 0));
    }

    #[test]
    fn alpha_blends_with_background() {
        let p = render(&[Prim::Circle { x: 10.0, y: 10.0, r: 5.0, color: Rgba::new(1.0, 1.0, 1.0, 0.5) }], 20, 20, BLACK);
        let v = px(&p, 10, 10).0;
        assert!((120..=135).contains(&v), "{v}");
    }

    #[test]
    fn empty_list_is_background() {
        let p = render(&[], 4, 4, Rgba::new(0.0, 0.0, 1.0, 1.0));
        assert_eq!(px(&p, 1, 1), (0, 0, 255));
    }

    #[test]
    fn degenerate_prims_do_not_panic() {
        let c = Rgba::WHITE;
        render(
            &[
                Prim::Circle { x: 1.0, y: 1.0, r: 0.0, color: c },
                Prim::Glow { x: 1.0, y: 1.0, r: 0.0, color: c },
                Prim::Polygon { pts: vec![], color: c },
                Prim::Polyline { pts: vec![(1.0, 1.0)], width: 1.0, color: c },
                Prim::RoundRect { x: 0.0, y: 0.0, w: 0.0, h: 5.0, radius: 9.0, color: c },
                Prim::Line { x0: 1.0, y0: 1.0, x1: 1.0, y1: 1.0, width: 0.0, color: c },
            ],
            4,
            4,
            BLACK,
        );
    }
}
