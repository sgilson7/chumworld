# Plan

Stage 2. Approved in advance (`BRIEF.md`, Autonomy). Built from the journal's opening entry and the 15 entries of its "Your description" tab, with the person's answers of 2026-10-20 (`DECISIONS.md`, T1 to T5). `B2 Plan before code`.

## What the MVP is

One 4 × 4 km region of plains, drawn to a 16 km horizon. The player wakes in a village during an orc raid; death to an orc returns them to bed ("Huh. What a funny dream."); crossing 250 m from the village centre ends the opening. After that: a bandit camp in a north-east forest (lookout, bandits, a leader who casts one magic pattern and carries the sword; the leader drops the key to the chest holding the boomerang), a merchant with a donkey cart on the road (stopped by a throwable landing on the cart), a trainer in a hut on the hill west of the river who teaches the imbue through a practice round (T2), five speedster pieces in five places, a river with one bridge whose water moves with the fishing game's solver (T4). Over-the-shoulder third-person camera always (T1). Duels through 3D Verlet arms steered by the mouse.

## Milestones and owners

Each milestone is one subagent task in its own worktree and branch (`CLAUDE.md`, "Working with subagents"). Notebook rows are numbered by task so parallel tasks never collide.

| Milestone | Task | Rows | Owns | Depends on |
| --- | --- | --- | --- | --- |
| M0 Foundation | F | R2.x | `fx.rs`, `geom.rs`, `rng.rs`, `ticks.rs`, `world.rs` (the tick and its order), `camera.rs`, `render.rs` (instance array, mesh table, passes), `input.rs`, `api.rs`, shim, `web/` (gl.js, input.js, app.js, hud shell), `data/shaders/`, `tools/shoot.py`, flicker baseline tests, `MEASUREMENTS.md` harness | nothing |
| M1 Duel | D | R3.x | `arm.rs`, `blade.rs`, `motion.rs` (feet, grip, ice), `pilot.rs` (behaviour trees, `data/pilots.json`), `health.rs`, throwables (rock, knife) | M0 |
| M2 World | W | R4.x | `terrain.rs`, `lod.rs`, `water.rs` (from the fishing game), `sky.rs` (light, haze), `scatter.rs` (trees, rocks, grass), `places.rs` + `data/places.json` (village, camp, hut, shrine, road, bridge), terrain shaders | M0 |
| M3 Models | A | R5.x | `mesh.rs` (faceted primitives), `models.rs` + `data/models.json`: player, orc, bandit, leader, merchant, donkey, trainer, villagers; houses, tents, cart, chest, scarecrow, shrine, hut, bridge, weapons, boomerang, rocks, the five speedster pieces | M0 |
| M4 Menus | U | R6.x | `items.rs` + `data/items.json` (CW-6, CW-7, CW-8 tests), `inventory.rs`, `ui.rs` (view model), `web/menus/` (the ring of four screens, HUD, dialogue, shop, title, pause, settings) | M0 |
| M5 Places and story | S | R7.x | `story.rs` (bed, raid, dream loop, boundary), `merchant.rs`, `camp.rs` (lookout, leader, key, chest), `trainer.rs` (T2), `boomerang.rs`, speedster pickups | M1–M4 |
| M6 Magic | M | R8.x | `orbs.rs` (Long Time Comin's patterns with shapes, CW-10), `imbue.rs` (CW-9), the leader's pattern, the trainer's practice pattern, per-orb deflection | M1, M5 |
| M7 Polish | P | R9.x | flicker audit (CW-18), frame budget (CW-11), sound, the art pass against the references, the full check | all |

M1 to M4 run in parallel after M0 merges; M5 and M6 run after them; M7 last. The main agent merges each branch, reruns `./scripts/check.sh`, looks at every screenshot, writes the journal entry and publishes (T5).

## Numbers the code starts from

| Thing | Value | Source |
| --- | --- | --- |
| Tick | 60 per second | Vagrancy, Long Time Comin' |
| Unit | Q16.16 metres | R1.4 |
| Walkable square | 4 km × 4 km, origin at its south-west corner | CW-Q14 |
| Village boundary | 250 m from the village centre at (2.0 km, 1.2 km) | journal map (north up: map y = 4 km − world z) |
| Relaxation passes | 8 | Vagrancy |
| Short sword / sword | 70 cm / 100 cm blade | journal, items |
| Boomerang | 18 m reach, 150 ticks out and back | journal |
| Speedster | speed × (1 + 0.2 n), grip × 0.55ⁿ, base run 5 m/s | CW-8 |
| Imbue | 600 ticks on, 1,200 ticks cooldown from the cast | CW-9 |
| Merchant | 1.4 m/s; waits 1,800 ticks after a hit | journal |
| Terrain rings | 2 m cells to 256 m, doubling out to 128 m cells at 16 km; dither over the outer 15% | journal |
| Camera | over the right shoulder, 2.6 m behind, 0.6 m right, 0.5 m above the head; near plane 0.4 m | T1, Iron Vector |
| Frame budget | 16.7 ms, core tick ≤ 2 ms | journal |

## Flicker (T3, CW-18)

Tests, in the core unless noted:

1. No two static faces are coplanar and overlapping: a scan over every static instance's faces in the region.
2. Depth precision: for the near and far planes in use, the depth step at each ring's outer distance is smaller than the smallest separation between surfaces drawn there.
3. Detail changes only through the dithered hand-over: for camera paths at the fastest speed, no instance changes level without passing through the dither band.
4. Instance order is stable: the same world gives the same instance array, byte for byte, and two ticks with nothing moving give identical arrays.
5. Browser: with the camera still, two consecutive frames are identical pixel for pixel; on a slow pan, no pixel shows an A-B-A change across three frames beyond a small threshold.

## Files touched first

M0 replaces the template's start screen with the game's canvas and keeps `ui()` and `act()` as the one door. The realtime kit is copied in (`kits/realtime/`). The WebGL renderer is ported from Long Time Comin' (`/Users/samgilson/Documents/LongTimeComin`, `web/gl.js`, `crates/core/src/render.rs`), recorded in `LICENSES.md`.
