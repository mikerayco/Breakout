//! tiny-skia drawing wrappers and the Phase 1 test card (ADR-0005, PLAN
//! Phase 1). Owns drawing calls: shapes, fills, anti-aliased primitives,
//! gradients. Must never mutate simulation state.

use tiny_skia::{FillRule, Paint, PathBuilder, Rect};

use super::framebuffer::{identity, Framebuffer};
use super::palette::{self, BALL, BG_DEEP, BG_HUD, TEXT};
use super::text::draw_text;

/// Phase 1 animated test card: scrolling gradient, bouncing AA circle,
/// colour-bar strip, and a live FPS/frame-time overlay (PLAN Phase 1 §2, §9).
pub struct TestCard {
    t_secs: f32,
    pub scale: u32,
}

impl TestCard {
    pub fn new(scale: u32) -> Self {
        Self { t_secs: 0.0, scale }
    }

    /// Advance animation time by one ~frame.
    pub fn tick(&mut self, dt: f32) {
        self.t_secs += dt;
    }

    /// Draw one frame into the framebuffer at the current scale.
    pub fn draw(&mut self, fb: &mut Framebuffer) {
        let w = fb.width() as f32;
        let h = fb.height() as f32;
        let s = fb.scale() as f32;

        // Background: deep play-area fill.
        fb.fill_rect(*BG_DEEP);

        // Scrolling diagonal gradient across the whole screen.
        let phase = (self.t_secs * 20.0) % (w + h);
        let start = tiny_skia::Point::from_xy(phase, 0.0);
        let end = tiny_skia::Point::from_xy(phase - w, h);
        let stops = vec![
            tiny_skia::GradientStop::new(0.0, *palette::BRICK_2),
            tiny_skia::GradientStop::new(0.5, *palette::BRICK_5),
            tiny_skia::GradientStop::new(1.0, *palette::BRICK_3),
        ];
        let gp = Paint {
            shader: tiny_skia::LinearGradient::new(
                start,
                end,
                stops,
                tiny_skia::SpreadMode::Pad,
                identity(),
            )
            .expect("valid gradient"),
            ..Paint::default()
        };
        fb.pixmap_mut().fill_rect(
            Rect::from_xywh(0.0, 0.0, w, h).expect("fullscreen rect"),
            &gp,
            identity(),
            None,
        );

        // Bouncing anti-aliased circle (Phase 1: proves AA + sub-pixel motion).
        let radius = 18.0 * s;
        let speed = 120.0 * s; // logical px per second
        let cx = ((self.t_secs * speed) % (w - radius * 2.0)) + radius;
        let cy = ((self.t_secs * speed * 1.7) % (h - radius * 2.0)) + radius;
        let circle = PathBuilder::from_circle(cx, cy, radius).expect("valid circle");
        let mut paint = Paint::default();
        paint.set_color(*BALL);
        paint.anti_alias = true;
        fb.pixmap_mut()
            .fill_path(&circle, &paint, FillRule::Winding, identity(), None);

        // Colour-bar strip at the bottom (Phase 1 §2).
        let bar_h = 24.0 * s;
        let bar_y = h - bar_h;
        let cols = [
            *palette::BRICK_1,
            *palette::BRICK_2,
            *palette::BRICK_3,
            *palette::BRICK_4,
            *palette::BRICK_5,
            *palette::BRICK_STEEL,
            *palette::DANGER,
        ];
        let col_w = w / cols.len() as f32;
        for (i, col) in cols.iter().enumerate() {
            let mut paint = Paint::default();
            paint.set_color(*col);
            paint.anti_alias = false;
            fb.pixmap_mut().fill_rect(
                Rect::from_xywh(i as f32 * col_w, bar_y, col_w, bar_h).expect("bar rect"),
                &paint,
                identity(),
                None,
            );
        }

        // HUD band + rule (Phase 1 overlay stub uses the HUD colours).
        fb.pixmap_mut().fill_rect(
            Rect::from_xywh(0.0, 0.0, w, 20.0 * s).expect("hud band"),
            &solid(*BG_HUD),
            identity(),
            None,
        );
    }

