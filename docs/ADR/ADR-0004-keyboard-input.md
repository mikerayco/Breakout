---
title: ADR-0004 — Kitty keyboard protocol for key-release events, with a degraded fallback
type: adr
tags: [project-plan, adr, breakout]
status: accepted
date: 2026-08-29
---

# ADR-0004 — Kitty keyboard protocol for key-release events, with a degraded fallback

**Status:** Accepted · **Date:** 2026-08-29

## Context

An ordinary terminal only reports key *presses*, and a held key arrives as the system autorepeat
stream: one event, a ~500 ms gap, then a burst at ~30 Hz. That is fine for a text editor and
disqualifying for a game — the paddle would stutter on every direction change and the player
would feel a half-second of dead air every time they start moving.

The Kitty keyboard protocol fixes this. With `REPORT_EVENT_TYPES` the terminal distinguishes
press, repeat and release, so the program can hold a set of currently-pressed keys and drive
movement from state rather than from events. `REPORT_ALL_KEYS_AS_ESCAPE_CODES` is additionally
required to get repeat/release for plain-text keys such as `h` and `l`. Ghostty implements the
protocol. crossterm 0.29 exposes it as `PushKeyboardEnhancementFlags` /
`PopKeyboardEnhancementFlags` with `KeyEventKind::{Press, Repeat, Release}` on `KeyEvent`.

## Decision

**Push `DISAMBIGUATE_ESCAPE_CODES | REPORT_EVENT_TYPES | REPORT_ALL_KEYS_AS_ESCAPE_CODES` at
startup and drive the paddle from a held-key set.**

- The flags are pushed by `TerminalGuard` on construction and popped on `Drop`, in the same place
  as raw mode and the alternate screen (ADR-0010). They are never pushed anywhere else.
- Input is state, not events: `InputState` holds a `HashSet<KeyCode>` updated by `Press`/`Repeat`
  (insert) and `Release` (remove). The simulation reads the set each tick and sets a *target*
  velocity; acceleration and friction do the rest (PRD FR-13).
- On focus loss or an unexpected event gap, the held set is cleared, so the paddle cannot get
  stuck moving.
- **Fallback.** If the `CSI ? u` query shows no support, or the pushed flags read back empty,
  the game switches to *repeat-decay*: a press sets the paddle velocity and it decays to zero
  over ~140 ms, so autorepeat sustains movement while a single tap gives a short nudge. Forced
  for testing with `BREAKOUT_INPUT=legacy`. It is worse; PRD's Phase 3 gate requires it to remain
  playable, not to feel good.
- Keyboard-only. Mouse and gamepad are out of scope (PRD §6) — the user chose keyboard.

## Consequences

**Good**

- Sub-frame-accurate paddle control with no autorepeat dead zone; PRD NFR-2 (≤ 32 ms key to
  movement) is achievable.
- Simultaneous keys work correctly, which matters for `Space`-while-moving and for the laser.
- The same mechanism gives clean modifier handling for the pause menu.

**Bad, and accepted**

- The keyboard protocol flags are terminal state. If the program dies without popping them, the
  user's shell can be left with an odd keyboard mode. This is exactly why they are owned by
  `TerminalGuard` and why the forced-panic test is re-run at every phase gate.
- Two input paths to maintain and test. Bounded: the fallback is ~30 lines and has its own gate.
- Terminal multiplexers may swallow or mangle the protocol. tmux is not a supported environment
  (ADR-0001 already excludes non-graphics terminals, and tmux is one).

## Alternatives rejected

- **Autorepeat only, no protocol.** The dead zone at the start of a hold is unacceptable for a
  paddle game. Kept only as the fallback.
- **Reading the keyboard device directly** (`/dev/input` on Linux, IOKit HID on macOS). Gives
  perfect key state and requires elevated permissions, platform-specific code, and works even
  when the terminal is not focused — which is a bug, not a feature. Rejected.
- **Mouse control.** Arcade-authentic and genuinely the most fun control scheme, but the user
  chose keyboard. Recorded here so the decision is not silently revisited.

## Related

- [[project-plan/Breakout/ADR/ADR-0010-terminal-state-safety|ADR-0010]]
- [[project-plan/Breakout/ADR/ADR-0005-stack|ADR-0005]] — crossterm 0.29
- [[project-plan/Breakout/PLAN|PLAN.md]] — Phase 3
- [[project-plan/Breakout/PRD|PRD]] — FR-13, NFR-2
