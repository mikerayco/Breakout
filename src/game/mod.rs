//! Pure simulation — no I/O, no terminal, no wall clock (ADR-0005).
//!
//! Owns the game state machine, fixed-timestep physics, levels, powerups,
//! perks, the 8-level run, scoring and the seeded deterministic RNG. This
//! module must never import `term/` or `render/`; that rule is what keeps
//! the simulation deterministic and unit-testable (NFR-10, Phase 2+).

#[allow(dead_code)]
pub mod level;
#[allow(dead_code)]
pub mod perk;
#[allow(dead_code)]
pub mod physics;
#[allow(dead_code)]
pub mod powerup;
#[allow(dead_code)]
pub mod rng;
#[allow(dead_code)]
pub mod run;
#[allow(dead_code)]
pub mod score;
#[allow(dead_code)]
pub mod state;
#[allow(dead_code)]
pub mod tuning;
