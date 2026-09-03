//! EVERY numeric tuning constant, documented, in one file (FR-22, AGENTS §3).
//!
//! Owns all gameplay numbers (speeds, ramps, caps, drop rates, durations).
//! No magic numbers anywhere else in the simulation. Values approved by
//! Mike 2026-09-03 (review-gated proposal TODO-a6652e6a).
//!
//! Units: logical pixels (320x240 space, ADR-0003), seconds, degrees where
//! named. All speeds are logical px/s unless noted.

// --- Fixed timestep (ADR-0006, locked) ---
/// Simulation step, seconds (240 Hz).
pub const DT: f32 = 1.0 / 240.0;
/// Max simulation steps per rendered frame; remainder is dropped (no spiral).
pub const MAX_CATCHUP: u8 = 5;

// --- Geometry (MOCKUP §1, verbatim) ---
/// Logical screen size (ADR-0003).
pub const LOGICAL_W: f32 = 320.0;
/// Logical screen size (ADR-0003).
pub const LOGICAL_H: f32 = 240.0;
/// Play area origin (top-left of the area the ball may occupy).
pub const PLAY_X: f32 = 7.0;
/// Play area origin.
pub const PLAY_Y: f32 = 28.0;
/// Play area size.
pub const PLAY_W: f32 = 306.0;
/// Play area size.
pub const PLAY_H: f32 = 212.0;
/// Right wall (exclusive): ball centre must stay < this minus radius.
pub const PLAY_RIGHT: f32 = PLAY_X + PLAY_W; // 313.0
/// Top wall: ball centre must stay > this plus radius.
pub const PLAY_TOP: f32 = PLAY_Y; // 28.0
/// Brick grid origin (top-left of cell 0,0).
pub const GRID_ORIGIN_X: f32 = 7.0;
/// Brick grid origin.
pub const GRID_ORIGIN_Y: f32 = 36.0;
/// One brick cell, including the 1px gap (drawn 16x7).
pub const BRICK_CELL_W: f32 = 17.0;
/// One brick cell.
pub const BRICK_CELL_H: f32 = 8.0;
/// Grid dimensions (ADR-0007: 18 cols, 1-14 rows).
pub const GRID_COLS: usize = 18;
/// Grid dimensions.
pub const GRID_ROWS_MAX: usize = 14;
/// Drawn brick size (cell minus gap).
pub const BRICK_DRAW_W: f32 = 16.0;
/// Drawn brick size.
pub const BRICK_DRAW_H: f32 = 7.0;
/// Paddle rest line (top edge of paddle).
pub const PADDLE_Y: f32 = 222.0;
/// Default paddle width.
pub const PADDLE_W: f32 = 51.0;
/// Wide paddle width (powerup).
pub const PADDLE_W_WIDE: f32 = 75.0;
/// Narrow paddle width (perk).
pub const PADDLE_W_NARROW: f32 = 35.0;
/// Paddle height (always).
pub const PADDLE_H: f32 = 5.0;
/// Ball radius.
pub const BALL_R: f32 = 3.0;
/// Kill line: ball lost when centre passes this y.
pub const KILL_Y: f32 = 240.0;

// --- Ball speed (FR-22) ---
/// Base ball speed at level 0 with no bricks destroyed, px/s.
pub const BALL_BASE_SPEED: f32 = 160.0;
/// Added per brick destroyed this level, px/s.
pub const BALL_SPEED_PER_BRICK: f32 = 1.5;
/// Added per level index (0-7), px/s.
pub const BALL_SPEED_PER_LEVEL: f32 = 10.0;
/// Hard cap, px/s. At 240 Hz this moves 1.4px/step: no tunnelling (FR-16).
pub const BALL_MAX_SPEED: f32 = 340.0;

// --- Paddle feel (FR-13) ---
/// Paddle top speed, px/s.
pub const PADDLE_MAX_VEL: f32 = 260.0;
/// Paddle acceleration toward held direction, px/s^2.
pub const PADDLE_ACCEL: f32 = 1800.0;
/// Paddle friction when no key held, px/s^2.
pub const PADDLE_FRICTION: f32 = 2200.0;
/// Fraction of paddle velocity imparted to ball x on contact.
pub const PADDLE_MOMENTUM: f32 = 0.25;

// --- Reflection (FR-15) ---
/// Max bounce angle from vertical at paddle edge, degrees.
pub const ENGLISH_MAX_DEG: f32 = 65.0;
/// Minimum vertical fraction of ball velocity after any bounce.
/// Kills horizontal loops: |vy| is never below this fraction of speed.
pub const MIN_VERTICAL_FRAC: f32 = 0.40;
/// Launch angle range from ball offset on paddle, degrees from vertical.
pub const LAUNCH_ANGLE_DEG: f32 = 50.0;

// --- Rules (FR-20/21/23) ---
/// Starting lives per run.
pub const STARTING_LIVES: i32 = 3;
/// Base score per brick destroyed; multiplied by min(combo, COMBO_CAP).
pub const SCORE_BASE: u32 = 100;
/// Combo multiplier cap.
pub const COMBO_CAP: u32 = 8;

// --- Powerups (FR-31, Phase 6; default only) ---
/// Default drop chance per brick destroyed.
pub const DROP_RATE_DEFAULT: f32 = 0.08;
/// Falling capsule speed, px/s (MOCKUP §4).
pub const POWERUP_FALL_SPEED: f32 = 55.0;
/// Multiball hard cap, balls alive (resolved 2026-08-29).
pub const MULTIBALL_CAP: usize = 8;

/// Current ball speed for this level progress: base + ramp, capped.
pub fn ball_speed(bricks_destroyed_this_level: u32, level_index: u32) -> f32 {
    let s = BALL_BASE_SPEED
        + BALL_SPEED_PER_BRICK * bricks_destroyed_this_level as f32
        + BALL_SPEED_PER_LEVEL * level_index as f32;
    s.min(BALL_MAX_SPEED)
}

/// Score for destroying one brick at the given combo (combo >= 1).
pub fn score_for_brick(combo: u32) -> u32 {
    SCORE_BASE * combo.clamp(1, COMBO_CAP)
}
