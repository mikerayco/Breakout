//! Scoring and the combo counter (FR-23).
//!
//! Owns base scores, the combo multiplier (increments on brick destroyed,
//! resets on paddle contact), and the brick-destroy scoring call. Pure;
//! no I/O.

use super::tuning;

/// Combo + score accumulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Score {
    /// Total score.
    pub points: u64,
    /// Current combo (bricks destroyed since last paddle contact).
    pub combo: u32,
    /// Best combo this run/level.
    pub best_combo: u32,
    /// Bricks destroyed (this level).
    pub bricks_destroyed: u32,
}

impl Score {
    /// Fresh score.
    pub fn new() -> Self {
        Self::default()
    }

    /// One brick destroyed without paddle contact: combo+1, add points.
    /// Returns points awarded for this brick.
    pub fn on_brick_destroyed(&mut self) -> u32 {
        self.combo = self.combo.saturating_add(1);
        self.best_combo = self.best_combo.max(self.combo);
        self.bricks_destroyed = self.bricks_destroyed.saturating_add(1);
        let award = tuning::score_for_brick(self.combo);
        self.points = self.points.saturating_add(u64::from(award));
        award
    }

    /// Ball touched the paddle: combo resets (FR-23).
    pub fn on_paddle_contact(&mut self) {
        self.combo = 0;
    }

    /// Current multiplier shown in the HUD: 0 combo shows x1.
    pub fn multiplier(&self) -> u32 {
        self.combo.clamp(1, tuning::COMBO_CAP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combo_increments_and_resets() {
        let mut s = Score::new();
        s.on_brick_destroyed();
        s.on_brick_destroyed();
        assert_eq!(s.combo, 2);
        s.on_paddle_contact();
        assert_eq!(s.combo, 0);
        assert_eq!(s.best_combo, 2);
    }

    #[test]
    fn scoring_uses_combo_multiplier_capped() {
        let mut s = Score::new();
        let first = s.on_brick_destroyed();
        assert_eq!(first, tuning::SCORE_BASE);
        for _ in 0..20 {
            s.on_brick_destroyed();
        }
        // combo is high but award is capped at COMBO_CAP.
        assert_eq!(s.multiplier(), tuning::COMBO_CAP);
        assert_eq!(s.points, {
            let mut expect = 0u64;
            for c in 1..=21 {
                expect += u64::from(tuning::score_for_brick(c));
            }
            expect
        });
    }
}
