//! Kitty graphics escape writer and the `t=d` base64 fallback (ADR-0002).
//!
//! Owns the direct-base64 transport (4096-byte chunks), used automatically
//! when shared memory is unavailable and forced by `BREAKOUT_TRANSPORT=direct`.
//! Correctness required; the frame budget is not (Phase 1).

pub fn stub() -> ! {
    todo!("term/kgp: implemented in Phase 1")
}
