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