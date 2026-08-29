---
title: ADR-0003 — Fixed 320×240 logical playfield with integer scaling
type: adr
tags: [project-plan, adr, breakout]
status: accepted
date: 2026-08-29
---

# ADR-0003 — Fixed 320 × 240 logical playfield with integer scaling

**Status:** Accepted · **Date:** 2026-08-29

## Context

The game presents an image sized in real pixels, but a terminal window can be any size and the
user resizes it freely. Two things follow. First, gameplay must not change with window size — a
wider window must not mean a wider playfield or an easier game. Second, the cost of a frame
(rasterisation, memory traffic, GPU upload) scales with the presented area, and PRD NFR-1 puts a
hard budget on it.

Terminal pixel dimensions are discoverable: `ioctl(TIOCGWINSZ)` gives `ws_xpixel`/`ws_ypixel`,
with `CSI 14 t` and `CSI 16 t` as fallbacks.

## Decision

**The game simulates and draws in a fixed logical playfield of 320 × 240 logical pixels, and
presents it scaled by an integer factor S.**

- `S = clamp(min(floor(win_px_w / 320), floor(win_px_h / 240)), 1, 4)`.
- The framebuffer is `320·S × 240·S`. Every drawing primitive multiplies logical coordinates by
  `S`, so the rasteriser draws at full presented resolution — this is scaling the *drawing*, not
  upscaling a small bitmap, so edges and text stay crisp.
- The image is centred in the terminal; the surrounding area is painted with the background
  colour so there is no visible letterbox seam.
- If `S` would be 0 (window smaller than 320 × 240 px), the game shows the "make the window
  bigger" screen (PRD FR-11) and recovers live.
- `--scale <n>` overrides the computed `S`, for testing and for deliberately running smaller.
- 4:3 is chosen over a widescreen ratio because Breakout is a vertical-pressure game; a wide
  playfield makes the paddle's job trivial.

## Consequences

**Good**

- Gameplay is identical at every window size. A physics bug reproduces on any machine, and the
  determinism requirement (NFR-10) survives resizing mid-run.
- The frame cost is bounded and predictable: at most 1280 × 960 × 3 bytes ≈ 3.7 MB. The `S ≤ 4`
  cap is the single most effective lever if Phase 1 misses the frame budget.
- All tuning constants are in one unit system with no display units leaking into `physics.rs`.
- Integer scaling means no resampling blur and no shimmering on moving edges.

**Bad, and accepted**

- On a very large window the game does not fill the screen; it caps at scale 4 and centres.
  Acceptable, and arguably better than a 4K frame at 60 fps.
- Non-integer window sizes leave a border. Painting it in the background colour makes it read as
  a deliberate frame rather than a gap.
- 320 × 240 is a small canvas for HUD text. The 5 × 7 bitmap font is sized for it; anything more
  elaborate will not fit, which is a constraint on the HUD design, not a defect.

## Alternatives rejected

- **Playfield sized to the window.** Ties difficulty to window size and breaks reproducibility.
- **Fractional scaling to fill the window exactly.** Resampling artifacts on a pixel-art game,
  and either blur or shimmer on every moving edge.
- **Rendering at 320 × 240 and letting the terminal scale the image up** (via the protocol's
  `c=`/`r=` cell sizing). Cheapest possible frame, but the terminal's filtering is out of our
  control and the result is either blurry or blocky in a way we cannot tune. Drawing at
  presented resolution costs more CPU and looks correct.

## Related

- [[project-plan/Breakout/ADR/ADR-0001-kitty-graphics-renderer|ADR-0001]]
- [[project-plan/Breakout/ADR/ADR-0002-frame-transport|ADR-0002]]
- [[project-plan/Breakout/PRD|PRD]] — FR-8, FR-10, FR-11, NFR-1
