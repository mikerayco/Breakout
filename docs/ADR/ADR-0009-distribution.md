---
title: ADR-0009 — Distribution: package breakout-tui, binary breakout, cargo install only
type: adr
tags: [project-plan, adr, breakout]
status: accepted
date: 2026-08-29
---

# ADR-0009 — Distribution: package `breakout-tui`, binary `breakout`, `cargo install` only

**Status:** Accepted · **Date:** 2026-08-29

## Context

The requirement is that the game runs by typing `breakout` in a terminal, on macOS and Linux, and
that installation happens through `cargo install` — no Homebrew tap, no release binaries, no
install script (PRD §6).

One complication: **the crate name `breakout` is already taken on crates.io** by Andrew Kane's
`breakout` (breakout *detection*, a statistics library), currently at 0.4.0. `breakout-tui` is
unclaimed as of 2026-08-29.

## Decision

- **Package name: `breakout-tui`. Binary name: `breakout`.** Cargo allows these to differ:

  ```toml
  [package]
  name = "breakout-tui"

  [[bin]]
  name = "breakout"
  path = "src/main.rs"
  ```

  `cargo install breakout-tui` therefore puts a binary called `breakout` on `$PATH`, which is
  what the user actually asked for.
- **`cargo install` is the only supported installation path.** No Homebrew formula, no `.deb`,
  no `curl | sh`, no GitHub Releases binaries in v1.
- **CI builds and tests on macOS and Linux runners** plus an MSRV job, so "works on both
  machines" is verified rather than hoped for. CI does not publish anything.
- **Cargo metadata** carries `description`, `keywords`, `categories = ["games", "command-line-utilities"]`,
  the licence (PRD **OQ-3**), and an `include` list so `assets/` ships but `docs/` and the plan
  do not bloat the package.
- **Whether to publish to crates.io at all is open (PRD OQ-2).** If the answer is no, the
  documented install is `cargo install --git <repo-url>`, which needs no name at all and makes
  this ADR's naming decision moot but harmless. Resolve before Phase 9.

## Consequences

**Good**

- The user's actual requirement — type `breakout`, play — is met by the standard Rust toolchain
  with no packaging work.
- One install path means one thing to document, test and keep working.
- If publishing does happen, `breakout-tui` is descriptive and available, and the name collision
  is handled at the metadata level rather than by picking a worse command name.

**Bad, and accepted**

- Playing requires a Rust toolchain. That excludes anyone who is not already a Rust developer —
  acceptable for an audience of one, and reversible later by adding a releases workflow.
- `cargo install` compiles from source, so first install takes minutes rather than seconds.
- Package name and binary name differing is a small, permanent source of confusion. It has to be
  stated at the top of the README.
- If `breakout-tui` is claimed before publication, the package name changes and the binary name
  does not. That is exactly why this ADR separates them.

## Alternatives rejected

- **Naming the binary something else** (`brk`, `bkout`) to match an available crate name. The
  user asked for `breakout`. The command name is the requirement; the package name is an
  implementation detail.
- **Homebrew tap plus prebuilt binaries.** Offered and declined by the user in favour of
  `cargo install` only. Recorded so it is not silently re-added; adding it later is a new ADR
  and a release workflow, not a code change.
- **A `cargo-dist` release pipeline.** Same reasoning — good machinery, not wanted for v1.
- **Vendoring the levels as external data files installed alongside the binary.** `cargo install`
  installs binaries, not data. ADR-0007 bakes the levels in for this reason.

## Related

- [[project-plan/Breakout/ADR/ADR-0007-level-format|ADR-0007]]
- [[project-plan/Breakout/PLAN|PLAN.md]] — Phase 9
- [[project-plan/Breakout/PRD|PRD]] — FR-1, §6, OQ-2, OQ-3
