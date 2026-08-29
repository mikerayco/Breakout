//! Named colours from MOCKUP.md §2 (normative; locked verbatim in Phase 4).
//!
//! Owns every colour constant used by rendering. No magic numbers outside
//! `game/tuning.rs` and here (AGENTS §3). Must never encode game rules.
//!
//! The whole MOCKUP palette is declared now even though Phase 1 uses a
//! subset; Phase 4 (juice) consumes the rest. Until then the unused statics
//! are intentional.
#![allow(dead_code)]

use std::sync::LazyLock;

use tiny_skia::Color;

/// Construct an opaque tiny-skia color from hex RGB (0xRRGGBB).
fn rgb(hex: u32) -> Color {
    Color::from_rgba8(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
        0xFF,
    )
}

macro_rules! color {
    ($name:ident, $hex:expr, $doc:expr) => {
        #[doc = $doc]
        pub static $name: LazyLock<Color> = LazyLock::new(|| rgb($hex));
    };
}

color!(
    BG_VOID,
    0x07080f,
    "Area outside the 320×240 logical screen."
);
color!(BG_DEEP, 0x0b0d17, "Play area background.");
color!(BG_HUD, 0x10131f, "HUD band background.");
color!(
    GRID_LINE,
    0x141829,
    "Faint background grid in the play area."
);
color!(BEZEL, 0x232a45, "Walls and the HUD rule.");
color!(
    BEZEL_LIT,
    0x3a4370,
    "Bezel inner highlight; wall flash on impact."
);
color!(TEXT, 0xe6edf7, "Primary HUD text.");
color!(TEXT_DIM, 0x7b86a8, "Labels, inactive elements.");
color!(PADDLE, 0x4de3ff, "Paddle body.");
color!(PADDLE_CAP, 0xa8f4ff, "Paddle top edge.");
color!(BRICK_1, 0x4d96ff, "1 HP brick.");
color!(BRICK_2, 0x06d6a0, "2 HP brick.");
color!(BRICK_3, 0xffd166, "3 HP brick.");
color!(BRICK_4, 0xff9f1c, "4 HP brick.");
color!(BRICK_5, 0xff4d6d, "5 HP brick.");
color!(BRICK_STEEL, 0x6b7394, "Indestructible brick.");
color!(BRICK_EXPLOSIVE, 0xff2e63, "Explosive brick.");
color!(POWERUP, 0xc77dff, "Powerup capsules and their trails.");
color!(COMBO, 0xff6ec7, "Combo counter at ×3 and above.");
color!(DANGER, 0xff2e63, "Last life, low timers.");
color!(BALL, 0xfffbe8, "Ball core.");
color!(BALL_GLOW, 0xffd166, "Ball trail and bloom tint.");
