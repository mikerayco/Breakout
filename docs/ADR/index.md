---
title: Breakout — ADRs
type: index
tags: [index, project-plan, adr, breakout]
---

# 🧱 ADRs — Breakout

Every architectural decision this build depends on. One decision per file, numbered in the order
it was made. A decision that constrains later code gets an ADR **even if it is not implemented
yet**. Superseding an ADR means writing a new one, not editing the old.

| ADR | Decision | Status |
| --- | --- | --- |
| [[project-plan/Breakout/ADR/ADR-0001-kitty-graphics-renderer\|ADR-0001]] | Pixel framebuffer via the Kitty graphics protocol, no text-cell renderer | Accepted |
| [[project-plan/Breakout/ADR/ADR-0002-frame-transport\|ADR-0002]] | POSIX shared-memory frame transport, double-buffered image ids, base64 fallback | Accepted, built in Phase 1 |
| [[project-plan/Breakout/ADR/ADR-0003-logical-resolution\|ADR-0003]] | Fixed 320 × 240 logical playfield, integer scaling | Accepted, built in Phase 1 |
| [[project-plan/Breakout/ADR/ADR-0004-keyboard-input\|ADR-0004]] | Kitty keyboard protocol for key-release, with a degraded fallback | Accepted, built in Phase 3 |
| [[project-plan/Breakout/ADR/ADR-0005-stack\|ADR-0005]] | Stack, dependency budget and module layout | Accepted |
| [[project-plan/Breakout/ADR/ADR-0006-fixed-timestep\|ADR-0006]] | 240 Hz fixed-timestep simulation, decoupled rendering, determinism | Accepted, built in Phase 2 |
| [[project-plan/Breakout/ADR/ADR-0007-level-format\|ADR-0007]] | `.lvl` format: TOML header plus an ASCII grid | Accepted, built in Phase 5 |
| [[project-plan/Breakout/ADR/ADR-0008-run-progression\|ADR-0008]] | Roguelite runs, data-driven perks, atomic JSON save | Accepted, built in Phase 8 |
| [[project-plan/Breakout/ADR/ADR-0009-distribution\|ADR-0009]] | Package `breakout-tui`, binary `breakout`, `cargo install` only | Accepted, built in Phase 9 |
| [[project-plan/Breakout/ADR/ADR-0010-terminal-state-safety\|ADR-0010]] | One RAII guard owns all terminal state; panic hook and signal handling | Accepted, built in Phase 0 |
| [[project-plan/Breakout/ADR/ADR-0011-no-ci\|ADR-0011]] | No CI in v1; both-machines verification is manual at Phase 9 | Accepted, supersedes ADR-0009's CI provision |

**Format for a new ADR:** Context → Decision → Consequences (good *and* bad, both stated) →
Alternatives rejected. Filename `ADR-00NN-<slug>.md`.

## Related

- [[project-plan/Breakout/index|Breakout]]
- [[project-plan/Breakout/PLAN|PLAN.md]]
- [[project-plan/index|Project Plans]]
