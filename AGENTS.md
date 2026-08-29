---
title: Breakout — AGENTS
type: agents
tags: [project-plan, agents, breakout, rust]
status: draft
date: 2026-08-29
---

# AGENTS.md — Breakout

House rules for the coding agent. Read this before writing any code, at the start of every
session. Then read [[project-plan/Breakout/PLAN|PLAN.md]] and state which phase you are in.

## 0. The five rules that override everything

1. **Do not start a phase before the previous phase's stop rule has been met.**
2. **If a decision is not in the PRD or an ADR, stop and ask.** Do not choose a library, a file
   format, a key binding or a tuning value on your own.
3. **Do not add a runtime dependency.** The list in
   [[project-plan/Breakout/ADR/ADR-0005-stack|ADR-0005]] is the complete budget. Wanting a game
   engine, an ECS, a font loader, an image crate or an async runtime means writing a new ADR
   first and having it accepted.
4. **Never leave the terminal broken.** Every path out of the program goes through
   `TerminalGuard::drop`. If you write a code path that can exit without it, that is a defect
   regardless of what else works.
5. **The simulation is deterministic.** It never reads the wall clock, never uses a global RNG,
   never depends on frame rate. Violating this breaks the test strategy (PRD NFR-10).

## 1. Repository layout

Exactly this. Do not invent directories.

```
$REPO/
├── Cargo.toml               # package breakout-tui, [[bin]] name = "breakout"
├── build.rs                 # generates the compiled-in level manifest (Phase 5)
├── rust-toolchain.toml      # pinned stable channel
├── README.md
├── AGENTS.md                # a copy of this file, kept in sync
├── assets/
│   ├── levels/campaign/NN-slug.lvl
│   └── audio/*.wav
├── docs/
│   ├── adr/                 # copies of the accepted ADRs, same filenames
│   ├── PERF.md              # frame-time measurements, updated at every gate
│   └── BALANCE.md           # tuning history (Phase 9)
└── src/
    ├── main.rs              # startup, guard, main loop, shutdown
    ├── cli.rs               # flag parsing (PRD FR-3), nothing else
    ├── term/
    │   ├── mod.rs
    │   ├── caps.rs          # capability probe (unsafe allowed)
    │   ├── guard.rs         # RAII terminal state
    │   ├── shm.rs           # POSIX shared memory transport (unsafe allowed)
    │   ├── kgp.rs           # Kitty graphics escape writer + direct fallback
    │   └── input.rs         # keyboard protocol, held-key state
    ├── render/
    │   ├── mod.rs           # frame orchestration
    │   ├── framebuffer.rs
    │   ├── draw.rs          # tiny-skia wrappers
    │   ├── text.rs          # 5x7 bitmap font
    │   ├── particles.rs
    │   ├── camera.rs        # screenshake
    │   ├── bloom.rs
    │   ├── palette.rs       # named colours, from MOCKUP.md
    │   └── hud.rs
    ├── game/
    │   ├── mod.rs
    │   ├── state.rs         # Title / Playing / Paused / LevelClear / RunOver
    │   ├── tuning.rs        # EVERY numeric constant, nowhere else
    │   ├── physics.rs       # swept collision only; no perk special-cases
    │   ├── level.rs         # .lvl parsing and the brick grid
    │   ├── powerup.rs
    │   ├── perk.rs          # data-driven RunModifiers
    │   ├── run.rs           # the 8-level run
    │   ├── score.rs
    │   └── rng.rs           # seeded, threaded, deterministic
    ├── audio/mod.rs
    └── save/mod.rs
```

## 2. Commands the gates call

These names are fixed. Phase gates in `PLAN.md` invoke them verbatim.

| Purpose | Command |
| --- | --- |
| Build | `cargo build --release` |
| Run | `cargo run --release -- <flags>` |
| Test | `cargo test` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Format check | `cargo fmt --check` |
| Capability report | `./target/release/breakout --caps` |
| Level validation | `cargo run --release -- --validate assets/levels/campaign` |
| Install | `cargo install --path .` |

Environment switches used only for testing, never documented to end users:

