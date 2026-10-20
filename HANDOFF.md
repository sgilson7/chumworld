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

Waiting on the person: R6.2 (CW-6 and the speedster set's shared mechanic), R6.7 (test acts reachable from the address in the published build), R6.8 (the item names, descriptions, prices and every M4 string, listed under "### M4" at the foot of `SECOND-ORDER.md`).
