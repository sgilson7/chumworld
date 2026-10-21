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

Waiting on the person: R9.261 (the key, the numbers, the opening's hold, and the strings under "### M7 sprint (P8)" at the foot of `SECOND-ORDER.md`).

Open: R9.257 (pop-in and grass reach at speed, for P2), R9.258 (the gait at speed reads stiff), R9.260 (the merge with P2).

