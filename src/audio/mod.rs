//! Audio via kira behind a `Sound` trait (FR-46..49, Phase 7).
//!
//! Owns the ten effects (WAV, from `assets/audio/`, compiled in with
//! `include_bytes!`), the looping music bed with hit-stop ducking, the combo
//! pitch ramp and the persisted mute state. A no-op backend keeps the game
//! silent-but-working when audio is unavailable (FR-49). Phase 7.

#[allow(dead_code)]
pub fn stub() -> ! {
    todo!("audio: implemented in Phase 7")
}
