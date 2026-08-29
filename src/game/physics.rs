//! Swept collision only; no perk/powerup special-cases (FR-15/16, AGENTS §3).
//!
//! Owns the fixed-timestep integration and circle-vs-AABB swept solver for
//! walls, paddle, bricks. Reads `RunModifiers`/`ActiveEffects`, never
//! branches on individual perks. Pure, deterministic, unit-tested (Phase 2).

pub fn stub() -> ! {
    todo!("game/physics: implemented in Phase 2")
}
