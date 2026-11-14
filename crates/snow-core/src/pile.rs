//! A pile of snow on a horizontal edge, stored as column heights.

/// Width of one column, px.
pub const COL_W: f32 = 4.0;
/// Largest height difference between neighbours that stays put, px.
pub const MAX_STEP: f32 = 3.0;
/// Smoothing passes per simulated second.
pub const RELAX_RATE: f32 = 30.0;

#[derive(Clone, Debug)]
pub struct HeightMap {
    x0: f32,
    x1: f32,
    cap: f32,
    cols: Vec<f32>,
    relax_acc: f32,
    sweep_right: bool,
}

impl HeightMap {
    pub fn new(x0: f32, x1: f32, cap: f32) -> Self {
        let n = if x1 > x0 && (x1 - x0).is_finite() { ((x1 - x0) / COL_W).ceil() as usize } else { 0 };
        Self { x0, x1, cap, cols: vec![0.0; n], relax_acc: 0.0, sweep_right: true }
    }

    pub fn x0(&self) -> f32 {
        self.x0
    }

    pub fn x1(&self) -> f32 {
        self.x1
    }

    pub fn cap(&self) -> f32 {
        self.cap
    }

    pub fn len(&self) -> usize {
        self.cols.len()
    }

    fn index(&self, x: f32) -> Option<usize> {
        if x >= self.x0 && x < self.x1 {
            Some((((x - self.x0) / COL_W) as usize).min(self.cols.len().saturating_sub(1)))
        } else {
            None
        }
    }

    /// Centre x of column `i`.
    pub fn col_x(&self, i: usize) -> f32 {
        self.x0 + (i as f32 + 0.5) * COL_W
    }

    pub fn col_height(&self, i: usize) -> f32 {
        self.cols[i]
    }

    pub fn set_col(&mut self, i: usize, h: f32) {
        self.cols[i] = h.clamp(0.0, self.cap);
    }

    /// Height at `x`; 0 outside the span.
    pub fn height_at(&self, x: f32) -> f32 {
        self.index(x).map_or(0.0, |i| self.cols[i])
    }

    /// Set every column to `h` (clamped to the cap).
    pub fn fill(&mut self, h: f32) {
        let h = h.clamp(0.0, self.cap);
        self.cols.iter_mut().for_each(|c| *c = h);
    }

    /// Add `area` px² to the column under `x`. Returns the area that did not fit.
    pub fn deposit(&mut self, x: f32, area: f32) -> f32 {
        let Some(i) = self.index(x) else { return area };
        let h = self.cols[i] + area / COL_W;
        if h > self.cap {
            self.cols[i] = self.cap;
            (h - self.cap) * COL_W
        } else {
            self.cols[i] = h;
            0.0
        }
    }

    /// Remove `fraction` of every column whose centre lies in `[xa, xb)`
    /// (either order). Returns the area removed.
    pub fn take(&mut self, xa: f32, xb: f32, fraction: f32) -> f32 {
        let (lo, hi) = if xa <= xb { (xa, xb) } else { (xb, xa) };
        let fraction = fraction.clamp(0.0, 1.0);
        let mut area = 0.0;
        for i in 0..self.cols.len() {
            let cx = self.col_x(i);
            if cx >= lo && cx < hi {
                let removed = self.cols[i] * fraction;
                self.cols[i] -= removed;
                area += removed * COL_W;
            }
        }
        area
    }

    /// Let steep parts slump toward a natural slope. Conserves snow.
    pub fn relax(&mut self, dt: f32) {
        if self.cols.len() < 2 {
            return;
        }
        self.relax_acc += dt * RELAX_RATE;
        while self.relax_acc >= 1.0 {
            self.relax_acc -= 1.0;
            self.relax_pass();
        }
    }

    fn relax_pass(&mut self) {
        let n = self.cols.len();
        // Alternate sweep direction so drifts do not creep to one side.
        self.sweep_right = !self.sweep_right;
        for k in 0..n - 1 {
            let i = if self.sweep_right { k } else { n - 2 - k };
            let d = self.cols[i] - self.cols[i + 1];
            if d.abs() > MAX_STEP {
                let moved = (d.abs() - MAX_STEP) / 2.0 * d.signum();
                self.cols[i] -= moved;
                self.cols[i + 1] += moved;
            }
        }
    }

