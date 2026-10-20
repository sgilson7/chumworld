# Second-order notebook

Stage 6, kept by the agent as the work happens, not afterwards. `B3 Build in self-questioning`.

The code and tests are the work; this file is the record about the work. It is where a decision the agent made alone, an assumption that turned out wrong, or something only the person can do becomes visible to the person. With the plan approved in advance, it is the main place those decisions surface before the person uses the tool. [`docs/procedure.md`](https://github.com/sgilson7/builder-setting/blob/main/docs/procedure.md#the-second-order-notebook) says why.

One row per assumption, surprise or open item. **Kind** is *divergence* (what was assumed, what turned out to be true, what changed), *finding*, or *worklist* (the person's to do). Number rows by round (R1.1, R1.2, then R2.1 after the first triage) so a later round does not renumber an earlier one. **Rule** is the rule the row produced for later sessions, and where it now lives (`CLAUDE.md`, a test, `tests/breaks.json`), or blank. **Status** is *done*, *open*, or *person*; every *person* row is named in `HANDOFF.md`. `scripts/notebook_check.py` checks the form of each row and that the notebook is not empty once `PLAN.md` holds a decision, not what the rows say.

| # | Kind | Assumed or found | What is true | What changed | Rule | Status |
| --- | --- | --- | --- | --- | --- | --- |
| R1.1 | divergence | Builder's procedure copies `template/` to a sibling of the clone. | The person asked for Builder to be cloned into CHUMWORLD with its `.git` removed. | Builder sits in `CHUMWORLD/builder-setting/` (read only), the game in `CHUMWORLD/chumworld/`, the journal in `CHUMWORLD/chumworld-journal/`, the layout of the Ape Race map workspace. | | done |
| R1.2 | finding | Vagrancy's arms could be lifted into a 3D game. | Vagrancy is 2D, side view, steered with keys per joint (I/K shoulder, J/L elbow), Verlet in Q19.12 cm. Its 3D cousin, the fishing game's `arms.rs`, is also keyed. | The arm, hinge, swept contact and input encoding are rewritten for 3D and the mouse; the servo, grips, solid blades, pilots and replay structure are ported. | | open |
| R1.3 | finding | Iron Vector has terrain LOD and dithering for its draw distance. | It has a flat ground, a 3.2 km far plane (14 km on Tethys), haze, static instance buffers and no LOD. Pursuer adds a heightfield and culling; the fishing game's `vista.rs` has near, mid and far rings. | The terrain rings and dithered hand-overs are new work with their own measurements. | | open |
| R1.4 | finding | The core's fixed point needs a second scale for a large world. | Q16.16 in metres spans ±32 km at 0.015 mm, enough for the walkable square and the 16 km horizon; Vagrancy's Q19.12 cm spans ±2.6 km. | One unit everywhere: Q16.16 metres (the realtime kit's `fx.rs`). | | open |
| R1.5 | finding | Protection of the Spheres' figures can hold a sword. | Its heads are about 40% of height; arms that short cannot reach past the head with a 1 m sword. | Default: its faces, faceting, palette and light, with heads near 25% of height (CW-Q6). | | person |
| R1.6 | finding | The MVP has no magic. | The imbue does nothing without a caster. | Default: the bandit leader casts one pattern (CW-Q10). | | person |
| R1.7 | worklist | | The game's name and every place and character name are the person's. | Placeholders in the journal; no names in strings (CW-Q1). | | person |
| R1.8 | worklist | | Publishing needs a named repository and a grant in the brief. | No repository, no push (CW-Q2). | | person |
| R1.9 | worklist | | A save between visits needs browser storage, which D17 forbids unless the brief allows it. | Default: no save (CW-Q3). | | person |
| R1.10 | worklist | | Where the five speedster pieces are found is a list I generated. | My list is in the journal (CW-Q8). | | person |
| R1.11 | finding | A kit holds the WebGL renderer. | `kits/canvas/` is 2D; the 3D games each carry a hand-written WebGL2 `gl.js` (Long Time Comin', Iron Vector). Iron Vector builds at `opt-level = 3` with LTO, against D11's size profile. | Copy Long Time Comin's instanced renderer, record it in `LICENSES.md`, and record the release profile change in `BUILDER-SPEC.md` when it is made. | | open |
| R1.12 | finding | The browser check can measure frame rate. | It runs headless, where WebGL falls back to software (Long Time Comin' measured 17 fps on SwiftShader against 60 on Metal). | Frame rate is measured on the person's machine (CW-Q13); the browser check shows loading, no off-origin request and no error. | | open |
| R1.13 | worklist | | The opening entry puts defaults to the person: controls, the opening loop, the boomerang, money, the imbue, damage, the target machine and the region size. | I build on the defaults in the journal until answered (CW-Q4, CW-Q5, CW-Q7, CW-Q9, CW-Q11 to CW-Q14). | | person |

## Strings written by the agent

Each reader-visible string the agent wrote, with its key, for the person to read.

| Key | String |
| --- | --- |
