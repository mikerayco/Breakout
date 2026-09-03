//! tiny-skia drawing wrappers and the Phase 1 test card (ADR-0005, PLAN
//! Phase 1). Owns drawing calls: shapes, fills, anti-aliased primitives,
//! gradients. Must never mutate simulation state.

use tiny_skia::{FillRule, Paint, PathBuilder, Rect};

use super::framebuffer::{identity, Framebuffer};
use super::palette::{self, BALL, BG_DEEP, BG_HUD, TEXT};
use super::text::draw_text;

/// Ball trail length, stored positions (MOCKUP §4: 12).
pub const TRAIL_N: usize = 12;
/// Trail head radius, logical px (MOCKUP §4: 3).
pub const TRAIL_R_HEAD: f32 = 3.0;
/// Trail tail radius (MOCKUP §4: 0.5).
pub const TRAIL_R_TAIL: f32 = 0.5;
/// Trail alpha (MOCKUP §4: 55%).
pub const TRAIL_ALPHA: u8 = 140;
/// Explosion ring max radius, logical px (MOCKUP §4: 24).
pub const RING_MAX_R: f32 = 24.0;
/// Combo pop window, seconds (MOCKUP §4: 120ms).
pub const COMBO_POP_SECS: f32 = 0.12;

/// Per-frame juice inputs for [`draw_world`], assembled by the main loop
/// from simulation events. The camera offset is the decayed sub-pixel
/// value (FR-25).
#[derive(Debug, Clone, Copy, Default)]
pub struct JuiceView<'a> {
    /// Camera offset x (shake), logical px.
    pub ox: f32,
    /// Camera offset y (shake), logical px.
    pub oy: f32,
    /// Brick cells flashing white this frame (non-fatal hits, FR-28).
    pub brick_flashes: &'a [(u8, u8)],
    /// Paddle flashes paddle-cap for 2 frames on contact (FR-28).
    pub paddle_flash: bool,
    /// Explosion rings as (x, y, age01): 0 fresh, 1 done (MOCKUP §4).
    pub rings: &'a [(f32, f32, f32)],
    /// Combo counter scale-pops (FR-30).
    pub combo_pop: bool,
}

/// Drawn colour for one brick (MOCKUP §2: damaged recolours to the tier
/// below; white flash is applied by the caller on top).
pub fn brick_draw_color(b: &crate::game::physics::Brick) -> tiny_skia::Color {
    use crate::game::physics::BrickKind;
    match b.kind {
        BrickKind::Steel => *palette::BRICK_STEEL,
        BrickKind::Explosive => *palette::BRICK_EXPLOSIVE,
        BrickKind::Normal => match b.hp {
            1 => *palette::BRICK_1,
            2 => *palette::BRICK_2,
            3 => *palette::BRICK_3,
            4 => *palette::BRICK_4,
            _ => *palette::BRICK_5,
        },
    }
}

pub fn brick_rgb(b: &crate::game::physics::Brick) -> (u8, u8, u8) {
    let c = brick_draw_color(b);
    let r = (c.red() * 255.0) as u8;
    let g = (c.green() * 255.0) as u8;
    let b2 = (c.blue() * 255.0) as u8;
    (r, g, b2)
}

/// Ball trail from past positions (oldest first), drawn additively
/// (FR-27, MOCKUP sec 4). Takes the ring buffer directly: no allocation.
pub fn draw_trail(
    fb: &mut Framebuffer,
    pts: &std::collections::VecDeque<(f32, f32)>,
    ox: f32,
    oy: f32,
) {
    use tiny_skia::BlendMode;
    let s = fb.scale() as f32;
    let n = pts.len().max(1) as f32;
    for (i, (x, y)) in pts.iter().enumerate() {
        let t = (i as f32 + 1.0) / n; // 0 tail → 1 head
        let r = (TRAIL_R_TAIL + (TRAIL_R_HEAD - TRAIL_R_TAIL) * t) * s;
        let Some(circle) = PathBuilder::from_circle((x + ox) * s, (y + oy) * s, r.max(0.5)) else {
            continue;
        };
        let mut paint = Paint::default();
        let glow = *palette::BALL_GLOW;
        let gr = (glow.red() * 255.0) as u8;
        let gg = (glow.green() * 255.0) as u8;
        let gb = (glow.blue() * 255.0) as u8;
        paint.set_color_rgba8(gr, gg, gb, TRAIL_ALPHA);
        paint.blend_mode = BlendMode::Plus;
        paint.anti_alias = true;
        fb.pixmap_mut()
            .fill_path(&circle, &paint, FillRule::Winding, identity(), None);
    }
}

/// Phase 2: draw the simulation as flat rectangles + a flat circle, plus
/// Phase 4 juice (particles are drawn separately; shake, flashes, rings,
/// trail and combo pop arrive here via [`JuiceView`]).
/// Read-only over the world (rendering never mutates simulation).
/// Paddle flash duration, seconds (MOCKUP §4: 2 frames at 60fps).
pub const PADDLE_FLASH_SECS: f32 = 0.066;

