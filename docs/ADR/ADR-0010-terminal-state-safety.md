---
title: ADR-0010 — One RAII guard owns all terminal state, and it always runs
type: adr
tags: [project-plan, adr, breakout]
status: accepted
date: 2026-08-29
---

# ADR-0010 — One RAII guard owns all terminal state, and it always runs

**Status:** Accepted · **Date:** 2026-08-29

## Context

This program mutates a surprising amount of state that belongs to the user's terminal, not to
the program: raw mode, the alternate screen, cursor visibility, the Kitty keyboard protocol
flags (ADR-0004), and images transmitted into the terminal's own storage (ADR-0002). If the
program exits without undoing all of that, the user is left with a shell that does not echo,
does not respond to `Ctrl-C`, has no cursor, and possibly has a game frame stuck in their
scrollback. Recovering means typing `reset` blind.

This is the single most likely way for the project to be *annoying* rather than merely buggy,
and it regresses easily: every new phase adds an exit path.

## Decision

**All terminal state is acquired and released by one type, `TerminalGuard`, in `src/term/guard.rs`.
Nothing else in the codebase is allowed to enable raw mode, enter the alternate screen, push
keyboard flags, or leave images behind.**

- **Construction** enables raw mode, enters the alternate screen, hides the cursor and pushes the
  keyboard enhancement flags — in that order.
- **`Drop`** reverses it exactly — pops the keyboard flags, emits `ESC _G a=d,d=A,q=2 ESC \` to
  delete all placements, shows the cursor, leaves the alternate screen, disables raw mode —
  and is written to be **idempotent and infallible**: every step ignores its own errors and
  continues to the next. A failure to delete images must not prevent leaving raw mode.
- **A panic hook** installed at startup runs the same teardown *before* the default hook prints
  the panic message, so a panic backtrace lands on a usable terminal instead of scrolling
  diagonally across the alternate screen.
- **`panic = "unwind"`** in the release profile (ADR-0005), specifically so `Drop` and the hook
  can run. `panic = "abort"` is forbidden.
- **SIGINT and SIGTERM** set an atomic shutdown flag that the main loop checks each iteration,
  so the guard drops normally rather than the process dying mid-frame.
- **`BREAKOUT_PANIC_TEST=1`** forces a panic inside the render loop. Re-running it is on the
  definition-of-done checklist for **every** phase, not just Phase 0, because this is what
  regresses.

The one case not defended against is `SIGKILL`, which cannot be caught. The README says so, and
says `reset` fixes it.

## Consequences

**Good**

- There is exactly one place to look when the terminal is left broken, and exactly one place to
  add a new piece of state when one appears.
- A panic during development — which will happen often, with an agent writing the code — costs
  a moment instead of a lost terminal session.
- The `Drop`-is-infallible rule means a partially-failed teardown still gets the important parts
  done.

**Bad, and accepted**

- `Drop` cannot report errors, so a genuine failure to restore is silent. Accepted: the
  alternative is propagating errors out of a destructor, which is worse.
- The guard is effectively a process-global singleton. Constructing two would be a bug, and
  Rust's type system will not stop it; a debug assertion on a static flag is the mitigation.
- The panic hook has to be careful not to allocate or panic itself. It is deliberately dumb:
  raw escape writes to a duplicated stdout handle, no formatting, no allocation.
- Signal handling adds a small amount of platform code and one atomic check per loop iteration.

## Alternatives rejected

- **Restoring the terminal at each `return` site in `main`.** Works until it does not; every new
  early return is a chance to forget one.
- **`catch_unwind` around the game loop instead of a panic hook.** Catches panics in the loop and
  not in threads, and encourages treating a panic as recoverable. The hook plus `Drop` is both
  simpler and more complete.
- **`libc::atexit`.** Runs too late and cannot be relied on for a panic path.
- **Ignoring signals and relying on `Drop` alone.** `Drop` does not run when a signal terminates
  the process by default, which is exactly the `Ctrl-C` case the user will hit most.

## Related

- [[project-plan/Breakout/ADR/ADR-0004-keyboard-input|ADR-0004]]
- [[project-plan/Breakout/ADR/ADR-0002-frame-transport|ADR-0002]]
- [[project-plan/Breakout/ADR/ADR-0005-stack|ADR-0005]] — `panic = "unwind"`
- [[project-plan/Breakout/PRD|PRD]] — FR-5, NFR-4
