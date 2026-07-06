//! Deterministic xorshift RNG, seed-compatible with `sim-tests/src/scenarios.rs`
//! so chaos runs reproduce from a printed seed.

/// Simple xorshift64 generator. Same-seed → same sequence, forever.
pub struct Rng(u64);

impl Rng {
    /// `seed | 1` avoids the xorshift fixed point at 0.
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Uniform integer in `[0, n)`. Returns 0 when `n == 0`.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// Pick a reference to a random element, or `None` if the slice is empty.
    pub fn choose<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            Some(&items[self.below(items.len())])
        }
    }

    /// Derive an independent child seed for round `idx`.
    pub fn derive(seed: u64, idx: u64) -> u64 {
        let mut r = Rng::new(seed ^ idx.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        r.next_u64()
    }
}
