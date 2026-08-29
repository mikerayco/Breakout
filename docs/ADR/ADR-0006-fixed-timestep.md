---
title: ADR-0006 — Fixed-timestep simulation decoupled from rendering
type: adr
tags: [project-plan, adr, breakout]
status: accepted
date: 2026-08-29
---

# ADR-0006 — Fixed-timestep simulation decoupled from rendering

**Status:** Accepted · **Date:** 2026-08-29

## Context

Frame times in a terminal are not stable. Ghostty's graphics path has variable cost, a resize
can stall a frame, and the shared-memory write competes with whatever else the machine is doing.
If physics advances by "however long the last frame took", three things break: collision
resolution gets less accurate exactly when the machine is busiest (the worst possible time),
the game becomes non-reproducible, and a long stall lets the ball skip through a brick.

PRD NFR-10 requires a deterministic simulation — same seed plus same inputs gives the same run —
because that is the only practical regression net for physics and progression changes.

## Decision

**The simulation runs on a fixed 240 Hz timestep (`dt = 1/240 s`), accumulated against real time,
and rendering happens at up to 60 fps independently.**

```
accumulator += frame_elapsed
steps = 0
while accumulator >= DT && steps < MAX_CATCHUP {   // MAX_CATCHUP = 5
    simulate(DT)
    accumulator -= DT
    steps += 1
}
if accumulator >= DT { accumulator = 0 }           // give up, do not spiral
render(interpolation = accumulator / DT)
```

- **240 Hz, not 60.** Four sub-steps per rendered frame keeps the swept-collision solver working
  on short spans, which is what makes PRD FR-16 (no tunnelling) achievable at high ball speed.
- **`MAX_CATCHUP = 5`** and dropping the remainder prevents the death spiral where catching up
  costs more than the stall did.
- **`simulate(dt)` takes no wall clock and no global RNG.** It reads `InputState`, `RunModifiers`
  and the seeded `Rng` passed to it. This is the rule that makes NFR-10 hold.
- **Rendering interpolates** ball and paddle positions between the previous and current
  simulation states, so motion is smooth even though physics is quantised.
- **Hit-stop (PRD FR-26) freezes the simulation, not the renderer.** During hit-stop the
  accumulator is consumed without stepping, so particles and screenshake keep animating while
  the world holds still. That is the whole effect.
- A recorded input log plus a seed can be replayed through `simulate` with no terminal at all,
  which is how the determinism test in Phase 8 works.

## Consequences

**Good**

- Collision accuracy is independent of machine load and frame rate.
- Deterministic replay makes physics and balance regressions catchable by `cargo test`.
- Slow machines get a lower frame rate, not a different game.
- Interpolated rendering means the game looks smoother than 60 discrete physics steps would.

**Bad, and accepted**

- 240 Hz costs ~4× the physics work of a 60 Hz step. Breakout's physics is a handful of bodies,
  so this is negligible next to rasterisation.
- Rendering interpolated positions means the drawn frame is up to one step behind "true" state.
  At 240 Hz that is 4 ms — below perception.
- Input is sampled at frame rate but applied at step rate, so a key press lands on the next
  frame's steps. Bounded by NFR-2's 32 ms budget.
- Every piece of state that affects the simulation must be threaded explicitly rather than read
  from a global. More plumbing, and the reason the determinism guarantee is real.

## Alternatives rejected

- **Variable timestep.** Simplest, and it makes collision accuracy worst under load and
  determinism impossible.
- **Fixed 60 Hz simulation locked to the frame rate.** Fewer sub-steps, so tunnelling protection
  depends entirely on the swept solver being perfect, and a dropped frame becomes a physics
  glitch.
- **Substepping only when the ball is fast.** Adaptive, and non-deterministic unless the
  adaptation is itself a pure function of state. Not worth the subtlety.

## Related

- [[project-plan/Breakout/PRD|PRD]] — FR-9, FR-16, FR-26, NFR-10
- [[project-plan/Breakout/PLAN|PLAN.md]] — Phase 2, Phase 8
