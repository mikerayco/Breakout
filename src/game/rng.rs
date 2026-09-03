//! Seeded, deterministic RNG threaded through the simulation (NFR-10).
//!
//! Owns the concrete generator used by physics/powerups; it is passed into
//! `simulate`, never read from a global, and never seeded by the wall clock.
//! Same-machine reproducibility only (PRD §8 scope).

/// Deterministic RNG: wraps `fastrand::Rng` so the simulation never touches
/// a global generator or the wall clock.
#[derive(Debug, Clone)]
pub struct Rng {
    inner: fastrand::Rng,
}

impl Rng {
    /// New generator from an explicit seed (`--seed`, run seed, or level seed).
    pub fn from_seed(seed: u64) -> Self {
        Self {
            inner: fastrand::Rng::with_seed(seed),
        }
    }

    /// Uniform `f32` in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        self.inner.f32()
    }

    /// Uniform `u32` below `n`.
    pub fn next_u32_below(&mut self, n: u32) -> u32 {
        self.inner.u32(..n)
    }

    /// `true` with probability `p` (p in 0..=1).
    pub fn gen_bool(&mut self, p: f32) -> bool {
        self.inner.f32() < p
    }
}
