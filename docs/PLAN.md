---
title: Breakout — PLAN
type: plan
tags: [project-plan, plan, breakout, rust, terminal]
status: draft
date: 2026-08-29
---

# PLAN — Breakout

Ten phases, 0 through 9. **Every phase ends in something that runs on Mike's machine and a test
gate he can execute.** Do not begin a phase until the previous phase's stop rule is met. If a
decision you need is not in the [[project-plan/Breakout/ADR/index|ADRs]] or the
[[project-plan/Breakout/PRD|PRD]], **stop and ask** — do not choose.

Throughout, `$REPO` is the repository root (see PRD **OQ-1**; ask before creating it).
All phase gates are run from `$REPO`.

---

## Phase 0 — Skeleton, terminal guard, capability probe

**Goal:** the program can take over the terminal and give it back, and can tell you what the
terminal can do. No game yet.

**Runnable outcome:** `breakout --caps` prints a capability report and exits. `breakout` enters
the alternate screen, shows a placeholder frame of text, and `q` returns you to a clean prompt.

**Tasks**

1. `cargo new --bin breakout-tui` at `$REPO`; in `Cargo.toml` set `[[bin]] name = "breakout"`,
   `path = "src/main.rs"`, and `rust-version` (MSRV).
2. Add exactly the dependencies listed in
   [[project-plan/Breakout/ADR/ADR-0005-stack|ADR-0005]]. No others.
3. Create the module skeleton from ADR-0005 §Layout. Every module compiles with `todo!()` bodies.
4. `src/term/guard.rs` — an RAII `TerminalGuard` that on construction enables raw mode, enters
   the alternate screen, hides the cursor and pushes keyboard enhancement flags; on `Drop`
   reverses all of it and emits `\x1b_Ga=d,d=A,q=2\x1b\\` to clear images. Install a panic hook
   that runs the same teardown before printing the panic, and a SIGINT/SIGTERM handler that
   triggers a clean shutdown (NFR-4, FR-5).