/// Combo HUD fragment (MOCKUP §3): hidden below x2.
fn combo_text(combo: u32) -> String {
    if combo >= 2 {
        format!("  X{}", combo.min(crate::game::tuning::COMBO_CAP))
    } else {
        String::new()
    }
}

/// Combo HUD colour (MOCKUP §3): text at x2, combo-pink at x3+.
fn combo_color(combo: u32) -> tiny_skia::Color {
    if combo >= 3 {
        *palette::COMBO
    } else {
        *TEXT
    }
}

#[allow(clippy::too_many_lines)]
pub fn draw_world(
    fb: &mut Framebuffer,
    world: &crate::game::physics::World,
    muted: bool,
    fx: &JuiceView<'_>,
) {
    use crate::game::tuning;
    let s = fb.scale() as f32;
    let si = fb.scale();
    let fb_w = fb.width() as f32;
    fb.fill_rect(*BG_DEEP);
    // HUD band + rule.
    let hud_band = Rect::from_xywh(0.0, 0.0, fb_w, 20.0 * s).expect("hud band");
    fb.pixmap_mut()
        .fill_rect(hud_band, &solid(*BG_HUD), identity(), None);
    let hud_rule = Rect::from_xywh(0.0, 20.0 * s, fb_w, s).expect("hud rule");
    fb.pixmap_mut()
        .fill_rect(hud_rule, &solid(*palette::BEZEL), identity(), None);
    // Bezel walls (MOCKUP §1 logical coords, scaled).
    for (x, y, w, h) in [
        (0.0, 21.0, 7.0, 219.0),
        (313.0, 21.0, 7.0, 219.0),
        (7.0, 21.0, 306.0, 7.0),
    ] {
        fb.pixmap_mut().fill_rect(
            Rect::from_xywh(x * s, y * s, w * s, h * s).expect("bezel"),
            &solid(*palette::BEZEL),
            identity(),
            None,
        );
    }
    // Bricks: flat rects coloured by tier; damaged recolours to tier below
    // because hp already decremented (MOCKUP §2). Non-fatal hits flash
    // white for one frame (FR-28). World space shifts by the shake offset.
    for b in &world.bricks {
        let (bx, by, bw, bh) = b.aabb();
        let col = brick_draw_color(b);
        fb.pixmap_mut().fill_rect(
            Rect::from_xywh((bx + fx.ox) * s, (by + fx.oy) * s, bw * s, bh * s).expect("brick"),
            &solid(col),
            identity(),
            None,
        );
        if fx.brick_flashes.contains(&(b.col, b.row)) {
            fb.pixmap_mut().fill_rect(
                Rect::from_xywh((bx + fx.ox) * s, (by + fx.oy) * s, bw * s, bh * s)
                    .expect("brick flash"),
                &solid(tiny_skia::Color::WHITE),
                identity(),
                None,
            );
        }
    }
    // Paddle: flat rect, paddle-cap across the whole body for 2 frames on
    // contact (FR-28).
    {
        let w = world.paddle_width();
        let col = if fx.paddle_flash {
            *palette::PADDLE_CAP
        } else {
            *palette::PADDLE
        };
        fb.pixmap_mut().fill_rect(
            Rect::from_xywh(
                (world.paddle_x - w / 2.0 + fx.ox) * s,
                (tuning::PADDLE_Y + fx.oy) * s,
                w * s,
                tuning::PADDLE_H * s,
            )
            .expect("paddle"),
            &solid(col),
            identity(),
            None,
        );
    }
    // Balls: flat AA circles.
    for ball in &world.balls {
        let circle = PathBuilder::from_circle(
            (ball.x + fx.ox) * s,
            (ball.y + fx.oy) * s,
            tuning::BALL_R * s,
        )
        .expect("ball");
        let mut paint = Paint::default();
        paint.set_color(*BALL);
        paint.anti_alias = true;
        fb.pixmap_mut()
            .fill_path(&circle, &paint, FillRule::Winding, identity(), None);
    }
    // Explosion rings: 1-frame white ring expanding to 24px (MOCKUP §4).
    // Age01 0→1 maps to radius 2→RING_MAX_R, fading out.
    for (rx, ry, age) in fx.rings {
        let r = (2.0 + (RING_MAX_R - 2.0) * age) * s;
        let Some(circle) = PathBuilder::from_circle((rx + fx.ox) * s, (ry + fx.oy) * s, r.max(1.0))
        else {
            continue;
        };
        let mut paint = Paint::default();
        let a = ((1.0 - age) * 255.0) as u8;
        paint.set_color_rgba8(255, 255, 255, a);
        paint.anti_alias = true;
        let stroke = tiny_skia::Stroke {
            width: s.max(1.0),
            ..Default::default()
        };
        fb.pixmap_mut().stroke_path(
            &circle,
            &paint,
            &stroke,
            tiny_skia::Transform::identity(),
            None,
        );
    }
    // HUD text (MOCKUP §3 slots, 5x7 font). Combo is hidden below x2,
    // text at x2, combo-pink at x3+ with a scale-pop on increment (FR-30).
    // Mute indicator is visible at a glance when muted, hidden otherwise
    // (FR-48; persists to the profile in Phase 8).
    let hud = if muted {
        format!(
            "SCORE {:07}  LIVES {}  LVL {}/8{}  MUTED",
            world.score.points.min(9_999_999),
            world.lives,
            world.level_index + 1,
            combo_text(world.score.combo),
        )
    } else {
        format!(
            "SCORE {:07}  LIVES {}  LVL {}/8{}",
            world.score.points.min(9_999_999),
            world.lives,
            world.level_index + 1,
            combo_text(world.score.combo),
        )
    };
    draw_text(
        fb.pixmap_mut(),
        6 * si as i32,
        6 * si as i32,
        &hud,
        *TEXT,
        si,
    );
    // Combo pop: double-size combo counter in its ramp colour for 120ms
    // after an increment (MOCKUP §4: 1.0 → 1.4 → 1.0; the bitmap font pops
    // to 2x, the only other size it has).
    if fx.combo_pop && world.score.combo >= 2 {
        let pop = format!("X{}", world.score.multiplier());
        draw_text(
            fb.pixmap_mut(),
            196 * si as i32,
            2 * si as i32,
            &pop,
            combo_color(world.score.combo),
            si * 2,
        );
    }
    // State overlays.
    let overlay: Option<&str> = match world.state {
        crate::game::state::GameState::Title => Some("BREAKOUT - SPACE TO START - Q TO QUIT"),
        crate::game::state::GameState::Paused => Some("PAUSED - ESC RESUME - Q QUIT"),
        crate::game::state::GameState::LevelClear => Some("LEVEL CLEAR - SPACE CONTINUE - Q QUIT"),
        crate::game::state::GameState::RunOver => Some("GAME OVER - SPACE RETRY - Q QUIT"),
        crate::game::state::GameState::Playing => {
            if world.balls.iter().any(|b| b.stuck) {
                Some("SPACE TO LAUNCH")
            } else {
                None
            }
        }
    };
    if let Some(msg) = overlay {
        let tw = super::text::text_width(msg, si) as f32;
        let ox = ((fb_w - tw) / 2.0) as i32;
        let oy = (120 * si) as i32;
        draw_text(fb.pixmap_mut(), ox, oy, msg, *TEXT, si);
    }
}

