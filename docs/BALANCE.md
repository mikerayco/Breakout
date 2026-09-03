# BALANCE — tuning history (Phase 9)

The Phase 9 balance pass is an authorized exception to AGENTS §0 rule 2:
values may change freely here, reviewed via Mike's three runs + this table.

## Verdict: no changes

The agent cannot playtest feel (no Ghostty in this environment), and blind
retuning risks more than it fixes. Every value below ships exactly as
Mike-approved (Phase 2 proposal) or as specified by the PRD/MOCKUP/ADRs,
plus the Phase 6/8 additions recorded alongside. First real balance pass
belongs after Mike's three runs (Phase 9 stop rule).

## Core loop (approved 2026-09-03, unchanged)

| Knob | Value | Source |
| --- | --- | --- |
| Ball base / per-brick / per-level / cap (px/s) | 160 / +1.5 / +10 / 340 | proposal |
| Paddle max vel / accel / friction (px/s, px/s²) | 260 / 1800 / 2200 | proposal |
| Paddle momentum carry | 0.25 | proposal |
| English max / min-vertical fraction | 65° / 0.40 | proposal |
| Launch angle range | ±50° | proposal |
| Lives / score base / combo cap | 3 / 100 / ×8 | proposal |
| Drop rate default | 0.08 | proposal |
| Fixed timestep / catch-up | 240 Hz / 5 | ADR-0006 |
| Multiball cap | 8 | PRD §8 |

## Powerups (Phase 6 additions, unchanged)

| Knob | Value |
| --- | --- |
| Durations Laser / Sticky / Wide / Slow / Pierce (s) | 10 / 12 / 15 / 10 / 8 |
| Laser speed / max shots | 420 px/s / 4 |
| Capsules max concurrent | 8 |
| Slow factor | ×0.7 (FR-32) |
| Multiball split half-angle | 0.35 rad |

## Juice (MOCKUP §4, unchanged)

Hit-stop 40 ms → 12 ms (combo 1 → 8+); shake 0.6 / 2.5 / 4.0 px,
decay `e^(-t/0.08)`, clamp ±5; particles 8–14/brick, 40/explosion,
40–90 px/s, gravity 220, life 350–600 ms; trail 12, bloom 0.72/60%.

## Run economy (Phase 8 additions, unchanged)

| Knob | Value |
| --- | --- |
| Run length / tier map | 8 levels: 1,1,2,2,3,3,4,4 |
| Shards | 4/level + bricks/8 + best-combo/2 |
| Unlock rings | 0 (6 perks) / 15 (+3) / 40 (+3) |

Perk table (12): Overclock 1.12×spd/1.25×score; Second Serve refund 1;
Long Fuse 1.5× durations; Greedy 1.5×score/−1 life; Magnet 60 px/s;
Steady 1.1×width/0.95×spd; Bargain +0.05 drop; Glass Cannon 1 life/2×score;
Lightning 1.15×spd; Insurance +2 lives/0.85×score; Showtime 1.3×score;
Phoenix refund 2.

## Next pass (after Mike's three runs)

Watch: ball cap vs tier-4 density, drop rate feel at 0.08, Glass Cannon
and Greedy pick rates, shard pace to 15/40.
