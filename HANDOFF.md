# Handoff

Stage 9. Written at the end of each session.

## 2026-10-20: stage 0 only

Built: nothing yet. The template is copied and passes `./scripts/check.sh` untouched (9 steps, 38 core tests, 48 s). `BRIEF.md` holds the description word for word. The design journal at `../chumworld-journal/` holds the opening entry (Builder tab) and a reading of each point of the description with a picture (Your description tab).

Answered 2026-10-20 ("go with defaults for all", `DECISIONS.md`), so these rows are done: R1.5 (proportions, CW-Q6), R1.6 (a caster in the MVP, CW-Q10), R1.7 (names, CW-Q1), R1.8 (publishing, CW-Q2), R1.9 (saves, CW-Q3), R1.10 (where the speedster pieces are, CW-Q8), R1.13 (the other defaults, CW-Q4, CW-Q5, CW-Q7, CW-Q9, CW-Q11 to CW-Q14).

Next: `PLAN.md`, written.

## 2026-10-20: M0 foundation (task F)

Built: the core's fixed-point geometry, input record, tick clock, over-the-shoulder camera, renderer (instance array, two depth layers, six passes, camera-relative drawing), flicker scan, the player's capsule on a test field, the world's tick order with the extension contract for M1–M7 (`crates/core/src/world.rs`), 23 stub modules, the shim, the page (`web/gl.js`, `app.js`, `input.js`), the shaders, `tools/shoot.py`, and the browser's flicker checks. `SECOND-ORDER.md` R2.1 to R2.25 say what was decided alone; the open rows are R2.5, R2.9, R2.17, R2.19 (for later milestones) and R2.24, R2.25 (for the person: the strings at the foot of the notebook, and the repository's licence).

Check: `./scripts/check.sh` passed all 9 steps in 45 s on 2026-10-20 (127 core tests by `./scripts/count-tests.sh`, one probe ignored; the browser check in Chromium, Firefox and WebKit). Breaks: `python3 scripts/break_tests.py`, 99 of 99 caught on 2026-10-20.

Waiting on the person: R2.24 (the strings the agent wrote, listed at the foot of `SECOND-ORDER.md`, and the working title). R2.25 is settled: an MIT `LICENSE`.

Published by the coordinator under T5: the public repository `sgilson7/chumworld` and https://sgilson7.github.io/chumworld/.

## 2026-10-20: M4 menus (task U)

Built: `data/items.json` and `items.rs` (every MVP item, its one route and mechanic; CW-6, CW-7 and CW-8 as tests; the doors `speedster_worn`, `held`, `speedster_mask`, `speedster_speed`, `speedster_grip`, `blade_cm`), `inventory.rs` (carried, worn, the sword hand, quick items on 1, 2 and 3, coins; acts `inventory.give`, `inventory.take`, `inventory.coins`), and `ui.rs`: the menu ring (Select Item, Map, Quest, Equipment), the map's fog, the HUD, dialogue (`ui::say`), the shop (`ui::open_shop`), settings, and the settings button on the title and pause screens. The page builds them in `web/menus/`; the icons are SVG in `data/icons/` (made by `tools/icons/make_icons.py`).

Doors for other milestones: M1 replaces `ui::health` and `ui::foe` and reads `items::speedster_speed`, `speedster_grip`, `held`, `blade_cm`, `inventory::ready`, `inventory::take` and `w.ui.settings.hand_pct`; M5 calls `inventory::give`, `ui::say`, `ui::open_shop`, sets `w.inventory.knows_imbue`, and replaces `ui::task`; M6 replaces `ui::imbue`; M2's `places::markers` kinds should match R6.12.

Check (2026-10-20, after merging main at e426a3f): `./scripts/check.sh` steps 1 to 7 and 9 pass; the browser check passes in Chromium, Firefox and WebKit. Step 8 (journal) fails only because the journal does not yet name this task's person rows R6.2, R6.7 and R6.8. 184 tests by `./scripts/count-tests.sh` (2 ignored); `python3 scripts/break_tests.py "M4 "`: 39 of 39 caught.

Waiting on the person: R6.2 (CW-6 and the speedster set's shared mechanic), R6.7 (test acts reachable from the address in the published build), R6.8 (the item names, descriptions, prices and every M4 string, listed under "### M4" at the foot of `SECOND-ORDER.md`).
## 2026-10-20: M3 models (task A)

Built: seven faceted primitives (`mesh.rs`, appended to `prim::Mesh`); every figure kind dressed on a `Pose` with dot eyes, catch-lights and blush (`models::figure`, and `figure_dressed` for the speedster pieces and the villagers' four looks); `rest_pose`; every held item along grip to tip (`held`, `grip_to_tip`); every prop baked to one mesh and one outline mesh per kind (`prop`, `prop_radius`); the donkey, the pickups (five speedster pieces and the chest key) and `prop_shapes` for things that move; colours and outline widths in `data/models.json`; the lineup (`act("models.lineup", …)`) and ten model views in `tools/shoot.py`. `SECOND-ORDER.md` R5.1 to R5.15 say what was decided alone.

Check: see the M3 results in the coordinator's merge; `cargo test` and the 17 M3 breaks pass on this branch. `./scripts/check.sh` fails its first step on `pose.rs`'s formatting as committed before M3 branched (R5.9); with that file formatted in the working tree only, the other steps pass.

Waiting on the person: R5.15 (the models' colours, the villagers' looks and each character's identifying prop are the agent's choices, shown in the lineup shots).

For the coordinator: door proposals R5.4 (`figure_dressed` and a speedster mask from M4), R5.5 (`grip_to_tip` for M1's tip), R5.6 (draw distances for figures and props), R5.8 (`?act=` on the page), R5.9 (format `pose.rs`).

## 2026-10-20: M1 duel (task D)

Built: fighters (the player and every foe) on feet with footing, grip and the speedster's ice (`motion.rs`, CW-8 pinned for 0 to 5 pieces); 3D Verlet arms with two-handed and one-handed holds, capped servos on the hand and the blade, committed swings, eight passes in Q16.16 (`arm.rs`); solid blades with Vagrancy's swept contact and rewind, and blades that stop on bodies (`blade.rs`); health by blade speed and weakened arms (`health.rs`, CW-16); behaviour-tree pilots for the orc, the bandit and the leader, with reaction delays, groups that ring and throw (`pilot.rs`, `data/pilots.json`, CW-3); rocks and knives with a landed-impact hook and prop boxes for M5 (`throwables.rs`); the sheathed and drawn controls, the hold on the nearest foe, the retuned camera (R3.10), health on the HUD, the test acts `duel.spawn`, `duel.reset`, `duel.pilot`, `duel.gap`, `duel.sticks`, `motion.speedster` and their address flags, the duel screenshot views in `tools/shoot.py`, and the raid probe (0.143 ms a tick with eight fighters). Rows R3.1 to R3.25 say what was decided alone; open: R3.3 (M4 must set `HudItem::value`), R3.4, R3.9, R3.13, R3.14, R3.16, R3.23.

Merged main with M3 (models) and M4 (menus): health reaches M4's bars through `ui::health` and `ui::foe`, the player throws M4's ready quick item, motion reads `items::speedster_speed` and `speedster_grip`.

Check: `./scripts/check.sh` passed all 9 steps on 2026-10-20 after merging M4 (223 tests by `./scripts/count-tests.sh`, 3 probes ignored; three engines). Breaks: `python3 scripts/break_tests.py` over M1's rows, the eight M0 rows M1 repaired and M4's full-health row: 55 of 55 caught.


## 2026-10-20: M1 polish (task D2)

Built:
- **Bodies.** Ten capsules a body, sized to M3's figures (`blade::body`, R3.30). Bodies are kept apart capsule against capsule by an exact, mass-weighted pair shift that keeps momentum (R3.31). A guard held across the line keeps a walker off (R3.46).
- **Blades and hands.** Every weapon is rails as M3 draws it, the cleaver's offset slab included. Weapons and hands are kept out of every body, their own included, with a 15 mm skin. They are swept so they cannot tunnel, then settled where another body is near (R3.23, R3.33, R3.34, R3.51).
- **The blade's line** reads the mouse's path: overhead, side cut and thrust (R3.35).
- **Reach.** Each foe keeps a band outside its own reach, steps in to attack and steps out to recover. Bandits use short swords; foes hold a middle guard to their left (R3.37 to R3.39, R3.49).
- **Scabbard.** A scabbard on the left hip, with the draw and sheathe animated through it over 28 and 30 ticks (R3.40).
- **Feel** (`feel.rs`): hit-stop, a camera knock, sparks, stagger, all decaying to nothing (R3.41).
- **Death.** A fall onto the back with the weapon dropped and lying still. `World::player_down()` and `feel.downed` are M5's hooks (R3.42).
- **Damage** is scaled to the faster cuts (R3.44).
- **Staging.** Test duels move 30 m south, clear of the lantern (R3.43). New test act `duel.health` (R3.45). `tools/shoot.py` has eight D2 views.

Check: `./scripts/check.sh` passed all 9 steps on 2026-10-20 (three engines); 242 tests by `./scripts/count-tests.sh` (3 probes ignored; M1: 223); `python3 scripts/break_tests.py M1`: 89 of 89 caught (M1's rows and D2's). Main had not moved since the M1 merge (`git merge main`: already up to date).

Open: R3.37, R3.41 and R3.44 are first numbers for play. R3.50 needs a test that a blade pressed along a body takes no second blow.

For the coordinator:
- R3.30 copies M3's figure millimetres into `blade.rs`, and R3.40 copies M3's blade widths into the scabbard. A change to M3's sizes needs these changed too; a door from M3 giving the sizes would remove the copy.
- The block view's spark is not caught in a still from the browser; the spark is covered by `blades_that_meet_throw_a_spark_that_burns_out`.
## 2026-10-20: M2 world (task W)

Built: the land from a seed (`terrain.rs`: plains, the trainer's hill and five others, the north-east forest, ridges beyond the square to the 16 km horizon cut by the river's valley; the village plateau at height 0; roads, lanes, trails and every footprint flattened; Protection of the Spheres' faceted colours), drawn as a quadtree of tiles by detail level with dithered hand-overs and built ahead of the camera (`lod.rs`), the river solved with the fishing game's step method and drawn with its water look, flow round the bridge's piers and the shrine's islet, and the village pond (`water.rs`, `water/solve.rs`, `data/shaders/water.*`), the haze made whole before the far plane and tinted over the middle distance (`sky.rs`), trees, rocks, grass and flowers by detail level (`scatter.rs`), and the places from `data/places.json` through M3's `models::prop` (`places.rs`). `field.rs` is gone. `SECOND-ORDER.md` R4.1 to R4.24 say what was decided alone.

Doors filled: `places::markers` (seven places, keys `place_<kind>`, the hollow tree as M4's `place_tree`), `terrain::is_water`; `places::road`, `roads`, `at`, `prop_spots` for M5; `?go=` on the address (`places.go`, R4.15).

Waiting on the person: R4.24 (the water's, trees' and land's colours and the mountains are mine, tuned by eye; whether the water is as lifelike as the fishing game's is the person's call).

For the coordinator: R4.4 (files outside M2's list that M2 had to add to), R4.6 (about 1.3 M triangles resident, 140 MB of vertex buffers), R4.7 (about 600 draws a frame), R4.14 (prop sizes assumed), R4.15 (`?go=` for M5 to keep or gate).

Check, 2026-10-20 after merging M1, M3 and M4: `./scripts/check.sh` steps 1 to 7 pass (format, lint, 254 core tests by `./scripts/count-tests.sh` with 7 probes ignored, the release build, the import audit, art, the notebook's form); step 8 (the journal) fails only because R4.24 has status *person* and the journal, which M2 does not edit, does not name it yet; step 9, the browser check, passes in Chromium, Firefox and WebKit run on its own on the same build. Breaks: `python3 scripts/break_tests.py M2`, 31 of 31 caught, and M0's pass-order row repointed for `Pass::Water`, caught; the four `field.rs` rows went with `field.rs`.

## 2026-10-20: M1 polish (task D2), after M2 merged

Merged main at d07518d (M2 World). The conflicts in `world.rs` (land, water and arrival beside feel), `tools/shoot.py` (D2 and M2 views), `tests/breaks.json` (D2's rows kept as repaired, M2's 31 new rows, M2's one repair to a shared row, the M1 row removed in R3.50 left out: 276 rows), and the notes, are all kept on both sides. The shove test's fixture stood on the map's corner with a drawn orc; it now stands at the duel spot on level ground with the orc's blade put away (R3.52). The scabbard hangs from the belt, 40° back, tapered with a chape (R3.53). Held on a foe the view moves 0.4 m right (R3.54). Fence rounds 8 to 16 (R3.33). New test act `duel.guard` (R3.54).

Check: `./scripts/check.sh` passed all 9 steps on 2026-10-20 after the merge (three engines); 275 tests by `./scripts/count-tests.sh` (7 probes ignored); `python3 scripts/break_tests.py M1`: 91 of 91 caught (with the scabbard row repaired, R3.55).

## 2026-10-20: M6 magic (task M)

Built: `orbs.rs` (Long Time Comin's volleys in 3D: fans with sweep, rings that hang and close or fly past, spirals, waves with a moving hole, curl; six kinds of orb, each its own shape and colour, CW-10; orbs hit the player's body, stop on the ground and on `throwables.props`, are spent after 7 s, 60 m or 4 m past the player; an imbued edge turns one orb at a time, reflected about the edge's direction of travel, with a 45° aim help toward the caster; a turned orb takes the turned colour, flies 40% faster and hits the foes' side, its caster included), `orbs/shapes.rs` (three meshes appended to `prim::Mesh`: `Tetra`, `Star`, `Torus`), `imbue.rs` (Q when taught and ready: 600 ticks on, cooldown 1,200 from the cast, CW-9; the blade's glow, rising over 10 ticks and pulsing in its last 2 s), `data/orbs.json` (kinds and both patterns). `ui::imbue`'s body now reads `imbue::status`. The title lists Q. A turn uses M1's `feel.rs`: its spark, a hit-stop and a camera knock; the orbs hold still during a hit-stop. Rows R8.1 to R8.16.

Doors: `orbs::cast`, `deflected`, `flying` and `imbue::learn`, `learned`, `on` filled as set on main. New for M5: `imbue::lend(w, ticks)` (the trainer imbues the player's blade for a practice round; no cooldown, works before `learn`; `lend(w, 0)` takes it back), `imbue::ready`, `status`, `left`; `orbs::hands` (where a caster's volley leaves). `flying` holds from the cast until the pattern's last orb is spent (R8.6). Test acts: `magic.leader`, `magic.practice`, `magic.freeze`, `magic.clear`, `magic.lineup`, `magic.lend`, `magic.stage_turn`, `test.imbue` (R8.14). `tools/shoot.py` has nine M6 views.

Merged main at 0f7a23a (M1 polish): conflicts in `SECOND-ORDER.md`, `tests/breaks.json` and `tools/shoot.py` kept both sides; orbs moved to M1's new `blade::capsules` and rails (`blade::rails_swept`).

Check (2026-10-20, after the merge): `./scripts/check.sh` steps 1 to 7 pass (format, lint, tests, release build, import audit, art, notebook form); step 8 (the journal) fails only because R8.15 has status *person* and the journal, which M6 does not edit, does not name it yet; step 9, the browser check run on its own on the same build, passes in Chromium, Firefox and WebKit, including the frozen volley's still frames and the thirteen controls. 306 tests by `./scripts/count-tests.sh` (9 probes ignored). `python3 scripts/break_tests.py M6`: 45 of 45 caught (two missed on the first run, R8.16; one rotted by the merge, repaired).

Waiting on the person: R8.15 (the orb kinds' colours, shapes and effects, both patterns, the turned colour and glow, the strings under "### M6" at the foot of `SECOND-ORDER.md`). Also R8.14 goes with R6.7.

For the coordinator: R8.7 (files outside M6's list; mesh ids 16 to 18 in `prim::Mesh`, so M5 appending meshes takes the next ids), R8.13 (the camp's props need boxes in `throwables.props` for orbs to stop on them), R8.4 (balance: a still player dies to the leader's pattern; first guesses).

## 2026-10-20: M6 balance (task M, branch `m6-balance`)

Built: the imbue as the answer to a volley (R8.17, R8.18): an imbued edge turns an orb within 15 cm of M1's rail, the glow drawn that wide; imbued, the player's guard lays the blade across the chest's front (`imbue::guard_line`, one line in `player.rs`); the leader's pattern retuned in `data/orbs.json`. The outcome table is a test (`the_imbue_is_the_answer_to_the_leader_s_volley`) with the coordinator's thresholds, met by a scripted defender (`orbs::defend`, also the test act `magic.defend`). The faceted orbs shade each face and tumble slowly (R8.20; meshes `Gem` 19 and `Box` 20). The leader raises its hands as each volley leaves, through its input record (R8.21, `orbs::pose`; `camp.rs` untouched). Before and after in `MEASUREMENTS.md`.

Check: `./scripts/check.sh` passed all 9 steps on 2026-10-20 (three engines, the frozen volley's still frames included); 310 tests by `./scripts/count-tests.sh` (9 probes ignored); `python3 scripts/break_tests.py M6`: 53 of 53 caught.

Waiting on the person: R8.15 still; R8.22 (the across guard is hidden behind the player's head from the camera) is open for play.
## 2026-10-20: M5 places and story (task S)

Built: the opening (`story.rs`): the game opens in bed in the player's house, drawn cut away so the room shows, while six orcs walk their rounds of the village and four villagers flee; up after 1.5 s or at a movement key; an orc that sees the player runs, draws and duels by M1's pilot; a fallen orc is followed by another; down to an orc, the view fades and the world is the opening again from the same seed, with "Huh. What a funny dream." (a second line from the third dream); 250 m from the village centre the opening ends, the orcs turn back and the quest moves on. Walls and floors for every body (`solid.rs`: buildings, fences, the player's house with its door, the bridge's deck). The merchant's cart on the road (`merchant.rs`): 1.4 m/s along a 1.6 km beat over the bridge, stopped 1,800 ticks by a throwable or the boomerang, E for a greeting and M4's shop. The bandit camp (`camp.rs`): a lookout who raises the alarm, three bandits and the leader, who casts its pattern; their drops (coins, knives, the gloves, the sword, and the key once the camp is down); the chest opens with the key and gives the boomerang. The trainer (`trainer.rs`): talk, draw, volleys of the practice pattern with the imbue lent through M6's `imbue::lend`, five orbs turned, `imbue::learn` once. The boomerang (`boomerang.rs`): an 18 m loop home to the hand in 150 ticks. E for everything (`story::nearest_use`) with a prompt, a line over the game, the fade, the quest line (`ui::task`), the pickups (`data/story.json`). Rows R7.1 to R7.22 in `SECOND-ORDER.md` say what was decided alone.

Merged: `m1-polish` while building (R7.1), then main with M1's polish and M6 Magic; the trainer uses M6's `imbue::lend`, and `imbue.rs` is M6's as merged.

Waiting on the person: R7.8 (what death after the opening does), R7.14 (where the rocks, bandage and coins lie, and what the bandits carry), R7.22 (the strings under "### M5" at the foot of `SECOND-ORDER.md`).

For the coordinator: R7.2 and R7.3 changed M2's `places.rs` (a `moving` flag) and `places.json` (the bed's place); R7.9 the browser check's first view and new test acts (with R6.7); R7.12 the practice depends on standing in front of the trainer (M6's guard rule, R8.3); R7.13 the lookout's gloves are not drawn on it; R7.11 the cart's box; R7.4 throwables and orbs still pass through buildings (R8.13).

Check, 2026-10-20 after merging main at a205e91 (M1 polish and M6): `./scripts/check.sh` steps 1 to 7 pass (format, lint, 344 core tests by `./scripts/count-tests.sh` with 10 probes ignored, the release build, the import audit, art, the notebook's form); step 8 (the journal) fails only because R7.8, R7.14 and R7.22 have status *person* and the journal, which M5 does not edit, does not name them yet; step 9, the browser check run on its own, passes in Chromium, Firefox and WebKit (Chromium's slow pan on the green at 23 of 230 400 pixels, at the bound's edge: R7.9). Breaks: `python3 scripts/break_tests.py M5`, 41 of 41 caught. Screenshots in `shots/m5/` (not committed).

## 2026-10-20: M7 polish, swings (task P5, branch `m7-swings`, T10)

The person: "the combat should be made easier to decide between horizontal and vertical swings, right now all i do are these like limp diagonal slashes".

Found (R9.160, before/after table in `MEASUREMENTS.md`): the left button only raised the servos' caps while the hand followed the mouse at 0.15° a pixel, so a cut was as long and fast as the mouse's own stroke (the short sword's tip 4.7 m/s at a slow stroke, 11 at a fast one, 100 to 108° swept); a stroke down from the guard had 36° of room and swept 21°; the blade's line kept a stroke's slant (24 to 30° off level for a stroke 20° off sideways); the arm's limb drag stopped it with the mouse; and the blade lagged the hand by over 100° in a fast sweep. The camera's pitch does not couple in.

Built: `swing.rs`. Holding the left button gathers the mouse's travel; at 40 px its direction, snapped to an axis within 30°, picks a level cut (its height by where the hand was), a chop, a rising cut or a 45° diagonal; the wheel turned away picks a thrust. A cut is a plane and an arc: a wind-up back past the start, a strike driven by a motor that turns the weapon and hand round the arc's pivot (Vagrancy's joint motor; heft slows it), a follow-through. Contact ends it. Level cuts 1° off level and about 190° swept, chops 90°, the short sword's tip 16 m/s, the cleaver's 11.6. Pilots cut the same way through their records, weighed in `data/pilots.json`. A swing cue (beads where the tip will run, the cut's name below the middle of the view; setting Swing cue, on by default). The title and pause panels say how to cut. Rows R9.160 to R9.174.

Changed numbers and others' code, each with a row: `SWING_ACCEL`, `SWING_TIP_ACCEL`, limb drag during `Goals::swing` (R9.163, shared with the sheathing, P1); `blade::rewind` moves pinned hands (R9.167); M1's block and spark tests restaged (R9.168); M1's cut test renamed (R9.169); M6's recorded-volley test perturbs a key (R9.170); the browser check lists five settings (R9.173); the fallen-orc test no longer covered ground drag along x, so a hand-worked test does (`a_point_on_the_ground_loses_a_third_of_its_slide_a_tick`).

Check (2026-10-20, after merging main at fa69349): `./scripts/check.sh` steps 1 to 7 pass (format, lint, tests, release build, import audit, art, notebook form); step 8 (the journal) fails only because R9.174 has status *person* and the journal, which P5 does not edit, does not name it yet; step 9, the browser check run on its own on the same build, passes in Chromium, Firefox and WebKit. Tests: 359 by `./scripts/count-tests.sh` (12 ignored). Breaks: `python3 scripts/break_tests.py M7` 18 of 18 caught; `M1 M6` 143 of 144 before the ground-drag repair, which is now caught. Screenshots in `shots/m7swings/` (not committed).

Waiting on the person: R9.174 (the strings under "### M7 swings (P5)" at the foot of `SECOND-ORDER.md`). Open for play: R9.163, R9.164 (blows land harder; P4's damage unchanged), R9.170 (the pilots' cut weights), R9.171 (the thrust is slow), R9.172.

## 2026-10-20: M7 travel (part P6, branch `m7-travel`, T11)

Built (`crates/core/src/travel.rs`, `data/travel.json`, `web/menus/song.js`):
- **The flute.** The trainer gives it the tick after the practice is passed, in a three-line talk whose second line teaches the travel song with its notes on the dialogue box (its one route, `trainer_gift`; CW-6's test now covers it). It sits in Select Item's fifth cell and goes on a quick key; its key plays it, it is never held ready.
- **The song screen** (`Screen::Song`, the world held still): five notes on the arrows (or W A S D) and Space, shown on a five-line staff as they sound; at six notes the phrase is judged: the travel song (↑ → ← ↑ → Space) holds, then the map lists the woken waystones; anything else fades and clears. Esc puts the flute away.
- **Five waystones** (`Prop::Waystone`, places `waystone_<id>` in `data/places.json`): by the village outside the boundary, by the bridge, on the trainer's hill, on the shrine's islet, down the trail from the camp outside the lookout's sight. E wakes one: its marks glow (no blending to sort, nothing pulses), the map marks it. A journey fades out, puts the player 4 m in front of the waystone facing out, and fades back.
- **Refusals**, each with its line: during the opening, with a foe's blade out within 32 m or a foe holding on the player, and mid-dialogue.
- **The traveller** at the crossroads with a pack mule: E, a greeting, then the waystones not yet been near, 10 coins and one more every 80 m (13 to 32); a ride takes exactly its price, refuses a short purse, says a line, fades, and wakes the waystone on arrival; a place been near is never offered.
- **Quest**: `quest_flute` from the gift until a waystone is woken; the Quest screen's Songs card shows the song's notes.
- Sound events for P1: `ui().travel_sound` (R9.202).

Rows R9.190 to R9.208 in `SECOND-ORDER.md`; the strings under "### M7 travel" at its foot.

The village's waystone stands at (2000, 2540), beside the lane north, where P7 leaves its gap (T12, R9.194).

Check (2026-10-20, after merging main at f289b59, which touched only `DECISIONS.md` and `TRIAGE.md`): `./scripts/check.sh` steps 1 to 7 pass (format, clippy, 376 tests passed and 11 ignored by `./scripts/count-tests.sh`, the release build, the import audit, art, the notebook's form). Step 8 (the journal) fails only because R9.190, R9.193, R9.194, R9.195 and R9.207 have status *person* and the journal, which P6 does not edit, does not name them. Step 9, the browser check run on its own on the same build, passes in Chromium, Firefox and WebKit, the woken waystone's still frames and slow pan included (13, 15 and 15 A-B-A pixels). `python3 scripts/break_tests.py M7`: 35 of 35 caught (one missed on the first run, its test then split, R9.198). Screenshots in `shots/m7travel/` (not committed).

Waiting on the person: R9.190 (the flute's route: the trainer's gift), R9.193 (the song's notes), R9.194 (where the waystones stand, and their look), R9.195 (the traveller: place, look, prices), R9.207 (the strings).

For the coordinator: R9.200 (the files P6 added entries to: `places::markers` for P2, `web/menus/` for P3, `models` for M3's owner), R9.202 (sound events for P1), R9.205 (four stale `field.rs` rows in `tests/breaks.json`).

## 2026-10-20: M7 polish, swings, second pass (task P5, branch `m7-swings`)

The coordinator's three fixes after the first pass (merged at 11b55e6, merged back here first): a blade trail for every fighter's cut, fading to nothing 4 ticks after it (R9.175, on the Swing cue setting); the cue as a clear thin line and an end bead (R9.176); the thrust pushed along its line at up to 15.6 m/s with the body leaning in 14° (R9.177: 12.7 m/s, 50 cm, was 5.6 m/s, 25 cm); and the middle level cut raised to shoulder height so it shows beside the head (R9.178). Open for play: R9.179 (the trail is pale on grass; the thrust still points away from the camera). Screenshots in `shots/m7swings2/` (not committed); before is `shots/m7swings/`.

Check (2026-10-20, after merging main at 11b55e6): `./scripts/check.sh` steps 1 to 7 pass; step 8 (the journal) fails only because the journal cites rows R9.190, R9.193 to R9.196, R9.206 and R9.207, which other parts' branches hold and this branch does not; step 9, the browser check run on its own, passes in Chromium, Firefox and WebKit. 362 tests by `./scripts/count-tests.sh` (12 ignored); `python3 scripts/break_tests.py M7` 25 of 25 caught.

## 2026-10-20: M7 wounds (task P4, branch `m7-wounds`, T9)

Built (`crates/core/src/wounds.rs`, `data/wounds.json`, CW-16 as rewritten):
- **Damage on each part.** A blow names the capsule it struck (`blade::Hit::seg`, and the blade's travel, `vel`); each of the ten parts keeps its damage (`health.limbs`), health still decides death. A blow on a stump counts on the part above (R9.121).
- **The flash.** The struck part goes 80% red, the rest of the body 22%, over 15 ticks to exactly nothing; per-instance colour from the core, labels by capsule on a spread pose (R9.122). The struck part flinches as drawn: a head snaps, a trunk doubles, an elbow is knocked, a knee buckles (R9.131).
- **Blood.** Faceted drops sprayed along the blade's travel, more and faster the faster the blade; splats on the ground at non-overlapping heights, a pool under the fallen; stains on the struck part; dither fades only; caps of 160 drops, 64 marks, 12 pieces. The setting Blood (on by default, M4's settings, `settings.blood`) turns it off (R9.132).
- **Parts taken off** at the joint at their share of damage (`data/wounds.json`), or at once to a blow at 12.5 m/s (16 for a head): the part falls as a rigid three-point body and lies exactly still (46 ticks for a forearm; bound 300), the stump is capped red with the bone's end and bleeds for 2.5 s; a sword hand lost drops the weapon, which settles where it falls (R9.123 to R9.126). A head off is down.
- **Moving after.** One leg: a topple, then a hop at 45% of a walk; both: on its front, crawling at 22% (R9.128). Sword arm: the weapon dropped; with the other hand whole, the draw key beside it takes it up in the left hand (the arm swapped side for side, so `arm.rs`'s swing code is untouched), weaker (R9.126). Pilots, per kind in `data/pilots.json`: orcs take theirs up when near and keep coming; bandits run; the leader goes back for its sword (R9.129). An off-hand lost: the guard at 70%.
- **The player** the same (R9.120, for the person): 160% of a foe's take-off; one-handed at 75%; the line "X · Take up your sword in your other hand"; the dream and the coming-to make the player whole.
- **Death** bleeds a pool; pieces lie until the player is 150 m off, then dither out.
- **Cues for sound** (P1): `wounds::cues_since(w, tick)`, kinds `cut`, `sever`, `gore` (R9.134).
- Test acts `wounds.cut`, `take_off`, `take_off_me`, `left_hand_me`, `freeze`, `blood` (R9.135); ten screenshot views `w-*` in `tools/shoot.py`; a browser check that a frozen view after a part is taken off draws identical frames.

Touched outside P4's own files, each a hook or an added field: `blade.rs` (`Hit::seg`, `vel`; `armed` needs a hand; the sink test counts held weapons, R9.127), `health.rs` (`limbs`, `fall`), `player.rs` (the figure through `wounds::figure`, a hook at the end of `pose_from` and of `goals`, the stoop and hop in `leaning`, a fall below 0 lies face down), `arm.rs` (one line, `wounds::sided`), `motion.rs` (`Gait::stoop`, `hop`, `lift`; `wounds::gait_want`; `motion::hop`), `pilot.rs` (conditions and moves), `world.rs`, `game.rs`, `ui.rs` (the Blood switch), `models.rs` (`figure_shadow`), `throwables.rs`, `data/pilots.json`, `data/strings.json`, `GLOSSARY.md`.

Waiting on the person: R9.120 (what losing a part does to the player), R9.136 (the take-off shares, blood's amount and colours: the gore level), R9.137 (the strings under "### M7 wounds" at the foot of `SECOND-ORDER.md`).

Open: R9.130 (a foe without an off-hand still throws), R9.131 (per-part physical reactions need the resting-blade sink fixed), R9.134 (P1 to read the cues), R9.138 (what still looks wrong).

Check, 2026-10-20, after merging main at 11b55e6 (P5 swings; conflicts in `lib.rs`, `world.rs`, `ui.rs`, `data/pilots.json`, the browser check and the notes, both sides kept: the settings list Swing cue, then Blood): `./scripts/check.sh` steps 1 to 7 pass (format, lint, tests, release build, import audit, art, notebook form); step 8 fails only on this task's person rows R9.120, R9.136 and R9.137, which the journal does not yet name. Step 9, the browser check: before the merge it passed in Chromium, Firefox and WebKit, the frozen view after a part is taken off included; after the merge it passed in Chromium alone (`BUILDER_ENGINES=chromium`), while two three-engine runs timed out waiting on the opening's line and on Play with the machine's load average at 35 to 58 (other tasks running): to rerun on a quiet machine. 380 tests by `./scripts/count-tests.sh` (13 ignored, the probes). Breaks: `python3 scripts/break_tests.py "M7 wounds"`, 30 of 30 caught; `"M7 swing"` 12 of 12 after the merge. Screenshots in `shots/m7wounds/` (not committed).

## 2026-10-20: M7 roadside (part P7, branch `m7-roadside`, T12)

The person: "make sure there is stuff to do see roads on the sides of the village you are most likely to run out of at theb eginning of the game".

Found (R9.220): the opening's data says which sides. The raid enters from the south; the four villagers flee north, north-west, north-east and west, so those are the ways a player runs out.

Built (`crates/core/src/roadside.rs`, `data/roadside.json`):
- **Placement.** Each side's stops are anchored by distance from the village centre and an offset from the side's path (R9.221). The path is the road crossing the 250 m line that way, else the villager's flee line. Each stop becomes a place `roadside_<id>` in the layout: its ground is flattened, trees keep off, its model is baked and dithered with the places' props, and its footprint is a wall.
- **North (the lane).** An overturned cart at 336 m (6 coins, a bandage), a lone tree 15 m tall at 439 m, and the villager who fled north under it.
- **North-west.** A watchtower at 300 m, 10 m tall, with a stair up to a railed platform. The villager who fled that way at 469 m. An overturned cart at 640 m (3 rocks, 8 coins).
- **North-east.** A well at 311 m (E: health whole again). A burning cart at 446 m with a 23 m column of smoke (2 knives). An orc straggler beside the cart, the first real duel outside the dream; it stands only once the opening is over. The villager who fled that way at 622 m.
- **West (no road).** A tor to climb at 316 m, with rocks at its foot. An overturned cart at 455 m (6 coins, 3 rocks). The villager who fled west at 601 m.
- **Doing.** E searches a cart once; what the player cannot carry is left beside it. E at the well drinks. E at a villager gives two lines pointing the way: the crossroads and the merchant, the bridge and the trainer, the camp. The watchtower and the tor are climbed through a height-aware guard (R9.226).
- **Map.** The landmarks and the well are marked on the map (R9.233).
- **Views.** `tools/shoot.py` has `ROADSIDE_VIEWS` (22 views), driven by the test acts `roadside.line`, `roadside.look` and `roadside.top` (R9.234).

Rows: R9.220 to R9.237 in `SECOND-ORDER.md`, and the strings under "### M7 roadside (P7)" at its foot. Tests: 23 in `roadside/tests.rs`, one probe in `crates/core/tests/roadside_probe.rs`. 37 rows in `tests/breaks.json` (`python3 scripts/break_tests.py "T12 roadside"`: 37 of 37 caught).

Merged main at c9819a7 (travel and swings). Kept both sides: `Prop::ALL` holds the waystone and the five roadside props, and `Use` has both parts' arms. P6's waystone at (2000, 2540) sits in the gap left for it. P6's marker-count test now adds the roadside's marks.

Waiting on the person: R9.222 (what stands where, and its look: `shots/roadside/`), R9.223 (the loot amounts, and the well healing to whole), R9.237 (the strings).

For the coordinator:
- R9.221: after P2 merges, the north-west and north-east stops move onto its roads. This was checked locally with `m7-land`'s `places.json`.
- R9.225: the files P7 added entries to.
- R9.227: the flicker scan seems to miss two flush faces that face the same way when each lies on the other box's surface.
- R9.236: T12 is not measured against the frame budget.
- R9.230: the west side has nothing past 601 m.


## 2026-10-20: M7 save (part P10, branch `m7-save`, T16)

The person: "add areas to save your game to browser / to a save file json so you stop reviving in the village, the game should auto save as well in the browser".

Built (`crates/core/src/save.rs`, `data/save.json`, `web/save.js`, `web/save.css`):
- **What a save holds** (R9.310, R9.317): progress, decided in the core. The seed, the inventory and equipment, coins, quick keys, the merchant's stock, the quest flags, woken and reached waystones, `taken` and `beaten` by stable key, `parts` (each part's own ids), the save point, the settings, the walked map and the ticks played, under format `chumworld-save`, version 1, with a checksum over the content. Nothing about the person.
- **Reading** (R9.316): every field is checked (type, range, known id, stack, CW-6 route, the flags against each other, the checksum). A refused stored save means a new game and a line; a refused file leaves the game as it was, with a line. Never half-loaded.
- **Where a part adds its state** (R9.314): a beaten foe or encounter adds its keys to `save::known_foe` and stands down with `save::beaten`/`save::lay_down`; a drop goes through `save::drop` with a key in `save::known_taken`; anything else (P11's rumours heard, quest steps, revealed marks) is one `save::Part` in `save::PARTS`. No version bump either way. Main's roadside (T12) is saved this way: carts searched (`parts.roadside`), the straggler beaten, the tor's rocks and the straggler's coins taken.
- **Save points** (R9.312): each woken waystone and two lantern posts (the crossroads, the camp trail's foot), on the map. E saves with "Saved.".
- **Autosave** (R9.315): when the opening ends, when a waystone is first woken, when the quest moves on, on entering or leaving a major area, and every two minutes outside a duel. Never mid-duel or mid-dialogue. A small Saved mark shows each one.
- **A fall after the opening** (R9.311): you come to at your last save point, keeping everything; foes beaten since the save stand again. The dream loop is unchanged.
- **The page**: `web/save.js` is the only file that names storage (one key, `chumworld.save`, every access in try/catch). Title: Continue (with a save), New game (asks first), Load save file. Pause: Download save, Load save file. `crates/core/tests/one_save.rs` replaces `no_storage.rs`.

Rows: R9.310 to R9.321 in `SECOND-ORDER.md`, strings under "### M7 save (P10)" at its foot. Tests: 14 in `save.rs`, 3 in `one_save.rs`, and the browser check's `save` section. 17 rows in `tests/breaks.json` (`python3 scripts/break_tests.py "M7 save"`).

Waiting on the person: R9.311 (what a fall costs), R9.312 (the save points and the post's look, `shots/save/`), R9.321 (the strings).

Checks (after merging main at ca463be): `./scripts/check.sh` steps 1 to 8 run by hand; 1 to 7 pass, 8 (journal) fails only on R9.311, R9.312 and R9.321, which the journal is to name. `python3 scripts/break_tests.py "M7 save"`: 17 of 17 caught. Step 9, the browser check with its new `save` section, is left to the coordinator's batched run (R9.323).
## 2026-10-20: M7 polish, menus, shop, models, flicker audit and a playthrough (task P3, qa)

Built:
- **Menus.** The Equipment screen draws the player's own model, lit, dressed in what is worn and holding the sword in hand, turning once in 16 s on a canvas of its own (`stage.rs`, R9.82). The ring's side names change when the turn ends (R9.83). Map marks stand as pins clear of the arrow (R9.85). Quick keys in the ring are described on hover and focus (R9.99). The title and the pause are laid out to fit 1280 × 720, with the keys in two columns and the pause as one card; the ring leaves room for its turn buttons (R9.86).
- **Shop and items.** The merchant does not buy key items, the sword in hand, worn pieces or quest items. Each is listed with the reason and a refusal line. Only what the merchant sold goes back on the cart (R9.80, CW-6). The purse holds 999 coins, and coins never go below none (R9.81). A bandage's key now heals (R9.84; it did nothing before).
- **Models.** The trainer carries its own sword (R9.89). The lookout wears the gloves until it drops them (R9.90, R7.13). The cart's wheels roll (R9.96, R5.14). Thin things keep a width edge-on (R9.91). Pickups float, bob and turn over a ring of light; the jacket and the trousers stand up, and the wings are thicker (R9.87). Bodies are drawn 8% broader (R9.88; the capsules are unchanged, open). The flicker scan's window is the worst step short of its far plane (R9.95, R5.2).
- **Earlier milestones' open rows.** R2.19 (value hash, R9.92), R6.13 (a glossary slip no longer stops the game, and the check names it; R9.93), R3.50 (tested, R9.94).
- **Checks and tools.**
  - `tests/browser/layout.py` checks every screen at three sizes for words and focus. Chromium, on its software renderer, checks 1280 × 720 only.
  - The browser check's flicker audit stages a still view and a slow pan at the bed, the village street, the bridge, the river, the camp, the hut, the horizon and the merchant's cart. It also covers the menus open (the figure's canvas included) and dialogue open (R9.98).
  - `crates/core/tests/popin_probe.rs` counts near pop-in (R9.97).
  - `tools/m7qa_shots.py` takes the screenshots.
  - `tools/playthrough.py` plays the published flow by keyboard and mouse (R9.102).

Check, 2026-10-20, after merging main at f86779b (swings, travel, triage T12 and T13):
- `./scripts/check.sh` steps 1 to 7 pass: format, lint, 411 core tests by `./scripts/count-tests.sh` (14 probes ignored), the release build, the import audit, art, and the notebook's form.
- Step 8 (the journal) fails only on this task's person rows R9.100, R9.101 and R9.104, which the journal does not yet name.
- Step 9, the browser check run on its own in Chromium, Firefox and WebKit, failed once: the river's slow pan in Firefox, 100 of 921 600 pixels, with the water moving under the pan. That pan is now measured, not asserted (R9.98). The flicker audit was then rerun in Firefox and passed. The rest of that run passed in all three engines.
- Breaks: `python3 scripts/break_tests.py` over this task's rows, 45 of 45 caught.
- Under the other agents' load, a full three-engine browser check took about 45 minutes, most of it in Chromium's software renderer.
- Screenshots are in `shots/m7qa/` (not committed).

Waiting on the person: R9.104 (the hood cannot be bought the first time the cart is met), R9.100 (item names capitalised inside a line: a lower-case form, or keep them) and R9.101 (the strings this task wrote, under "### M7 (P3, qa)" at the foot of `SECOND-ORDER.md`).

## 2026-10-20: M7 rumours (part P11, branch `m7-rumours`, T17)

The person: "you should also be able to talk to people to get rumors / start quest lines, so you should be able to talk to some people who point out the speedsters outfit, where to get the long sword, boomerang, imbue, etc".

Built (`crates/core/src/rumours.rs`, `data/rumours.json`):
- **Rumours.** Twelve, one for each route that matters: the five speedster's pieces, the sword (the leader's duel), the chest key and the boomerang, the imbue, the flute, the traveller and the waystones. Each names its thing's one route; a test holds it against `data/items.json` (R9.347). None is told during the opening; each is moot once its thing is in hand.
- **Who tells.** The four roadside villagers (after their own lines, the first time), the merchant (after the greeting), the traveller (after the greeting), the trainer (once the practice is passed), and three new people beside woodpiles: a woodcutter at the crossroads, a fisher by the bridge, a herder on the trainer's hill (R9.343). A talk tells the next rumour neither heard nor moot; then a closing line (R9.345).
- **Quest lines.** Speedster's armour, The leader's sword, The bandits' chest, The trainer, The waystones (R9.348), on the Quest screen with their latest rumour and progress, done ones last. A rumour that names a place marks it on the map with a question mark until walked (R9.351).
- **Saved.** `rumours::State` holds it all by stable ids (R9.344); it is the part "rumours" in `save::PARTS` and comes back from a save (R9.358).

Rows: R9.340 to R9.359; strings under "### M7 rumours (P11)" in `SECOND-ORDER.md`. Tests: 12 in `rumours/tests.rs`; 19 rows in `tests/breaks.json` (`python3 scripts/break_tests.py "T17 rumours"`).

Check (after merging main at da5ffa3): steps 1 to 8 of `./scripts/check.sh` pass but for the journal check, which fails only on this part's three person rows (R9.343, R9.353, R9.355) until the journal names them; step 9, the browser check, was left to the coordinator's batch. `python3 scripts/break_tests.py "T17 rumours"`: 19 of 19 caught.

Waiting on the person: R9.343 (the three new people, where they stand, the woodpile), R9.353 (no rumours for rocks, knives and bandages), R9.355 (the strings).

For the coordinator: R9.344 (P10 saves `rumours::State`), R9.349 (the Quest glossary entry is out of date), R9.350 (files touched, three other parts' tests updated), R9.354 (not measured), R9.357 and R9.359 (browser check not run here; a page error in the new Quest card was found by screenshot and fixed, so the batch should open Quest with a line open).
## 2026-10-20: M7 wanderers (part P9, branch `m7-wanderers`, T15)

The person: "there needs to be a bit more in the overworld in between these major areas ... plus, add more random enemies in the overworld".

Built (`crates/core/src/wanderers.rs`, `data/wanderers.json`):
- **Where.** Eight ways between the major areas (R9.280), measured where clear of the village's 700 m band (P7's), the camp's 400 m, every other place, the waystones, the trainer and the traveller (R9.283). Slots every 400 m or less, placed from the world's seed; something to meet every 500 m or less along each way (R9.281).
- **What.** 1 or 2 ambushes behind a tree or big rock beside a trail, springing at 14 m with a line; 1 or 2 bandit patrols on the east road and the trails (the merchant has the west road, R9.282); 2 to 4 held sites: orc fires (two orcs, a fire) and bandit lean-tos (a tent, a fire, a lookout and a bandit), each marked on the map once seen; orc pairs roaming a loop across or beside the way in the rest. Only orcs, bandits and the lookout's look; the trees in `pilots.json` unchanged; no captain (R9.291).
- **When.** Only after the opening. An encounter stands within 250 m if the hostile foes near the player stay at 6 or under, and is parked past 300 m: its foes leave the world (R9.284). Every foe moves through its input record (CW-3).
- **Kept.** One `Record` per encounter (`State::records`, by `id`): beaten, when, which foes are down, seen, rewarded. In the save by key (R9.287). Roaming groups come back 20 minutes after being beaten; held sites and ambushes never (R9.286).
- **Cost.** 0.55 ms a tick with the cap's six fighting, 0.07 ms with everything parked; placing (30 ms) happens at load (R9.285, `MEASUREMENTS.md`).
- **Views.** `tools/shoot.py` `WANDERER_VIEWS` (`wd-*`), driven by `wanderers.near:<kind>` and `wanderers.seen`. Screenshots in `shots/wanderers/` (not committed).

Rows: R9.280 to R9.293 in `SECOND-ORDER.md`, strings under "### M7 wanderers". Tests: 18 in `wanderers/tests.rs`, one probe (`probe_wanderers_tick_cost`). Break rows: 21, all caught.

Merged main at 88e45bd (roadside), 60a1d64 (save) and 4763a93 (feel, sprint): `lib.rs`, `world.rs`, `GLOSSARY.md` and `tests/breaks.json` kept both sides.

Waiting on the person: R9.286 (whether beaten groups come back, and after how long), R9.290 (the kinds, counts, rewards and the lean-to's look), R9.291 (a captain would need its own kind and name), R9.293 (the strings).

For the coordinator:
- `./scripts/check.sh` steps 1 to 7 pass; step 8 (journal) fails only on R9.286, R9.290, R9.291 and R9.293, the journal entry being yours; step 9 (the browser check) was not run, per the new rule. Please include in the batch: the `wd-*` views load, and a still view by a held site renders the same pixels on consecutive frames (the core's coplanar scan passes at each site).
- R9.288: wanderers appear at 250 m with no dither. R9.289: an ambush on the open hill trail is easy to see.
- R9.292: the one-line hooks outside `wanderers.rs`.

## 2026-10-20: M7 sprint (part P8, branch `m7-sprint`, T14)

The person: "you should be able to set yourself to sprinting, and if you've been sprinting for 5seconds you start to go faster and faster until you are running at triple speed".

Built (`crates/core/src/sprint.rs`, its tests in `sprint/tests.rs`):
- **The key**: a tap of Shift while moving sets a sprint on; another tap ends it. Holding Shift still runs. A tap is a short press (at most 0.2 s) after a quiet spell, counted once Shift stays up 0.1 s, so the orcs' and villagers' running never sprints (R9.250). The controls panel says "Shift, tapped · Sprint on or off, while moving".
- **The build**: 5 s at the run speed, then a smoothstep over 4 s to 3 times it, as a multiplier on `motion::RUN` and the road (15 m/s now, 21 m/s once P2's 7 m/s run lands; R9.251). The speedster's pieces add 20% of the run each on top, capped at 4 times the bare run (R9.252).
- **What ends it**: a tap, a stop, the sword drawn or a foe held on, a blow, a dodge-step, a hard turn (over 100°), falling or a leg off.
- **Feel**: steering falls from 10° a tick at the run to 2° at the top; a stop at speed skids at 30 m/s² (3.5 m from 15 m/s); uphill the build gives way; ice unchanged (R9.253). The camera pulls back up to 0.8 m and the view widens up to 6°, eased; a still view is unchanged (R9.254). No speed streaks.
- **The opening**: the build does not count until the boundary is crossed, so the escape takes as long as running (R9.255).
- **HUD**: a Sprint bar with words ("Builds in 3 s", "2.4 × run speed") under health, a chevron fill; glossary entry Sprint.
- Test acts `sprint.set` (ticks) and `sprint.off` (R9.259); five views `s-*` in `tools/shoot.py`; a cost probe `probe_sprint_frame_cost`.

Touched outside P8's own files, each a hook or an added entry: `motion.rs` (`Gait::sprint`; three hook lines, R9.260), `world.rs` (the act route; the camera's pull and widening), `ui.rs` (`HudView::sprint`), `api.rs` (a controls row), `game.rs` (glossary values), `page.rs`, `data/strings.json`, `GLOSSARY.md`, `web/menus/menus.js` and `.css` (the bar), `tests/breaks.json`, `tools/shoot.py`, `crates/core/tests/frame_probe.rs`.

- **T18 (sprint2)**: the person's numbers replace the build's above: 4 s, then 4 s to 5 times the run; the speedster's pieces multiply it, to 10 times the bare run (70 m/s) with five, the cap (R9.420). Bodies meet walls in steps of at most 0.25 m, so nothing passes through at speed (R9.422); the land at 35 and 70 m/s (R9.423).

Waiting on the person: R9.421 (the stop from 70 m/s is 80 to 90 m). R9.261 is answered by T18 (R9.420).

Open: R9.257 (pop-in and grass reach at speed, for P2), R9.258 (the gait at speed reads stiff), R9.260 (the merge with P2).

Check, 2026-10-20, after merging main at 88e45bd (roadside T12, triage T17; conflicts in `lib.rs`, `world.rs`, `tools/shoot.py`, `tests/breaks.json` and the notes, both sides kept): `./scripts/check.sh` steps 1 to 7 pass (format, lint, tests: 427 core tests, the release build, the import audit, art, the notebook's form). Step 8 (the journal) fails on this part's person row R9.261 and on R9.97 to R9.105, which the journal cites from a branch not yet on main. Step 9 was not run here, by the person's rule (the coordinator's batched check): a first run before that rule failed one real check, the title's controls count (13, now 14 with the tap of Shift), which `tests/browser/interactions.py` now expects, with a check of the new row; that change is unverified in a browser. Breaks: `python3 scripts/break_tests.py "M7 sprint"`, 24 of 24 caught (one missed on the first run, its test then held Shift up long enough to count). Screenshots in `shots/sprint/` (not committed).

## 2026-10-20: M7 polish, feel (task P1, branch `m7-feel`)

Built:
- **Sound** (`sound.rs`, `sound/synth.rs`, `data/sound.json`, `web/sound.js`). Every sound is a named row of layers the core makes into samples on the reader's first click; the core lists each tick's sounds and each frame's levels, sides and loops, and the page plays them. Steps by what they land on (grass, road, the bridge's planks, the house's floor), the sword drawn and put away, a blade cutting the air (louder and higher the faster), blades meeting, a blow landing, a body falling, the imbue starting, pulsing in its last two seconds and ending, a volley leaving, an orb turned, throws and landings, the donkey when the cart is stopped, the orcs' shouts, the menus' tick, the dialogue's blip, the pickup chime, the dream's pad and the opening's end; loops for wind, the river by distance, the fires, the cart and the boomerang. The volume setting (M4's) scales everything; pause silences every loop. P4's wound sounds are one row and one `sound::cue` each (R9.1 to R9.4).
- **The camera's boom** (`boom.rs`): kept 0.45 m clear of every building, tent, fence, trunk and crown, easing in before a wall and out after it, rising over the shoulder when it is short (R9.5).
- **The player's house** hands over by the ordered dither over 24 ticks, a tick at a time (R9.11).
- **The opening**: two houses burning either side of the view from the door (flames, embers, smoke), a seventh orc in sight, a villager running past, the view lifting as you get up (R9.8, R9.9).
- **The practice** from a circle of light 8 m in front of the trainer, in steps: one orb, three, the pattern (R9.12).
- **The duel**: the spark burns 20 ticks with a white-hot core; the draw takes 20 ticks (0.33 s); the scabbard angled back was tried and put back, since it read barely shorter and shimmered (R9.6, R9.7, R9.14). Test acts `feel.freeze_spark`, `feel.stage_spark`, `trainer.mark` (R9.10).
- **The fade** eases in going dark and out lifting; the story's line drifts in (R9.15).
- `tools/shoot.py` has ten M7 views (`m7-*`); the browser check sees the sound set up after Play, a burning house held still, and a frozen spark.

- **QA's findings** (R9.105): a held world (`story.still`, `magic.freeze`) hurts no one and the leader casts nothing while held (R9.24); the camp comes in a member every 2.5 s after the alarm, at most two pressing (R9.25); the camera in a village corner is the boom's, confirmed (R9.26).

Not built here, moved by the coordinator: the hit flash (P4), the imbued guard's framing (P5).

For other tasks: R9.6 (P5: M1's fence fails for draw timings near the one chosen), R9.13 (P3: the house's door is shut and walked through), R9.16 (P3: the HUD and the fade hidden off the play screen), R9.19 (P2: the step's surface should read P2's road rule), R9.20 (a scatter chunk's trees cost 5 to 6 ms to build).

Check (2026-10-20, after merging main at ca463be and later): `./scripts/check.sh` steps 1 to 7 pass (format, lint, 488 core tests by `./scripts/count-tests.sh` with 20 probes ignored, the release build, the import audit, art, the notebook's form); step 8 (the journal) fails only because R9.17 and R9.18 have status *person* and the journal does not name them yet, and because the journal cites rows of branches not yet on main (R9.250 to R9.322). Step 9 is left to the coordinator's batched browser check (the person's rule of 2026-10-20); before that rule, run on this branch it passed in all three engines except the frozen spark in Firefox and WebKit, since fixed (R9.23) and seen still there by a pixel diff, and the village green's pan, since back at 23 (R9.7). Breaks: `python3 scripts/break_tests.py "M7 feel"` 38 of 38 caught (and `M7`, all parts, 93 of 93 before the last two merges). Screenshots in `shots/m7feel/` (not committed).

Waiting on the person: R9.17 (the sounds and the mix: `shots/m7feel/sound-mix.wav`), R9.18 (the strings under "### M7 feel" at the foot of `SECOND-ORDER.md`).

## 2026-10-20: M7 polish, the land (task P2, branch `m7-land`; T8, T12, T13)

Built:
- **Faster movement (T8, R9.40).** Walk 3 m/s, run 7 (speedster 7 to 14 m/s, CW-8 pinned again by hand); footwork drawn stays 2.2 m/s.
- **Roads out of the village (T8, T12, R9.41).** West-north-west to the merchant's road by the bridge (and on up the trainer's trail), north-east to the trail's end for the forest; 18% faster on any road, eased over half a second. Each road's profile smoothed and pinned where roads meet, a crown, a 3 m shoulder, and near, the road's own surface with ruts, a grassy crown and soft edges (R9.46).
- **Nothing near pops (T8, R9.44).** Every blade, tuft, rock and tree fades by its own distance in the shader through bands the core sends; grass whole to 28 m, then tufts to 62 to 78 m, then the meadow's clumps to 200 to 260 m (T13, R9.53), then grass shading on the ground itself; grass sinks as it fades. A probe and a browser check: no chunk within 25 m ever undrawn while running and turning. QA's pop-in probe: 0 pops in all six walks.
- **Wind (T13, R9.54).** Four travelling waves after the fishing game's water, phases wrapping a tick, bending every blade and rolling sheen and shadow over the field; held still with `story.still`.
- **The river (R9.47).** Meanders at three scales held at the bridge and the islet, banks varying with gravel bars inside the bends, about twenty boulders the flow goes round, reeds, a stony bed, a second knee-deep ford.
- **The map (R9.48).** The land's own colours where walked, forest dotted, relief in flat steps, contours, shore, roads, roofs.
- **Things in flight meet the world (R9.45).** Every prop's parts, the hollow player's house, trees and rocks; rocks glance off, knives stick, the boomerang knocks back, orbs stop; the cart's box turns with it.
- **Far forest (R9.49), camp floor (R9.51), resources (R9.52):** a skyline of crowns; the clearing blended; every detail mesh handed over indexed (121 MB of buffers for 2.37 M triangles, against 256 MB unindexed).
- QA's items: the trousers and boots moved clear of their props (a test that every pickup can be reached from every side); the camp's stuck spots did not reproduce in a plain world (the player walks out of both at 3 m/s every way); the bed's slow pan now passes (Firefox 35, WebKit 45 of 921 600, bound 92); the camp's does not (98 and 100): about 25 of those pixels are on the player's figure, about 20 on the bandits and the lookout, the rest tents' poles and tree tips, so the camp stays "measure" in QA's audit (R9.56).

Not fixed: M3's roof trim did not reproduce at 35 m (R9.50).

Waiting on the person: R9.58 (the land's, grass's, wind's, river's, roads' and map's looks are mine).

For the coordinator: R9.42 (M1's fence leaves a blade inside a body on other recordings; settle at 5 passes keeps the recorded duels), R9.46 (the coplanar scan now measures the gap where two faces overlap; P3 to review), R9.55 (frame cost on a loaded machine), R9.56 (the player's figure takes half the village's slow-pan bound).

Check, 2026-10-20, after merging main at 88e45bd and then f1945f7 (save, sprint; the sprint run at 21 m/s keeps every chunk within 25 m whole, tested), roadside, swings, travel, wounds; conflicts in `lib.rs`, `world.rs`, `motion.rs`, `game.rs`, `tests/breaks.json` and the notes, both sides kept): `./scripts/check.sh` steps 1 to 7 pass (format, lint, tests, release build, import audit, art, the notebook's form); step 8 (the journal) fails on this part's new person row R9.58 and on rows the journal cites from other parts (R9.7, R9.13, R9.17, R9.18, R9.21); step 9, the browser check, is left to the coordinator's batch (the person's rule), which should include the new near-scatter probe check and the slow pans (the village's was 17 to 18 of 230 400 in Chromium on this branch). 508 tests by `./scripts/count-tests.sh` (17 ignored); `python3 scripts/break_tests.py "M7 land"`: 41 of 41 caught.

## 2026-10-21: M7 polish, the forest (part P13, branch `m7-forest`, T19)

The person: "the bandit camp is really hard to find in the woods right now without the waypoint, and the rest of the woods are completely barren."

Built:
- **The camp's flag and smoke (R9.441, R9.445).** A 38 m lashed flagpole in the camp (`Prop::Flagpole`), its cloth in five panels turning with the wind and rippling, still under `story.still`; seen from the trail's end over every tree as drawn (tested). A soft column of ten tall faceted puffs over the fire that fades in, widens and thins away (`smoke.rs`).
- **Clearings (R9.442 to R9.444).** Four to six from the land's seed (this land has four), each with a stump and a log, clear of the camp, trails, places and each other; two hold a bandit each (wanderers' glade kinds, through inputs, saved as beaten), two hold resting deer. Wanderers keep 90 m off them.
- **Animals (R9.446, R9.447).** Deer, rabbits and boar (`animals.rs`, `data/animals.json`), grazing, walking, resting and running from the player; capped at 12 near, parked far. None can be fought.
- **Trees and floor (R9.440, R9.451, R9.453).** Broadleaves, birches and snags among the pines; logs, stumps, bushes, ferns and mushrooms; all fading per triangle. Every tree, stump, log and big rock is now solid to bodies in T18's swept steps, to things in flight and (trees) to the camera. Chunk builds cost no more than main's (5.6 ms).

Waiting on the person: R9.445 and R9.456 (the cart's and the raid's smoke), R9.446 (a boar to fight), R9.452 (the forest's looks), R9.454 (the strings under "### M7 forest (P13)"), R9.457 (the flag's size). Ideas only from Long Time Comin' (`LICENSES.md`, R9.450).

Check (2026-10-21, after merging main at 08a01f6): `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test` pass; `python3 scripts/break_tests.py "forest:"` 30 of 30 caught (two missed on the first run, their tests strengthened). The browser check was not run (the coordinator batches it, R9.455). Screenshots in `shots/forest/` (not committed).

