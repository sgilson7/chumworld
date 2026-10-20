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

## 2026-10-20: M3 models (task A)

Built: seven faceted primitives (`mesh.rs`, appended to `prim::Mesh`); every figure kind dressed on a `Pose` with dot eyes, catch-lights and blush (`models::figure`, and `figure_dressed` for the speedster pieces and the villagers' four looks); `rest_pose`; every held item along grip to tip (`held`, `grip_to_tip`); every prop baked to one mesh and one outline mesh per kind (`prop`, `prop_radius`); the donkey, the pickups (five speedster pieces and the chest key) and `prop_shapes` for things that move; colours and outline widths in `data/models.json`; the lineup (`act("models.lineup", …)`) and ten model views in `tools/shoot.py`. `SECOND-ORDER.md` R5.1 to R5.15 say what was decided alone.

Check: see the M3 results in the coordinator's merge; `cargo test` and the 17 M3 breaks pass on this branch. `./scripts/check.sh` fails its first step on `pose.rs`'s formatting as committed before M3 branched (R5.9); with that file formatted in the working tree only, the other steps pass.

Waiting on the person: R5.15 (the models' colours, the villagers' looks and each character's identifying prop are the agent's choices, shown in the lineup shots).

For the coordinator: door proposals R5.4 (`figure_dressed` and a speedster mask from M4), R5.5 (`grip_to_tip` for M1's tip), R5.6 (draw distances for figures and props), R5.8 (`?act=` on the page), R5.9 (format `pose.rs`).

## 2026-10-20: M1 duel (task D)

Built: fighters (the player and every foe) on feet with footing, grip and the speedster's ice (`motion.rs`, CW-8 pinned for 0 to 5 pieces); 3D Verlet arms with two-handed and one-handed holds, capped servos on the hand and the blade, committed swings, eight passes in Q16.16 (`arm.rs`); solid blades with Vagrancy's swept contact and rewind, and blades that stop on bodies (`blade.rs`); health by blade speed and weakened arms (`health.rs`, CW-16); behaviour-tree pilots for the orc, the bandit and the leader, with reaction delays, groups that ring and throw (`pilot.rs`, `data/pilots.json`, CW-3); rocks and knives with a landed-impact hook and prop boxes for M5 (`throwables.rs`); the sheathed and drawn controls, the hold on the nearest foe, the retuned camera (R3.10), health on the HUD, the test acts `duel.spawn`, `duel.reset`, `duel.pilot`, `duel.gap`, `duel.sticks`, `motion.speedster` and their address flags, the duel screenshot views in `tools/shoot.py`, and the raid probe (0.143 ms a tick with eight fighters). Rows R3.1 to R3.25 say what was decided alone; open: R3.3 (M4 must set `HudItem::value`), R3.4, R3.9, R3.13, R3.14, R3.16, R3.23.

Check: `./scripts/check.sh` passed all 9 steps in 63 s on 2026-10-20 (185 tests by `./scripts/count-tests.sh`, 3 probes ignored; three engines). Breaks: `python3 scripts/break_tests.py` over M1's rows and the eight M0 rows M1 repaired: 54 of 54 caught (after four were rewritten to bite).

