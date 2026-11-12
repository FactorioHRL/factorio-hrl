use rand::seq::IndexedRandom;
pub use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Seeded generator with a value-stable algorithm: the same seed gives the same design on
/// every platform and across `rand` releases (unlike `StdRng`).
pub type Rng = ChaCha8Rng;

/// A uniformly chosen element of a non-empty slice.
pub fn pick<'a, T>(rng: &mut Rng, items: &'a [T]) -> &'a T {
    items.choose(rng).expect("cannot pick from an empty slice")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_in_range() {
        let (mut a, mut b) = (Rng::seed_from_u64(7), Rng::seed_from_u64(7));
        for _ in 0..100 {
            let v = a.random_range(-3..5);
            assert_eq!(v, b.random_range(-3..5));
            assert!((-3..5).contains(&v));
        }
        assert!(!Rng::seed_from_u64(1).random_bool(0.0));
        assert!(Rng::seed_from_u64(1).random_bool(1.0));
    }
}
