---
title: Breakout — GPU-accelerated terminal arcade
type: index
tags: [index, project-plan, breakout, rust, terminal]
---

# 🧱 Breakout

A modern Breakout that runs inside Ghostty, drawn as real pixels on the GPU, launched by typing
`breakout`. Rust, macOS and Linux, keyboard only.

> **Agent: read this file first, then `AGENTS.md`, then `PLAN.md`.** Do not start writing code
> until you can state which phase you are in and what its test gate is. If a decision you need is
> not in the PRD or an ADR, stop and ask.

## One-paragraph summary

Ghostty renders its screen on the GPU and implements the Kitty graphics protocol, so a terminal
program can hand it a full pixel image every frame and have it composited by the GPU. Almost
nothing does. This game rasterises a 320 × 240 logical playfield in Rust, integer-scales it to
the window, and pushes it to Ghostty through POSIX shared memory 60 times a second — which buys
sub-pixel ball motion, anti-aliased shapes, particles, bloom and screenshake in a terminal. On
top of that sits a roguelite Breakout: eight levels a run, a perk chosen after each, seven
powerups, hand-authored levels in a plain-text format, and persistent unlocks. It installs with
`cargo install` and runs as `breakout`.

## Document map

| Document | What it is for | Read it when |
| --- | --- | --- |
| [[project-plan/Breakout/AGENTS\|AGENTS.md]] | House rules, repo layout, exact commands, guardrails, definition of done | Before writing any code, every session |
| [[project-plan/Breakout/PRD\|PRD.md]] | What we are building; numbered FR-* / NFR-*; what is out of scope | When deciding whether something is in scope |
| [[project-plan/Breakout/PLAN\|PLAN.md]] | Ten phases, each with a runnable outcome and a local test gate | Every phase, start and end |
| [[project-plan/Breakout/MOCKUP\|MOCKUP.md]] | Layout coordinates, palette, HUD, effect specification | Phase 4, and before drawing anything |
| `mockup.html` (in this folder) | The three screens rendered — open in a browser, not in Obsidian | Alongside MOCKUP.md |
| [[project-plan/Breakout/ADR/index\|ADR/]] | Index of all ten architectural decisions | Before opening any single ADR |
| [[project-plan/Breakout/ADR/ADR-0001-kitty-graphics-renderer\|ADR/ADR-0001]] | Pixel framebuffer, not text cells — and what "GPU-accelerated" honestly means | Before touching the renderer |
| [[project-plan/Breakout/ADR/ADR-0002-frame-transport\|ADR/ADR-0002]] | Shared-memory frame transport, image-id double buffering | Phase 1, or when frames are slow |
| [[project-plan/Breakout/ADR/ADR-0003-logical-resolution\|ADR/ADR-0003]] | 320 × 240 logical, integer scaling | Before any coordinate maths |
| [[project-plan/Breakout/ADR/ADR-0004-keyboard-input\|ADR/ADR-0004]] | Kitty keyboard protocol, held-key state, fallback | Phase 3 |
| [[project-plan/Breakout/ADR/ADR-0005-stack\|ADR/ADR-0005]] | The complete dependency budget and module layout | Before adding any dependency |
| [[project-plan/Breakout/ADR/ADR-0006-fixed-timestep\|ADR/ADR-0006]] | 240 Hz simulation, determinism | Phase 2, and before any physics change |
| [[project-plan/Breakout/ADR/ADR-0007-level-format\|ADR/ADR-0007]] | The `.lvl` format | Phase 5 |
| [[project-plan/Breakout/ADR/ADR-0008-run-progression\|ADR/ADR-0008]] | Runs, perks as data, the save profile | Phase 8 |
| [[project-plan/Breakout/ADR/ADR-0009-distribution\|ADR/ADR-0009]] | `breakout-tui` the package, `breakout` the command | Phase 9 |
| [[project-plan/Breakout/ADR/ADR-0010-terminal-state-safety\|ADR/ADR-0010]] | The RAII guard that must always run | Phase 0, and re-checked every phase |

## Status

- **Current phase:** not started. Phase 0 is next, and **PRD OQ-1 (repo location) must be
  answered first** — no repo exists yet and the plan deliberately does not assume one.
- **Mockup:** `mockup.html` in this folder — open it in a browser, not in Obsidian's preview.
  Also published at https://claude.ai/code/artifact/d8046bd7-4a50-455b-a206-dc9e718786c9
  for opening on a phone. The playfield panel is a live simulation, not a still.
- **Phase 1 is the risk.** It exists second, not last, because if Ghostty's graphics path cannot
  hold 60 fps for full-screen frames, that invalidates ADR-0002 and possibly the whole approach.
  Find out in an evening, not in a month.
- **Decisions locked:** pixel framebuffer via the Kitty graphics protocol, no text-cell fallback;
  shared-memory transport; 320 × 240 logical with integer scaling; Kitty keyboard protocol for
  key-release with a degraded fallback; keyboard only, no mouse or gamepad; audio on by default
  with a mute key; roguelite runs with data-driven perks; plain-text levels; `cargo install` only;
  macOS and Linux, no Windows.
- **Still open:** OQ-1 (repo — Mike will add it later; the agent does not create it). All
  other OQs resolved 2026-08-29; see [[project-plan/Breakout/PRD|PRD]] §8.

## Two things worth knowing before you start

**"GPU-accelerated" is a real thing here, but not the thing it sounds like.** Ghostty does not
expose its GPU to the program. It exposes a fast path for putting an image on screen. The game
draws on the CPU; Ghostty uploads, scales and composites on the GPU. ADR-0001 states this
plainly so nobody builds on a wrong assumption.

**The crate name `breakout` is taken on crates.io.** The package is `breakout-tui`; the binary
is `breakout`, which is what the user types. See ADR-0009.

## Related

- [[project-plan/index|Project Plans]]
- [[Projects/index|Projects]]
- [[Topics/Building and Shipping Side Projects|Building and Shipping Side Projects]]
