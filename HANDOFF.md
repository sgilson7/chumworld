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
