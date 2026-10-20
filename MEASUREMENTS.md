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
