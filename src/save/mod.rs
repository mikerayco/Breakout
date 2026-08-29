//! The JSON save profile: atomic writes, corruption recovery (ADR-0008,
//! FR-44/45, Phase 8).
//!
//! Owns serialisation to `profile.json` (macOS / Linux config dir), the
//! temp-file+fsync+rename atomic write, rename-aside corruption recovery,
//! and `--reset-profile`. Written at run end, never per frame. Phase 8.

pub fn stub() -> ! {
    todo!("save: implemented in Phase 8")
}
