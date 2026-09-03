//! Bloom: threshold + 3x3 blur + additive composite (FR-29).
//!
//! Owns the cheap glow pass over the finished frame (MOCKUP §4: threshold
//! luminance 0.72, 3x3 box blur, added back at 60%). Toggleable with `F4`
//! and `--no-bloom`. Must stay inside NFR-1 at default scale — the gate
//! records the saving with bloom off in `docs/PERF.md`.
//!
//! The framebuffer pixmap is premultiplied RGBA, but the game always draws
//! opaque (bg fill first), so premultiplied == straight here.

/// Luminance threshold, 0..1 (MOCKUP §4: 0.72).
pub const THRESHOLD: f32 = 0.72;
/// Blurred-bright add-back strength (MOCKUP §4: 60%).
pub const STRENGTH: f32 = 0.6;

/// Apply the bloom pass in place. Fast path: with no bright pixels the
/// frame is untouched (no blur work at all).
pub fn apply(pixmap: &mut tiny_skia::Pixmap) {
    let w = pixmap.width() as usize;
    let h = pixmap.height() as usize;
    let data = pixmap.data();
    // Bright mask: one byte per pixel, 0 or the pixel's luminance weight.
    let mut bright = vec![0u8; w * h];
    let mut any = false;
    for (i, px) in data.as_chunks::<4>().0.iter().enumerate() {
        // Premultiplied RGBA, alpha 255 throughout (opaque scene).
        let lum = (0.299 * f32::from(px[0]) + 0.587 * f32::from(px[1]) + 0.114 * f32::from(px[2]))
            / 255.0;
        if lum > THRESHOLD {
            bright[i] = ((lum - THRESHOLD) / (1.0 - THRESHOLD) * 255.0) as u8;
            any = true;
        }
    }
    if !any {
        return;
    }
    // 3x3 box blur of the bright mask, added back at STRENGTH.
    let data = pixmap.data_mut();
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0u32;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                    let ny = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                    sum += u32::from(bright[ny * w + nx]);
                }
            }
            let avg = sum / 9;
            let add = (avg as f32 * STRENGTH).min(255.0) as u8;
            let o = (y * w + x) * 4;
            data[o] = data[o].saturating_add(add);
            data[o + 1] = data[o + 1].saturating_add(add);
            data[o + 2] = data[o + 2].saturating_add(add);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_frame_untouched() {
        let mut px = tiny_skia::Pixmap::new(8, 8).expect("pixmap");
        px.fill(tiny_skia::Color::BLACK);
        let before = px.data().to_vec();
        apply(&mut px);
        assert_eq!(px.data(), before.as_slice());
    }

    #[test]
    fn white_pixel_blooms_neighbours() {
        let mut px = tiny_skia::Pixmap::new(8, 8).expect("pixmap");
        px.fill(tiny_skia::Color::BLACK);
        // Opaque white centre pixel.
        let o = (4 * 8 + 4) * 4;
        let data = px.data_mut();
        data[o] = 255;
        data[o + 1] = 255;
        data[o + 2] = 255;
        data[o + 3] = 255;
        apply(&mut px);
        // A neighbour of the white pixel gained some light.
        let n = (4 * 8 + 5) * 4;
        assert!(px.data()[n] > 0, "bloom did not spread");
    }
}
