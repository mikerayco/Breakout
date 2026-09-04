//! Bloom: threshold + 3x3 blur + additive composite (FR-29).
//!
//! Owns the cheap glow pass over the finished frame (MOCKUP §4: threshold
//! luminance 0.72, 3x3 box blur, added back at 60%). Toggleable with `F4`
//! and `--no-bloom`. Must stay inside NFR-1 at default scale — the gate
//! records the saving with bloom off in `docs/PERF.md`.
//!
//! The framebuffer pixmap is premultiplied RGBA, but the game always draws
//! opaque (bg fill first), so premultiplied == straight here.
//!
//! Performance: the threshold scan is one linear pass, but the 3x3 blur +
//! composite runs only over the bright bounding box expanded by the kernel
//! radius — a sparse night-soil scene blooms a handful of cells instead of
//! 1.2M pixels. The mask buffer is reused across frames (no per-frame
//! allocation on the hot path).

/// Luminance threshold, 0..1 (MOCKUP §4: 0.72).
pub const THRESHOLD: f32 = 0.72;
/// Integer scaled threshold (THRESHOLD * 255000): the hot loop compares
/// integer luminance against this instead of float-dividing per pixel.
const THRESHOLD_SCALED: u32 = (THRESHOLD * 255_000.0) as u32;
/// Blurred-bright add-back strength (MOCKUP §4: 60%).
pub const STRENGTH: f32 = 0.6;
/// Blur kernel radius (3x3).
const RADIUS: i32 = 1;

/// Reusable bloom scratch: the bright mask, zeroed and refilled per frame.
#[derive(Debug, Default)]
pub struct Scratch {
    bright: Vec<u8>,
}

impl Scratch {
    /// Fresh scratch (allocates nothing until first use).
    pub fn new() -> Self {
        Self::default()
    }

    /// Borrow the mask sized for `w*h`, zeroed. Reuses the allocation.
    fn mask(&mut self, len: usize) -> &mut [u8] {
        if self.bright.len() != len {
            self.bright.resize(len, 0);
        } else {
            self.bright.fill(0);
        }
        &mut self.bright
    }
}

/// Apply the bloom pass in place with a reused scratch buffer.
/// Pixel-identical to the naive full-frame blur.
pub fn apply_to(pixmap: &mut tiny_skia::Pixmap, scratch: &mut Scratch) {
    let w = pixmap.width() as usize;
    let h = pixmap.height() as usize;
    let data = pixmap.data();
    let bright = scratch.mask(w * h);
    // Threshold pass: integer luminance over one linear scan, tracking the
    // bright bounding box. (lum/1000 in 0..255; weight maps 0.72..1.0 to
    // 0..255.) Float math here cost milliseconds at scale 4.
    let mut bbox: Option<(usize, usize, usize, usize)> = None; // x0,y0,x1,y1 incl
    for (i, px) in data.as_chunks::<4>().0.iter().enumerate() {
        // Premultiplied RGBA, alpha 255 throughout (opaque scene).
        let lum = 299 * u32::from(px[0]) + 587 * u32::from(px[1]) + 114 * u32::from(px[2]);
        if lum > THRESHOLD_SCALED {
            bright[i] = ((lum - THRESHOLD_SCALED) * 17 / 4760).min(255) as u8;
            let (x, y) = (i % w, i / w);
            bbox = Some(match bbox {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        }
    }
    let Some((x0, y0, x1, y1)) = bbox else {
        return; // no bright pixels: frame untouched, no blur work at all
    };
    // Blur + composite over the bbox expanded by the kernel radius: a blur
    // tap reaches exactly RADIUS past a bright pixel, so everything outside
    // this region composites zero (identical to the full-frame result).
    let rx0 = x0.saturating_sub(RADIUS as usize);
    let ry0 = y0.saturating_sub(RADIUS as usize);
    let rx1 = (x1 + RADIUS as usize).min(w - 1);
    let ry1 = (y1 + RADIUS as usize).min(h - 1);
    let data = pixmap.data_mut();
    for y in ry0..=ry1 {
        for x in rx0..=rx1 {
            let mut sum = 0u32;
            for dy in -RADIUS..=RADIUS {
                for dx in -RADIUS..=RADIUS {
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

/// Apply the bloom pass in place (allocates its mask; the game loop uses
/// [`apply_to`] with a reused [`Scratch`] instead).
#[allow(dead_code)]
pub fn apply(pixmap: &mut tiny_skia::Pixmap) {
    apply_to(pixmap, &mut Scratch::new());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::chunks_exact_to_as_chunks)]
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

    /// Naive full-frame reference: the bbox fast path must match it
    /// pixel-for-pixel on scattered brights near edges and corners.
    fn reference(pixmap: &mut tiny_skia::Pixmap) {
        let w = pixmap.width() as usize;
        let h = pixmap.height() as usize;
        let data = pixmap.data();
        let mut bright = vec![0u8; w * h];
        for (i, px) in data.as_chunks::<4>().0.iter().enumerate() {
            let lum = 299 * u32::from(px[0]) + 587 * u32::from(px[1]) + 114 * u32::from(px[2]);
            if lum > THRESHOLD_SCALED {
                bright[i] = ((lum - THRESHOLD_SCALED) * 17 / 4760).min(255) as u8;
            }
        }
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
                let add = (sum / 9) as f32 * STRENGTH;
                let add = add.min(255.0) as u8;
                let o = (y * w + x) * 4;
                data[o] = data[o].saturating_add(add);
                data[o + 1] = data[o + 1].saturating_add(add);
                data[o + 2] = data[o + 2].saturating_add(add);
            }
        }
    }

    #[test]
    fn bbox_matches_full_frame() {
        for seed in [0u32, 1, 7, 42] {
            let mut a = tiny_skia::Pixmap::new(32, 24).expect("pixmap");
            let mut b = tiny_skia::Pixmap::new(32, 24).expect("pixmap");
            // Deterministic pseudo-random brights incl. edges/corners.
            let mut s = u64::from(seed).wrapping_mul(2654435761).wrapping_add(97);
            let mut next = move || {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                (s >> 33) as u8
            };
            for (pa, pb) in a
                .data_mut()
                .chunks_exact_mut(4)
                .zip(b.data_mut().chunks_exact_mut(4))
            {
                let v = if next() % 5 == 0 { 255 } else { next() % 40 };
                pa[0] = v;
                pa[1] = v / 2;
                pa[2] = v / 3;
                pa[3] = 255;
                pb.copy_from_slice(pa);
            }
            apply(&mut a);
            reference(&mut b);
            assert_eq!(a.data(), b.data(), "bbox diverged (seed {seed})");
        }
    }
}