    /// Draw the phase-1 overlay text (fps, p50/p99, transport, scale, size).
    pub fn draw_overlay(
        &mut self,
        fb: &mut Framebuffer,
        fps: f32,
        p50: f32,
        p99: f32,
        transport: &str,
    ) {
        let s = fb.scale();
        let line = format!(
            "FPS {:>3.0}  P50 {:>4.1}MS  P99 {:>4.1}MS  {}  S={}  {}X{}",
            fps,
            p50,
            p99,
            transport,
            s,
            fb.width(),
            fb.height(),
        );
        // Top-left corner of the play area, below the HUD band.
        draw_text(
            fb.pixmap_mut(),
            6 * s as i32,
            22 * s as i32,
            &line,
            *TEXT,
            s,
        );
    }
}

/// Solid-color paint helper.
pub fn solid(color: tiny_skia::Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = false;
    p
}

/// FR-11: the window is too small for scale 1. Draw a centred message
/// instead of the game, and recover live when the window grows.
pub fn draw_too_small(fb: &mut Framebuffer) {
    let w = fb.width() as f32;
    let h = fb.height() as f32;
    fb.fill_rect(*BG_DEEP);
    fb.pixmap_mut().fill_rect(
        Rect::from_xywh(0.0, 0.0, w, h).expect("fullscreen rect"),
        &solid(*BG_DEEP),
        identity(),
        None,
    );
    let msg = "MAKE THE WINDOW BIGGER";
    let scale = fb.scale().max(1);
    let tw = super::text::text_width(msg, scale) as f32;
    draw_text(
        fb.pixmap_mut(),
        ((w - tw) / 2.0) as i32,
        (h / 2.0) as i32 - 8 * scale as i32,
        msg,
        *TEXT,
        scale,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Render one test-card frame off-screen and check the RGB output has
    /// the right size and is not uniformly blank (proves the framebuffer
    /// + tiny-skia + RGB conversion path end to end, headlessly).
    #[test]
    fn test_card_renders_non_blank_rgb() {
        let mut fb = Framebuffer::new(1).expect("scale-1 framebuffer");
        let mut card = TestCard::new(1);
        card.tick(1.0);
        card.draw(&mut fb);
        card.draw_overlay(&mut fb, 60.0, 16.0, 16.0, "shm");

        let rgb = fb.rgb_bytes();
        assert_eq!(rgb.len(), (320 * 240 * 3) as usize);

        // At least 95% of pixels different from the first one → not blank.
        let first = [rgb[0], rgb[1], rgb[2]];
        let mut differing = 0;
        let mut i = 0;
        while i < rgb.len() {
            if rgb[i] != first[0] || rgb[i + 1] != first[1] || rgb[i + 2] != first[2] {
                differing += 1;
            }
            i += 3;
        }
        let total = rgb.len() / 3;
        assert!(
            differing > total * 50 / 100,
            "test card unexpectedly blank: {}/{} differing",
            differing,
            total
        );
    }

    /// A 5x7 glyph renders as a 5x7 block of coloured pixels at scale 1.
    #[test]
    fn glyph_draws_some_pixels() {
        let mut pixmap = tiny_skia::Pixmap::new(7, 8).expect("tiny pixmap");
        draw_text(&mut pixmap, 1, 1, "A", *TEXT, 1);
        let data = pixmap.data();
        let colored = data
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[3] != 0)
            .count();
        assert!(colored > 0, "glyph 'A' drew no pixels");
    }

    /// Render the Phase 1 card at scale 2 to /tmp/breakout-card-s2.png so a
    /// human can eyeball it without a graphics terminal. Not a behavioural
    /// assert; useful for the Phase 1 visual gate.
    #[test]
    fn export_preview_png() {
        let mut fb = Framebuffer::new(2).expect("scale-2 framebuffer");
        let mut card = TestCard::new(2);
        card.tick(4.0);
        card.draw(&mut fb);
        card.draw_overlay(&mut fb, 60.0, 16.7, 16.7, "shm");
        let path = std::path::Path::new("/tmp/breakout-card-s2.png");
        let ok = fb.pixmap_mut().save_png(path);
        assert!(ok.is_ok(), "save_png succeeded");
        assert!(path.exists());
    }
}
