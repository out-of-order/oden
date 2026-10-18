// A small deterministic linear congruential generator, matching d3-force's
// `lcg.js`. It exists purely to perturb coincident points (so distance-based
// forces don't divide by zero / produce NaNs), while keeping simulation runs
// reproducible from one tick to the next.
const A: u64 = 1_664_525;
const C: u64 = 1_013_904_223;
const M: u64 = 1 << 32;

#[derive(Debug, Clone)]
pub(crate) struct Lcg {
    state: u64,
}

impl Lcg {
    pub(crate) fn new() -> Self {
        Self { state: 1 }
    }

    fn next(&mut self) -> f32 {
        self.state = (A.wrapping_mul(self.state).wrapping_add(C)) % M;
        self.state as f32 / M as f32
    }

    /// Matches d3-force's `jiggle`: a tiny random nudge used to perturb
    /// coincident points so forces don't produce NaN/zero vectors.
    pub(crate) fn jiggle(&mut self) -> f32 {
        (self.next() - 0.5) * 1e-6
    }
}

impl Default for Lcg {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Lcg;

    #[test]
    fn jiggle_is_deterministic_and_tiny() {
        let mut a = Lcg::new();
        let mut b = Lcg::new();
        for _ in 0..5 {
            let ja = a.jiggle();
            let jb = b.jiggle();
            assert_eq!(ja, jb);
            assert!(ja.abs() < 1e-5);
        }
    }
}
