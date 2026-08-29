---
title: Breakout — Visual spec
type: design
tags: [project-plan, design, breakout]
status: draft
date: 2026-08-29
---

# MOCKUP — visual specification

The companion file `mockup.html` in this folder renders this spec. **Open it in a browser, not in
Obsidian's preview.** Everything below is in the 320 × 240 logical pixel space of
[[project-plan/Breakout/ADR/ADR-0003-logical-resolution|ADR-0003]]; multiply by the integer scale
factor `S` at draw time.

These values are normative. They go into `src/render/palette.rs` and `src/game/tuning.rs`
verbatim in Phase 4.

## 1. Layout — all coordinates in logical pixels, origin top-left

| Region | x | y | w | h |
| --- | --- | --- | --- | --- |
| Screen | 0 | 0 | 320 | 240 |
| HUD band | 0 | 0 | 320 | 20 |
| HUD rule (1 px) | 0 | 20 | 320 | 1 |
| Bezel — left | 0 | 21 | 7 | 219 |
| Bezel — right | 313 | 21 | 7 | 219 |
| Bezel — top | 7 | 21 | 306 | 7 |
| Play area | 7 | 28 | 306 | 212 |
| Brick grid origin | 7 | 36 | 18 cells × 17 | up to 14 rows × 8 |
| Paddle rest line | — | 222 | 51 (default) | 5 |

- **Brick cell:** 17 × 8, drawn as a 16 × 7 rounded rect with a 1 px gap right and below, so the
  grid reads as separated tiles without drawing gridlines.
- **Ball:** radius 3, anti-aliased, drawn with a 1 px brighter core.
- **Kill line:** y = 240. The ball is lost when its centre passes it.
- **Paddle widths:** 51 default, 75 with Wide, 35 with the narrow perk. Height always 5, with a
  2 px brighter cap along the top edge.
- The screen is centred in the terminal; the surrounding area is painted `bg-void`.

## 2. Palette

Named exactly as they will appear in `render/palette.rs`.

| Name | Hex | Used for |
| --- | --- | --- |
| `bg-void` | `#07080f` | Area outside the 320 × 240 screen |
| `bg-deep` | `#0b0d17` | Play area background |
| `bg-hud` | `#10131f` | HUD band |
| `grid-line` | `#141829` | Faint background grid in the play area, 17 × 8 lattice, 1 px |
| `bezel` | `#232a45` | Walls and the HUD rule |
| `bezel-lit` | `#3a4370` | 1 px inner highlight on the bezel, and wall flash on impact |
| `text` | `#e6edf7` | Primary HUD text |
| `text-dim` | `#7b86a8` | Labels, inactive elements |
| `paddle` | `#4de3ff` | Paddle body |
| `paddle-cap` | `#a8f4ff` | Paddle top edge, 2 px |
| `ball` | `#fffbe8` | Ball core |
| `ball-glow` | `#ffd166` | Ball trail and bloom tint |
| `brick-1` | `#4d96ff` | 1 HP |
| `brick-2` | `#06d6a0` | 2 HP |
| `brick-3` | `#ffd166` | 3 HP |
| `brick-4` | `#ff9f1c` | 4 HP |
| `brick-5` | `#ff4d6d` | 5 HP |
| `brick-steel` | `#6b7394` | Indestructible |
| `brick-explosive` | `#ff2e63` | Explosive, with a 1 px `#ffd166` core pixel |
| `powerup` | `#c77dff` | Capsules and their trails |
| `combo` | `#ff6ec7` | Combo counter at ×3 and above |
| `danger` | `#ff2e63` | Last life, low timers |

A damaged brick **recolours to the tier below** rather than cracking: a 5 HP brick hit once is
drawn as `brick-4`. That is the whole damage-state visual language — no crack sprites.

## 3. HUD, left to right in the 20 px band

| Slot | x | Content |
| --- | --- | --- |
| Score | 6 | `SCORE 0128400`, `text`, 5 × 7 font |
| Lives | 96 | Up to 5 3 × 3 pips in `paddle`; the last one blinks in `danger` |
| Level | 140 | `LVL 3/8`, `text-dim` label, `text` value |
| Combo | 196 | `×7` — hidden below ×2, `text` at ×2, `combo` at ×3+, scale-pops on increment |
| Powerup timers | 232 | Up to 4 icons, 9 × 9, each with a depleting 1 px bar beneath |
| Mute | 306 | A small speaker glyph, `text-dim` when muted, hidden when not |

Perks taken sit as 7 × 7 chips along the bottom-left of the play area at y = 231, `text-dim`
outlines, only visible between levels and on pause — they must not clutter the playfield.

## 4. Motion and effect specification

| Effect | Spec |
| --- | --- |
| Ball trail | 12 previous positions, radius tapering 3 → 0.5, `ball-glow` at 55 % alpha, additive |
| Brick particles | 8–14 per brick, 1–2 px, initial speed 40–90 px/s outward from impact, gravity 220 px/s², life 350–600 ms, colour from the brick's tier, fading to transparent |
| Explosion | 40 particles, 2 × radius, plus a 1-frame white ring at the brick centre expanding to 24 px |
| Screenshake | brick 0.6 px · explosion 2.5 px · life lost 4 px; decay `e^(-t/0.08)`, applied as a sub-pixel camera offset, clamped to ±5 px |
| Hit-stop | 40 ms at combo 1, scaling to 12 ms at combo 8+; simulation frozen, rendering continues |
| Brick flash | 1 frame at `#ffffff`, then the tier-below colour |
| Paddle flash | `paddle-cap` across the whole body for 2 frames on contact |
| Combo pop | Counter scales 1.0 → 1.4 → 1.0 over 120 ms on increment |
| Bloom | Threshold luminance 0.72, 3 × 3 box blur, added back at 60 %. Toggle `F4` / `--no-bloom` |
| Powerup capsule | 11 × 7 rounded rect in `powerup`, 1-letter glyph inside, gentle 2 px vertical bob, falls at 55 px/s |

## 5. Screen states the mockup shows

1. **In play** — the main state, with a partly-cleared grid, two balls, a falling capsule, an
   explosion mid-frame, and a ×7 combo.
2. **Perk offer** — three cards over a dimmed, frozen playfield, one highlighted.
3. **Run summary** — score, levels cleared, best combo, shards earned, perks taken.

Only state 1 is specified pixel-by-pixel above. States 2 and 3 inherit the palette and the 5 × 7
font, use the play area's full width with a 12 px margin, and dim the frozen playfield behind
them to 25 % brightness.

## 6. Typography

One font: a hand-rolled 5 × 7 bitmap face in `render/text.rs`, uppercase A–Z, 0–9, and
`: . / × % - + ! ?`. 1 px letter spacing, 2 px word spacing. Double-size (10 × 14) is achieved by
pixel doubling for headings on the summary and perk screens. **No second font, no font crate**
([[project-plan/Breakout/ADR/ADR-0005-stack|ADR-0005]]).

## Related

- [[project-plan/Breakout/index|Breakout]]
- [[project-plan/Breakout/PLAN|PLAN.md]] — Phase 4 gates against this file
- [[project-plan/Breakout/ADR/ADR-0003-logical-resolution|ADR-0003]]