/// Phase 1 animated test card: scrolling gradient, bouncing AA circle,
/// colour-bar strip, and a live FPS/frame-time overlay (PLAN Phase 1 §2, §9).
/// Kept for headless regression + PNG preview; the game loop draws draw_world.
#[allow(dead_code)]
pub struct TestCard {
    t_secs: f32,
    pub scale: u32,
}

#[allow(dead_code)]
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

/// Small FPS / debug line under the HUD (Phase 2+ overlay).
pub fn draw_fps_line(fb: &mut Framebuffer, line: &str) {
    let s = fb.scale();
    draw_text(fb.pixmap_mut(), 6 * s as i32, 22 * s as i32, line, *TEXT, s);
}

/// Pause menu (FR-6): Resume / Restart run / Mute / Quit. `selected` is
/// the highlighted row. Up/Down (or j/k) moves, Enter/Space confirms,
/// Esc resumes.
pub fn draw_pause_menu(fb: &mut Framebuffer, selected: usize, muted: bool) {
    let s = fb.scale();
    let items = [
        "RESUME".to_string(),
        "RESTART RUN".to_string(),
        format!("MUTE: {}", if muted { "ON" } else { "OFF" }),
        "QUIT".to_string(),
    ];
    let title = "PAUSED";
    let tw = super::text::text_width(title, s) as f32;
    let fb_w = fb.width() as f32;
    draw_text(
        fb.pixmap_mut(),
        ((fb_w - tw) / 2.0) as i32,
        (80 * s) as i32,
        title,
        *TEXT,
        s,
    );
    for (i, item) in items.iter().enumerate() {
        let marker = if i == selected % items.len() {
            "> "
        } else {
            "  "
        };
        let line = format!("{marker}{item}");
        let iw = super::text::text_width(&line, s) as f32;
        draw_text(
            fb.pixmap_mut(),
            ((fb_w - iw) / 2.0) as i32,
            (100 * s + i as u32 * 12 * s) as i32,
            &line,
            if i == selected % items.len() {
                *palette::PADDLE
            } else {
                *TEXT
            },
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
