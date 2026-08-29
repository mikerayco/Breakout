//! The RGB framebuffer the game draws into (ADR-0002, ADR-0003).
//!
//! Owns a `320·S × 240·S` tiny-skia RGBA pixmap used as the draw surface,
//! and an `f=24` RGB byte array produced by `rgb_bytes()` for the frame
//! transport. Reallocated only on scale change (never per frame).
//! Must not know about game rules (Phase 2+ reads state through here).

use tiny_skia::{Pixmap, Transform};

/// Logical playfield size (ADR-0003).
pub const LOGICAL_W: u32 = 320;
pub const LOGICAL_H: u32 = 240;

/// Max integer scale factor (ADR-0003).
pub const MAX_SCALE: u32 = 4;
/// Min (and default-at-unknown-size) integer scale factor.
pub const MIN_SCALE: u32 = 1;

pub struct Framebuffer {
    w: u32,
    h: u32,
    scale: u32,
    rgba: Pixmap,
    rgb: Vec<u8>,
}

impl Framebuffer {
    /// Allocate a framebuffer at the given integer scale.
    pub fn new(scale: u32) -> Option<Self> {
        let scale = scale.clamp(1, MAX_SCALE);
        let w = LOGICAL_W * scale;
        let h = LOGICAL_H * scale;
        let rgba = Pixmap::new(w, h)?;
        let rgb = vec![0u8; (w * h * 3) as usize];
        Some(Self {
            w,
            h,
            scale,
            rgba,
            rgb,
        })
    }

    pub fn width(&self) -> u32 {
        self.w
    }
    pub fn height(&self) -> u32 {
        self.h
    }
    pub fn scale(&self) -> u32 {
        self.scale
    }

    /// The RGBA draw surface; drawing calls go straight into it.
    pub fn pixmap_mut(&mut self) -> &mut Pixmap {
        &mut self.rgba
    }

    /// Everything currently in the pixmap, as premultiplied RGBA.
    /// Used by tests and future bloom; kept for the public surface.
    #[allow(dead_code)]
    pub fn rgba_data(&self) -> &[u8] {
        self.rgba.data()
    }

    /// `f=24` RGB view: drop the alpha channel. This is the byte layout the
    /// transport (shm / direct base64) expects (ADR-0002).
    pub fn rgb_bytes(&mut self) -> &[u8] {
        let rgba = self.rgba.data();
        let rgb = &mut self.rgb;
        let px = self.w as usize * self.h as usize;
        debug_assert_eq!(rgba.len(), px * 4);
        debug_assert_eq!(rgb.len(), px * 3);
        for i in 0..px {
            let s = i * 4;
            let d = i * 3;
            rgb[d] = rgba[s];
            rgb[d + 1] = rgba[s + 1];
            rgb[d + 2] = rgba[s + 2];
        }
        rgb
    }

    /// Draw a full-screen rect with the given color/transform.
    pub fn fill_rect(&mut self, color: tiny_skia::Color) {
        self.rgba.fill(color);
    }
}

/// Compute the integer scale factor from a window pixel size (ADR-0003).
/// Returns `None` when the window is too small for scale 1 (FR-11).
pub fn compute_scale(window_w_px: u32, window_h_px: u32) -> Option<u32> {
    if window_w_px < LOGICAL_W || window_h_px < LOGICAL_H {
        return None;
    }
    let sx = window_w_px / LOGICAL_W;
    let sy = window_h_px / LOGICAL_H;
    Some(sx.min(sy).clamp(1, MAX_SCALE))
}

/// The `Transform::identity()` for tiny-skia drawing calls.
pub fn identity() -> Transform {
    Transform::identity()
}
