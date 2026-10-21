# Measurements

Stage 4, recorded by the agent before building and again at handoff. `B2 Require step-by-step justification`.

The state measured rather than recalled, so later changes can be compared with it. A probe prints numbers and asserts loose bounds at most; record its output here with the command that produced it.

The machine: Sam's Mac, Apple M2 Max, macOS (Darwin 25.3). Frame budget (journal, `PLAN.md`): 16.7 ms a frame at 60 fps, the core's share 2 ms or less (CW-11).

| What | Command | Result | When |
| --- | --- | --- | --- |
| Toolchain | `rustc --version; wasm-bindgen --version` | rustc 1.99.0 (b940084d7 2026-09-28); wasm-bindgen 0.2.127 | 2026-10-20, M0 |
| Core tests | `./scripts/count-tests.sh` | template: 38 passed; M0: 127 passed, 0 failed, 1 ignored (the frame probe) | 2026-10-20, M0 |
| Full check | `./scripts/check.sh` | template: 9 steps passed in 48 s; M0: 9 steps passed in 45 s, three engines | 2026-10-20, M0 |
| WebAssembly size | printed by `./scripts/build.sh` | 343 KB (351 266 bytes; 100 080 gzipped at level 9); `dist/` 432 KB in all. Release profile unchanged (D11, `opt-level = "s"`) | 2026-10-20, M0 |
| Breaks caught | `python3 scripts/break_tests.py` | M0: 99 of 99 caught | 2026-10-20, M0 |
| Load to the title, Chromium on Metal, served from this machine | `.venv/bin/python tools/shoot.py OUT --measure` (`load_s`: from the navigation's first byte to `body[data-ready]`) | 0.07 s | 2026-10-20, M0 |
| Frame rate, headless Chromium on Metal (`ANGLE Metal Renderer: Apple M2 Max`), 1920 × 1080 | `tools/shoot.py OUT --measure` | 60.0 fps on the title, standing, and running in a circle; frame intervals median 16.7 ms, 95th percentile 16.8 ms, worst 16.8 ms. The display's 60 Hz caps it, so the headroom under 16.7 ms is not measured | 2026-10-20, M0 |
| Frame rate, headless Chromium on SwiftShader, 1920 × 1080 window (drawn at 960 × 540, `SOFTWARE_SCALE_PCT`) | `tools/shoot.py OUT --measure --soft` | 23 fps standing and running, 20 on the title; median 50 ms. Before the ground became four meshes (R2.16): 3.3 fps | 2026-10-20, M0 |
| Frame rate, SwiftShader, by window size | scratch script over `tools/shoot.py`'s helpers | 640 × 360: 40 fps; 960 × 540: 35; 1920 × 1080: 23 | 2026-10-20, M0 |
| What headless Firefox and WebKit draw with (the browser check's engines) | `getParameter(UNMASKED_RENDERER_WEBGL)` in each | Firefox: "Apple M1, or similar" (hardware); WebKit: "Apple GPU" (hardware); Chromium without flags: SwiftShader | 2026-10-20, M0 |
| The core's cost of one frame, native release | `cargo test --release -p tool_core --test frame_probe -- --ignored --nocapture` | 0.005 ms mean, 0.007 ms worst (ticks, camera, instance array, draw list), against the core's 2 ms | 2026-10-20, M0 |
| Start, native release | same probe | 4.6 ms (parse the data, lay out the field, build its meshes and the first static array) | 2026-10-20, M0 |
| Laying out the field and packing the static array again, native release | same probe | 4.4 ms (an origin move repacks the groups only; it does not rebuild the meshes) | 2026-10-20, M0 |
| What a frame sends | same probe | static array: 444 instances, 27 KB, uploaded once a 256 m origin cell; ground: 4 meshes, 17 568 triangles, uploaded once; moving instances a frame: 6; draws a frame: 46 | 2026-10-20, M0 |
| Flicker: still frames | `tests/browser/check.py` | two consecutive frames identical pixel for pixel in Chromium (640 × 360, software), Firefox and WebKit (1280 × 720) | 2026-10-20, M0 |
| Flicker: a slow pan, pixels that flip and flip back across three frames | `tests/browser/check.py` (look speed 25%, a fifth of a pixel a frame) | Chromium 7 of 230 400; Firefox 22 of 921 600; WebKit 19 of 921 600 (bound: 0.01%) | 2026-10-20, M0 |
| Flicker: the same pan at full look speed (0.84 px a frame at 640 wide) | scratch script, A-B-A count | before camera-relative drawing (R2.8): 1 064 of 921 600 on Metal; after: 112 on Metal, 172 of 230 400 on SwiftShader, on cone tips, far trunks and a pole's top (R2.9) | 2026-10-20, M0 |
| Depth precision | `cargo test depth_precision_holds_at_every_ring_distance` | worst step (24-bit, two layers): 2.4 mm at 128 m (near layer), 0.14 m at 16 384 m (far layer). One layer from 0.4 m would be 40 m at the horizon | 2026-10-20, M0 |

## M3 Models (task A)

| What | Command | Result | When |
| --- | --- | --- | --- |
| Triangles and instances a model draws | `cargo test -p tool_core --test model_counts -- --ignored --nocapture` | Figures 2 252 (bandit) to 3 216 (trainer) solid triangles, drawn twice with their outlines; 64 to 91 instances with the shadow; the player in all five speedster pieces 3 184 and 91. Held items 80 (rock) to 640 (boomerang) triangles, 2 to 12 instances. Donkey 1 916 triangles, 53 instances | 2026-10-20, M3 |
| A prop's static cost | same probe | One body mesh and one outline mesh per kind, two instances a placement plus its glowing parts (campfire 2, shrine 5, cart 1). Body triangles: fence 78, tent 192, signpost 194, chest 250, bed 292, barn 396, well 592, scarecrow 724, bridge 780, hut 848, campfire 872, shrine 926, hollow tree 1 144, house 1 160, cart 1 168 | 2026-10-20, M3 |
| The lift a mark needs | `cargo test the_lift_a_mark_needs` | 4 depth steps at the worst distance out to 1 km: 636 Q16 units, 9.7 mm (near layer at 128 m); out to 3 km 19 mm. Marks stand 10 mm off, prop details 25 mm | 2026-10-20, M3 |
| WebAssembly size | printed by `./scripts/build.sh` | 502 KB (M0: 343 KB): the models' code and `data/models.json` compiled in | 2026-10-20, M3 |

## M1 Duel (task D)

| What | Command | Result | When |
| --- | --- | --- | --- |
| The core's tick with eight fighters (the raid: the player, three orcs, three bandits, a leader), native release | `cargo test --release -p tool_core --test frame_probe -- --ignored --nocapture` (`probe_raid_tick_cost`) | 0.143 ms mean, 0.362 ms worst over 600 ticks, against the core's 2 ms (CW-11) | 2026-10-20, M1 |
| `World::hash` with eight fighters | same probe | 0.186 ms native (100 KB of debug text); about 1 ms under the test profile (R3.18) | 2026-10-20, M1 |
| One frame, the M0 probe, M1 merged (one fighter) | same command, `probe_frame_cost` | 0.031 ms mean, 0.051 ms worst; 32 moving instances a frame; start 12.7 ms (M0: 4.6 ms; the pilots' book is read at start) | 2026-10-20, M1 |
| WebAssembly size, M1 before M3 merged | printed by `./scripts/build.sh` | 527 KB (M0: 343 KB) | 2026-10-20, M1 |
| Speed and grip by speedster pieces (CW-8), pinned | `grip_and_speed_are_pinned_for_zero_to_five_pieces` | run 5, 6, 7, 8, 9, 10 m/s; grip 21 845, 12 014, 6 607, 3 633, 1 998, 1 098 of 65 536 (⅓ × 0.55ⁿ) | 2026-10-20, M1 |
| Slide to a stop from a full run | `five_speedster_pieces_slide_ten_metres_to_a_stop_and_bare_feet_a_hand` | 0.17 m bare, 9.8 m with five pieces (by hand: v (1 − g) / g) | 2026-10-20, M1 |
| Tip speed of a swing from a high guard to a low strike (one arm, a scratch probe over `arm::Arm`) | scratch probe, not kept | plain / committed: short sword 7.4 / 10.7 m/s, sword 6.4 / 9.4, cleaver 4.9 / 7.5; the blade holds its guard line to under 1° at rest | 2026-10-20, M1 |
| A still fighter settling after a walk | scratch probe | every point still (under 0.37 mm a tick, then held exactly) within about a second (R3.7) | 2026-10-20, M1 |

## M1 polish (task D2)

| What | Command | Result | When |
| --- | --- | --- | --- |
| The core's tick with eight fighters (the raid), native release | `cargo test --release -p tool_core --test frame_probe -- --ignored --nocapture` (`probe_raid_tick_cost`) | 0.466 ms mean, 1.121 ms worst over 600 ticks (M1: 0.143 / 0.362), against the core's 2 ms (CW-11): the fences, the sweep against every body and the separation's capsules | 2026-10-20, D2 |
| One frame, one fighter, native release | same command, `probe_frame_cost` | 0.063 ms mean, 0.109 ms worst; start 5.0 ms | 2026-10-20, D2 |
| How deep a blade or hand sits in a body at a tick's end, over 3 600 ticks of recorded duels (an orc, a bandit, a leader, three bandits) | `no_blade_or_hand_sits_inside_a_body_over_a_recorded_duel` (a scratch count over the same duels) | worst 25 units (0.4 mm) into the 15 mm skin the fences keep, so never inside a capsule (M1: a cleaver sat through the player's head) | 2026-10-20, D2 |
| How deep two bodies' trunks and legs sink into each other over a shove, a duel and a group | `no_two_bodies_sink_into_each_other_in_a_shove_a_duel_or_a_group` | worst 1 unit; bound 200 (3 mm) | 2026-10-20, D2 |
| Reach, shoulder to tip at full stretch | `a_fighter_s_reach_is_its_arm_at_full_stretch_and_its_blade` | player and short sword 1.388 m, orc and cleaver 1.435, bandit and short sword 1.405, leader and sword 1.771 | 2026-10-20, D2 |
| Where an orc keeps to against a still player | a scratch run over `pilot::decide` | holds 158 to 188 cm, lunges to about 125, back at 165 after each attack | 2026-10-20, D2 |
| A side cut and an overhead through the arm | `an_overhead_a_side_cut_and_a_thrust_each_read_from_the_mouse_s_path` (a scratch print of the tip) | side cut: 2.5 m across, within 9 cm of one height; the edge meets a body at 10 to 13 m/s | 2026-10-20, D2 |
| The view's framing held on a foe (1920 × 1080, an orc at its band) | `held_on_a_foe_the_view_frames_the_duel_with_its_blade_clear_of_the_player_s_head` | the orc's blade's nearest point 107 px from the player's head, 93 px without the framing; the head about 47 px round | 2026-10-20, D2 |
| Raid tick after M2 merged, native release | `probe_raid_tick_cost` | 0.498 ms mean, 0.695 ms worst (fence rounds 16) | 2026-10-20, D2 |
| A guard held at a body walking into it | `a_body_cannot_walk_into_a_blade_held_at_it` | the walker held 1.37 m off a still orc | 2026-10-20, D2 |
## M2 World (task W)

| What | Command | Result | When |
| --- | --- | --- | --- |
| Frame rate at 1920 × 1080, headless Chromium on Metal (M2 Max): standing in the village, running across the plains (Shift + W, turning), turning slowly on the trainer's hill toward the horizon | `.venv/bin/python tools/shoot.py OUT --measure` (M2's walks) | 60 fps in all three, frame intervals median 16.7 ms, worst 16.8 ms (the display's 60 Hz caps it) | 2026-10-20, M2 |
| The page's own frame callback (the core's advance in WebAssembly, the meshes handed over, about 550 draw calls), same walks | same command (`callback_*`: requestAnimationFrame wrapped in the tool's page) | village 1.9 ms median, 2.3 ms 95th percentile, 3.1 ms worst; plains running 1.4 / 1.9 / 3.5 ms; hill 1.3 / 1.5 / 3.4 ms. Against the 16.7 ms frame with half spare (8.3 ms) | 2026-10-20, M2 |
| Load to the title | same command (`load_s`) | 0.89 s (M0: 0.07 s): the first frame builds about 380 terrain tiles, the river's 113 pieces and the scatter near the start | 2026-10-20, M2 |
| One frame of the core, native release, the M0 probe running and turning round the village | `cargo test --release -p tool_core --test frame_probe -- --ignored --nocapture` | 0.743 ms mean, 1.60 ms worst; start 516 ms; 690 moving instances, 547 draws a frame; static array empty (the props are drawn by distance, R4.13) | 2026-10-20, M2 |
| The detail levels' share of a frame, native, standing after the build-ahead settles | `M2_STILL=1 cargo test --release -p tool_core --test m2_probe probe_frame_parts -- --ignored --nocapture` | terrain 0.17 ms, scatter 0.28 ms, places and water 0.01 ms | 2026-10-20, M2 |
| Building a terrain tile (32 × 32 cells, 2 304 triangles), native | `... --test m2_probe probe_terrain_cost` | plains 0.39 ms, at the river and bridge 0.50 ms; the land (parse, the river's 19 000 stations, the solve) 24 ms | 2026-10-20, M2 |
| A minute running north at 6 m/s, terrain alone, native | same probe | 0.18 ms a frame mean, 1.77 ms worst (two tiles built in a frame at the most); 194 tiles built on the way, none needed before it was built | 2026-10-20, M2 |
| Triangles drawn round the village, terrain, all round (before the frustum drops those behind) | same probe | 758 016 in 329 tiles (56, 43, 38, 48, 47, 36, 60 tiles by level 0 to 6) | 2026-10-20, M2 |
| Resident meshes after the build-ahead settles at the village | `probe_frame_parts` | 1.55 M triangles: terrain 1.08 M (64 to 79 tiles a level), river and pond 0.20 M (113 pieces), forest crowns 0.20 M, cones, trees, rocks and grass 0.08 M. As vertex buffers (36 bytes a vertex, three a triangle): about 168 MB (R4.6) | 2026-10-20, M2 |
| WebAssembly size, M1, M3, M4 and M2 merged | printed by `./scripts/build.sh` | 1 381 KB (M1: 527 KB); M2's data files are in it (R4.8) | 2026-10-20, M2 |
| Flicker: a slow pan at the village (M0's check) and from the hill over the terrain to the horizon | `tests/browser/check.py` (look speed 25%) | village: Chromium 16 of 230 400, Firefox 26 and WebKit 13 of 921 600; hill: 6, 11 and 12 (bound 0.01%: 23 and 92) | 2026-10-20, M2 |
| Flicker: still frames from the hill, westward | same | identical pixel for pixel in all three engines | 2026-10-20, M2 |
| Flicker test 3 (CW-18): three camera paths at 5 and 6 m/s, 10 000 frames | `cargo test no_surface_changes_level_without_passing_through_the_dither_band` | no surface's share of the pixels changed by more than one dither cell (1/16) a frame; none waited to be built | 2026-10-20, M2 |
| Water against its banks (CW-18): every facet crossing the water's level, levels 0 to 5, nine stretches and the pond | `cargo test the_water_meets_its_banks_steeply_enough_for_the_depth_buffer` | the band where the depth buffer cannot tell land from water is narrower than a pixel at the farthest each level is drawn | 2026-10-20, M2 |
| Core tests | `./scripts/count-tests.sh` | 254 passed, 0 failed, 7 ignored (the probes) | 2026-10-20, M2 |

## M5 Places and story (task S)

| What | Command | Result | When |
| --- | --- | --- | --- |
| The opening's tick: the game's start, six orcs on their rounds and chasing, four villagers, the merchant, the walls and floors, a player standing on the green, 20 s; native release | `cargo test --release -p tool_core --test frame_probe probe_opening_tick_cost -- --ignored --nocapture` | 0.387 ms mean, 0.591 ms worst, 7 fighters (budget 2 ms) | 2026-10-20, M5 |
| The same world at the bandit camp, alarmed, the raid kept: 12 fighters | same probe | 0.705 ms mean, 0.966 ms worst | 2026-10-20, M5 |
| A playthrough of every item's route (the wake to the last speedster piece) | `cargo test every_item_s_one_route_can_be_taken_in_one_play` | about 15 s in the test profile | 2026-10-20, M5 |
| Core tests | `./scripts/count-tests.sh` | 344 passed, 0 failed, 10 ignored (the probes) | 2026-10-20, M5 |
| Flicker with the raid in view: the still and slow-pan views on the green, two orcs and a villager in sight, held still | `tests/browser/check.py` | still frames identical in Chromium, Firefox and WebKit; slow pan Chromium 23 of 230 400 pixels (bound: under 23.04, so at the edge), Firefox and WebKit 75 of 921 600 (bound 92); in bed the slow pan saw 80 (the near grass's dithered edge looked down on, R7.9), so the check stands out of bed | 2026-10-20, M5 |

## Not measured

- The headroom under 16.7 ms on Metal: requestAnimationFrame is held to the display's 60 Hz, and the page keeps no timer of its own. A GPU timer query (`EXT_disjoint_timer_query_webgl2`) in a probe would give it.
- The WebAssembly build's own cost a frame (only the native release build's); the browser's frame rate covers it at 60 fps.
- Any machine but this Mac; Safari and Firefox with their own windows rather than headless; a phone (out of scope, `BRIEF.md`).
- Load over a network: GitHub Pages serves the same 432 KB.
- M2: the GPU's own time a frame (only the page's callback, which submits the draws, and the 60 fps the display caps); the frame rate on a machine with less memory than the M2 Max's for the 168 MB of meshes (R4.6); the software renderer's frame rate with M2's terrain (R2.17, for M7).

## M6 Magic (task M)

| What | Command | Result | When |
| --- | --- | --- | --- |
| The core's tick through the leader's whole pattern, the player imbued and swinging, native release | `cargo test --release -p tool_core --test magic_probe -- --ignored --nocapture` (`probe_leader_volley_tick_cost`) | 0.04 to 0.10 ms mean, 0.06 to 0.7 ms worst over 1 020 ticks across runs (spikes vary run to run), against the core's 2 ms (CW-11); 20 orbs at most in flight | 2026-10-20, M6 |
| Draws the orbs add | same probe; `every_orb_of_a_kind_is_one_draw` | one run of solids and one of outlines per kind in flight, one run of halos for every orb: at most 6 × 2 + 1 = 13 draws, whatever the number of orbs | 2026-10-20, M6 |
| What the leader's pattern takes (66 orbs) | same command (`probe_what_the_leader_s_pattern_takes`) | standing still 132; stepping side to side 33; imbued, guard held 84 (9 turned, the leader takes 48); imbued, swinging blindly 88 (10 turned, 44); side to side with the guard held 55 (R8.4) | 2026-10-20, M6 |
| WebAssembly size | printed by `./scripts/build.sh` | 1 529 KB with M1's polish and M6 merged | 2026-10-20, M6 |
| Flicker: a frozen volley in sight | `tests/browser/interactions.py` (`?act=magic.leader&act=magic.freeze:150`) | two consecutive frames identical, pixel for pixel | 2026-10-20, M6 |

## M6 balance (task M, M6b)

What the leader's whole pattern takes from a player at full health, by how it is met (`cargo test --release -p tool_core --test magic_probe -- --ignored --nocapture`, `probe_what_the_leader_s_pattern_takes`; "imbued" is a lend over the whole pattern), 2026-10-20:

| How it is met | Before (M6) | After (M6b) |
| --- | --- | --- |
| standing still, not imbued | 132 | 80 |
| stepping side to side every 50 ticks | 33 | 35 |
| imbued, guard held | 84 (9 turned) | 14 (21 of 41 turnable turned) |
| imbued, swinging blindly | 88 (10 turned) | 44 (16 turned) |
| imbued, stepping side to side, guard held | 55 | 15 (17 turned) |
| imbued, defending (`orbs::defend`) | not measured | 10 (23 of 41 turned, 56%) |

Thrown: 66 orbs before, 62 after (41 turnable). The tick through the pattern, the player imbued and swinging: 0.11 ms mean, 0.24 ms worst, native release. WebAssembly 1 531 KB.

## M7 swings (task P5, T10)

What swings come out of scripted input: the player drawn and settled at the guard, then the left button held and the mouse moved one way, 600 px in 30, 15 or 6 ticks (1 200, 2 400 or 6 000 px a second; after T10 any of these is one gesture, since 40 px picks the cut), then held still. "Plane from horizontal" is the angle between the plane the blade turned in (the mean of its tick-to-tick rotation axes) and the ground: 0° level, 90° upright. "Arc" is how far the blade turned in that plane; after T10 it is measured over the strike and the follow-through, the wind-up left out. "Ticks" is from the first tick the tip passes 1.5 m/s to the last (before), or from the button to the cut's end (after). `cargo test --release -p tool_core --test swing_probe -- --ignored --nocapture` (`probe_swings_from_scripted_mouse`), 2026-10-20.

Before (main at 7be283c):

| Weapon | Mouse | Speed | Plane from horizontal | Arc | Peak tip | Ticks |
| --- | --- | --- | --- | --- | --- | --- |
| Short sword | sideways (left) | slow | 12° | 100° | 4.7 m/s | 29 |
| Short sword | sideways (left) | medium | 11° | 107° | 9.4 m/s | 17 |
| Short sword | sideways (left) | fast | 11° | 108° | 11.0 m/s | 16 |
| Short sword | up | slow | 90° | 86° | 4.7 m/s | 31 |
| Short sword | up | medium | 90° | 88° | 9.1 m/s | 19 |
| Short sword | up | fast | 90° | 87° | 10.9 m/s | 16 |
| Short sword | down | slow | 90° | 21° | 3.1 m/s | 12 |
| Short sword | down | medium | 90° | 22° | 5.6 m/s | 9 |
| Short sword | down | fast | 90° | 22° | 7.5 m/s | 8 |
| Short sword | diagonal (down-left) | slow | 32° | 84° | 4.0 m/s | 31 |
| Short sword | diagonal (down-left) | medium | 36° | 85° | 8.4 m/s | 18 |
| Short sword | diagonal (down-left) | fast | 38° | 86° | 11.4 m/s | 13 |
| Short sword | 20° off sideways | slow | 24° | 106° | 4.5 m/s | 31 |
| Short sword | 20° off sideways | medium | 27° | 106° | 9.4 m/s | 18 |
| Short sword | 20° off sideways | fast | 30° | 106° | 10.9 m/s | 16 |
| Short sword | wheel away (thrust) | medium | 90° | 19° | 5.7 m/s | 6 |
| Sword | sideways (left) | medium | 18° | 117° | 12.9 m/s | 25 |
| Sword | up | medium | 90° | 95° | 11.8 m/s | 20 |
| Sword | down | medium | 90° | 23° | 7.1 m/s | 9 |
| Sword | diagonal (down-left) | medium | 38° | 86° | 11.8 m/s | 18 |
| Sword | 20° off sideways | medium | 31° | 108° | 13.0 m/s | 19 |
| Cleaver | sideways (left) | medium | 26° | 121° | 9.1 m/s | 28 |
| Cleaver | up | medium | 89° | 95° | 7.9 m/s | 27 |
| Cleaver | down | medium | 89° | 25° | 6.0 m/s | 9 |
| Cleaver | diagonal (down-left) | medium | 41° | 88° | 8.4 m/s | 17 |
| Cleaver | 20° off sideways | medium | 33° | 112° | 8.6 m/s | 20 |

The diagnosis is R9.160: a cut was as long and fast as the mouse's stroke, a downward stroke from the guard had 36° of shell left, the blade's line kept a stroke's slant, and the arm's drag stopped it with the mouse.

After (this branch):

| Weapon | Mouse | Mouse speed | Plane from horizontal | Arc | Peak tip | Ticks to the end |
| --- | --- | --- | --- | --- | --- | --- |
| Short sword | sideways (left) | medium | 1° | 189° | 16.4 m/s | 37 |
| Short sword | up | medium | 89° | 176° | 15.9 m/s | 34 |
| Short sword | down | medium | 90° | 184° | 16.8 m/s | 40 |
| Short sword | diagonal (down-left) | medium | 45° | 190° | 15.6 m/s | 39 |
| Short sword | 20° off sideways | medium | 1° | 189° | 16.4 m/s | 38 |
| Short sword | wheel away (thrust) | one notch | (line) | 25 cm ahead | 5.6 m/s | 17 |
| Sword | sideways (left) | medium | 1° | 192° | 18.4 m/s | 42 |
| Sword | up | medium | 90° | 169° | 17.5 m/s | 47 |
| Sword | down | medium | 89° | 154° | 18.4 m/s | 51 |
| Sword | diagonal (down-left) | medium | 45° | 192° | 17.6 m/s | 44 |
| Sword | 20° off sideways | medium | 1° | 192° | 18.4 m/s | 43 |
| Sword | wheel away (thrust) | one notch | (line) | 23 cm ahead | 6.3 m/s | 17 |
| Cleaver | sideways (left) | medium | 2° | 194° | 11.6 m/s | 49 |
| Cleaver | up | medium | 90° | 181° | 11.3 m/s | 45 |
| Cleaver | down | medium | 88° | 158° | 11.6 m/s | 60 |
| Cleaver | diagonal (down-left) | medium | 45° | 192° | 11.3 m/s | 52 |
| Cleaver | 20° off sideways | medium | 2° | 194° | 11.6 m/s | 50 |
| Cleaver | wheel away (thrust) | one notch | (line) | 25 cm ahead | 4.3 m/s | 20 |

The slow and fast strokes give the same rows as the medium within a tick (any stroke past 40 px is one gesture). A thrust is a line, not an arc: its row gives how far the tip went ahead over the strike. The sword's level cut is with the free hand off the hilt (R9.166). M1's staged blow (a level cut stepping in at a still orc 1.35 m off) lands on its torso at 11.1 m/s, inside the 8 to 16 m/s band; the tip itself tops 16 m/s.

| What | Command | Result | When |
| --- | --- | --- | --- |
| Which cuts the foes make, a still player, 30 s of an orc's tree | `a_pilot_s_level_cuts_and_chops_come_through_its_input_record` (a scratch count) | level cuts and chops both made, by the weights in `data/pilots.json` (R9.170) | 2026-10-20, P5 |
| WebAssembly size | printed by `./scripts/build.sh` | 1 669 KB | 2026-10-20, P5 |

## M7 travel (P6, T11)

| What | Command | Result | When |
| --- | --- | --- | --- |
| Travel's own tick (five waystones' nearness, the gift's check, a journey), native release | `cargo test --release -p tool_core --test frame_probe -- --ignored --nocapture probe_travel_tick_cost` | 0.32 µs a tick with nothing reached (the worst case: five distances), 0.013 µs with every waystone woken; against the core's 2 ms (CW-11): negligible. A first version read the ground under each waystone every tick: 6.7 µs, so the nearness now reads the layout's centres | 2026-10-20, P6 |
| Drawing the woken waystones' light and the traveller | same probe | 11.4 µs a frame, 30 shapes (two waystones in reach of the glow and the traveller from where the probe ends) | 2026-10-20, P6 |
| WebAssembly size | printed by `./scripts/build.sh` | 1 740 KB | 2026-10-20, P6 |

### M7 swings, second pass (R9.175 to R9.178)

| What | Command | Result | When |
| --- | --- | --- | --- |
| The thrust, after R9.177 | `swing_probe` (`probe_swings_from_scripted_mouse`) | short sword: tip 12.7 m/s, 50 cm ahead over the strike, 16 ticks (was 5.6 m/s, 25 cm); sword 11.4 m/s, 55 cm; cleaver 10.1 m/s, 65 cm | 2026-10-20, P5 |
| The level cut, middle plane at −5% (R9.178) | same | short sword 1° off level, 188°, 16.5 m/s; sword 1°, 190°, 18.5 m/s; cleaver 2°, 192°, 11.8 m/s | 2026-10-20, P5 |
| The trail | `a_cut_leaves_a_trail_that_fades_to_nothing` | 7 places kept, 3 to 6 quads drawn in a strike; 5, 3, 1, 0 the ticks after the cut ends | 2026-10-20, P5 |

## M7 wounds (task P4)

| What | Command | Result | When |
| --- | --- | --- | --- |
| The raid's tick (8 fighters), native release, unchanged by wounds when no one is cut | `cargo test --release -p tool_core --test frame_probe probe_raid -- --ignored --nocapture` (`probe_raid_tick_cost`) | 0.63 ms mean, 0.98 to 1.40 ms worst over 600 ticks (two runs) | 2026-10-20, P4 |
| The same raid with wounds: every foe cut on the torso at 14 m/s every 20 ticks, a part taken off every foe every 60, so blood stays at its caps | same command (`probe_raid_wounds_cost`) | tick 0.81 ms mean, 1.32 to 2.05 ms worst (two runs; one run on a loaded machine saw 14.7 ms once and 0.82 ms mean); drawing what moves (`World::draw`) 0.34 ms mean, 0.56 to 0.70 ms worst; at most 160 drops, 60 marks, 12 pieces, 1 287 shapes. Against the core's 2 ms a tick (CW-11): within it on the mean | 2026-10-20, P4 |
| Drawing pieces from a model built every frame (the first version) | same probe | draw 1.89 ms mean, 18.9 ms worst with 19 pieces: each piece now keeps its own parts from the cut, and the labels are worked out once a kind | 2026-10-20, P4 |
| How long a cut forearm takes to lie still | `a_part_taken_off_falls_and_comes_to_rest_exactly_within_its_bound` (a scratch print of the tick) | 46 ticks; bound 300 | 2026-10-20, P4 |
| WebAssembly size | printed by `./scripts/build.sh` | 1 768 KB (M6: 1 531 KB) | 2026-10-20, P4 |
| Flicker: a frozen view after a part is taken off (the stump, the piece, the dropped cleaver, blood on the ground) | `tests/browser/interactions.py` (`?duel=orc&foe=still&act=wounds.take_off:forearm_r&act=wounds.cut:torso,14&act=wounds.freeze:420`) | see the check's result in `HANDOFF.md` | 2026-10-20, P4 |

## M7 polish, menus, shop, models and the flicker audit (task P3, qa)

| What | Command | Result | When |
| --- | --- | --- | --- |
| Near pop-in (T8), before P2's fix: frames of 480 in which a lit shape within 25 m, in view in both frames, appears or disappears without a dither hand-over; the story held still | `cargo test --release -p tool_core --test popin_probe -- --ignored --nocapture` | the village: walking straight 2, running 4, a whole turn on the spot 0; the meadow: 2, 4, 0; the camp's edge: 2, 4, 0. Every pop is a whole grass chunk coming in 6 to 9 m ahead (one instance, a mesh the detail levels built); nothing near goes (R9.97) | 2026-10-20, P3 |
| What the Equipment figure costs | the browser check's figure canvas; `stage::frame` | one more WebGL context, drawn only while the ring is open at Equipment (the world behind holds still); its canvas about 230 × 430 CSS pixels; the static meshes uploaded to it once | 2026-10-20, P3 |
| The screens' layout at 1280 × 720, 1920 × 1080 and 2560 × 1440 (words off the window, cut, hidden or overlapping; key caps over words; the keyboard's focus ring) | `.venv/bin/python tools/m7qa_shots.py shots/m7qa/sizes` (Chromium on Metal); the browser check's `menus_m7` | before: 251 findings (title and pause off the window at 1280 × 720, "Mouse, sword drawn" over its label, the imbue ring's words touching, the turn button over the Equipment set panel); after: 0 in Chromium, Firefox and WebKit | 2026-10-20, P3 |

## M7 feel (task P1)

Sound, the camera's boom, the burning houses, the staged practice, the spark, 2026-10-20, Apple M2 Max.

| What | Command | Result |
| --- | --- | --- |
| Frames in the raid, waking with two houses burning and sound on, Chromium on Metal, 1920 × 1080 | `shots/m7feel/tools/feel_measure.py` (shoot.py's `frame_times` and `CALLBACK_TIMES`, not committed) | 60.0 fps; frame intervals median 16.7 ms, 95th percentile 16.8, worst 16.8; the page's frame callback median 3.5 ms, 95th 4.5, worst 5.3 |
| Running west out of the door, sound on | same | 60.0 fps; callback median 3.2 ms, 95th 4.2, worst 5.1 |
| A test duel, drawn, sound on | same | 60.0 fps; callback median 2.4 ms, 95th 2.8, worst 3.3 |
| The world's places as M2 measured them, with M7 merged | `tools/shoot.py OUT standing --measure` | 60.0 fps everywhere; village standing callback median 3.9 ms (worst 5.8), plains running 3.3 (5.3), horizon turning 3.6 (4.6) |
| The core's tick in the raid, 1 200 ticks walking then running west, native release | `cargo test --release -p tool_core --test sound_probe probe_feel -- --ignored --nocapture` | 0.66 to 0.95 ms mean a tick across runs (the same run with sound and the boom left out of the tick: 0.42 ms); sound's listening 0.001 ms a tick, the boom 0.08 to 0.13 ms; the boom's worst tick 6.5 ms, when it builds a scatter chunk's trees (one tick in about 128 m walked, R9.20); the worst tick 43 ms, a dream's wake (17 ms rebuilding the opening, `probe_opening_build_cost`) and the near chunks built at once, under the dark veil |
| Making every voice | `probe_the_mix` | 29 voices, 688 400 samples (31 s at 22 050 a second), 26 to 29 ms natively, once, on the reader's first click |
| The mix, offline | `probe_the_mix` (writes `shots/m7feel/sound-mix.wav`) | 31 s: the opening, the run west, a duel, then Esc. Peak 0.38 (−8.5 dBFS) before the limiter, 0 of 682 620 samples over full scale; loudness by second −33 to −36 dBFS RMS in the opening (wind and fire, steps, shouts), −25 to −29 in the duel; nothing within half a second of Esc. Started: 131 steps on grass, 25 on the house's floor, 6 shouts, 11 hits, 9 swishes, 1 clash, 1 draw, 1 tick |
| Moving shapes on the opening's first frame | `probe_feel_tick_cost` | 1 553 with both houses burning (each fire 9 tongues of two cones, a glow, 6 embers of two shapes, 9 smoke puffs: 54 shapes) |
| WebAssembly size | printed by `./scripts/build.sh` | 1 820 KB (from 1 531 KB at M6b; with M5 merged in between) |
| Flicker: the village green's slow pan | `tests/browser/interactions.py` | 23 of 230 400 pixels in Chromium (as at M5, R7.9), after the smoke was made opaque and round (dithered or faceted smoke made it 25, R9.9) |

Not measured: the voices' making in WebAssembly (only natively), and the page's own audio cost (Web Audio runs off the page's thread).
