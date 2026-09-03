# Breakout — GPU-accelerated Breakout for your terminal

A modern Breakout that runs inside a graphics-capable terminal. The game
rasterises a 320×240 playfield in Rust, integer-scales it to your window,
and pushes it to the terminal through the Kitty graphics protocol over
POSIX shared memory at up to 144 fps — sub-pixel motion, particles,
bloom and screenshake, no window server required. On top sits a roguelite
loop: 8-level runs, a perk draft after each level, 7 powerups,
hand-authored text levels, and persistent shard unlocks.

## Terminals

You need a terminal with the **Kitty graphics protocol**: **Ghostty**
(primary), Kitty, or WezTerm — on macOS or Linux. Without it the game
exits with status 2 and tells you so; there is no text-cell fallback.
`tmux` is not supported.

## Install

`cargo install` is the only distribution path (no crates.io publish):

```sh
cargo install --git https://github.com/mikerayco/Breakout.git
# or from a checkout:
cargo install --path .
```

Then from any shell:

```sh
breakout            # title screen, no setup
breakout --help     # every flag
breakout --caps     # capability report, exits 0
```

## Controls

| Key | Action |
| --- | --- |
| `←`/`→` or `h`/`l` (hold) | Move paddle |
| `Space` | Launch ball / fire laser / release sticky / confirm |
| `Esc` | Pause menu / resume |
| `1`/`2`/`3`, arrows + `Enter` | Perk offer pick |
| `q` | Quit |
| `m` | Mute (persisted) |
| `F3` | Debug overlay (FPS, frame times, input mode, latency, audio) |
| `F4` | Bloom toggle |
| `F5` | Debug powerup spawner |

Without the Kitty keyboard protocol the game falls back to a degraded
repeat-decay mode (`BREAKOUT_INPUT=legacy` forces it); playable, worse.

Useful flags: `--level <file>` (play one level standalone), `--seed <n>`
(reproducible runs), `--fps <30-144>`, `--scale <n>`, `--no-audio`,
`--no-bloom`, `--validate <dir>` (check levels), `--reset-profile`.

## Level authoring

Levels are plain text: a TOML header, a `---` line, and an 18-column
ASCII grid (1–14 rows). One character is one brick:

```
name = "Crossfire"
tier = 2
---
..SS..........SS..
..1111....1111....
```

`. ` empty · `1`–`5` brick HP · `S` steel (indestructible, doesn't count)
· `E` explosive (1 HP, clears its 3×3 neighbourhood, chains allowed).
`tier` 1–4 picks the run pool. Errors name file, line, column and
character — and never panic. Author in any editor, test instantly:

```sh
breakout --level ./scratch.lvl
breakout --validate assets/levels/campaign
```

The 16-level campaign ships inside the binary; installed players need no
data files.

## Save data

One JSON profile: `~/Library/Application Support/breakout/profile.json`
(macOS) or `$XDG_CONFIG_HOME/breakout/profile.json` (Linux). Written
atomically at run end and on settings changes. A corrupt profile is
renamed aside (`profile.corrupt.<ts>.json`) and play continues.

## Credits

No third-party creative assets ship in v1, so there is nothing to
attribute — and that is deliberate:

- **Graphics:** self-generated every frame (tiny-skia rasterisation +
  a hand-rolled 5×7 bitmap font). No image files.
- **Audio:** all ten effects and the music bed are synthesized at build
  time by `build.rs` (deterministic 22050 Hz WAVs) and compiled into the
  binary. No samples were sourced, so OQ-5's CC0-sourcing requirement is
  satisfied vacuously; if sourced beds ever replace the synth, their
  source URLs and licences will be recorded here per NFR-12.
- **Code:** MIT licence. Dependency budget in `Cargo.toml` (clap,
  crossterm, tiny-skia, kira, libc, base64, serde, toml, fastrand,
  anyhow).

## Capture

An asciinema/GIF capture of a full run goes here once recorded on Mike's
machines (Phase 9 gate).