5. `src/term/caps.rs` — probe and report:
   - graphics protocol: send `\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\` and wait up to 200 ms
     for an `_Gi=31;OK` reply;
   - keyboard protocol: query with `\x1b[?u` and read the `CSI ? <flags> u` reply;
   - cell pixel size and window pixel size: `ioctl(TIOCGWINSZ)` reading `ws_xpixel`/`ws_ypixel`,
     falling back to `CSI 14 t` and `CSI 16 t`;
   - terminal identity from `$TERM_PROGRAM` / `$TERM`.
6. `src/cli.rs` — the exact flag set in PRD **FR-3**. `--caps` prints the report and exits 0.
7. Implement PRD **FR-4**: if graphics support is absent, print the message and exit 2.

**Test gate**

```sh
cargo build --release
cargo clippy --all-targets -- -D warnings
cargo fmt --check
./target/release/breakout --caps
```

Then, in Ghostty:

```sh
./target/release/breakout          # placeholder screen appears
# press q
tput cols; echo "terminal alive"   # prompt is normal, echo works, cursor visible
```

Then prove the failure paths:

```sh
BREAKOUT_PANIC_TEST=1 ./target/release/breakout ; stty -a | head -1   # panic path restores
./target/release/breakout & sleep 1 ; kill -INT %1 ; wait             # SIGINT path restores
TERM=dumb ./target/release/breakout ; echo "exit=$?"                  # expect exit=2 + message
```

**Pass when:** `--caps` reports `graphics: kitty (ok)`, `keyboard: kitty (flags=…)` and a
non-zero cell pixel size in Ghostty; all three failure paths above leave a working shell; clippy
and fmt are clean.

**Stop rule:** do not start Phase 1 until you have typed in the terminal *after* each of the
three failure paths and seen normal echo.

**Not in this phase:** any pixel drawing, any game logic, any audio.

---

## Phase 1 — First pixels: the framebuffer and the graphics transport

**Goal:** put a real, moving, GPU-composited image in the terminal at 60 fps. This is the
riskiest phase; it is deliberately second.

**Runnable outcome:** `breakout` shows an animated test card — a scrolling gradient, a bouncing
anti-aliased circle, a colour-bar strip and a live FPS/frame-time overlay — filling the terminal.

**Tasks**

1. `src/render/framebuffer.rs` — an RGB (`f=24`) buffer sized `320·S × 240·S`, with `clear`,
   `pixel_mut`, and a `as_bytes()` view. Compute `S` per
   [[project-plan/Breakout/ADR/ADR-0003-logical-resolution|ADR-0003]].
2. Wire **tiny-skia** as the rasteriser drawing into that buffer (ADR-0005). Draw the test card
   with it: gradient, AA circle, rects.
3. `src/render/text.rs` — a hand-rolled 5 × 7 bitmap font (uppercase, digits, `:./×%-`), enough
   for the overlay and later the HUD. No font crate.
4. `src/term/shm.rs` — POSIX shared memory frame transport per
   [[project-plan/Breakout/ADR/ADR-0002-frame-transport|ADR-0002]]: `shm_open` a uniquely named
   object, `ftruncate`, `mmap`, write the frame, `munmap`/`close`, then emit
   `\x1b_Ga=T,f=24,s=<w>,v=<h>,t=s,i=<id>,p=1,q=2,C=1;<base64(name)>\x1b\\`
   with the cursor parked at the image's top-left cell. Rotate across 3 names; the terminal
   unlinks each after reading.
5. `src/term/kgp.rs` — the fallback direct transport (`t=d`, `f=24`, 4096-byte base64 chunks with
   `m=1`/`m=0`), selected automatically when shm fails and forced by `BREAKOUT_TRANSPORT=direct`.
6. Double-buffer image ids (alternate `i=1`/`i=2`): transmit the new frame, then delete the
   previous with `\x1b_Ga=d,d=I,i=<prev>,q=2\x1b\\`. Never delete before transmitting (ADR-0002).
7. Frame pacing: a 60 Hz presentation clock that drops, never queues (PRD **FR-9**).
8. Handle `SIGWINCH`/crossterm resize: recompute `S`, reallocate, delete all images, redraw
   (**FR-10**). Implement the too-small screen (**FR-11**).
9. Overlay: fps, p50/p99 frame time, current transport (`shm`/`direct`), scale factor, image size.

**Test gate**

```sh
cargo run --release
```

In Ghostty, watch for 30 seconds, then:

```sh
BREAKOUT_TRANSPORT=direct cargo run --release      # fallback path renders too
cargo run --release -- --scale 2                   # forced scale honoured
```

Resize the Ghostty window slowly, then quickly, in both dimensions. Shrink it below the minimum.

**Pass when:** the overlay reports **p99 frame time ≤ 16.6 ms** and ≥ 55 fps in the shm transport
(NFR-1); the circle's motion is smooth with no tearing or flicker; resizing never leaves a
stale image band or a wrongly-scaled frame; the too-small screen appears and recovers; the
`direct` transport renders correctly (it may be slower — record the number); after `q` the
scrollback contains no leftover image.

**Stop rule:** the p99 number must be written into `docs/PERF.md` in the repo before Phase 2.
If shm cannot hit the budget, stop and ask — that invalidates ADR-0002, not the plan.

**Not in this phase:** paddles, balls, bricks, input handling beyond `q`, audio.

---

## Phase 2 — The core loop: paddle, ball, bricks, lives

**Goal:** a boring but correct Breakout. Correctness first, feel later.

**Runnable outcome:** a single hard-coded level you can actually clear or lose.

**Tasks**

1. `src/game/tuning.rs` — every constant, one file, documented units (PRD **FR-22**).
2. `src/game/state.rs` — `GameState { Title, Playing, Paused, LevelClear, RunOver }` and the
   transitions between them.
3. `src/game/physics.rs` — **swept** circle-vs-AABB collision resolution with iterative
   time-of-impact within the tick, walls, paddle and bricks (**FR-16**). Reflection with paddle
   english and the minimum-vertical-angle clamp (**FR-15**).
4. Fixed-timestep simulation at 240 Hz decoupled from rendering, with an accumulator and a
   maximum catch-up of 5 steps per frame
   ([[project-plan/Breakout/ADR/ADR-0006-fixed-timestep|ADR-0006]]).
5. Brick grid with HP 1–5, steel and explosive types (**FR-17**, **FR-18**, **FR-19**), scoring,
   lives, combo (**FR-23**), speed ramp (**FR-22**).
6. Draw everything as flat rectangles and a flat circle. No particles, no glow, no shake.
7. `src/game/rng.rs` — a seeded deterministic RNG threaded through the whole simulation
   (NFR-10). The simulation must never read the system clock or a global RNG.
8. Unit tests: tunnelling at max speed, corner-case brick collisions, the angle clamp, explosive
   chain termination, combo reset, deterministic replay of a recorded input vector via the
   headless replay harness (OQ-7 = yes; the harness is built here and reused in Phase 8).

**Test gate**

```sh
cargo test
cargo run --release
```

Play it. Then:

```sh
cargo run --release -- --seed 42     # twice; identical brick drops and outcomes
```

**Pass when:** `cargo test` passes including the tunnelling and determinism tests; a level can be
cleared; losing three balls ends the run; the ball never escapes the playfield, sticks to a wall,
or enters a horizontal loop over a 5-minute session; two `--seed 42` sessions with the same
inputs produce the same score.

**Stop rule:** ten minutes of continuous play with no ball escape and no panic.

**Not in this phase:** juice, powerups, level files, audio, perks, save data.

---

## Phase 3 — Input that feels right

**Goal:** the paddle stops feeling like a text editor cursor.

**Runnable outcome:** held-key paddle movement with acceleration, and a degraded but playable
mode when the keyboard protocol is unavailable.

**Tasks**

1. `src/term/input.rs` — push `DISAMBIGUATE_ESCAPE_CODES | REPORT_EVENT_TYPES |
   REPORT_ALL_KEYS_AS_ESCAPE_CODES` via crossterm and maintain a held-key set from
   `KeyEventKind::{Press, Repeat, Release}`
   ([[project-plan/Breakout/ADR/ADR-0004-keyboard-input|ADR-0004]]).
2. Fallback mode when the protocol is absent or the flags come back empty: a key press sets
   paddle velocity which decays over ~140 ms, so autorepeat still yields continuous movement.
   Forced for testing by `BREAKOUT_INPUT=legacy`.
3. Paddle acceleration, friction, and a small amount of momentum carried into the ball on
   contact (**FR-13**).
4. Bindings: `←`/`→` and `h`/`l` move, `Space` launches, `Esc` pauses, `q` quits, `m` mutes,
   `F3` toggles the debug overlay.
5. Pause menu (**FR-6**).

**Test gate**

```sh
cargo run --release
BREAKOUT_INPUT=legacy cargo run --release
```

**Pass when:** holding `←` moves the paddle smoothly with no autorepeat stutter at the start;
releasing stops it within one frame; measured key-to-movement latency ≤ 32 ms on the debug
overlay (NFR-2); the legacy mode is *worse but playable* — a full level can be cleared in it.

**Stop rule:** clear one level in each input mode.

**Not in this phase:** any change to physics constants tuned in Phase 2.

---

## Phase 4 — Juice

**Goal:** the same game, but it feels good. This is the phase that decides whether the project
was worth doing.

**Runnable outcome:** breaking a brick is satisfying on its own.

**Tasks**

1. `src/render/particles.rs` — a fixed-capacity particle pool (no per-frame allocation) with
   velocity, gravity, lifetime, colour ramp (**FR-24**).
2. `src/render/camera.rs` — screenshake as a decaying sub-pixel offset, magnitude by event
   class (**FR-25**).
3. Hit-stop in the simulation loop, scaled down by combo (**FR-26**).
4. Ball trail from a ring buffer of positions, drawn additively (**FR-27**).
5. Brick and paddle hit flashes (**FR-28**).
6. `src/render/bloom.rs` — threshold + 3 × 3 blur + additive composite, toggleable with `F4`
   and via `--no-bloom` (**FR-29**). Must stay inside NFR-1 at default scale.
7. Combo counter scale-pop and colour ramp (**FR-30**).
8. Lock the palette from `MOCKUP.md` into `src/render/palette.rs` as named constants.

**Test gate**

```sh
cargo run --release
```

Play a level with the debug overlay on. Then:

```sh
cargo run --release -- --no-bloom
```

**Pass when:** p99 frame time is still ≤ 16.6 ms with bloom on, 200+ live particles and
screenshake active (NFR-1); with bloom off there is a visible frame-time saving recorded in
`docs/PERF.md`; the game visually matches `mockup.html` closely enough that the palette,
proportions and HUD placement are recognisably the same design.

**Stop rule:** Mike looks at it side by side with the mockup and says it is close.

**Not in this phase:** new gameplay mechanics. Juice only.

---

## Phase 5 — Level format and the campaign

**Goal:** levels stop being hard-coded and become content Mike can author in a text editor.

**Runnable outcome:** `breakout --level ./scratch.lvl` plays a level written by hand five minutes
earlier.

**Tasks**

1. Implement the format in
   [[project-plan/Breakout/ADR/ADR-0007-level-format|ADR-0007]]: TOML header, `---`, ASCII grid.
2. Parser with precise errors — path, line, column, character (**FR-36**). Property test: no
   input string, however malformed, panics.
3. `assets/levels/campaign/` with **16+ hand-authored levels**, named `NN-slug.lvl`, each with a
   `tier` (1–4) in its header (**FR-37**).
4. Compile the campaign into the binary with `include_str!` via a `build.rs`-generated manifest,
   so an installed binary needs no data directory.
5. `--level <path>` reads from disk instead (**FR-38**).
6. A `--validate <dir>` developer path that parses every level and reports problems without
   launching the game.

**Test gate**

```sh
cargo test
cargo run --release -- --validate assets/levels/campaign
printf 'name = "scratch"\ntier = 1\n---\n....11111....\n....22222....\n' > /tmp/scratch.lvl
cargo run --release -- --level /tmp/scratch.lvl
printf 'name = "bad"\n---\n....ZZZZ....\n' > /tmp/bad.lvl
cargo run --release -- --level /tmp/bad.lvl ; echo "exit=$?"
```

**Pass when:** `--validate` reports 16+ valid levels; the hand-written scratch level plays; the
malformed level exits non-zero with a message naming line 2, column 5 and the character `Z`,
and does not panic.

**Stop rule:** Mike authors one level himself without reading the parser source.

**Not in this phase:** run structure, perks, powerups.

---

## Phase 6 — Powerups and multiball

**Goal:** the board gets chaotic in a good way.

**Runnable outcome:** all seven powerups obtainable and visibly working.

**Tasks**

1. `src/game/powerup.rs` — the seven types in **FR-32**, drop chance from `tuning.rs`, capsule
   fall physics, paddle collection (**FR-31**).
2. Multiple balls: convert the single-ball field into a small `Vec<Ball>` with a hard cap; a life
   is lost only when the last ball is gone (**FR-34**).
3. Independent timers with HUD indicators and refresh-not-stack semantics (**FR-33**).
4. Laser projectiles, sticky catch-and-release, wide paddle, slow-mo, pierce.
5. A debug spawner (`F5` cycles and spawns a chosen powerup) so the gate is testable in seconds.

**Test gate**

```sh
cargo test
cargo run --release
# F5 through all seven; then play a normal level
```

**Pass when:** each of the seven behaves as specified; multiball with 8 balls plus particles
stays inside NFR-1; re-collecting a timed powerup refreshes rather than doubles; no powerup
leaves permanent state after its timer ends; losing the last ball with a Wide paddle active
resets the paddle correctly on the next life.

**Stop rule:** one level cleared using only powerups triggered naturally, no debug spawner.

**Not in this phase:** perks (they come with the run structure in Phase 8).

---

## Phase 7 — Audio

**Goal:** sound that adds to the feel and never gets in the way.

**Runnable outcome:** the game sounds like something.

**Tasks**

1. `src/audio/mod.rs` — kira setup behind a `Sound` trait so the whole subsystem can be replaced
   by a no-op (**FR-49**).
2. The ten effects in **FR-46**, loaded from `assets/audio/` as **WAV** (kira decodes WAV
   natively; no decoder dependency — see ADR-0005), compiled in with `include_bytes!`.
3. Music bed on its own track, ducked during hit-stop (**FR-47**).
4. Pitch-shift brick-hit effects by combo so a long combo rises in pitch — cheap, and the single
   highest-value audio detail in the whole game.
5. `m` toggle, persisted; `--no-audio` (**FR-48**). Mute state must be readable at a glance in
   the HUD.
6. Startup must not block on audio device enumeration; initialise it off the main loop.

**Test gate**

```sh
cargo run --release
cargo run --release -- --no-audio
```

On Linux, also:

```sh
# with no audio device available
PULSE_SERVER=/nonexistent cargo run --release ; echo "exit=$?"
```

**Pass when:** effects fire on the right events with no audible latency; the music bed sits under
the effects; `m` mutes instantly and the state survives a restart; with no audio device the game
plays silently, reports it in `--caps`, and exits 0; startup is still under 300 ms (NFR-7).

**Stop rule:** play one level muted and one unmuted; the muted run must not stutter differently.

**Not in this phase:** any music composition beyond the sourced CC0 bed (OQ-5 resolved).

---

## Phase 8 — The run: perks, progression, save

**Goal:** a reason to press "again".

**Runnable outcome:** an 8-level run with perk choices, a run summary, and progression that
persists between launches.

**Tasks**

1. `src/game/run.rs` — the 8-level sequence built from tier pools by the run seed (**FR-39**).
2. `src/game/perk.rs` — 12+ perks per **FR-41**, each a data-driven modifier applied to a
   `RunModifiers` struct the simulation reads. No perk may be implemented as a special case
   inside `physics.rs`.
3. Perk offer screen: 3 of the unlocked pool, keyboard selection (**FR-40**).
4. Run summary screen (**FR-42**) and shard award (**FR-43**).
5. `src/save/mod.rs` — the JSON profile, atomic write (temp file + rename), corruption recovery
   by renaming aside (**FR-44**), `--reset-profile` with confirmation (**FR-45**).
6. Unlock table: which perks and starting modifiers each shard threshold opens.
7. Determinism test: a scripted run with a fixed seed and a recorded input log produces a
   byte-identical summary (NFR-10, same-machine scope) via the headless replay harness.

**Test gate**

```sh
cargo test
cargo run --release -- --seed 7    # play a full run to the summary
cat "${XDG_CONFIG_HOME:-$HOME/Library/Application Support}/breakout/profile.json"
echo 'garbage' > "$HOME/Library/Application Support/breakout/profile.json"
cargo run --release                # recovers, does not crash
cargo run --release -- --reset-profile
```

**Pass when:** a full run reaches the summary; shards accumulate across two runs; an unlock
appears after crossing its threshold; the corrupt profile is renamed aside and play continues;
`--reset-profile` asks before wiping; the determinism test passes.

**Stop rule:** three consecutive runs with different seeds, no crash, no lost progress.

**Not in this phase:** balance tuning beyond obvious brokenness. Balance is Phase 9.

---

## Phase 9 — Balance, docs and ship

**Goal:** `cargo install` and play, on both machines.

**Runnable outcome:** the command `breakout` works from a clean shell on macOS and Linux.

**Tasks**

1. Balance pass on `tuning.rs`: difficulty curve across the 8 run levels, drop rates, perk
   strength. Record the before/after values in `docs/BALANCE.md`.
2. `README.md`: what it is, the terminals it needs, install, controls, level authoring, an
   asciinema or GIF capture, and a **Credits** section listing every third-party asset (sound
   effects, music, any graphics), its source URL, and its licence (PRD NFR-12).
3. `docs/PERF.md` final numbers on both machines.
4. Package metadata for `cargo install`: description, keywords, categories, licence (**OQ-3**),
   `include` list, and `[[bin]] name = "breakout"` on package `breakout-tui`
   ([[project-plan/Breakout/ADR/ADR-0009-distribution|ADR-0009]]).
5. Resolve **OQ-2**: document `cargo install --git` (resolved 2026-08-29 — no crates.io publish).

**No CI in v1** ([[project-plan/Breakout/ADR/ADR-0011-no-ci|ADR-0011]] supersedes the CI provision
of ADR-0009). Both-machines verification is manual: Mike plays a full run on macOS **and** on
Linux at this gate (PRD §7). The local quality gates (`cargo build --release`, `cargo test`,
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`) still must be clean; they are
run by the agent, not by a CI runner. The Phase 9 balance pass (task 1) is an authorized
exception to AGENTS §0 rule 2: tuning values may be changed freely, reviewed via the stop rule
(Mike plays three runs) and the before/after table in `docs/BALANCE.md`.

**Test gate**

On both machines, from a shell with the repo *not* on `$PATH`:

```sh
cargo install --path .
cd /tmp && breakout --version && breakout
```

**Pass when:** the binary is on `$PATH` as `breakout` on macOS and Linux; a full run plays on
both; `--help` documents every flag in **FR-3**; CI is green; PRD §7 success criteria 1–5 all
hold.

**Stop rule:** success criterion 5 — Mike plays three runs in a row without being asked to.

---

## Related

- [[project-plan/Breakout/index|Breakout]]
- [[project-plan/Breakout/PRD|PRD.md]]
- [[project-plan/Breakout/AGENTS|AGENTS.md]]
- [[project-plan/Breakout/MOCKUP|MOCKUP.md]]
- [[project-plan/Breakout/ADR/index|ADRs]]
