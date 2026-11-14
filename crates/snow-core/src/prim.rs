//! Draw-list primitives. Coordinates are physical pixels, origin top-left,
//! y down. Colours are straight (non-premultiplied) RGBA in 0..=1.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const WHITE: Rgba = Rgba::new(1.0, 1.0, 1.0, 1.0);

    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    Circle { x: f32, y: f32, r: f32, color: Rgba },
    /// Soft disc: alpha falls linearly to 0 at `r`.
    Glow { x: f32, y: f32, r: f32, color: Rgba },
    Line { x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: Rgba },
    Polyline { pts: Vec<(f32, f32)>, width: f32, color: Rgba },
    /// Filled, implicitly closed.
    Polygon { pts: Vec<(f32, f32)>, color: Rgba },
    RoundRect { x: f32, y: f32, w: f32, h: f32, radius: f32, color: Rgba },
}
