//! The 8-level run: tier pools sampled by seed (FR-39, ADR-0008).
//!
//! Owns run construction (levels 1-2 from tier 1, 3-4 from tier 2, 5-6
//! from tier 3, 7-8 from tier 4, sampled without replacement), the shard
//! economy (FR-43) and the run summary (FR-42). Deterministic per seed:
//! the same seed builds the same 8 ids, which is what the Phase 8
//! determinism test asserts.

use super::rng::Rng;

/// Levels in a run.
pub const RUN_LEN: usize = 8;

/// Tier per run position: 1,1,2,2,3,3,4,4 (ADR-0008).
pub const RUN_TIERS: [u8; RUN_LEN] = [1, 1, 2, 2, 3, 3, 4, 4];

/// One run level: campaign id + tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLevel {
    /// Campaign id (`NN-slug`).
    pub id: String,
    /// Difficulty tier 1-4.
    pub tier: u8,
}

/// Build the 8-level run from the campaign pool by seed (FR-39). Each
/// position draws without replacement from its tier pool, so small pools
/// still vary across seeds. Unknown tiers are skipped, never panicked on.
pub fn build_run(campaign: &[(String, super::level::Level)], seed: u64) -> Vec<RunLevel> {
    let mut rng = Rng::from_seed(seed);
    let mut run = Vec::with_capacity(RUN_LEN);
    for tier in RUN_TIERS {
        // Pool of ids at this tier not already taken this run.
        let mut pool: Vec<&(String, super::level::Level)> = campaign
            .iter()
            .filter(|(id, lvl)| lvl.tier == tier && !run.iter().any(|r: &RunLevel| r.id == *id))
            .collect();
        if pool.is_empty() {
            // Tier pool exhausted (tiny campaign): reuse the tier pool.
            pool = campaign
                .iter()
                .filter(|(_, lvl)| lvl.tier == tier)
                .collect();
        }
        if pool.is_empty() {
            continue;
        }
        let pick = rng.next_u32_below(pool.len() as u32) as usize;
        run.push(RunLevel {
            id: pool[pick].0.clone(),
            tier,
        });
    }
    run
}

/// Shards earned for a finished run (FR-43): 4 per level cleared plus
/// brick and combo bonuses. Persistent currency for unlocks.
pub fn shards_earned(levels_cleared: u32, bricks_destroyed: u32, best_combo: u32) -> u64 {
    u64::from(levels_cleared) * 4 + u64::from(bricks_destroyed) / 8 + u64::from(best_combo) / 2
}

/// End-of-run summary (FR-42).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    /// Total score across the run.
    pub score: u64,
    /// Levels cleared (0-8).
    pub levels_cleared: u32,
    /// Bricks destroyed.
    pub bricks_destroyed: u32,
    /// Best combo seen.
    pub best_combo: u32,
    /// Perk ids taken, in pick order.
    pub perks: Vec<String>,
    /// Shards earned this run.
    pub shards: u64,
    /// The run seed (reproducibility).
    pub seed: u64,
}

