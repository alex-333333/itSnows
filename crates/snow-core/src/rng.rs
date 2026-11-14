//! Small deterministic RNG (xorshift64*), so the core needs no dependencies
//! and tests are reproducible.

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Scramble the seed so small seeds (0, 1, 2…) still diverge quickly.
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        Self(if z == 0 { 0x2545_F491_4F6C_DD1D } else { z })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..100 {
            let x = a.next_f32();
            assert_eq!(x, b.next_f32());
            assert!((0.0..1.0).contains(&x), "{x}");
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let same = (0..20).filter(|_| a.next_f32() == b.next_f32()).count();
        assert!(same < 20);
    }

    #[test]
    fn range_stays_inside() {
        let mut r = Rng::new(3);
        for _ in 0..1000 {
            let x = r.range(-5.0, 5.0);
            assert!((-5.0..5.0).contains(&x), "{x}");
        }
    }

    #[test]
    fn chance_extremes() {
        let mut r = Rng::new(4);
        assert!((0..100).all(|_| !r.chance(0.0)));
        assert!((0..100).all(|_| r.chance(1.0)));
    }
}
