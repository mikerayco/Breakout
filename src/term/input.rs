//! Keyboard protocol input and the held-key set (ADR-0004, Phase 3).
//!
//! Owns crossterm key events (Press/Repeat/Release), the held-key set, and
//! the repeat-decay fallback when the protocol is unavailable. Game logic
//! reads `InputState` only (Phase 3).

#[allow(dead_code)]
pub fn stub() -> ! {
    todo!("term/input: implemented in Phase 3")
}
