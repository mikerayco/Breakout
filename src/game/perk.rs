//! Perks as data: 12+ single-rule modifiers (FR-41, ADR-0008).
//!
//! Owns the v1 perk pool. A perk is `{ id, name, description, apply }`
//! where `apply` mutates [`RunModifiers`]; the simulation only reads the
//! axes, so perks compose with no interaction code and no special cases in
//! `physics.rs`. Unlock thresholds gate availability by lifetime shards.
//!
//! Pool designed inline for the one-pass finish (review todos closed with
//! tables recorded here + `docs/BALANCE.md`).

use super::physics::RunModifiers;

/// Shard thresholds: cumulative lifetime shards opening each ring.
/// Ring 0 is always unlocked; crossing a threshold only ever adds options.
pub const UNLOCK_BASE: u64 = 0;
/// Second ring unlock threshold.
pub const UNLOCK_SILVER: u64 = 15;
/// Third ring unlock threshold.
pub const UNLOCK_GOLD: u64 = 40;

/// Unique perk id (stable across runs; stored in summaries).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PerkId(pub &'static str);

/// One perk: a readable rule plus its modifier function.
#[derive(Debug, Clone, Copy)]
pub struct Perk {
    /// Stable id.
    pub id: PerkId,
    /// Display name.
    pub name: &'static str,
    /// One-line rule.
    pub description: &'static str,
    /// Minimum lifetime shards to appear in offers.
    pub unlock_at: u64,
    /// The rule, as a modifier mutation.
    pub apply: fn(&mut RunModifiers),
}

fn overclock(m: &mut RunModifiers) {
    m.ball_speed_mul *= 1.12;
    m.score_mul *= 1.25;
}
fn second_serve(m: &mut RunModifiers) {
    m.life_refund_per_level = m.life_refund_per_level.saturating_add(1);
}
fn long_fuse(m: &mut RunModifiers) {
    m.powerup_duration_mul *= 1.5;
}
fn greedy(m: &mut RunModifiers) {
    m.score_mul *= 1.5;
    m.starting_lives -= 1;
}
fn magnet(m: &mut RunModifiers) {
    m.magnet_strength += 60.0;
}
fn steady(m: &mut RunModifiers) {
    m.paddle_width_mul *= 1.1;
    m.ball_speed_mul *= 0.95;
}
fn bargain(m: &mut RunModifiers) {
    m.drop_rate_add += 0.05;
}
fn glass_cannon(m: &mut RunModifiers) {
    m.starting_lives = 1 - super::tuning::STARTING_LIVES;
    m.score_mul *= 2.0;
}
fn lightning(m: &mut RunModifiers) {
    m.ball_speed_mul *= 1.15;
}
fn insurance(m: &mut RunModifiers) {
    m.starting_lives += 2;
    m.score_mul *= 0.85;
}
fn showtime(m: &mut RunModifiers) {
    m.score_mul *= 1.3;
}
fn phoenix(m: &mut RunModifiers) {
    m.life_refund_per_level = m.life_refund_per_level.saturating_add(2);
}