impl RunSummary {
    /// Assemble from run totals.
    pub fn new(
        score: u64,
        levels_cleared: u32,
        bricks_destroyed: u32,
        best_combo: u32,
        perks: Vec<String>,
        seed: u64,
    ) -> Self {
        Self {
            score,
            levels_cleared,
            bricks_destroyed,
            best_combo,
            perks,
            shards: shards_earned(levels_cleared, bricks_destroyed, best_combo),
            seed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_campaign() -> Vec<(String, crate::game::level::Level)> {
        // 2 per tier: exactly what a run needs.
        let mut v = Vec::new();
        for tier in 1..=4u8 {
            for n in 0..2 {
                let src =
                    format!("name = \"t{tier}-{n}\"\ntier = {tier}\n---\n....11111.........\n");
                v.push((
                    format!("t{tier}-{n}"),
                    crate::game::level::parse_str("test", &src).expect("valid"),
                ));
            }
        }
        v
    }

    #[test]
    fn same_seed_same_run() {
        let camp = fake_campaign();
        let a = build_run(&camp, 7);
        let b = build_run(&camp, 7);
        assert_eq!(a, b);
        assert_eq!(a.len(), RUN_LEN);
        assert_eq!(
            a.iter().map(|l| l.tier).collect::<Vec<_>>(),
            RUN_TIERS.to_vec()
        );
    }

    #[test]
    fn run_uses_both_pool_members_across_seeds() {
        // Without replacement, positions 0-1 (tier 1) hold both members in
        // some order for every seed.
        let camp = fake_campaign();
        for seed in [1, 2, 3, 42, 99] {
            let run = build_run(&camp, seed);
            let mut tier1: Vec<_> = run.iter().take(2).map(|l| l.id.clone()).collect();
            tier1.sort();
            assert_eq!(tier1, vec!["t1-0".to_string(), "t1-1".to_string()]);
        }
    }

    #[test]
    fn shards_scale_with_progress() {
        assert_eq!(shards_earned(0, 0, 0), 0);
        let full = shards_earned(8, 200, 8);
        let part = shards_earned(3, 50, 4);
        assert!(full > part && part > 0);
        assert_eq!(shards_earned(8, 0, 0), 32);
    }

    /// Headless full-run sim: AI paddle per level, first offer always
    /// picked. Same seed + same script must give a byte-identical summary
    /// (NFR-10, same-machine scope).
    fn simulate_run(seed: u64) -> RunSummary {
        use super::super::perk;
        use super::super::physics::{InputState, RunModifiers, World};
        use super::super::rng::Rng;
        use super::super::tuning;

        // One tiny level per tier (pools reuse across the 8 positions).
        let mut camp = Vec::new();
        for tier in 1..=4u8 {
            let src = format!("name = \"rt{tier}\"\ntier = {tier}\n---\n....11111.........\n");
            camp.push((
                format!("rt{tier}"),
                super::super::level::parse_str("test", &src).expect("valid"),
            ));
        }
        let spec = build_run(&camp, seed);
        assert_eq!(spec.len(), RUN_LEN);
        let mut offer_rng = Rng::from_seed(seed ^ 0x51ED);
        let mut modifiers = RunModifiers::default();
        let mut taken = Vec::new();
        let (mut score, mut bricks, mut best) = (0u64, 0u32, 0u32);
        let mut lives = super::super::tuning::STARTING_LIVES;
        let mut cleared = 0u32;
        for (pos, rl) in spec.iter().enumerate() {
            let lvl = camp
                .iter()
                .find(|(id, _)| id == &rl.id)
                .map(|(_, l)| l)
                .unwrap();
            let mut world = World::new(
                lvl.bricks.clone(),
                seed ^ (pos as u64 + 1),
                pos as u32,
                modifiers,
            );
            world.lives = lives;
            // AI: track lowest ball, launch when stuck; 15 sim-seconds max.
            for step in 0..3600 {
                if world.state != super::super::state::GameState::Playing {
                    break;
                }
                let target = world
                    .balls
                    .iter()
                    .filter(|b| !b.stuck)
                    .min_by(|a, b| b.y.partial_cmp(&a.y).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|b| b.x)
                    .unwrap_or(world.paddle_x);
                let stuck = world.balls.iter().any(|b| b.stuck);
                world.step(
                    InputState {
                        left: target < world.paddle_x - 2.0,
                        right: target > world.paddle_x + 2.0,
                        launch: stuck && step % 30 == 0,
                    },
                    tuning::DT,
                );
            }
            score += world.score.points;
            bricks += world.score.bricks_destroyed;
            best = best.max(world.score.best_combo);
            lives = world.lives;
            if world.state == super::super::state::GameState::LevelClear {
                cleared += 1;
                if pos + 1 < RUN_LEN {
                    let choices = perk::offer(&mut offer_rng, 0, &taken);
                    if let Some(first) = choices.first() {
                        perk::apply_by_id(&mut modifiers, first.id);
                        taken.push(first.id);
                    }
                }
            } else {
                break;
            }
        }
        let perks = taken.iter().map(|id| id.0.to_string()).collect();
        RunSummary::new(score, cleared, bricks, best, perks, seed)
    }

    #[test]
    fn scripted_run_replays_identically() {
        let a = simulate_run(7);
        let b = simulate_run(7);
        assert_eq!(a, b);
    }

    #[test]
    fn summary_carries_shards_and_seed() {
        let s = RunSummary::new(1000, 8, 200, 8, vec!["overclock".into()], 7);
        assert_eq!(s.shards, shards_earned(8, 200, 8));
        assert_eq!(s.seed, 7);
    }
}
