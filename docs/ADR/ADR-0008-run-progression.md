---
title: ADR-0008 — Roguelite run structure, data-driven perks, and the save profile
type: adr
tags: [project-plan, adr, breakout]
status: accepted
date: 2026-08-29
---

# ADR-0008 — Roguelite run structure, data-driven perks, and the save profile

**Status:** Accepted · **Date:** 2026-08-29

## Context

Classic Breakout has no reason to be replayed once you have cleared it. The user asked for
roguelite runs with between-level perk choices and persistent unlocks, which is the mechanism
that turns "I cleared it" into "one more run". The design question is how to express perks
without turning the physics code into a pile of special cases, and how to store progression
without inventing a save system that can lose a run.

## Decision

### Run structure

- A **run** is 8 levels. Levels 1–2 come from tier 1, 3–4 from tier 2, 5–6 from tier 3, 7–8 from
  tier 4 (tiers as defined in ADR-0007), sampled without replacement from each pool by the run
  seed (PRD FR-39).
- The seed is random per run and printable; `--seed <n>` fixes it, which makes a run
  reproducible and is what the Phase 8 determinism test uses.
- After each cleared level the player is offered **3 perks drawn from the unlocked pool**, minus
  perks already taken, and picks one. Perks last for the rest of the run (PRD FR-40).
- A run ends on death or after level 8 and shows the summary in PRD FR-42.

### Perks are data, not code

**A perk is a value that mutates a `RunModifiers` struct. The simulation reads `RunModifiers`.
No perk may add a branch to `physics.rs`.**

```rust
struct RunModifiers {
    ball_speed_mul: f32,      paddle_width_mul: f32,
    score_mul: f32,           drop_rate_add: f32,
    starting_lives: i32,      life_refund_per_level: u8,
    powerup_duration_mul: f32, magnet_strength: f32,
    shrapnel_on_break: u8,    pierce_chance: f32,
    // …one field per mechanical axis, never one field per perk
}
```

A perk is `{ id, name, one-line description, fn apply(&mut RunModifiers) }`. Adding a perk means
adding a row to the table; if it cannot be expressed as a modifier, the correct move is to add a
*modifier axis* that the simulation reads, not a special case. This is the rule that keeps the
perk count growable without the physics rotting.

The v1 pool is 12+ perks (PRD FR-41). Examples and the axes they use: *Overclock*
(`ball_speed_mul`, `score_mul`), *Second Serve* (`life_refund_per_level`), *Shrapnel*
(`shrapnel_on_break`), *Magnet* (`magnet_strength`), *Glass Cannon* (`starting_lives`,
`score_mul`), *Long Fuse* (`powerup_duration_mul`).

### Meta progression

- A run awards **shards** based on levels cleared, bricks destroyed and best combo.
- Shards are persistent and cross fixed thresholds that unlock further perks and starting
  modifiers (PRD FR-43). Unlocks only ever *add options*; they never make a run easier by
  default, so a fresh profile and a veteran profile play the same game with different menus.

### Save profile

- One JSON file, `serde`-serialised, at
  `~/Library/Application Support/breakout/profile.json` (macOS) or
  `$XDG_CONFIG_HOME/breakout/profile.json` (Linux, defaulting to `~/.config`).
- **Atomic writes**: serialise to `profile.json.tmp` in the same directory, `fsync`, then
  `rename` over the target. A crash mid-save can never truncate the profile.
- Written at run end and on settings changes — not per level, and never per frame.
- A `schema_version` integer is the first field. An unknown future version is renamed aside
  rather than parsed.
- **Corruption recovery**: an unreadable or unparseable profile is renamed to
  `profile.corrupt.<unix-ts>.json` and replaced with a fresh one. Play never blocks on the save
  file (PRD FR-44).

## Consequences

**Good**

- Perks compose by construction: two perks touching `ball_speed_mul` multiply, with no
  interaction code to write and no combinatorial testing.
- The whole progression system is testable headlessly — apply perks, run a seeded replay, assert
  the summary.
- Atomic writes plus rename-aside recovery means the worst realistic failure costs the player
  their unlocks, not their ability to play.

**Bad, and accepted**

- The modifier-axis design cannot express a perk that changes a *rule* rather than a *number*
  (say, "the ball wraps around the screen edges"). Such a perk needs a new axis and a small,
  deliberate piece of simulation code — which is the point: it forces the decision into the open
  instead of into an `if`.
- Sampling levels without replacement from small tier pools means runs repeat levels early on
  until the campaign grows. Mitigated by shipping 16+ levels and weighting toward unseen ones.
- Persistent unlocks mean the first few runs have a smaller perk pool, so early runs are
  slightly less interesting. Accepted — that is the progression.
- JSON is not compact or fast. At this size, irrelevant, and being able to read the save in a
  text editor while debugging is worth more.

## Alternatives rejected

- **Perks as trait objects with a `on_brick_destroyed` hook.** More expressive, and it scatters
  gameplay across a dozen small implementations that interact unpredictably and cannot be
  reasoned about as a set.
- **A per-perk branch in the physics code.** The thing this ADR exists to prevent.
- **SQLite or a binary save.** Enormously more machinery than one JSON object needs.
- **Saving after every level.** More writes, more chances to corrupt, and it removes the risk
  that makes a roguelite run tense.

## Related

- [[project-plan/Breakout/ADR/ADR-0006-fixed-timestep|ADR-0006]] — determinism this depends on
- [[project-plan/Breakout/ADR/ADR-0007-level-format|ADR-0007]] — where `tier` comes from
- [[project-plan/Breakout/PLAN|PLAN.md]] — Phase 8
- [[project-plan/Breakout/PRD|PRD]] — FR-39 … FR-45
