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
    0x100e1a,
    "Area outside the 320×240 logical screen."
);
color!(BG_DEEP, 0x221c38, "Play area background.");
color!(BG_HUD, 0x2f2750, "HUD band background.");
color!(
    GRID_LINE,
    0x453a6b,
    "Faint background grid in the play area."
);
color!(BEZEL, 0xa8763e, "Walls and the HUD rule.");
color!(
    BEZEL_LIT,
    0xe0aa6e,
    "Bezel inner highlight; wall flash on impact."
);
color!(TEXT, 0xfff6e0, "Primary HUD text.");
color!(TEXT_DIM, 0xb8a8d8, "Labels, inactive elements.");
color!(PADDLE, 0x2fd47e, "Paddle body.");
color!(PADDLE_CAP, 0xa9f5c9, "Paddle top edge.");
color!(BRICK_1, 0x3fa7ff, "1 HP brick.");
color!(BRICK_2, 0x3ddc84, "2 HP brick.");
color!(BRICK_3, 0xffc93c, "3 HP brick.");
color!(BRICK_4, 0xff7a3d, "4 HP brick.");
color!(BRICK_5, 0xf43f6e, "5 HP brick.");
color!(BRICK_STEEL, 0x9a97b8, "Indestructible brick.");
color!(BRICK_EXPLOSIVE, 0xf4253f, "Explosive brick.");
color!(POWERUP, 0xa55cff, "Powerup capsules and their trails.");
color!(COMBO, 0xff4fa3, "Combo counter at ×3 and above.");
color!(DANGER, 0xff3050, "Last life, low timers.");
color!(BALL, 0xfff3d6, "Ball core.");
color!(BALL_GLOW, 0xffc93c, "Ball trail and bloom tint.");
