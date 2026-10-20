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
