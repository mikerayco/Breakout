---
title: ADR-0002 — Frame transport by POSIX shared memory, double-buffered image ids
type: adr
tags: [project-plan, adr, breakout]
status: accepted
date: 2026-08-29
---

# ADR-0002 — Frame transport by POSIX shared memory, double-buffered image ids

**Status:** Accepted · **Date:** 2026-08-29

## Context

ADR-0001 commits to sending a full pixel frame to the terminal 60 times a second. The Kitty
graphics protocol offers several ways to move that data:

- `t=d` — the payload is base64-encoded image data inline in the escape sequence, chunked into
  pieces with `m=1` on all but the last. Simple, universal, but base64 inflates the payload by
  4/3 and every byte crosses the pty.
- `t=f` / `t=t` — the payload names a file on disk that the terminal reads.
- `t=s` — the payload names a POSIX shared memory object created with `shm_open`. The terminal
  reads it, then unlinks and closes it.

Ghostty's maintainers explicitly recommend shared memory or temporary files for speed, precisely
to avoid the base64 path. Ghostty does **not** implement the protocol's animation-frame actions
(`a=a` / `a=f`), so "transmit an animation and let the terminal play it" is not available; each
game frame must be its own transmit-and-display.

## Decision

**Primary transport: `t=s`, POSIX shared memory, one object per frame, `f=24` raw RGB.**

Per frame:

1. `shm_open` an object named `/bkout-<pid>-<n mod 3>` (`O_CREAT | O_RDWR`, `0600`),
   `ftruncate` it to `w * h * 3`, `mmap` it, write the framebuffer, `munmap`, `close`.
2. Park the cursor at the image's top-left cell and emit
   `ESC _G a=T,f=24,s=<w>,v=<h>,t=s,i=<id>,p=1,q=2,C=1 ; <base64 of the shm name> ESC \`
3. Delete the *previous* frame's image with `ESC _G a=d,d=I,i=<prev_id>,q=2 ESC \`.

Key choices inside that:

- **`f=24` (raw RGB), not `f=100` (PNG).** PNG compression would cost more CPU per frame than it
  saves in transfer, and the transfer is a shared memory write, not a pty write.
- **`C=1`** so displaying the image does not move the cursor.
- **`q=2`** so the terminal sends neither OK nor error replies — the game must not have to drain
  responses from stdin at 60 Hz while also reading key events.
- **Two alternating image ids (`i=1`, `i=2`), transmit-then-delete.** Deleting the old image
  before the new one is displayed produces a visible flash. Deleting after does not.
- **Three rotating shm names.** The terminal unlinks each object after reading it, so a name is
  reusable, but rotating avoids racing a slow reader on the same name.

**Fallback transport: `t=d`, base64, 4096-byte chunks.** Used automatically when `shm_open`,
`ftruncate` or `mmap` fails, and forced for testing by `BREAKOUT_TRANSPORT=direct`. Correctness
is required of it; the frame budget is not.

**No protocol animation frames.** Ghostty does not implement them (open issue upstream). The
game must not be architected around them appearing later.

## Consequences

**Good**

- The pty carries roughly 80 bytes per frame instead of ~5 MB. The pixel data never passes
  through the terminal's escape-sequence parser.
- Raw RGB means zero encode cost — the rasteriser writes straight into the mapped shm region, so
  there is no extra copy between "draw" and "send".
- The fallback keeps the game working on a graphics-capable terminal where shared memory is
  unavailable (a sandbox, an unusual container), at a lower frame rate.

**Bad, and accepted**

- This is the only `unsafe` code in the project (`shm_open`, `mmap`, `munmap`). It is confined to
  `src/term/shm.rs` and PRD NFR-8 forbids it anywhere else.
- Shared memory is local-only. Playing over SSH silently falls back to base64 and will be slow.
  Acceptable — nobody is playing this over SSH.
- If the game is `kill -9`'d between `shm_open` and the terminal's read, a shm object can be
  leaked (a file in `/dev/shm` on Linux). Bounded to three small objects; the next launch reuses
  the same names. Not worth defending against further.
- Ghostty's graphics path is not as fast as Kitty's and has not been profiled upstream. If the
  Phase 1 gate cannot hit the frame budget even on shared memory, this ADR is what has to be
  revisited — probably by reducing the presented image size (ADR-0003) before anything else.

## Alternatives rejected

- **`t=d` base64 as the primary transport.** ~5 MB/frame of base64 through the pty at 60 fps.
  Kept as the fallback, never the default.
- **`t=f` temporary files.** Also recommended by Ghostty and simpler (no `unsafe` beyond normal
  file I/O), but it puts 60 file creations per second through the filesystem and, on macOS,
  through the security layer. Shared memory is the better primary; if `t=s` proves troublesome in
  Phase 1, `t=f` is the first thing to try before falling back to base64.
- **Protocol animation frames (`a=a`).** Unimplemented in Ghostty.
- **Transmitting only dirty regions.** The protocol can place multiple images, but managing
  placement lifetimes for dozens of small images per frame is far more complexity — and more
  escape traffic — than one full-frame blit. Revisit only if the frame budget is missed.

## Related

- [[project-plan/Breakout/ADR/ADR-0001-kitty-graphics-renderer|ADR-0001]]
- [[project-plan/Breakout/ADR/ADR-0003-logical-resolution|ADR-0003]]
- [[project-plan/Breakout/PLAN|PLAN.md]] — Phase 1
- [[project-plan/Breakout/PRD|PRD]] — NFR-1, NFR-8
