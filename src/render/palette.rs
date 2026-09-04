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
    0x171422,
    "Area outside the 320×240 logical screen."
);
color!(BG_DEEP, 0x242031, "Play area background.");
color!(BG_HUD, 0x2e2845, "HUD band background.");
color!(
    GRID_LINE,
    0x3a3354,
    "Faint background grid in the play area."
);
color!(BEZEL, 0x9c7a54, "Walls and the HUD rule.");
color!(
    BEZEL_LIT,
    0xc9a876,
    "Bezel inner highlight; wall flash on impact."
);
color!(TEXT, 0xfff8e7, "Primary HUD text.");
color!(TEXT_DIM, 0xa89bc4, "Labels, inactive elements.");
color!(PADDLE, 0x8fd6b4, "Paddle body.");
color!(PADDLE_CAP, 0xd8f3dc, "Paddle top edge.");
color!(BRICK_1, 0x7fb6d9, "1 HP brick.");
color!(BRICK_2, 0x8fd6a0, "2 HP brick.");
color!(BRICK_3, 0xffd166, "3 HP brick.");
color!(BRICK_4, 0xff9e6d, "4 HP brick.");
color!(BRICK_5, 0xef6f8b, "5 HP brick.");
color!(BRICK_STEEL, 0x8d8aa8, "Indestructible brick.");
color!(BRICK_EXPLOSIVE, 0xe5484d, "Explosive brick.");
color!(POWERUP, 0xb892ff, "Powerup capsules and their trails.");
color!(COMBO, 0xff7ab8, "Combo counter at ×3 and above.");
color!(DANGER, 0xef476f, "Last life, low timers.");
color!(BALL, 0xfff3d6, "Ball core.");
color!(BALL_GLOW, 0xffd166, "Ball trail and bloom tint.");
