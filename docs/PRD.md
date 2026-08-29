---
title: Breakout — PRD
type: prd
tags: [project-plan, prd, breakout, rust, terminal]
status: draft
date: 2026-08-29
---

# PRD — Breakout (terminal, GPU-accelerated)

**Product:** a modern Breakout that runs in a terminal, launched with the command `breakout`.
**Platforms:** macOS and Linux. **Language:** Rust. **Primary terminal:** Ghostty.

---

## 1. Problem

Terminal games look like 1985 because they draw with text cells. Ghostty renders its screen on
the GPU (Metal on macOS, OpenGL on Linux) and implements the Kitty graphics protocol, which means
a terminal program can hand it a real pixel image every frame and get it composited by the GPU.
Almost nothing takes advantage of this. Breakout is a good vehicle: the rules are known, so all
the work goes into *feel* — collision, particles, screenshake, powerups, progression.

This is a build-for-fun project. There is no user research to do and no market to serve. Success
is that Mike opens a terminal, types `breakout`, and it is genuinely fun for twenty minutes.

## 2. Primary user

Mike. One player, keyboard, on a Mac laptop in Ghostty, and on a Linux machine (Omarchy) in
Ghostty. Anyone who runs `cargo install` is a welcome secondary user but shapes no decision.

## 3. What "GPU-accelerated" actually means here

Ghostty does **not** expose its GPU to the program. There is no shader API, no surface handle.
What it exposes is a very fast path for putting an image on screen. So the game:

- rasterises its own frame into an RGB pixel buffer in Rust (CPU),
- hands that buffer to Ghostty through the Kitty graphics protocol using POSIX shared memory,
- and Ghostty uploads and composites it on the GPU.

The GPU is doing the scaling, compositing and presenting. The game is doing the drawing. This is
the honest framing and every requirement below assumes it. See
[[project-plan/Breakout/ADR/ADR-0001-kitty-graphics-renderer|ADR-0001]] and
[[project-plan/Breakout/ADR/ADR-0002-frame-transport|ADR-0002]].

---

## 4. Functional requirements

### 4.1 Command and lifecycle

- **FR-1** The binary is named `breakout`. Running `breakout` with no arguments opens the title
  screen and needs no configuration or setup step.
- **FR-2** `breakout --level <path>` loads one `.lvl` file and plays it standalone, outside the
  run structure. Used for authoring and testing levels.
- **FR-3** The CLI supports exactly: `--level <path>`, `--no-audio`, `--fps <n>` (default 60,
  range 30–144), `--scale <n>` (force integer scale factor; default auto), `--seed <n>` (fixed
  RNG seed for a reproducible run), `--reset-profile`, `--caps` (print the terminal capability
  report and exit 0), `--version`, `--help`. No other flags without a new ADR.
- **FR-4** When the terminal does not support the Kitty graphics protocol, the game exits with
  status 2 and a message that names what is missing and lists terminals known to work
  (Ghostty, Kitty, WezTerm). It does not fall back to a text renderer.
- **FR-5** The terminal is fully restored — alt screen exited, raw mode off, cursor shown,
  keyboard protocol flags popped, all transmitted images deleted — on normal quit, on `panic!`,
  on SIGINT and on SIGTERM.
- **FR-6** `q` quits from the title screen. `Esc` opens the pause menu during play; `Esc` again
  resumes. Pause menu offers Resume / Restart run / Mute / Quit.
- **FR-7** Startup performs a capability probe (graphics protocol, keyboard protocol, cell pixel
  size, terminal pixel size) and caches the result for the session.

### 4.2 Rendering

- **FR-8** The game renders into a fixed logical playfield of **320 × 240 logical pixels**,
  scaled to the terminal by an integer factor so pixels stay square and crisp
  ([[project-plan/Breakout/ADR/ADR-0003-logical-resolution|ADR-0003]]).
- **FR-9** Frames are presented at up to `--fps` (default 60). If the game falls behind, frames
  are **dropped**, never queued; simulation stays on its own fixed timestep
  ([[project-plan/Breakout/ADR/ADR-0006-fixed-timestep|ADR-0006]]).
- **FR-10** A terminal resize is absorbed within 2 frames: recompute scale, reallocate the
  framebuffer, delete stale images, redraw. No tearing, no leftover image rows.
- **FR-11** If the terminal is too small for scale 1 (needs ≥ 320 × 240 pixels of usable area),
  the game shows a "make the window bigger" screen instead of playing, and recovers live when
  the window grows.
