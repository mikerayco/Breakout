---
title: ADR-0012 — Stardew pastel palette replaces the neon MOCKUP palette
type: adr
tags: [project-plan, adr, breakout, palette]
status: accepted
date: 2026-09-04
---

# ADR-0012 — Stardew pastel palette replaces the neon MOCKUP palette

**Status:** Accepted · **Date:** 2026-09-04 · **Supersedes:** the MOCKUP.md §2
palette table and the PRD OQ-4 resolution ("ship the MOCKUP neon palette
only in v1"), on Mike's direct order 2026-09-04.

## Context

The v1 look was specified as neon-on-near-black (MOCKUP §2, normative).
Mike asked for something cute in the spirit of Stardew Valley: warm
pastels, cream text, dusky-purple night-soil backgrounds.

## Decision

**Replace every hex in `src/render/palette.rs` with the Stardew pastel
set below.** Names, roles and the "damaged recolours to the tier below"
rule are unchanged — only the hex values move, so no rendering, physics
or HUD code changes.

| Name | Hex | Used for |
| --- | --- | --- |
| `bg-void` | `#171422` | Area outside the 320×240 screen |
| `bg-deep` | `#242031` | Play area background |
| `bg-hud` | `#2e2845` | HUD band |
| `grid-line` | `#3a3354` | Faint background grid, 17×8 lattice |
| `bezel` | `#9c7a54` | Fence-wood walls and the HUD rule |
| `bezel-lit` | `#c9a876` | Inner highlight, wall flash |
| `text` | `#fff8e7` | Primary HUD text (cream) |
| `text-dim` | `#a89bc4` | Labels, inactive elements |
| `paddle` | `#8fd6b4` | Paddle body (mint leaf) |
| `paddle-cap` | `#d8f3dc` | Paddle top edge |
| `ball` | `#fff3d6` | Ball core (cream) |
| `ball-glow` | `#ffd166` | Ball trail and bloom tint (sunny yolk) |
| `brick-1` | `#7fb6d9` | 1 HP (soft well-water blue) |
| `brick-2` | `#8fd6a0` | 2 HP (spring leaf) |
| `brick-3` | `#ffd166` | 3 HP (chicken gold) |
| `brick-4` | `#ff9e6d` | 4 HP (peach) |
| `brick-5` | `#ef6f8b` | 5 HP (berry) |
| `brick-steel` | `#8d8aa8` | Indestructible (stone) |
| `brick-explosive` | `#e5484d` | Explosive, 1px `#ffd166` core |
| `powerup` | `#b892ff` | Capsules and trails (lilac) |
| `combo` | `#ff7ab8` | Combo counter at ×3+ (pink) |
| `danger` | `#ef476f` | Last life, low timers |

Luminance ordering of the roles is preserved (bright ball/trail/combo on
dark soil), so the bloom threshold (0.72) and flash language behave as
before. `docs/MOCKUP.md` §2 is updated to this table.

## Consequences

**Good:** the requested look with zero code churn outside two files.

**Bad, and accepted:** screenshots/mockup comparisons in PERF.md from the
neon era no longer match; the mockup.html companion still shows neon and
is stale until regenerated. OQ-4's deferral is spent — a future palette
variant would need another ADR.

## Related

- [[project-plan/Breakout/MOCKUP|MOCKUP.md]] §2 (updated to match)
- [[project-plan/Breakout/PRD|PRD.md]] OQ-4
- `src/render/palette.rs`
