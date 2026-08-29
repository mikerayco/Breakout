//! The HUD band and overlays (FR-12, MOCKUP §3).
//!
//! Owns score/lives/level/combo/powerup-timer/mute drawing in the 20px HUD
//! band. Must not compute game state — it only renders what `game/` says.
//! Phase 1 draws the debug overlay; the full HUD arrives in Phase 2.

pub fn stub() -> ! {
    todo!("render/hud: implemented in Phase 1 (overlay) / Phase 2 (HUD)")
}