- **FR-12** The HUD shows: score, lives, level index (`3/8`), combo multiplier, active powerup
  icons with remaining time, active perks, and a mute indicator.

### 4.3 Core game

- **FR-13** The paddle moves left and right with acceleration and friction, clamped to the walls.
  It never teleports; input sets a target velocity, not a position.
- **FR-14** The ball launches from the paddle on `Space`. Launch angle is derived from where the
  ball sits on the paddle.
- **FR-15** Ball–paddle contact point determines the reflection angle ("english"): the further
  from paddle centre, the wider the angle, clamped to a minimum vertical component so the ball
  can never enter a horizontal loop.
- **FR-16** Collision is **swept**, not sampled: at the maximum ball speed and the minimum frame
  rate, the ball can never pass through a brick, a wall or the paddle.
- **FR-17** Bricks carry 1–5 hit points. Each hit decrements HP and changes the brick's colour to
  the tier below. At 0 the brick is destroyed and scores.
- **FR-18** **Steel** bricks are indestructible, reflect normally, and do not count toward
  clearing the level.
- **FR-19** **Explosive** bricks destroy every destructible brick in their 3 × 3 neighbourhood
  when destroyed. Chains are allowed and resolve in one pass (no infinite recursion).
- **FR-20** Losing the last ball in play costs one life. Reaching 0 lives ends the run.
- **FR-21** A level is cleared when no destructible bricks remain.
- **FR-22** Ball speed ramps with bricks destroyed and level index, up to a hard cap defined in
  `game/tuning.rs`. All tuning constants live in that one file.
- **FR-23** A **combo** counter increments for every brick destroyed without the ball touching
  the paddle, and resets on paddle contact. Score for a brick is `base × combo_multiplier`.

### 4.4 Feel ("juice")

- **FR-24** Brick destruction emits a particle burst coloured from the brick's palette entry,
  with gravity and fade.
- **FR-25** Screenshake is applied as a sub-pixel camera offset, its magnitude proportional to
  the event (brick break < explosion < life lost), decaying over ~200 ms.
- **FR-26** Hit-stop: on brick destruction, simulation freezes for a short window (~40 ms at
  combo 1, scaling down as combo rises) while rendering continues.
- **FR-27** The ball leaves a fading trail built from its last N positions, drawn additively.
- **FR-28** Bricks flash white for one frame on a non-fatal hit; the paddle flashes on contact.
- **FR-29** A cheap bloom pass (threshold + 3 × 3 blur, added back) makes the ball, powerups and
  combo text glow. It must be possible to disable it and stay inside the frame budget.
- **FR-30** The combo counter scale-pops when it increments.

### 4.5 Powerups

- **FR-31** Destroying a brick has a tunable chance to drop a powerup capsule, which falls and is
  collected by paddle contact.
- **FR-32** The powerup set for v1 is exactly: **Multiball** (splits every ball in two),
  **Laser** (paddle fires on `Space`), **Sticky** (ball catches on the paddle, re-launch with
  `Space`), **Wide** (paddle grows), **Slow** (ball speed × 0.7), **Pierce** (ball passes through
  bricks it destroys), **1-Up** (extra life).
- **FR-33** Timed powerups (Laser, Sticky, Wide, Slow, Pierce) run on independent timers shown in
  the HUD. Re-collecting refreshes the timer rather than stacking duration twice.
- **FR-34** Multiball and 1-Up are instant, not timed. Losing balls down to one does not end the
  life while balls remain.

### 4.6 Levels

- **FR-35** Levels are plain-text files with a TOML header and an ASCII brick grid, documented in
  [[project-plan/Breakout/ADR/ADR-0007-level-format|ADR-0007]]. A human can write one in a text
  editor with no tooling.
- **FR-36** The parser reports errors with the file path, line number, column and the offending
  character. It never panics on a malformed level.
- **FR-37** The game ships with a campaign of at least 16 hand-authored levels under
  `assets/levels/campaign/`, tagged by difficulty tier, compiled into the binary so a
  `cargo install` needs no data files on disk.
- **FR-38** `breakout --level ./my.lvl` loads a level from disk, so authoring does not require a
  rebuild.

### 4.7 Run structure and progression

- **FR-39** A **run** is 8 levels drawn from the campaign pools in ascending difficulty, chosen
  by the run seed. `--seed` makes a run reproducible.
- **FR-40** After each cleared level, the player is offered **3 perks out of the unlocked pool**
  and picks one. Perks last for the rest of the run.
