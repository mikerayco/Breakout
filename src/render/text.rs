//! The hand-rolled 5x7 bitmap font (ADR-0005: no font crate).
//!
//! Owns glyph bitmaps (uppercase, digits, `:./×%`) and text drawing at 1x
//! and doubled size. Phase 1 needs just enough for the overlay; the full
//! character set is locked when the HUD lands in Phase 2.

pub fn stub() -> ! {
    todo!("render/text: implemented in Phase 1")
}
