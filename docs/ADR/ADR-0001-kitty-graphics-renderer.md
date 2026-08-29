---
title: ADR-0001 — Render pixels through the Kitty graphics protocol, not text cells
type: adr
tags: [project-plan, adr, breakout]
status: accepted
date: 2026-08-29
---

# ADR-0001 — Render pixels through the Kitty graphics protocol, not text cells

**Status:** Accepted · **Date:** 2026-08-29

## Context

The brief is a Breakout that "utilises the GPU acceleration in Ghostty". Ghostty renders its
screen with Metal on macOS and OpenGL on Linux, but it does **not** expose that GPU to the
program running inside it. There is no shader API, no surface handle, no framebuffer to share.
What Ghostty does expose is the Kitty graphics protocol: the program hands it image data and
Ghostty uploads it as a texture and composites it on the GPU.

That leaves two honest options for a terminal game:

1. Draw with text cells — Unicode half-blocks or sextants plus truecolour. The terminal's GPU
   accelerates the text rendering, so it is fast, but the resolution is the cell grid (at best
   2 × 3 sub-cells) and the palette is per-cell.
2. Rasterise a real pixel image in the program and present it through the graphics protocol
   every frame.

The user chose option 2 explicitly, for the visual ceiling: sub-pixel ball motion, particles,
glow, anti-aliased shapes.

## Decision

**The game rasterises its own RGB framebuffer on the CPU and presents it to the terminal as an
image every frame via the Kitty graphics protocol.** There is no text-cell renderer, not even
as a fallback (PRD §6). A terminal without graphics protocol support gets a clear error and
exit code 2 (PRD FR-4).

The division of labour, stated plainly so nobody claims more than is true:

| Work | Where it happens |
| --- | --- |
| Geometry, collision, particles, bloom, rasterisation | Rust, on the CPU |
| Frame upload, scaling, compositing, presentation, vsync | Ghostty, on the GPU |

## Consequences

**Good**

- Real pixel art at whatever resolution the window allows. Motion is smooth because ball
  positions are sub-pixel, not snapped to a character cell.
- Anti-aliasing, additive blending and bloom are available, which is most of what makes a modern
  arcade game look modern.
- The renderer is completely under our control and testable in isolation — it produces a byte
  buffer, and nothing about it depends on the terminal's font, theme or cell geometry.

**Bad, and accepted**

- The game will not run in Alacritty, tmux, a plain xterm, or over most SSH sessions without a
  graphics-capable client. This is a real loss of portability, accepted because the target is
  Mike's two machines, both running Ghostty.
- Rasterisation cost is ours. A full 1280 × 960 frame is ~3.7 MB of RGB per frame; at 60 fps
  that is a meaningful amount of memory traffic. ADR-0002 and ADR-0003 exist to keep it inside
  budget.
- Ghostty's graphics implementation is acknowledged by its own maintainers to be slower than
  Kitty's and has not been profiled. If it cannot hold 60 fps, that is a project-level risk, not
  a bug we can fix. Phase 1 exists to find this out early rather than at the end.
- No text will be selectable or copyable from the game screen. Irrelevant here.

## Alternatives rejected

- **Unicode half-block / sextant cells (ratatui).** Portable and simple, and Ghostty would render
  it very fast. Rejected because the look is exactly the 1985 aesthetic the project is trying to
  escape, and because per-cell colour makes particles and glow impossible.
- **A hybrid renderer with auto-detection.** Two renderers to build, two to keep visually in
  sync, two to debug, for an audience of one whose terminal supports the good one. Rejected as
  scope.
- **Sixel or the iTerm2 inline image protocol.** Sixel is slower and colour-limited; iTerm2's
  protocol is not supported by Ghostty. No reason to carry either.
- **A real GUI window (wgpu, macroquad, bevy).** Would trivially be faster and prettier, and
  would miss the entire point of the request.

## Related

- [[project-plan/Breakout/ADR/ADR-0002-frame-transport|ADR-0002]] — how the frame gets there
- [[project-plan/Breakout/ADR/ADR-0003-logical-resolution|ADR-0003]] — how big the frame is
- [[project-plan/Breakout/PRD|PRD]] — FR-4, FR-8, NFR-1
