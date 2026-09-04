# PERF — frame-time measurements

Gate outputs with numbers live here, updated at every phase gate (AGENTS §1).

## Phase 0 — Skeleton, terminal guard, capability probe

Gate date: 2026-08-29. No frame-time measurements exist yet (no pixels —
Phase 1 is the first rendering phase). Recorded: the capability report from
Ghostty, and the automated gate results.

### `breakout --caps` in Ghostty

```
breakout capability report
terminal program: ghostty
TERM:                xterm-ghostty
graphics protocol:  kitty (ok)
keyboard protocol:  kitty (query ok, current state flags=0)
cell pixel size:    17x40
window pixel size:  1564x1240
```

Notes:
- `graphics: kitty (ok)` — the graphics probe succeeds once replies are read
  in raw mode (the canonical-mode read bug is fixed, see Phase 0 wrap-up).
- `keyboard flags=0` is the *current pushed state* before the guard pushes
  any flags, not a capability mask. Support is the fact the query answered
  at all. Phase 3 verifies pushed flags stick.
- Cell 17x40, window 1564x1240 — consistent with a MacBook Retina + Ghostty.

### Automated gate results

| Command | Result |
| --- | --- |
| `cargo build --release` | clean |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `cargo test` | 0 tests (Phase 0 defines none) |
| `breakout --fps 200` | rejected: "200 is not in 30..=144" (exit 2) |
| `TERM=dumb breakout` | exit 2 + FR-4 message |
| `breakout --caps` (pipe) | report, no escape-leak when stdout is not a tty |

### Interactive gates — Mike

- [ ] `breakout` placeholder screen; `q` restores the shell
- [ ] `BREAKOUT_PANIC_TEST=1` panic path restores the terminal
- [ ] `breakout &` + `kill -INT` SIGINT path restores the terminal
- [ ] typed after each failure path, echo normal (Phase 0 stop rule)
## Phase 1 — First pixels: framebuffer and graphics transport

Gate date: 2026-08-29 (implementation). Automated gates green:
`cargo build --release`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`, `cargo test` (5 tests: shm names, shm roundtrip, test
card non-blank, glyph pixels, scale-2 preview PNG export).

### Headless verification (done)

- Test card renders off-screen at scale 2 to `/tmp/breakout-card-s2.png`
  (640×480, valid PNG, non-trivial content) — proves framebuffer + tiny-skia
  + RGBA→RGB conversion + glyph drawing end to end.
- Transport escapes: `t=s` shm + `t=d` base64 chunking written per ADR-0002
  (transmit new id, then delete previous; C=1; q=2). Verified by unit tests
  where possible; real Ghostty transmission is the open gate below.

### Ghostty gates — Mike (numbers to record here)

The Phase 1 stop rule: **p99 written into `docs/PERF.md` before Phase 2.**

```sh
cd /Users/mikerayco/Projects/breakout
cargo run --release                            # watch 30s; overlay shows
                                                #   FPS / P50 / P99 / shm / S / size
BREAKOUT_TRANSPORT=direct cargo run --release  # fallback path renders too
cargo run --release -- --scale 2               # forced scale honoured
```
Then resize slowly/quickly both dims; shrink below minimum (FR-11 screen
appears and recovers).

- [ ] **p99 ≤ 16.6 ms and ≥ 55 fps on shm** (NFR-1) — paste the overlay's
      P99 and FPS numbers here
- [ ] circle motion smooth, no tearing/flicker
- [ ] resize leaves no stale band, no wrongly-scaled frame
- [ ] `direct` transport renders correctly (may be slower — record its P99)
- [ ] after `q`, no leftover image in scrollback
- [ ] Phase 0's failure-path checks still pass (panic + SIGINT + typed echo)

## Phases 2-8 — implementation gates (this machine, no Ghostty)

All automated gates green on 2026-09-03 (Linux, no graphics/audio device):

| Command | Result |
| --- | --- |
| `cargo build --release` | clean |
| `cargo test` | 70 passed, 0 failed (physics, determinism, soak, parser+proptest, powerups, perks, run sim, save recovery, audio decode) |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `breakout --validate assets/levels/campaign` | 16 valid, 0 invalid |
| `breakout --level /tmp/bad.lvl` | exit 1, `/tmp/bad.lvl: line 2, column 5: unknown character 'Z'`, no panic |
| `echo n \| breakout --reset-profile` | exit 0, profile untouched |
| `echo y \| breakout --reset-profile` | exit 0, profile wiped |
| `breakout --caps` | report + `audio: available`, exit 0 |

Headless 10-minute AI soak (144k steps): no escape, no overspeed, no
horizontal loop. Scripted 8-level run replays byte-identically (NFR-10
harness). `PULSE_SERVER=/nonexistent` game run and muted/unmuted runs
need a graphics terminal: pending Mike.

## Phase 9 — final numbers (Mike's machines)

- [ ] macOS/Ghostty: p99 ≤ 16.6 ms with bloom on, 200+ particles, shake
- [ ] `--no-bloom` saving recorded here
- [ ] Linux/Ghostty: full run plays, same gates
- [ ] `cargo install --path .` then `breakout` from `/tmp` on both
- [ ] Three runs in a row, unprompted (success criterion 5)

## Wedge fix (post-Phase 9)

Report: ball wedging at the back of the brick formation — frozen with a
fast bouncing loop. Cause: float error landing exactly on a face
re-triggered the overlap branch at time-of-impact 0, flip-flopping
velocity without moving while every phantom contact fired a sound.
Fix: `CONTACT_EPS` separation after every contact, full depenetration
before/after each ball step (kill line stays open), per-effect 40 ms
audio throttle. Guards: `embedded_ball_is_pushed_out_and_keeps_moving`,
`top_pocket_does_not_machine_gun_or_freeze`, `throttle_collapses_repeats`.

## Perf pass + Stardew palette (2026-09-04, this Linux box, release, 1280x960)

Measured with a temporary black_box bench (removed after; suite stays
deterministic), sparse night scene:

| Stage | Before | After |
| --- | --- | --- |
| Bloom (threshold+blur+composite) | full-frame float blur | bbox blur + integer threshold + reused mask: ~7-8 ms worst-case (scattered brights), ~2-4 ms clustered |
| RGBA->RGB (`rgb_bytes`) | 0.8 ms warm (a 24 ms reading was cold zero-page artifact — production frames are always fully drawn first) | chunked over disjoint borrows (kept vectorized), reused buffer: 0.8 ms |
| Base64 (direct transport) | ~5 MB alloc/frame | reused scratch buffer: same ~2.2 ms CPU, zero alloc churn |

Fixes: `render/bloom.rs` (`Scratch`, bbox, integer luminance),
`render/framebuffer.rs` (vectorized `rgb_bytes`), `term/kgp.rs`
(`B64_BUF` scratch, pure `chunk_plan`, wire-identical).
Direct-mode pty write (5 MB/frame) still dominates in Ghostty and is not
measurable here — the F3 overlay on Mike's machine is the real gate.

Palette: ADR-0012 Stardew pastel set replaces neon (MOCKUP §2 updated).
