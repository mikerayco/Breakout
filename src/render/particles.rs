//! Fixed-capacity particle pool, no per-frame allocation (FR-24).
//!
//! Owns velocity, gravity, lifetime and colour-ramp particles for brick
//! bursts and explosions. Effect sizes come from MOCKUP §4 verbatim.
//! Cosmetic only: the simulation never reads this module, so determinism
//! (NFR-10) is untouched. The pool is allocated once and reused; spawning
//! into a full pool overwrites the oldest particle (no allocation, no drop
//! spike mid-frame).

use tiny_skia::{BlendMode, Paint, Rect, Transform};

/// Hard cap on live particles (AGENTS §3: no per-frame allocation).
pub const MAX_PARTICLES: usize = 512;
/// Brick-burst count range, particles per brick (MOCKUP §4: 8-14).
pub const BURST_N_MIN: u32 = 8;
/// Brick-burst count range.
pub const BURST_N_MAX: u32 = 14;
/// Explosion count (MOCKUP §4: 40 particles).
pub const EXPLOSION_N: u32 = 40;
/// Initial speed range, logical px/s (MOCKUP §4: 40-90 outward).
pub const SPEED_MIN: f32 = 40.0;
/// Initial speed range.
pub const SPEED_MAX: f32 = 90.0;
/// Explosion speed multiplier (MOCKUP §4: 2x radius).
pub const EXPLOSION_SPEED_MUL: f32 = 2.0;
/// Gravity, logical px/s^2 (MOCKUP §4: 220).
pub const GRAVITY: f32 = 220.0;
/// Lifetime range, seconds (MOCKUP §4: 350-600ms).
pub const LIFE_MIN: f32 = 0.35;
/// Lifetime range.
pub const LIFE_MAX: f32 = 0.60;
/// Particle size range, logical px (MOCKUP §4: 1-2px).
pub const SIZE_MIN: f32 = 1.0;
/// Particle size range.
pub const SIZE_MAX: f32 = 2.0;

/// One particle. Position/velocity in logical pixels / px/s.
#[derive(Debug, Clone, Copy)]
pub struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max_life: f32,
    size: f32,
    r: u8,
    g: u8,
    b: u8,
}

/// Fixed-capacity pool; allocated once, reused every frame.
#[derive(Debug)]
pub struct Pool {
    parts: Vec<Particle>,
    /// Ring cursor: next spawn overwrites here when full.
    next: usize,
}

impl Pool {
    /// Empty pool with capacity reserved up front (no later allocation).
    pub fn new() -> Self {
        Self {
            parts: Vec::with_capacity(MAX_PARTICLES),
            next: 0,
        }
    }

    /// Live particle count.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// True when no particles are alive.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Spawn one particle; overwrites the oldest when full.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &mut self,
        x: f32,
        y: f32,
        vx: f32,
        vy: f32,
        life: f32,
        size: f32,
        (r, g, b): (u8, u8, u8),
    ) {
        let p = Particle {
            x,
            y,
            vx,
            vy,
            life,
            max_life: life.max(0.001),
            size,
            r,
            g,
            b,
        };
        if self.parts.len() < MAX_PARTICLES {
            self.parts.push(p);
        } else {
            self.parts[self.next] = p;
            self.next = (self.next + 1) % MAX_PARTICLES;
        }
    }

    /// Brick burst: `n` particles of the brick's colour, outward from the
    /// impact point (MOCKUP §4). Visual RNG only — never the sim RNG.
    pub fn burst(&mut self, rng: &mut fastrand::Rng, x: f32, y: f32, color: (u8, u8, u8), n: u32) {
        for _ in 0..n {
            let ang = rng.f32() * std::f32::consts::TAU;
            let sp = SPEED_MIN + rng.f32() * (SPEED_MAX - SPEED_MIN);
            self.spawn(
                x,
                y,
                ang.cos() * sp,
                ang.sin() * sp,
                LIFE_MIN + rng.f32() * (LIFE_MAX - LIFE_MIN),
                SIZE_MIN + rng.f32() * (SIZE_MAX - SIZE_MIN),
                color,
            );
        }
    }

    /// Explosion: 40 particles at 2x speed (MOCKUP §4).
    pub fn explosion(&mut self, rng: &mut fastrand::Rng, x: f32, y: f32, color: (u8, u8, u8)) {
        for _ in 0..EXPLOSION_N {
            let ang = rng.f32() * std::f32::consts::TAU;
            let sp = (SPEED_MIN + rng.f32() * (SPEED_MAX - SPEED_MIN)) * EXPLOSION_SPEED_MUL;
            self.spawn(
                x,
                y,
                ang.cos() * sp,
                ang.sin() * sp,
                LIFE_MIN + rng.f32() * (LIFE_MAX - LIFE_MIN),
                SIZE_MIN + rng.f32() * (SIZE_MAX - SIZE_MIN),
                color,
            );
        }
    }

    /// Advance all particles by `dt` seconds (render time, not sim time).
    pub fn update(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.parts.len() {
            let p = &mut self.parts[i];
            p.life -= dt;
            if p.life <= 0.0 {
                self.parts.swap_remove(i);
                if self.next >= self.parts.len().saturating_add(1) {
                    self.next = 0;
                }
                continue;
            }
            p.vy += GRAVITY * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            i += 1;
        }
    }

    /// Draw all particles additively, fading to transparent (MOCKUP §4).
    pub fn draw(&self, pixmap: &mut tiny_skia::Pixmap, scale: f32, ox: f32, oy: f32) {
        for p in &self.parts {
            let a = ((p.life / p.max_life).clamp(0.0, 1.0) * 255.0) as u8;
            let mut paint = Paint::default();
            paint.set_color_rgba8(p.r, p.g, p.b, a);
            paint.blend_mode = BlendMode::Plus;
            paint.anti_alias = false;
            let rect = Rect::from_xywh(
                (p.x + ox) * scale,
                (p.y + oy) * scale,
                (p.size * scale).max(1.0),
                (p.size * scale).max(1.0),
            );
            if let Some(rect) = rect {
                pixmap.fill_rect(rect, &paint, Transform::identity(), None);
            }
        }
    }
}

impl Default for Pool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_caps_without_allocating() {
        let mut pool = Pool::new();
        let mut rng = fastrand::Rng::with_seed(1);
        for _ in 0..MAX_PARTICLES + 100 {
            pool.burst(&mut rng, 160.0, 120.0, (255, 0, 0), 1);
        }
        assert_eq!(pool.len(), MAX_PARTICLES);
        pool.update(10.0);
        assert!(pool.is_empty());
    }

    #[test]
    fn gravity_pulls_down() {
        let mut pool = Pool::new();
        pool.spawn(0.0, 0.0, 0.0, 0.0, 1.0, 1.0, (255, 255, 255));
        pool.update(0.5);
        assert!(pool.parts[0].vy > 0.0);
    }
}
