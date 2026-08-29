---
title: ADR-0007 — Level file format: TOML header plus an ASCII grid
type: adr
tags: [project-plan, adr, breakout]
status: accepted
date: 2026-08-29
---

# ADR-0007 — Level file format: TOML header plus an ASCII grid

**Status:** Accepted · **Date:** 2026-08-29

## Context

PRD FR-35 requires levels a human can author in a text editor with no tooling, and FR-37
requires 16+ of them shipped inside the binary. The format has to be readable in a diff, obvious
enough that a level looks like the level it describes, and strict enough that the parser can
give a line and column when it is wrong.

## Decision

**A `.lvl` file is a TOML header, a `---` separator line, and an ASCII grid where one character
is one brick cell.**

```
name = "Crossfire"
tier = 2
ball_speed = 1.15      # optional, multiplier on the base speed, default 1.0
drop_rate = 0.10       # optional, powerup drop chance per brick, default from tuning.rs
palette = "neon"       # optional, default "neon"
---
..SS..........SS..
..1111....1111....
..222222222222....
....33E33333......
..SS..........SS..
```

**Grid characters — the complete set for v1:**

| Char | Meaning |
| --- | --- |
| `.` | empty |
| `1`–`5` | destructible brick with that many hit points |
| `S` | steel, indestructible, does not count toward clearing (PRD FR-18) |
| `E` | explosive, 1 HP, destroys its 3 × 3 neighbourhood (PRD FR-19) |

**Rules the parser enforces:**

1. The grid is 1–14 rows of exactly 18 columns. Ragged rows are an error, not padded.
2. Blank lines and `#` comment lines are allowed in the header and *not* in the grid.
3. At least one destructible brick, or the level can never be cleared.
4. An unknown character is an error naming the file, line, column and the character itself
   (PRD FR-36).
5. `tier` is 1–4 and selects which run difficulty pool the level joins (ADR-0008).
6. The parser returns `Result`, never panics, for any input whatsoever — enforced by a property
   test in Phase 5.

**Loading:** campaign levels live in `assets/levels/campaign/NN-slug.lvl` and are baked into the
binary by a `build.rs`-generated manifest of `include_str!`s, so an installed binary needs no
data files. `--level <path>` reads from disk instead, so authoring does not require a rebuild.

## Consequences

**Good**

- A level file *looks like the level*. That is the entire argument for this format over anything
  structured, and it is worth a lot when hand-authoring 16 of them.
- Diffs are meaningful; a changed brick is a changed character.
- The header is TOML, so adding an optional per-level knob later costs nothing and old files
  keep parsing.
- Fixed 18 columns means the brick grid maps to the 320-wide playfield (ADR-0003) with no
  per-level layout maths.

**Bad, and accepted**

- The grid cannot express anything that is not on the grid: moving bricks, curved arrangements,
  per-brick properties beyond type. If v2 wants those, it needs a new ADR and probably an
  optional second section, not a rewrite of this one.
- Trailing whitespace is invisible and will cause "expected 18 columns, found 19" errors that
  look mysterious. The parser must say *trailing whitespace* explicitly when that is the cause.
- Baking levels into the binary means adding a level requires a rebuild. `--level` covers the
  authoring loop, so this only affects shipping.

## Alternatives rejected

- **JSON or TOML arrays of brick objects.** Precise, verbose, and impossible to read as a
  picture. Authoring 16 levels this way by hand would not happen.
- **A binary format.** No reason; these files are tiny and being diffable is worth more than
  being small.
- **Loading from a data directory at runtime.** Means `cargo install` alone does not produce a
  working game, which breaks PRD success criterion 1.
- **An in-game level editor.** Explicitly out of scope (PRD §6); a text editor is the editor.

## Related

- [[project-plan/Breakout/ADR/ADR-0003-logical-resolution|ADR-0003]]
- [[project-plan/Breakout/ADR/ADR-0008-run-progression|ADR-0008]]
- [[project-plan/Breakout/PLAN|PLAN.md]] — Phase 5
- [[project-plan/Breakout/PRD|PRD]] — FR-35 … FR-38
