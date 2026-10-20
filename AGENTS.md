# Instructions for a coding agent in a Builder project

`CLAUDE.md` holds the same rules for agents that read that file instead.

## Where code goes

1. Read `BRIEF.md`, `BUILDER-SPEC.md` and `GLOSSARY.md` before changing code.
2. The core (`crates/core`) owns every application decision and performs no input or output. Scripts and tools that need a rule read the core's output; they do not recompute it.
3. The shim (`crates/shim`) translates values and decides no application behaviour.
4. The page (`web/`) draws what the core returns and does not reproduce a core rule. The core describes the screen (`ui`) and takes settings, screens and flags through one function (`act`). Presentation numbers go in `data/*.json`; shaders go in `data/shaders/` and take rule values as uniforms.

## How to work

5. Run `cargo test` after each meaningful core change.
6. For each new behaviour test, add a row to `tests/breaks.json` and run `python3 scripts/break_tests.py`: the test must fail with its behaviour broken. Work expected values out by hand; never compare a value with the function under test. When a test needs a workaround to pass, the workaround is the finding: record it.
7. Keep `SECOND-ORDER.md` while you work: one row per assumption, surprise, divergence from the plan, or item the person must do, written when you notice it. It is how the person sees what you decided alone. A row that produced a rule names where the rule now lives; a row with status *person* is named in `HANDOFF.md`.
8. A scripted edit that replaces text asserts that it matched. A count in a document comes from `./scripts/count-tests.sh`.
9. Do not deploy unless `BRIEF.md`'s Autonomy paragraph says you may. Deploying is otherwise the person's decision (stage 8).
10. Before handoff, run `./scripts/check.sh` and record the result in `HANDOFF.md`.

## Words

11. Use each term in the sense `GLOSSARY.md` gives it, and never a word from its `Not:` line. A new term gets an entry before it appears in a string.
12. Reader-visible text follows `TONE.md`. A string that looks wrong is reported in `SECOND-ORDER.md`, not repaired. A string you write yourself is listed at the foot of `SECOND-ORDER.md` for the person to read. A list you generate that a reader will see (names, labels, word lists) is a worklist row for the person.

## The design journal

13. Keep the design journal unless `BRIEF.md`'s Journal section says `none`. Start it with `python3 tools/journal/new_journal.py`; it lives beside the project (by default `../<this folder>-journal/`) as its own repository and is never pushed. `tools/journal/ENTRIES.md` says what each kind of entry holds.
14. Before writing `PLAN.md`, write the journal's opening entry: read Builder and the brief back to the person, with numbered rules and what holds each, the question prefix, where you stop, and your questions. When the person answers, each rule that applies to every later session goes under "Rules agreed for this project" below, and each rule the core can enforce becomes a test.
15. Every decision put to the person, visible change, divergence from the plan and deploy gets a journal entry when it happens, citing its rows (T4, R2.3, XX-Q5). Never remove an entry. `scripts/journal_check.py` checks the form; commit in the journal's repository after each change.

## Rules agreed for this project

From the journal's opening entry (`../chumworld-journal/index.html#opening`) and the person's answers of 2026-10-20 (`DECISIONS.md`):

- CW-1 Every rule of the game lives in the core: arm physics, blade contact, enemy decisions, items, patterns, the opening, the merchant, the trainer, terrain heights, water, and the choice of detail level.
- CW-2 The core uses one fixed-point unit, Q16.16 metres (`fx.rs`), and no float.
- CW-3 An enemy (a pilot) acts only through the player's input record. No pilot moves a body directly.
- CW-4 The same seed and inputs give the same world, tick for tick; a recorded duel and a recorded raid hash the same every run.
- CW-5 The page replays one instance array per frame and holds no number; detail-level and dithering thresholds reach shaders as uniforms.
- CW-6 Every weapon and armour piece has exactly one way to be acquired and one mechanic no other item has (test over `data/items.json`).
- CW-7 The starting gear changes nothing.
- CW-8 Each speedster piece adds 20% speed and multiplies grip by 55/100; values pinned for 0 to 5 pieces.
- CW-9 The imbue lasts 600 ticks; its cooldown is 1,200 ticks counted from the cast. It works only after the trainer's practice is passed (T2).
- CW-10 Each kind of magic projectile has its own shape.
- CW-11 Each milestone is measured against the frame budget in `MEASUREMENTS.md`.
- CW-12 No request to another site, and no browser storage (CW-Q3: no save in the MVP).
- CW-13 Every visible change appears in the journal with screenshots of the running build.
- CW-14 Publishing is granted (T5): public repo `sgilson7/chumworld`, GitHub Pages, after `./scripts/check.sh` passes.
- CW-15 The camera is over the right shoulder, third person, always (T1).
- CW-16 Hits read on the body (T9, superseding CW-Q12's "keep limb cutting out" at the person's request of 2026-10-20): a hit takes health by blade speed, flashes the struck part red and sheds blood, weakens that limb, and enough damage to one limb takes it off; a body that has lost a leg crawls or hops, one that has lost its sword arm fights one-handed or flees.
- CW-17 Heads are about 25% of a character's height (CW-Q6).
- CW-18 Nothing flickers (T3): no coplanar overlapping faces, depth precision checked at every ring distance, detail changes only through the dithered hand-over, instance order stable between frames, and a browser test that a still view renders identical pixels on consecutive frames.
- CW-19 Water uses a solver ported from Extremely Lifelike Fishing (T4).
- CW-20 Names of places and people stay out of strings until the person gives them (CW-Q1); use descriptive names ("the village", "the trainer").

## Working with subagents

The main agent coordinates. Each subagent works in its own git worktree on its own branch, owns the files its task names, and edits a shared file (`api.rs`, `lib.rs`, `data/strings.json`, `GLOSSARY.md`, `tests/breaks.json`) only by adding entries. Before reporting done, a subagent runs `cargo test`, `python3 scripts/break_tests.py` for its new rows, and `./scripts/check.sh`, writes its `SECOND-ORDER.md` rows (numbered in its round, R2.x and on, as the task says), and saves screenshots of what it changed under `shots/<task>/` in its worktree (never committed). The main agent merges, reruns the check, looks at every screenshot, and writes the journal entry.

## Stopping

Write the journal's opening entry, then `PLAN.md` from the brief, and stop until the person has read it, unless `BRIEF.md`'s Autonomy paragraph approves the plan in advance. When a rule in `BUILDER-SPEC.md` seems to block the work, stop and say so in a worklist row; do not loosen a check to make it pass. When a request is refused on safety grounds, leave the feature out and say so; do not reword the request to get past the refusal.
