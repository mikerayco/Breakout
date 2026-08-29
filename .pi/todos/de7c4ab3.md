{
  "id": "de7c4ab3",
  "title": "Phase 9 — Balance, docs and ship",
  "tags": [
    "phase-9",
    "plan",
    "ship",
    "docs",
    "balance",
    "credits"
  ],
  "status": "open",
  "created_at": "2026-08-29T09:28:11.008Z"
}

## Goal
`cargo install` and play, on both machines.

## Runnable outcome
The command `breakout` works from a clean shell on macOS and Linux.

## Tasks (PLAN Phase 9)
1. Balance pass on `tuning.rs`: difficulty curve across the 8 run levels, drop rates, perk strength. Record before/after values in `docs/BALANCE.md`.
2. `README.md`: what it is, terminals it needs, install, controls, level authoring, asciinema/GIF capture, and a **Credits** section listing every third-party asset (sound effects, music, any graphics), source URL, licence (PRD NFR-12). (From TODO asset CREDITS.md.)
3. `docs/PERF.md` final numbers on both machines.
4. Package metadata for `cargo install`: description, keywords, categories = ["games","command-line-utilities"], licence = MIT (OQ-3 resolved), `include` list, `[[bin]] name = "breakout"` on package `breakout-tui` (ADR-0009).
5. OQ-2 resolved: document `cargo install --git` (no crates.io publish).

**No CI in v1** (ADR-0011 supersedes ADR-0009's CI provision). Both-machines verification is
manual: Mike plays a full run on macOS **and** on Linux at this gate (PRD §7). The local quality
gates (`cargo build --release`, `cargo test`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`) still must be clean; run them locally, not via a CI runner.

**Balance pass is an authorized exception** to AGENTS §0 rule 2: change tuning values freely in
Phase 9; review is via the stop rule (Mike plays three runs) + before/after table in
`docs/BALANCE.md`, not per-value pre-approval.

## Test gate (on both machines, repo not on $PATH)
```sh
cargo install --path .
cd /tmp && breakout --version && breakout
```

## Pass when
Binary on $PATH as `breakout` on macOS and Linux; a full run plays on both; `--help` documents every flag in FR-3; local quality gates clean (build/test/clippy/fmt); PRD §7 success criteria 1–5 all hold.

## Stop rule
Success criterion 5 — Mike plays three runs in a row without being asked to.

## Notes
- Licence = MIT (OQ-3 resolved).
- Distribution = `cargo install --git` only (OQ-2 resolved; no crates.io publish).
- No CI in v1 (ADR-0011); both-machines check is manual at Phase 9.
- Linux verification scope: phases 0–8 on macOS only; both-machines at Phase 9 (PRD §8).
- Balance pass = authorized exception to the tuning-value guardrail (PRD §8).
