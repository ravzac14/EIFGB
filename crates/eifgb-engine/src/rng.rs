//! A deterministic random number generator (SplitMix64).
//!
//! Its whole state is one `u64` stored in [`crate::GameState`], so undo
//! and restore also rewind randomness. Replaying the same commands from
//! the same save always gives the same results, which matters for bug
//! reports and automated playtests.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform integer in `lo..=hi`. Returns `lo` if `hi < lo`.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        let span = (hi - lo) as u64 + 1;
        // Rejection sampling avoids modulo bias.
        let zone = u64::MAX - (u64::MAX % span);
        loop {
            let r = self.next_u64();
            if r < zone {
                return lo + (r % span) as i64;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn range_stays_in_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let v = r.range(1, 6);
            assert!((1..=6).contains(&v));
        }
        assert_eq!(r.range(5, 5), 5);
        assert_eq!(r.range(5, 1), 5);
    }
}
