//! Powerup capsules: seven types, timers, multiball (FR-31..34).
//!
//! Owns the seven kinds (FR-32), drop rolls, capsule fall physics and
//! paddle collection (FR-31), independent timers with refresh-not-stack
//! semantics (FR-33), and instant effects (multiball split, 1-up).
//! Everything here mutates [`ActiveEffects`] or the ball list; `physics`
//! only reads the effects axes, never branches on a kind.

use super::tuning;

/// The seven v1 powerups (FR-32, exactly this set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerKind {
    /// Split every ball in two (instant).
    Multiball,
    /// Paddle fires on `Space` (timed).
    Laser,
    /// Ball catches on the paddle, re-launch with `Space` (timed).
    Sticky,
    /// Paddle grows (timed).
    Wide,
    /// Ball speed x0.7 (timed, FR-32 value).
    Slow,
    /// Ball passes through bricks it destroys (timed).
    Pierce,
    /// Extra life (instant).
    OneUp,
}

impl PowerKind {
    /// All seven, in `F5` cycle order.
    pub const ALL: [PowerKind; 7] = [
        PowerKind::Multiball,
        PowerKind::Laser,
        PowerKind::Sticky,
        PowerKind::Wide,
        PowerKind::Slow,
        PowerKind::Pierce,
        PowerKind::OneUp,
    ];

    /// One-letter capsule glyph (the 5x7 font covers A-Z, 0-9, `+`).
    pub fn glyph(self) -> char {
        match self {
            PowerKind::Multiball => 'M',
            PowerKind::Laser => 'L',
            PowerKind::Sticky => 'S',
            PowerKind::Wide => 'W',
            PowerKind::Slow => 'T',
            PowerKind::Pierce => 'P',
            PowerKind::OneUp => '+',
        }
    }

    /// True for Laser/Sticky/Wide/Slow/Pierce (timer-driven, FR-33).
    pub fn is_timed(self) -> bool {
        !matches!(self, PowerKind::Multiball | PowerKind::OneUp)
    }

    /// Full timer duration on collect/refresh, seconds (tuning).
    pub fn duration(self) -> f32 {
        match self {
            PowerKind::Laser => tuning::POWERUP_DUR_LASER,
            PowerKind::Sticky => tuning::POWERUP_DUR_STICKY,
            PowerKind::Wide => tuning::POWERUP_DUR_WIDE,
            PowerKind::Slow => tuning::POWERUP_DUR_SLOW,
            PowerKind::Pierce => tuning::POWERUP_DUR_PIERCE,
            PowerKind::Multiball | PowerKind::OneUp => 0.0,
        }
    }
}

/// Independent effect timers read by physics (FR-33). Refresh-not-stack:
///
/// collecting a timed kind resets its clock to full, never adds.
#[derive(Debug, Clone, Copy, Default)]
pub struct ActiveEffects {
    /// Laser remaining, seconds.
    pub laser_t: f32,
    /// Sticky remaining, seconds.
    pub sticky_t: f32,
    /// Wide remaining, seconds.
    pub wide_t: f32,
    /// Slow remaining, seconds.
    pub slow_t: f32,
    /// Pierce remaining, seconds.
    pub pierce_t: f32,
    /// Duration multiplier axis (perk hook, Phase 8; 1.0 until then).
    pub duration_mul: f32,
}

impl ActiveEffects {
    /// Fresh effects (durations unmodified).
    pub fn new() -> Self {
        Self {
            duration_mul: 1.0,
            ..Default::default()
        }
    }

    /// Collect a timed kind: refresh its clock to full (FR-33).
    pub fn refresh(&mut self, kind: PowerKind) {
        let d = kind.duration() * self.duration_mul;
        match kind {
            PowerKind::Laser => self.laser_t = d,
            PowerKind::Sticky => self.sticky_t = d,
            PowerKind::Wide => self.wide_t = d,
            PowerKind::Slow => self.slow_t = d,
            PowerKind::Pierce => self.pierce_t = d,
            PowerKind::Multiball | PowerKind::OneUp => {}
        }
    }

    /// Advance all clocks by `dt`; expired timers read exactly 0.
    pub fn tick(&mut self, dt: f32) {
        for t in [
            &mut self.laser_t,
            &mut self.sticky_t,
            &mut self.wide_t,
            &mut self.slow_t,
            &mut self.pierce_t,
        ] {
            *t = (*t - dt).max(0.0);
        }
    }

    /// Remaining time for a timed kind (0 for instant kinds).
    pub fn remaining(&self, kind: PowerKind) -> f32 {
        match kind {
            PowerKind::Laser => self.laser_t,
            PowerKind::Sticky => self.sticky_t,
            PowerKind::Wide => self.wide_t,
            PowerKind::Slow => self.slow_t,
            PowerKind::Pierce => self.pierce_t,
            PowerKind::Multiball | PowerKind::OneUp => 0.0,
        }
    }

    /// Laser axis for physics/menus.
    pub fn laser_active(&self) -> bool {
        self.laser_t > 0.0
    }

    /// Sticky axis for physics.
    pub fn sticky_active(&self) -> bool {
        self.sticky_t > 0.0
    }

    /// Wide axis for paddle sizing.
    pub fn wide_active(&self) -> bool {
        self.wide_t > 0.0
    }

    /// Slow axis for ball speed.
    pub fn slow_active(&self) -> bool {
        self.slow_t > 0.0
    }

    /// Pierce axis for brick resolution.
    pub fn pierce_active(&self) -> bool {
        self.pierce_t > 0.0
    }

    /// Any timed effect running.
    pub fn any_active(&self) -> bool {
        self.laser_active()
            || self.sticky_active()
            || self.wide_active()
            || self.slow_active()
            || self.pierce_active()
    }
}

/// A falling capsule. Position is the centre in logical pixels.
#[derive(Debug, Clone, Copy)]
pub struct Capsule {
    /// Centre x.
    pub x: f32,
    /// Centre y.
    pub y: f32,
    /// Kind.
    pub kind: PowerKind,
}

/// A laser projectile. Position is the tip in logical pixels.
#[derive(Debug, Clone, Copy)]
pub struct Shot {
    /// Tip x.
    pub x: f32,
    /// Tip y.
    pub y: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_not_stack() {
        let mut e = ActiveEffects::new();
        e.refresh(PowerKind::Laser);
        assert_eq!(e.laser_t, tuning::POWERUP_DUR_LASER);
        e.tick(3.0);
        e.refresh(PowerKind::Laser);
        // Refreshed to full, not full+remainder.
        assert_eq!(e.laser_t, tuning::POWERUP_DUR_LASER);
    }

    #[test]
    fn timers_expire_cleanly() {
        let mut e = ActiveEffects::new();
        e.refresh(PowerKind::Wide);
        e.tick(tuning::POWERUP_DUR_WIDE + 1.0);
        assert!(!e.wide_active());
        assert!(!e.any_active());
    }

    #[test]
    fn instant_kinds_have_no_timer() {
        let e = ActiveEffects::new();
        assert_eq!(e.remaining(PowerKind::Multiball), 0.0);
        assert_eq!(e.remaining(PowerKind::OneUp), 0.0);
    }

    #[test]
    fn all_seven_cycle() {
        assert_eq!(PowerKind::ALL.len(), 7);
        for k in PowerKind::ALL {
            let _ = k.glyph();
            let _ = k.duration();
        }
    }
}