    /// Lower every column by `rate * dt`, stopping at 0.
    pub fn melt(&mut self, dt: f32, rate: f32) {
        let d = rate * dt;
        self.cols.iter_mut().for_each(|c| *c = (*c - d).max(0.0));
    }

    pub fn total_area(&self) -> f32 {
        self.cols.iter().sum::<f32>() * COL_W
    }

    pub fn mean_height(&self) -> f32 {
        if self.cols.is_empty() {
            0.0
        } else {
            self.cols.iter().sum::<f32>() / self.cols.len() as f32
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cols.iter().all(|c| *c <= 0.0)
    }

    /// Change the span, keeping heights where old and new overlap.
    pub fn resample(&mut self, x0: f32, x1: f32) {
        let mut next = HeightMap::new(x0, x1, self.cap);
        for i in 0..next.cols.len() {
            next.cols[i] = self.height_at(next.col_x(i));
        }
        *self = next;
    }

    /// Closed outline of the pile sitting on `base_y`; empty when there is no snow.
    pub fn outline(&self, base_y: f32) -> Vec<(f32, f32)> {
        if self.is_empty() {
            return Vec::new();
        }
        let mut pts = Vec::with_capacity(self.cols.len() + 2);
        pts.push((self.x0, base_y));
        // Draw a moving average: single flakes landing make the raw columns
        // jagged, and snow should look soft.
        let n = self.cols.len();
        for i in 0..n {
            let window = &self.cols[i.saturating_sub(2)..(i + 3).min(n)];
            let h = window.iter().sum::<f32>() / window.len() as f32;
            pts.push((self.col_x(i).min(self.x1), base_y - h));
        }
        pts.push((self.x1, base_y));
        pts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform(x0: f32, x1: f32, cap: f32, h: f32) -> HeightMap {
        let mut m = HeightMap::new(x0, x1, cap);
        m.fill(h);
        m
    }

    #[test]
    fn deposit_raises_one_column() {
        let mut m = HeightMap::new(0.0, 40.0, 24.0);
        assert_eq!(m.len(), 10);
        assert_eq!(m.deposit(10.0, 8.0), 0.0);
        assert_eq!(m.height_at(10.0), 2.0);
        assert_eq!(m.height_at(20.0), 0.0);
    }

    #[test]
    fn deposit_outside_returns_all() {
        let mut m = HeightMap::new(0.0, 40.0, 24.0);
        assert_eq!(m.deposit(-5.0, 8.0), 8.0);
        assert_eq!(m.deposit(40.0, 8.0), 8.0);
        assert!(m.is_empty());
    }

    #[test]
    fn cap_returns_overflow() {
        let mut m = HeightMap::new(0.0, 40.0, 24.0);
        assert_eq!(m.deposit(10.0, 100.0), 4.0);
        assert_eq!(m.height_at(10.0), 24.0);
    }

    #[test]
    fn relax_conserves_and_smooths() {
        let mut m = HeightMap::new(0.0, 200.0, 90.0);
        m.deposit(100.0, 80.0);
        let before = m.total_area();
        for _ in 0..120 {
            m.relax(1.0 / 60.0);
        }
        assert!((m.total_area() - before).abs() < 1e-3);
        let hs: Vec<f32> = (0..m.len()).map(|i| m.height_at(i as f32 * COL_W + 1.0)).collect();
        let max_diff = hs.windows(2).map(|w| (w[0] - w[1]).abs()).fold(0.0, f32::max);
        assert!(max_diff <= 3.01, "{max_diff}");
        assert!(m.height_at(100.0) < 20.0);
    }

    #[test]
    fn relax_leaves_gentle_slopes_alone() {
        let mut m = HeightMap::new(0.0, 8.0, 90.0);
        m.deposit(1.0, 8.0);
        m.deposit(5.0, 16.0);
        m.relax(1.0);
        assert_eq!(m.height_at(1.0), 2.0);
        assert_eq!(m.height_at(5.0), 4.0);
    }

    #[test]
    fn take_removes_fraction() {
        let mut m = uniform(0.0, 40.0, 24.0, 10.0);
        assert!((m.take(0.0, 20.0, 0.8) - 160.0).abs() < 1e-3);
        assert!((m.height_at(10.0) - 2.0).abs() < 1e-4);
        assert_eq!(m.height_at(30.0), 10.0);
    }

    #[test]
    fn take_accepts_reversed_range() {
        let mut m = uniform(0.0, 40.0, 24.0, 10.0);
        assert!((m.take(20.0, 0.0, 1.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn stepping_take_hits_each_column_once() {
        let mut m = uniform(0.0, 400.0, 90.0, 10.0);
        let mut x = -7.3;
        let mut taken = 0.0;
        while x < 420.0 {
            taken += m.take(x, x + 2.67, 0.8);
            x += 2.67;
        }
        assert!((taken - 400.0 * 10.0 * 0.8).abs() < 0.5, "{taken}");
        assert!((m.height_at(200.0) - 2.0).abs() < 1e-3);
    }

    #[test]
    fn melt_floors_at_zero() {
        let mut m = uniform(0.0, 40.0, 24.0, 1.0);
        m.melt(1.0, 0.25);
        assert!((m.height_at(10.0) - 0.75).abs() < 1e-5);
        m.melt(100.0, 0.25);
        assert_eq!(m.height_at(10.0), 0.0);
        assert!(m.is_empty());
    }

    #[test]
    fn resample_keeps_overlap() {
        let mut m = uniform(0.0, 40.0, 24.0, 5.0);
        m.resample(20.0, 80.0);
        assert_eq!(m.x0(), 20.0);
        assert_eq!(m.x1(), 80.0);
        assert_eq!(m.height_at(30.0), 5.0);
        assert_eq!(m.height_at(60.0), 0.0);
        assert_eq!(m.height_at(10.0), 0.0);
    }

    #[test]
    fn zero_width_is_safe() {
        for (a, b) in [(10.0, 10.0), (10.0, 5.0), (f32::NAN, 5.0)] {
            let mut m = HeightMap::new(a, b, 24.0);
            assert_eq!(m.len(), 0);
            assert_eq!(m.deposit(10.0, 8.0), 8.0);
            assert_eq!(m.take(0.0, 100.0, 1.0), 0.0);
            m.relax(1.0);
            m.melt(1.0, 1.0);
            assert_eq!(m.mean_height(), 0.0);
            assert_eq!(m.total_area(), 0.0);
            assert!(m.outline(100.0).is_empty());
        }
    }

    #[test]
    fn mean_and_total() {
        let m = uniform(0.0, 40.0, 24.0, 5.0);
        assert_eq!(m.mean_height(), 5.0);
        assert_eq!(m.total_area(), 200.0);
    }

    #[test]
    fn outline_empty_when_no_snow() {
        assert!(HeightMap::new(0.0, 40.0, 24.0).outline(100.0).is_empty());
    }

    #[test]
    fn outline_softens_single_column_spikes() {
        let mut m = uniform(0.0, 80.0, 24.0, 2.0);
        m.deposit(41.0, 12.0);
        let top = m.outline(100.0).iter().map(|p| p.1).fold(f32::MAX, f32::min);
        assert!(top > 100.0 - 5.0 && top < 100.0 - 2.0, "{top}");
    }

    #[test]
    fn outline_follows_heights() {
        let m = uniform(0.0, 40.0, 24.0, 5.0);
        let o = m.outline(100.0);
        assert_eq!(o.first(), Some(&(0.0, 100.0)));
        assert_eq!(o.last(), Some(&(40.0, 100.0)));
        assert!(o.iter().any(|p| (p.1 - 95.0).abs() < 1e-4));
        assert!(o.iter().all(|p| p.0 >= 0.0 && p.0 <= 40.0 && p.1 <= 100.0 && p.1 >= 95.0));
    }
}