| Variable | Effect |
| --- | --- |
| `BREAKOUT_TRANSPORT=direct` | Force the base64 `t=d` transport instead of shared memory |
| `BREAKOUT_INPUT=legacy` | Force the degraded input mode (no keyboard protocol) |
| `BREAKOUT_PANIC_TEST=1` | Panic inside the render loop, to prove teardown |

## 3. Hard guardrails

- **`unsafe` lives only in `src/term/shm.rs` and `src/term/caps.rs`** (PRD NFR-8). Every block
  carries a comment naming the invariant. Add `#![forbid(unsafe_code)]`-equivalent discipline
  elsewhere by keeping those two modules the only ones with `#[allow(unsafe_code)]`.
- **No allocation in the per-frame path.** Particle pools, ball vectors and the framebuffer are
  allocated once and reused. If a profiler shows per-frame allocation, that is a bug.
- **No `unwrap()` or `expect()` outside tests and startup.** In the loop, recover or shut down
  cleanly.
- **No magic numbers outside `game/tuning.rs` and `render/palette.rs`.** If you are typing a
  number into `physics.rs`, you are in the wrong file.
- **Perks and powerups never branch inside `physics.rs`.** They mutate a `RunModifiers` /
  `ActiveEffects` struct that physics reads. If you find yourself writing
  `if perk == Perk::Overclock` in the collision code, stop.
- **No network calls, no telemetry, no update checks** (PRD NFR-5). There is no HTTP client in
  the dependency budget and there must never be one.
- **Every third-party creative asset must be free-to-use and credited.** Sound effects, music,
  and any image file bundled into the binary must carry a CC0 / public-domain / MIT-compatible
  licence; record source URL and licence in `assets/` and in a README **Credits** section
  (PRD NFR-12). The game's graphics are self-generated (no image assets in v1), so this binds
  on audio (WAV) and any future asset. Adding an asset that is not clearly free-to-use is a
  defect, not a tuning choice.
- **Do not write a text-cell renderer.** It is explicitly out of scope (PRD §6). A terminal
  without the graphics protocol gets the FR-4 error message.
- **Do not touch `docs/adr/` to change an accepted decision.** Supersede it with a new ADR.
- **Do not rename the binary.** The user types `breakout`. The package name differs from the
  binary name on purpose (ADR-0009).

## 4. Style

- Rust 2021 edition, `rustfmt` defaults, clippy pedantic where it is not noisy.
- Module-level `//!` doc comments explaining what the module owns and what it must not do.
- Prefer plain structs and enums over traits; use a trait only where a real second
  implementation exists (the audio no-op is the one legitimate case).
- Errors: `anyhow::Result` at the boundary, concrete error enums inside `term/` and `game/level`
  where the caller needs to distinguish cases.
- Comments explain *why*, never *what*. A comment restating the code gets deleted.

## 5. Testing strategy

- **Physics gets unit tests, not eyeballs.** Tunnelling, corner hits, angle clamping, explosive
  chain termination, combo reset.
- **The parser gets a property test.** No input string may panic (Phase 5).
- **The run gets a determinism test.** Fixed seed plus a recorded input log produces a
  byte-identical summary (NFR-10). This is the regression net for every later change.
- Rendering and audio are verified at the phase gates by a human, not by tests. Do not build a
  screenshot-diff harness; it is not worth it here.

## 6. Definition of done for any phase

- [ ] The phase's runnable outcome runs in Ghostty on macOS.
- [ ] Every task in the phase is implemented, not stubbed.
- [ ] `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` are clean.
- [ ] The test gate commands were run verbatim and their output pasted into the phase's section
      of `docs/PERF.md` (where it involves numbers) or reported back to Mike.
- [ ] The stop rule is met.
- [ ] No new runtime dependency appeared without an ADR.
- [ ] `TerminalGuard` still restores the terminal after a forced panic
      (`BREAKOUT_PANIC_TEST=1`) — re-check this every phase, it regresses easily.
- [ ] Anything you had to decide that was not in the PRD or an ADR is written up and raised,
      not silently absorbed.

## Related

- [[project-plan/Breakout/index|Breakout]]
- [[project-plan/Breakout/PLAN|PLAN.md]]
- [[project-plan/Breakout/PRD|PRD.md]]
- [[project-plan/Breakout/ADR/index|ADRs]]
