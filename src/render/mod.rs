//! Turns game state into pixels (Phase 1+).
//!
//! Owns the framebuffer, tiny-skia drawing, the 5x7 bitmap font, particles,
//! camera, bloom, palette and the HUD. Must not import from `game/`'s
//! simulation modules except through read-only state (ADR-0005: rendering
//! never mutates the simulation).

#[allow(dead_code)]
pub mod bloom;
#[allow(dead_code)]
pub mod camera;
#[allow(dead_code)]
pub mod draw;
#[allow(dead_code)]
pub mod framebuffer;
#[allow(dead_code)]
pub mod hud;
#[allow(dead_code)]
pub mod palette;
#[allow(dead_code)]
pub mod particles;
#[allow(dead_code)]
pub mod text;
