//! Screenshake as a decaying sub-pixel offset (FR-25).
//!
//! Owns the shake magnitude by event class; the offset is applied to
//! world-space drawing each frame. Magnitudes and decay come from
//! MOCKUP §4 verbatim. Cosmetic only — the simulation is untouched.

/// Decay time constant, seconds (MOCKUP §4: `e^(-t/0.08)`).
pub const DECAY_TAU: f32 = 0.08;
/// Brick break magnitude, logical px (MOCKUP §4: 0.6).
pub const SHAKE_BRICK: f32 = 0.6;
/// Explosion magnitude (MOCKUP §4: 2.5).
pub const SHAKE_EXPLOSION: f32 = 2.5;
/// Life-lost magnitude (MOCKUP §4: 4).
pub const SHAKE_LIFE_LOST: f32 = 4.0;
/// Hard clamp on the applied offset (MOCKUP §4: ±5px).
pub const SHAKE_MAX: f32 = 5.0;

/// Decaying shake state. Allocated once, reused every frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Shake {
    mag: f32,
}

impl Shake {
    /// Fresh shake with no offset.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add trauma from an event; stacks, clamped to [`SHAKE_MAX`].
    pub fn add(&mut self, mag: f32) {
        self.mag = (self.mag + mag).min(SHAKE_MAX);
    }

    /// Current magnitude (for tests / overlay).
    pub fn magnitude(&self) -> f32 {
        self.mag
    }

    /// Decay by `dt` seconds and return the sub-pixel offset to apply to
    /// world-space drawing. Direction is random per frame (visual RNG).
    pub fn offset(&mut self, rng: &mut fastrand::Rng, dt: f32) -> (f32, f32) {
        self.mag *= (-dt / DECAY_TAU).exp();
        if self.mag < 0.01 {
            self.mag = 0.0;
            return (0.0, 0.0);
        }
        let ang = rng.f32() * std::f32::consts::TAU;
        (ang.cos() * self.mag, ang.sin() * self.mag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shake_decays_to_zero() {
        let mut s = Shake::new();
        let mut rng = fastrand::Rng::with_seed(3);
        s.add(SHAKE_LIFE_LOST);
        assert_eq!(s.magnitude(), SHAKE_LIFE_LOST);
        for _ in 0..120 {
            s.offset(&mut rng, 1.0 / 60.0);
        }
        assert_eq!(s.magnitude(), 0.0);
    }

    #[test]
    fn shake_clamps() {
        let mut s = Shake::new();
        s.add(100.0);
        assert!(s.magnitude() <= SHAKE_MAX);
    }
}
