---
title: ADR-0011 — No CI in v1
type: adr
tags: [project-plan, adr, breakout]
status: accepted
date: 2026-08-29
---

# ADR-0011 — No CI in v1

**Status:** Accepted · **Date:** 2026-08-29

**Supersedes:** the CI provision of
[[project-plan/Breakout/ADR/ADR-0009-distribution|ADR-0009]] ("CI builds and tests on macOS and
Linux runners plus an MSRV job, so 'works on both machines' is verified rather than hoped for").
ADR-0009's distribution decisions (package `breakout-tui`, binary `breakout`, `cargo install`
only) stand unchanged.

## Context

ADR-0009 specified CI on macOS and Linux runners plus an MSRV job to automate the "works on
both machines" guarantee. On review, the user asked why CI is needed at all when installation is
`cargo install` (which builds from source on the target machine). The honest answer: CI is not
required for `cargo install` to function. Its only purpose was to catch regressions — especially
cross-platform drift — automatically, so the both-machines claim did not depend on manual
dual-machine checks every time.

PRD §7 success criteria already require Mike to play on both machines at the Phase 9 gate, which
is the verification that actually matters for an audience of one. CI would only catch drift
between those manual checks.

## Decision

**No CI in v1.** No CI workflow is written or committed; Phase 9 carries no CI task. Both-machines
verification is manual: Mike plays a full run on macOS and on Linux at the Phase 9 gate (PRD §7
success criteria 1–5), and `docs/PERF.md` records the final numbers on both machines.

The quality gates (`cargo build --release`, `cargo test`, `cargo clippy --all-targets -- -D
warnings`, `cargo fmt --check`, plus the MSRV pin in `Cargo.toml`) remain the definition of done;
they are run locally by the agent and at the phase gates, not by a CI runner. The MSRV is still
pinned in `Cargo.toml` (`rust-version = 1.98`) and in `rust-toolchain.toml`.

## Consequences

**Good**

- One fewer thing to build, maintain, and keep green. The project ships through `cargo install`
  and nothing about that path needs CI.
- No CI secrets, runner minutes, or platform-specific workflow YAML to debug — exactly the kind
  of moving part this project minimises.
- The both-machines guarantee stays where it is real: a human playing the game on both machines.

**Bad, and accepted**

- Cross-platform regressions are caught only when Mike runs on Linux, which by the
  Linux-verification decision (PRD §8) happens at Phase 9. A macOS-only bug introduced in
  Phase 4 that breaks Linux might not surface until Phase 9. Accepted: the simulation is
  deterministic and platform-agnostic by construction (ADR-0006, NFR-10 same-machine scope), and
  the platform-specific surface is confined to `term/` (caps, shm, input) which is exercised on
  macOS throughout.
- No automated MSRV enforcement. The MSRV pin in `Cargo.toml` documents intent but does not
  block a dependency that raises the real minimum. Mitigated by the small, fixed dependency
  budget (ADR-0005); revisited if a release workflow is ever added.

## Alternatives rejected

- **GitHub Actions matrix (the ADR-0009 provision).** Automates the both-machines check and
  MSRV. Rejected by the user as unnecessary for a personal `cargo install`-only project; the
  manual Phase 9 gate is sufficient.
- **Minimal CI: a single Linux smoke workflow.** Cheaper than the matrix, still some YAML to
  maintain, and it would only verify Linux — the macOS path would still need a manual check.
  Rejected for the same reason: not worth the moving parts in v1.
- **Reintroduce CI later via a new ADR.** Remains open; adding CI is a new ADR and a workflow
  file, not a code change. This ADR only removes it from v1 scope.

## Related

- [[project-plan/Breakout/ADR/ADR-0009-distribution|ADR-0009]] — superseded CI provision;
  distribution decisions unchanged
- [[project-plan/Breakout/PRD|PRD]] — §7 success criteria, §8 clarifications (no CI)
- [[project-plan/Breakout/PLAN|PLAN.md]] — Phase 9 (CI task removed)
