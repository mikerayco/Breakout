---
title: ADR-0005 — Stack, dependency budget and module layout
type: adr
tags: [project-plan, adr, breakout]
status: accepted
date: 2026-08-29
---

# ADR-0005 — Stack, dependency budget and module layout

**Status:** Accepted · **Date:** 2026-08-29

## Context

Rust is the chosen language. Beyond that, the constraint that matters is that this code will be
written largely by open-weight models driving the Pi coding agent. Those models write well
against boring, heavily-documented, widely-used APIs, and badly against clever ones. A second
constraint: nothing in the dependency tree may pull in a windowing library, a GPU backend, or an
async runtime — the whole point is that the terminal is the display.

## Decision

### Dependencies — this is the entire budget

| Crate | Version | Why it is here |
| --- | --- | --- |
| `crossterm` | 0.29 | Raw mode, alternate screen, resize events, and the keyboard enhancement flags of ADR-0004. The only terminal crate with all of that on both platforms. |
| `tiny-skia` | 0.12 | Pure-Rust software rasteriser: anti-aliased paths, blend modes, no C dependency, no GPU. Gives the AA shapes and additive blending that ADR-0001 exists for. |
| `kira` | 0.12 | Audio mixing with per-track volume and pitch control — needed for the combo pitch ramp and hit-stop ducking (PRD FR-47, Phase 7). |
| `libc` | 0.2 | `shm_open`, `ftruncate`, `mmap`, `TIOCGWINSZ`. Confined to `term/shm.rs` and `term/caps.rs`. |
| `base64` | 0.22 | Encoding the shm object name, and the `t=d` fallback payload. |
| `serde` + `serde_json` | 1 | The save profile (PRD FR-44). |
| `toml` | 0.8 | The `.lvl` header (ADR-0007). |
| `fastrand` | 2 | Seedable, zero-dependency RNG. Determinism (NFR-10) needs a generator we fully control. |
| `clap` | 4 (`derive`) | The flag set in FR-3. |
| `anyhow` | 1 | Error plumbing at the boundary. |

Dev-dependencies: `proptest` (the level-parser property test, Phase 5). Nothing else.

**Rule:** adding any other runtime dependency requires a new ADR in this folder, accepted before
the code is written. An agent that wants an ECS, a game framework, an image crate, a font
loader, `tokio`, `rayon` or a logging framework must stop and ask.

- **No `ratatui`.** There is no text UI (ADR-0001).
- **No `image` / `png` crates.** Frames are raw RGB (ADR-0002); nothing is encoded.
- **No font crate.** A hand-rolled 5 × 7 bitmap font in `render/text.rs`, which is both smaller
  and the correct aesthetic at 320 × 240 (ADR-0003).
- **No async runtime.** The loop is a loop.

### Toolchain

Rust stable, 2021 edition, MSRV pinned in `Cargo.toml` and enforced by a CI job.
`rustfmt` defaults. `clippy` with `-D warnings` in CI (PRD NFR-9). Release profile:
`opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `panic = "unwind"` — **not** `abort`,
because the panic hook must run to restore the terminal (ADR-0010).

### Module layout

The layout in [[project-plan/Breakout/AGENTS|AGENTS.md]] §1 is normative. The shape of it:
`term/` owns everything that talks to the terminal, `render/` turns game state into pixels,
`game/` is pure simulation with no I/O, `audio/` and `save/` are leaves. **`game/` must not
import from `term/` or `render/`.** That one rule is what keeps the simulation deterministic and
unit-testable.

## Consequences

**Good**

- Ten runtime crates, none of them exotic, all of them things a model has seen thousands of
  examples of. Build times stay short, which matters when the workflow is agent-driven iteration.
- No C toolchain requirement beyond what `kira`'s backend needs, so `cargo install` works on a
  clean machine.
- The `game/` isolation rule makes the physics tests and the determinism test possible.

**Bad, and accepted**

- `tiny-skia` is a software rasteriser, so all drawing cost is CPU. That is inherent to ADR-0001,
  not a consequence of this choice; the alternative would be writing the rasteriser by hand.
- `kira` pulls a platform audio backend (CoreAudio / ALSA), which is the one place where a Linux
  user may need a dev package installed. PRD FR-49 requires the game to run silently rather than
  fail if audio is unavailable.
- Hand-rolling the font and the CLI-adjacent bits means a little more code than a library would.
  At this size, fine, and it removes a whole class of API-hallucination failures.

## Alternatives rejected

- **`ratatui`.** The wrong renderer for this project (ADR-0001).
- **`macroquad` / `ggez` / `bevy`.** Real game frameworks that would make this easy and would
  open a window. The request is a terminal game.
- **`softbuffer` + a hand-written rasteriser.** More control, much more code, and the code would
  be the least interesting part of the project.
- **`rodio` instead of `kira`.** Simpler, but no per-track pitch or volume automation, which the
  combo pitch ramp and the hit-stop duck both need.
- **`rand` instead of `fastrand`.** `rand`'s API churns across versions and pulls a dependency
  tree; `fastrand` is one file's worth of API and trivially seedable.

## Related

- [[project-plan/Breakout/AGENTS|AGENTS.md]] — the normative layout and guardrails
- [[project-plan/Breakout/ADR/ADR-0001-kitty-graphics-renderer|ADR-0001]]
- [[project-plan/Breakout/PRD|PRD]] — NFR-8, NFR-9, NFR-11