/// The v1 pool: 12 entries (FR-41), each a single readable rule.
pub const PERKS: &[Perk] = &[
    Perk {
        id: PerkId("overclock"),
        name: "Overclock",
        description: "+12% ball speed, +25% score",
        unlock_at: UNLOCK_BASE,
        apply: overclock,
    },
    Perk {
        id: PerkId("second-serve"),
        name: "Second Serve",
        description: "First life lost per level is refunded",
        unlock_at: UNLOCK_BASE,
        apply: second_serve,
    },
    Perk {
        id: PerkId("long-fuse"),
        name: "Long Fuse",
        description: "+50% powerup durations",
        unlock_at: UNLOCK_BASE,
        apply: long_fuse,
    },
    Perk {
        id: PerkId("greedy"),
        name: "Greedy",
        description: "+50% score, -1 starting life",
        unlock_at: UNLOCK_BASE,
        apply: greedy,
    },
    Perk {
        id: PerkId("magnet"),
        name: "Magnet",
        description: "Paddle attracts falling capsules",
        unlock_at: UNLOCK_BASE,
        apply: magnet,
    },
    Perk {
        id: PerkId("steady"),
        name: "Steady",
        description: "+10% paddle width, -5% ball speed",
        unlock_at: UNLOCK_BASE,
        apply: steady,
    },
    Perk {
        id: PerkId("bargain"),
        name: "Bargain",
        description: "+5% powerup drop chance",
        unlock_at: UNLOCK_SILVER,
        apply: bargain,
    },
    Perk {
        id: PerkId("glass-cannon"),
        name: "Glass Cannon",
        description: "One life, double score",
        unlock_at: UNLOCK_SILVER,
        apply: glass_cannon,
    },
    Perk {
        id: PerkId("lightning"),
        name: "Lightning",
        description: "+15% ball speed",
        unlock_at: UNLOCK_SILVER,
        apply: lightning,
    },
    Perk {
        id: PerkId("insurance"),
        name: "Insurance",
        description: "+2 starting lives, -15% score",
        unlock_at: UNLOCK_GOLD,
        apply: insurance,
    },
    Perk {
        id: PerkId("showtime"),
        name: "Showtime",
        description: "+30% score",
        unlock_at: UNLOCK_GOLD,
        apply: showtime,
    },
    Perk {
        id: PerkId("phoenix"),
        name: "Phoenix",
        description: "First 2 lives lost per level refunded",
        unlock_at: UNLOCK_GOLD,
        apply: phoenix,
    },
];

/// Perks offered at `lifetime_shards`, minus already-taken ids.
pub fn unlocked(lifetime_shards: u64, taken: &[PerkId]) -> Vec<&'static Perk> {
    PERKS
        .iter()
        .filter(|p| lifetime_shards >= p.unlock_at && !taken.contains(&p.id))
        .collect()
}

/// Draw 3 offers (or all available when fewer than 3 remain) from the
/// unlocked pool by seed. Taken perks are never re-offered.
pub fn offer(
    rng: &mut super::rng::Rng,
    lifetime_shards: u64,
    taken: &[PerkId],
) -> Vec<&'static Perk> {
    let mut pool = unlocked(lifetime_shards, taken);
    // Fisher-Yates with the run RNG (deterministic per seed).
    for i in (1..pool.len()).rev() {
        let j = rng.next_u32_below(i as u32 + 1) as usize;
        pool.swap(i, j);
    }
    pool.truncate(3);
    pool
}

/// Apply a perk to modifiers by id. Unknown ids are ignored (forward
/// compatibility with future pools).
pub fn apply_by_id(modifiers: &mut RunModifiers, id: PerkId) {
    if let Some(p) = PERKS.iter().find(|p| p.id == id) {
        (p.apply)(modifiers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_has_twelve() {
        assert!(PERKS.len() >= 12);
    }

    #[test]
    fn unlocks_only_add_options() {
        let base = unlocked(0, &[]).len();
        let silver = unlocked(UNLOCK_SILVER, &[]).len();
        let gold = unlocked(UNLOCK_GOLD, &[]).len();
        assert!(base >= 6 && silver > base && gold > silver);
        assert_eq!(gold, PERKS.len());
    }

    #[test]
    fn offer_never_repeats_taken() {
        let mut rng = super::super::rng::Rng::from_seed(3);
        let taken = [PerkId("overclock")];
        for _ in 0..20 {
            let o = offer(&mut rng, UNLOCK_GOLD, &taken);
            assert!(o.len() <= 3);
            assert!(!o.iter().any(|p| p.id == PerkId("overclock")));
        }
    }

    #[test]
    fn offer_all_when_fewer_than_three() {
        let mut rng = super::super::rng::Rng::from_seed(3);
        let taken: Vec<PerkId> = PERKS[2..].iter().map(|p| p.id).collect();
        let o = offer(&mut rng, UNLOCK_GOLD, &taken);
        assert_eq!(o.len(), 2);
    }

    #[test]
    fn perks_compose_through_modifiers() {
        let mut m = RunModifiers::default();
        (overclock)(&mut m);
        (greedy)(&mut m);
        assert!((m.ball_speed_mul - 1.12).abs() < 1e-6);
        assert!((m.score_mul - 1.25 * 1.5).abs() < 1e-4);
        assert_eq!(m.starting_lives, -1);
    }
}
