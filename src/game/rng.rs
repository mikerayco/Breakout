//! Seeded, deterministic RNG threaded through the simulation (NFR-10).
//!
//! Owns the concrete generator used by physics/powerups; it is passed into
//! `simulate`, never read from a global, and never seeded by the wall clock.
//! Same-machine reproducibility only (PRD §8 scope). Phase 2.

#[allow(dead_code)]
pub fn stub() -> ! {
    todo!("game/rng: implemented in Phase 2")
}
