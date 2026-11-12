/// SplitMix64: a small, deterministic generator, to avoid pulling in a dependency.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number from the range `[lo, hi)`.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        assert!(lo < hi, "empty range");
        lo + (self.next_u64() % (hi - lo) as u64) as i32
    }

    pub fn chance(&mut self, p: f64) -> bool {
        ((self.next_u64() >> 11) as f64 / (1u64 << 53) as f64) < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.range(0, items.len() as i32) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_in_range() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        for _ in 0..100 {
            let v = a.range(-3, 5);
            assert_eq!(v, b.range(-3, 5));
            assert!((-3..5).contains(&v));
        }
        assert!(!Rng::new(1).chance(0.0));
        assert!(Rng::new(1).chance(1.0));
    }
}
