//! Data-driven perks over `RunModifiers` (ADR-0008, Phase 8).
//!
//! Owns the 12+ perk table, each a pure `apply(&mut RunModifiers)`. No perk
//! may add a branch to `physics.rs`; a new mechanical axis is the only way a
//! new perk enters the simulation. Table proposed to Mike for review first
//! (PRD §8; blocks Phase 8).

#[allow(dead_code)]
pub fn stub() -> ! {
    todo!("game/perk: implemented in Phase 8")
}