- **FR-41** The v1 perk pool has at least 12 entries, each a single readable rule (examples:
  *Overclock* — +12% ball speed, +25% score; *Second Serve* — first life lost per level is
  refunded; *Shrapnel* — destroyed bricks fire two damaging fragments; *Magnet* — paddle attracts
  falling powerups; *Glass Cannon* — one life, double score).
- **FR-42** A run ends on death or after level 8, and shows a summary: score, levels cleared,
  bricks destroyed, best combo, perks taken, shards earned.
- **FR-43** Shards earned per run are persistent currency that unlocks additional perks and
  starting modifiers for future runs.
- **FR-44** Progress is stored in a single JSON profile at the platform config directory
  (`~/Library/Application Support/breakout/profile.json` on macOS,
  `$XDG_CONFIG_HOME/breakout/profile.json` on Linux). A corrupt or unreadable profile is renamed
  aside and replaced with a fresh one — it never blocks play.
- **FR-45** `--reset-profile` wipes progression after an explicit `y/N` confirmation.

### 4.8 Audio

- **FR-46** Sound effects for: brick hit, brick destroy, paddle bounce, wall bounce, powerup drop,
  powerup collect, laser, life lost, level clear, perk pick.
- **FR-47** A looping music bed, quieter than the effects, that ducks during hit-stop.
- **FR-48** Audio is on by default. `m` toggles mute in-game and the choice persists in the
  profile. `--no-audio` disables the audio subsystem entirely for that session.
- **FR-49** If no audio device is available, or audio initialisation fails, the game logs one line
  to the capability report and plays silently. Audio failure never prevents play.

---

## 5. Non-functional requirements

- **NFR-1** **Frame budget.** At the default 60 fps and a typical 1280 × 960 presented image,
  p99 frame time (simulate + rasterise + transmit) ≤ 16.6 ms on an Apple-silicon Mac in Ghostty.
  The build carries a `--fps` overlay so this is measurable, not asserted.
- **NFR-2** **Input latency.** Key press to visible paddle movement ≤ 32 ms (2 frames).
- **NFR-3** **Platforms.** macOS (aarch64 and x86_64) and Linux (x86_64 and aarch64), Rust stable,
  MSRV pinned in `Cargo.toml` and enforced in CI. No Windows in v1.
- **NFR-4** **Terminal safety.** Under no exit path — including a panic inside the render loop —
  does the user get left in raw mode, in the alt screen, or with images stuck on their scrollback.
- **NFR-5** **No network.** The game makes no network call, ever. There is no telemetry, no
  update check, no leaderboard service.
- **NFR-6** **Footprint.** < 64 MB RSS in play; release binary < 10 MB.
- **NFR-7** **Startup.** `breakout` to interactive title screen in < 300 ms.
- **NFR-8** **Unsafe code** is confined to `src/term/shm.rs` and `src/term/caps.rs`. Every
  `unsafe` block carries a comment stating the invariant it relies on. No other module may use it.
- **NFR-9** `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` are clean.
- **NFR-10** Deterministic simulation: same seed plus same input sequence produces the same run.
  This is what makes physics regressions testable without a human. **Scope: same-machine only**
  — replay must match on the machine it was recorded on; cross-architecture byte-identical
  replay is not required, so `f32` and std math are acceptable.
- **NFR-11** The dependency list in
  [[project-plan/Breakout/ADR/ADR-0005-stack|ADR-0005]] is the entire runtime dependency budget.
  Adding anything to it requires a new ADR.
- **NFR-12** **Asset provenance and credits.** Every third-party creative asset bundled into the
  project — sound effects, music, or any image/graphic file — must be under a free-to-use
  licence (CC0 / public domain, or a permissive licence compatible with the project's MIT
  licence). The source URL and exact licence of each asset are recorded in `assets/` (a
  `CREDITS.md` or per-asset licence notes) and surfaced to users in a **Credits** section of the
  README (see PLAN Phase 9 task 2). The game's graphics are self-generated in Rust (ADR-0001 /
  ADR-0005: tiny-skia rasterisation + a hand-rolled 5 × 7 bitmap font), so v1 ships no image
  asset files; this requirement binds on audio (WAV) and on any future asset.

---

## 6. Out of scope for v1

Stated explicitly so the agent does not build these:

- Mouse control, gamepad control, touch.
- A Unicode/text-cell fallback renderer. The game requires a graphics-capable terminal (FR-4).
- Windows support.
- Online leaderboards, accounts, cloud save, telemetry — anything touching a network (NFR-5).
- Multiplayer or two-paddle modes.
- An in-game level editor. Levels are text files edited in a text editor (FR-35).
- Homebrew tap, `.deb`/`.rpm` packaging, an install script. `cargo install` is the only
  distribution path ([[project-plan/Breakout/ADR/ADR-0009-distribution|ADR-0009]]).
- Sixel or iTerm2 image protocols.
- Configurable key bindings, themes as user config files, or any config file at all beyond the
  save profile.
- Localisation.

## 7. Success criteria

1. `cargo install --path .` then `breakout` starts a run on both machines with no other steps.
2. A full 8-level run can be played start to finish without a crash, a stuck terminal, or a
   rendering artifact.
3. The FPS overlay shows ≥ 55 fps sustained during a heavy frame (multiball + explosion +
   particles) at the default scale in Ghostty.
4. Killing the game with `Ctrl-C` mid-frame leaves a usable terminal.
5. Mike plays three runs in a row without being asked to.

## 8. Open questions

- **OQ-1** Repo location and name. No repo exists yet; the plan does not assume one.
  Resolve before Phase 0. **Status:** deferred — Mike will add the repo later; the agent does
  not create it.
- **OQ-2** crates.io publication. **Resolved 2026-08-29:** `cargo install --git` only; no
  crates.io publish. See ADR-0009.
- **OQ-3** Licence. **Resolved 2026-08-29:** MIT only.
- **OQ-4** Colour-blind-safe palette variant. **Resolved 2026-08-29:** deferred — ship the
  MOCKUP neon palette only in v1.
- **OQ-5** Music. **Resolved 2026-08-29:** source CC0/public-domain WAVs for effects and the
  music bed; document provenance/licence in `assets/audio/` and credit them in the README
  (NFR-12). Affects Phase 7.
- **OQ-6** `--level` on a directory. **Resolved 2026-08-29:** no — `--level` takes a single
  `.lvl` file only (FR-2 as written).
- **OQ-7** Headless replay harness. **Resolved 2026-08-29:** yes — build it; Phase 2 and
  Phase 8 determinism tests use it.

### Additional clarifications (recorded 2026-08-29, not original OQs)

- **MSRV:** pin `1.98`; `rust-toolchain.toml` channel = `1.98` (exact, not rolling stable).
- **Determinism scope (NFR-10):** same-machine only. `f32` + std math is acceptable; no
  cross-architecture byte-identical guarantee.
- **Tuning values:** the agent proposes a complete `tuning.rs` for Mike's review before
  Phase 2; Mike approves before implementation.
- **Audio format:** WAV (kira decodes WAV natively; no new dependency, stays within
  ADR-0005's budget). Not OGG.
- **Multiball hard cap:** 8 (in `tuning.rs`; matches the NFR-1 stress test).
- **Perk offer when the unlocked pool minus taken perks is < 3:** offer all available (fewer
  than 3 cards); never re-offer taken perks.
- **Content proposals:** the agent proposes (for Mike's review before the relevant phase) the
  full 12+ perk table, the shard unlock table, the 16+ campaign levels, and the pixel-level
  perk-offer / run-summary screen layouts.
- **Linux verification scope:** phases 0–8 are verified on macOS/Ghostty only. The
  both-machines requirement (PRD §7) applies at Phase 9, where Mike plays a full run on macOS
  **and** on Linux.
- **Phase 9 balance pass:** authorized exception to AGENTS §0 rule 2. Tuning values may be
  changed freely in Phase 9; review is via the Phase 9 stop rule (Mike plays three runs) and the
  before/after table in `docs/BALANCE.md`, not per-value pre-approval.
- **No CI in v1:** supersede ADR-0009's CI provision via ADR-0011. No CI workflow is written or
  committed; `cargo install` is the only path. The local quality gates (build, test, clippy,
  fmt) are run by the agent at the phase gates, not by a CI runner. MSRV remains pinned in
  `Cargo.toml` (`rust-version = 1.98`) and `rust-toolchain.toml`.
- **Run-seed display:** where the seed is printed (HUD/summary) is unspecified; folded into the
  Phase 8 UI layout proposal.

## Related

- [[project-plan/Breakout/index|Breakout]]
- [[project-plan/Breakout/PLAN|PLAN.md]]
- [[project-plan/Breakout/AGENTS|AGENTS.md]]
- [[project-plan/Breakout/ADR/index|ADRs]]
