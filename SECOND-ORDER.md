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
| R1.5 | finding | Protection of the Spheres' figures can hold a sword. | Its heads are about 40% of height; arms that short cannot reach past the head with a 1 m sword. | Default: its faces, faceting, palette and light, with heads near 25% of height (CW-Q6). | | done |
| R1.6 | finding | The MVP has no magic. | The imbue does nothing without a caster. | Default: the bandit leader casts one pattern (CW-Q10). | | done |
| R1.7 | worklist | | The game's name and every place and character name are the person's. | Placeholders in the journal; no names in strings (CW-Q1). | | done |
| R1.8 | worklist | | Publishing needs a named repository and a grant in the brief. | No repository, no push (CW-Q2). | | done |
| R1.9 | worklist | | A save between visits needs browser storage, which D17 forbids unless the brief allows it. | Default: no save (CW-Q3). | | done |
| R1.10 | worklist | | Where the five speedster pieces are found is a list I generated. | My list is in the journal (CW-Q8). | | done |
| R1.11 | finding | A kit holds the WebGL renderer. | `kits/canvas/` is 2D; the 3D games each carry a hand-written WebGL2 `gl.js` (Long Time Comin', Iron Vector). Iron Vector builds at `opt-level = 3` with LTO, against D11's size profile. | Copy Long Time Comin's instanced renderer, record it in `LICENSES.md`, and record the release profile change in `BUILDER-SPEC.md` when it is made. | | open |
| R1.12 | finding | The browser check can measure frame rate. | It runs headless, where WebGL falls back to software (Long Time Comin' measured 17 fps on SwiftShader against 60 on Metal). | Frame rate is measured on the person's machine (CW-Q13); the browser check shows loading, no off-origin request and no error. | | open |
| R1.13 | worklist | | The opening entry puts defaults to the person: controls, the opening loop, the boomerang, money, the imbue, damage, the target machine and the region size. | I build on the defaults in the journal until answered (CW-Q4, CW-Q5, CW-Q7, CW-Q9, CW-Q11 to CW-Q14). | | done |
| R2.1 | divergence | The realtime kit is copied as it is. | Its `ticks.rs` holds an example `World` to replace, and its `input.js` has no wheel. | Copied `fx.rs`, `rng.rs`, `ticks.rs` and `web/input.js` (SHA-256 of each in `BUILDER-SPEC.md`). `ticks.rs` gained `alpha()`, `last()` and `rest()`, and keeps the example `World` in its tests, where the kit's six tests still run; `input.js` adds up the wheel. | `BUILDER-SPEC.md` change table; a `tests/breaks.json` row for each kit test | done |
| R2.2 | divergence | The kit's sine (Bhaskara's, within 0.002) is enough for a 3D camera. | A rotation basis built from it is up to 0.4% off unit length, so shapes and their lighting would swell and shrink as the view turns. | `fx::sin` is a Taylor series in Q32 rounded to Q16, exact to the last bit at the angles tested; `deg`, `wrap`, `atan2`, `lerp`, `lerp_angle`, `approach` and `decimal` added. | `fx.rs`: `sine_is_exact_to_the_last_bit_at_hand_computed_angles`, `a_rotation_basis_stays_unit_length_at_every_angle` | done |
| R2.3 | finding | One depth range from Iron Vector's 0.4 m near plane to a 16 km far plane is enough. | At 16 km one step of a 24-bit depth buffer is 40 m, so two surfaces 160 m apart could fight there. | Two depth layers: the far one (112 m to 16 384 m) drawn first, the depth buffer cleared, then the near one (0.4 m to 128 m); the 16 m both cover is drawn twice, so the seam has no crack. Worst step: 2.4 mm at 128 m, 0.14 m at the horizon. A static group is drawn in each layer its sphere reaches, so M2's rings need a centre and radius each. | `render.rs` `Layer`; `depth_precision_holds_at_every_ring_distance` (CW-18) | done |
| R2.4 | divergence | The far plane is 16 km. | The rings in `PLAN.md` double out to 16 384 m (2¹⁴). A Q16.16 length holds 32 767 m, so a shape 32 768 m wide overflows. | The far plane is 16 384 m; the test field's ground reaches 16 000 m from the start. | `render::FAR`, `field::HALF` | done |
| R2.5 | finding | Positions in floats are precise enough anywhere in the square. | 4 km from the world's origin a float keeps half a millimetre. | Positions reach the GPU relative to an origin snapped to a 256 m grid near the camera; the static array is repacked when the camera crosses into another cell (one frame's work). For M2: a large static array repacked at a crossing could cost a frame; a per-group origin would avoid repacking. | `render::origin_for`, `Scene::version` | open |
| R2.6 | divergence | `PLAN.md`: north up, map y = 4 km − world z, origin at the south-west corner. | Read as: world x east, y up, z south (right-handed; heading 0 faces north, −z). A map point (x, y) is the world point (x, 0, 4 km − y); the village centre is world (2000, 0, 2800). | The game opens at the village centre facing 20° west of north. M2 and M5 place things by this reading. | `geom.rs` module doc; `world::START` | done |
| R2.7 | finding | Shaders take every constant as a uniform; that is a matter of naming them. | An array's size counts as a number in the shader scan (`uniform vec4 x[4]` fails it), and so do the pass numbers a shader would compare against. | The dither table goes as a `mat4`. Each pass reaches the shaders as flags of 0 and 1 the page sets from `numbers().passes`, which also gives each pass's GL state by its constants' names, so the page knows no pass. | `look::UNIFORMS`; `render::pass_states` | done |
| R2.8 | finding | A Q16 view-projection matrix that carries the camera's translation is precise enough (Long Time Comin'). | Its entries' rounding, times positions up to 360 m from the origin, moved the body a pixel back and forth while the view turned: 1 064 pixels flipped and flipped back on a slow pan on Metal. | Camera-relative drawing: the shader subtracts the eye from each position, and the matrix holds the projection and the turn alone. The body is now steady to a hundredth of a pixel. | `render::Camera::matrix`; `the_body_holds_still_on_screen_while_the_view_turns`; the browser check's slow pan | done |
| R2.9 | finding | A browser test can pan slowly with synthetic mouse movement. | Chromium and Firefox round a synthetic `movementX` to whole pixels (WebKit keeps 0.3). One pixel turns the view 0.12°, 0.84 px a frame at 640 wide; at that speed cone tips, far trunks and a pole's top, thinner than a frame's step, pass over a pixel and leave it: 112 such pixels at 1280 × 720 on Metal. That is motion, but on thin far things it reads as crawl. | A look-speed setting the core holds (`act("look.speed", percent)`, `?look=` on the address, as the kit's `?pointer=free`), and the check pans at 25%: 7 to 22 pixels flip back, under the 0.01% bound. For M2: the detail levels should drop sub-pixel trunks and tips, and the check can then pan at full speed. | `tests/browser/interactions.py`; `camera::Aim::pct` | open |
| R2.10 | divergence | Long Time Comin's ink width (grows with a shape's longest side) fits every shape. | It gave a 25 m fence rail a 0.4 m slab of outline. | The width is held to a third of the shape's thinnest side, and to at least 1 cm. | `render::inflate` | done |
| R2.11 | finding | Rounding each tick's mouse movement to whole angle units is harmless. | Slow movement lost up to a unit a tick, so the same movement turned the view by different amounts depending on how the ticks divided it. | The aim keeps the remainder between ticks. | `camera::Aim::rest`; `slow_movement_turns_the_view_as_much_as_the_same_movement_at_once` | done |
| R2.12 | finding | Two detail levels handed over with dither fades t and −t cover every pixel once. | In integers −0 is 0, so at a fade of 0 "keep none" and "keep all" are one number. | The level coming in has fade t (0 to `ONE`), the level going out t − `ONE`; the shader's `keep` applies the same rule. | `look::keeps`, `look::hand_over`, `keep()` in `scene.frag`; `the_dither_hand_over_covers_every_pixel_once_at_every_step` | done |
| R2.13 | divergence | `PLAN.md` gives `mesh.rs` to M3. | The renderer needs its primitives now. | The mesh table is `prim.rs` (M0); M3 appends a variant there and builds it in `mesh.rs`. | `prim.rs` and `mesh.rs` module docs | done |
| R2.14 | divergence | The template's strings are a struct with two fields that refuses unknown keys. | Every later milestone adds strings, and a subagent edits shared files only by adding entries. | `data/strings.json` is a map of key to text, checked against `page::REQUIRED`. | `page.rs` | done |
| R2.15 | finding | Frame rate cannot be measured headless (R1.12). | Headless Chromium draws with Metal when launched with `--use-angle=metal --enable-gpu --ignore-gpu-blocklist` ("ANGLE Metal Renderer: Apple M2 Max"). Headless Firefox and WebKit draw with the GPU by default; only Chromium without flags uses SwiftShader. | `tools/shoot.py --measure` measures on Metal: 60 fps. | `MEASUREMENTS.md` | done |
| R2.16 | finding | Instancing a one-triangle mesh 17 500 times is cheap. | SwiftShader drew 3 frames a second whatever the window's size. Metal drew 60. | The ground is four meshes the core builds, a colour per vertex: SwiftShader 23 fps at 960 × 540, 35 in a smaller window. The vertex format carries a colour; `prim::MeshTable` holds the world's meshes after the primitives. | `prim.rs`; the Scene clause of the contract in `world.rs` | done |
| R2.17 | worklist | A software renderer holds a playable frame rate. | 23 fps at 960 × 540 on SwiftShader, where classroom machines and CI draw. MSAA and the two depth layers redraw most of the screen. | For M7: measure the software path against a budget of its own, drop MSAA or a layer on it if needed. | `MEASUREMENTS.md` | open |
| R2.18 | divergence | The release profile moves to `opt-level = 3` with LTO, as Iron Vector's did. | The core costs 5 µs a frame natively and the page holds 60 fps on Metal. | Reference default D11 stays (`opt-level = "s"`, 343 KB of WebAssembly). | `Cargo.toml` unchanged; `MEASUREMENTS.md` | done |
| R2.19 | finding | `World::hash` serves recorded duels (CW-4). | It hashes the state's debug text: stable under the pinned toolchain, but a hash pinned in a test would change with a toolchain bump or a renamed field. | For M1: a field-by-field hash before a recorded duel's hash is pinned. | `world.rs` | open |
| R2.20 | divergence | The journal draws no title screen. | M0 needs one: the loading state and a Play button. | The title is the working name over the field, the camera drifting round the start 8 m back; Play eases it in to the shoulder over 45 ticks. | `game.rs` `TITLE_*`, `ZOOM_TICKS` | done |
| R2.21 | finding | The page knows when to pause. | Pausing is a rule. | The page reports Esc, a lost pointer lock (only one that was held, after Long Time Comin's G9) and a hidden tab through `act`; the core pauses play and nothing else, and says through `ui().capture` when the page should let go of the pointer. | `game.rs` `act`; `escape_a_lost_pointer_and_a_hidden_tab_pause_play_and_only_play` | done |
| R2.22 | finding | Reading the canvas's pixels needs `preserveDrawingBuffer`. | A `requestAnimationFrame` callback queued after the game's runs in the same frame, before the buffer is shown. | The check reads pixels that way; the game's context keeps `preserveDrawingBuffer` off. | `tests/browser/interactions.py` | done |
| R2.23 | finding | Each break in `tests/breaks.json` breaks its behaviour. | Four did not on the first run: one replaced code with equivalent code, one left the hash depending on the text's length, one did not bite at the frame rates tested, and the pause test did not check that a key tapped while paused stays out of play. | Three breaks rewritten; the pause test now checks the tap. 99 rows. | `tests/breaks.json`; `a_paused_world_holds_still_and_resuming_runs_no_catch_up` | done |
| R2.24 | worklist | | The strings and the control labels at the foot of this file are mine, and the game's title is the working name (CW-Q1). | For the person to read; named in the journal's M0 entry. | | person |
| R2.25 | worklist | | The repository has no `LICENSE` file, and `LICENSES.md` says the code is under the repository's licence. The code taken from the same author's games is MIT. | The coordinator added an MIT `LICENSE` (the same author's sibling games, from which the code is taken, are MIT) before the repository went public under T5; the person may change it. | `LICENSE` | done |
| R2.26 | divergence | PLAN.md let M1 to M4 run in parallel on M0's contract alone. | M1 draws bodies that M3 models, M2 places props that M3 models, and M1's footing reads gear that M4 owns. Left alone, each would invent its own type. | The coordinator added three doors before branching: `pose.rs` (the `Pose` M1 fills and M3 draws, with `Kind` and `Held`), `models::figure`, `held` and `prop` (placeholders that M3 fills in), and `items::speedster_worn` and `items::held` (placeholders that M4 fills in). | `crates/core/src/pose.rs`, `models.rs`, `items.rs` | done |
| R2.27 | divergence | Each milestone would size its own bodies. | M1's arm lengths and M3's drawn limbs must agree, or the sword floats off the hand. A map (M4) needs places and water that M2 makes. | `pose::build` holds every kind's proportions: the player at 1.70 m with a 0.42 m head (CW-17) and long arms, the others scaled by height and girth. `places::markers` and `terrain::is_water` are placeholders that M2 fills in. | `pose.rs` tests, `tests/breaks.json` | done |
| R6.1 | divergence | Esc closes the ring and returns to play, as Tab does. | A browser takes the pointer back only inside a click or a key press, and not on Esc, so Esc back to play would leave the game running with no pointer. | Esc closes the ring, and the shop, to the pause screen, whose Resume takes the pointer. Tab, M and Space take it back as they close a screen. | `ui::escape`; `esc_closes_the_ring_to_the_pause_screen` | done |
| R6.2 | worklist | CW-6 read word for word: each armour piece has a mechanic no other item has. | The five speedster pieces share one mechanic, the set's (+20% speed, grip × 0.55 each). Read word for word, CW-6 fails for them. | The test treats a set as one mechanic: pieces of one set may share it, and nothing outside the set has it. The person may want each piece to do something of its own as well. | `items.rs` `every_weapon_and_armour_piece_has_a_mechanic_no_other_item_has` | person |
| R6.3 | divergence | Data files reach the core through `Game::new`, as the strings and the look do. | `items.json` holds rules (routes, prices, stacks, CW-8's numbers) that M1 and M5 read from free functions with no `Game` in reach, and changing `Game::new` would break the other branches. | `data/items.json` and `data/menus.json` (the map's parchment colours) are compiled into the core with `include_str!` and read once. | `items::catalogue`, `ui::menus` | done |
| R6.4 | divergence | The page draws the map from the view model. | Drawing parchment from the ground, the water and the fog is a rule, and a 250 × 250 image in JSON is 250 000 numbers. | The core draws the map as RGBA (`ui::map_pixels`); the shim gains one function, `map_pixels`, with no branch; the page puts the bytes on a canvas when the fog's version changes. | `api::map_pixels`, `crates/shim/src/lib.rs` | done |
| R6.5 | finding | M1 sends health through `World::hud`. | `HudItem` carries only text, so a bar's value and most would have to be parsed back out of words. | Doors in `ui.rs` with placeholders: `health` (full), `foe` (none), `imbue` (ready once known; M6), `task` (the opening's line; M5). The owners replace the bodies; the signatures stay. `test.hud` fills them for screenshots. | `ui.rs` doors | open |
| R6.6 | divergence | Settings are the page's. | Mouse sensitivity is the sword hand's speed, which M1 reads inside `World::step`; invert Y changes the input. | Settings sit in `World::ui.settings`: `hand_pct` for M1, `volume_pct` for M7 (no sound yet), invert Y applied where the page's movement enters the core. Look speed stays `aim.pct`. Nothing is stored (CW-12). | `ui::Settings`; `invert_y_turns_the_view_up_for_a_downward_mouse` | open |
| R6.7 | worklist | | Test-only acts (`test.give_all`, `test.hud`, `test.explore`, `test.dialogue`, `test.shop`, `inventory.give`) are reachable from the address, `?act=name:value`, as `?look=` is, so the screenshots and the browser check need no hook in the page. In the published build anyone can give themselves every item. | The person decides whether the test acts stay in the published build. | `web/app.js`; `ui::act` | person |
| R6.8 | worklist | | Item names and descriptions, prices (2, 6, 10, 60, 20, 80, 70 coins), stacks (20 rocks, 10 knives, 5 bandages), the bandage's 40 health, the map labels, the dialogue's two sample lines and every menu string are mine. | Listed under "### M4" at the foot of this file for the person to read. | `data/strings.json`, `data/items.json` | person |
| R6.9 | finding | The menus cost little in the WebAssembly. | The release module grew from 343 KB to 564 KB (the view models' serde code and the compiled-in JSON). | For M7: measure, and trim the view models if the load time suffers. | `MEASUREMENTS.md` | open |
| R6.10 | finding | The title and pause screens could take more buttons. | The M0 browser check reads `#title button` and `#paused button` as one button each, and `inner_text` sees CSS `text-transform`. | Settings and the menu's keys sit in panels of their own (`title-more`, `paused-more`); CSS orders both panels' contents into one column (`display: contents`); no heading is upper-cased by CSS where a check reads it. | `ui::panels`; `web/menus/menus.css` | done |
| R6.11 | divergence | Dialogue and the shop run over play. | Both need the pointer free and the world held, and M5 opens them from inside `World::step`, where no screen can change. | M5 calls `ui::say(w, who, lines)` or `ui::open_shop(w)`; the game opens the dialogue or shop screen after that tick, pausing the world. Dialogue text appears at 40 characters a second while the world holds. | `ui::wanted`; `dialogue_text_appears_at_a_steady_rate_and_space_reads_on` | done |
| R6.12 | finding | | The map's labels and icons for M2's places are guessed by kind (`place_village`, `place_camp`, `place_hut`, `place_shrine`, `place_bridge`, `place_tree`, `place_scarecrow`, `place_chest`). A kind M2 names otherwise shows its key as its label. | For M2 and the coordinator: match the kinds, or add strings and icons. | `data/menus.json` `place_icons`; `data/strings.json` | open |
| R6.13 | finding | | `GLOSSARY.md` now defines Health, Imbue and Cooldown. M1 or M6 may add the same terms, and a term defined twice stops the game loading. | For the coordinator at merge: keep one entry each. | `glossary.rs` `Duplicate` | open |
| R6.14 | finding | Each break in `tests/breaks.json` breaks its behaviour. | Two of 38 did not bite on the first run: a 1% colour mix rounded away, and a floor that rounds the right way at the start's heading. | Both rewritten; 39 M4 rows, all caught. | `tests/breaks.json` | done |
| R6.15 | divergence | M4's doors are `speedster_worn` and `held`. | M3 asked for the worn pieces as a bitmask, to dress the figure. | `items::speedster_mask` (HOOD 1, JACKET 2, GLOVES 4, TROUSERS 8, BOOTS 16). | `the_speedster_mask_has_a_bit_for_each_piece_worn` | done |
| R5.1 | finding | The flicker scan over the static array would catch a prop whose own parts share a plane. | `flicker::fights` skips two faces of one instance, and a prop is one baked mesh drawn as one instance, so the scan of the static array cannot see a prop's own fights. | Every prop, figure, pickup, item and the donkey is scanned part by part before baking, from near and from its farthest distance; the static-array scan still covers fights between props. | `models/tests.rs` `no_two_parts_of_any_prop_share_a_plane`, `no_two_parts_of_any_figure_share_a_plane`, `the_flicker_scan_reads_every_prop_in_the_static_array` | done |
| R5.2 | finding | `Seen { far }` makes the scan look at every pair the depth buffer cannot tell apart out to `far`. | The scan's window is `Seen::apart(far)`, but the depth step is not monotonic across the two layers: 9.7 mm at the near layer's 128 m against 2.1 mm in the far layer at 1 km. A scan with `far` past 128 m compares only pairs nearer than the far layer's step. | My tests scan each model twice, `far` = 128 m and `far` = its farthest distance. For M0 or M7: the window should be the worst step out to `far`, as `models::decal_lift_needed` computes it. | `models/tests.rs` `scan` | open |
| R5.3 | divergence | Presentation numbers in `data/` reach the core through `Game::new`, as the look does. | `figure`, `held` and `prop` take only `look`, and `game.rs` is not M3's. | `data/models.json` (colours, outline widths) is compiled into the core with `include_str!` and parsed once (`OnceLock`); the page does not fetch it. A colour change needs a rebuild. The coordinator may route it through `Game::new` later. | `models::table` | done |
| R5.4 | divergence | `Pose` carries what a figure wears. | It has no gear field, and `items::speedster_worn` gives a count, not which pieces. | Door proposal, added beside `figure`: `figure_dressed(kind, &pose, Dress { worn, variant }, look, out)`, `worn` a bitmask (`HOOD` 1, `JACKET` 2, `GLOVES` 4, `TROUSERS` 8, `BOOTS` 16). M4 needs a door that gives the mask, such as `items::speedster_mask(w) -> u8`. | `models::figure_dressed`, `each_speedster_piece_shows_on_its_own_part_of_the_body` | open |
| R5.5 | finding | M1 knows how long each drawn blade is. | Only the model knows where its point ends: hilt half, guard, blade. | `grip_to_tip(item)`: short sword 0.806 m, sword 1.138 m, knife 0.264 m, cleaver 0.75 m, boomerang 0.46 m, rock 0.12 m. M1 should put `Pose::tip` there. | `a_held_item_runs_from_its_grip_to_its_tip` | open |
| R5.6 | divergence | A model drawn at any distance keeps its marks without flicker. | `figure` has no camera, so it cannot drop its marks with distance; a mark must be proud enough wherever it is drawn. | Figures, items, pickups and the donkey hold to `FIGURE_FAR` = 1 km (marks 1 cm proud, 9.7 mm needed); props to `PROP_FAR` = 3 km (details 2.5 cm proud, 19 mm needed). M1 and M5 draw no figure past 1 km; M2 dithers a prop out or swaps a plainer one before 3 km. | `models::FIGURE_FAR`, `PROP_FAR`, `the_lift_a_mark_needs_is_four_depth_steps_at_the_worst_distance` | open |
| R5.7 | divergence | The lineup shows the props as M2 will draw them, from the baked mesh. | Meshes are uploaded once at start and `World::draw` has no mesh table; the lineup is asked for after start. | The lineup draws each prop as its parts (`prop_shapes`), the same parts the bake uses, each with its own outline instead of the baked outline mesh. A `// M3:` line in `World::scene` registering the prop meshes would let it use the bake. | `models/lineup.rs` | done |
| R5.8 | finding | `tools/shoot.py` can ask for the lineup. | The page sends acts only from buttons and `?look=`. | For model views, shoot.py serves `app.js` with one line added, `window.__act = act;` (it stops if the line it hooks onto has moved). Proposal: a generic `?act=name:value` on the page's address, as `?look=`. | `tools/shoot.py` `expose_act` | open |
| R5.9 | finding | `./scripts/check.sh` passes on the branch as given. | `pose.rs`, as committed before M1 to M4 branched, fails `cargo fmt --check` (two lines in its tests are over 110 columns). M3 may not edit it. | I left `pose.rs` alone; to run the other eight steps I formatted it in the working tree, ran the check, and restored it. The coordinator should commit `cargo fmt` on `pose.rs`. | | open |
| R5.10 | divergence | M3 adds one line and a field to `world.rs`. | The field needs its value in `World::new`, and `models.lineup` needs routing in `World::act` (the contract's Act). | Four `// M3:` additions: the field, its value, the draw line, and the `models.` route in `World::act`. | `world.rs` | done |
| R5.11 | finding | A grip and a tip place a blade fully. | They leave its roll free. The model takes the blade's width as near the world's up as it can; within about 7° of straight up or down it falls back to another axis, so a sword passing over the head turns its flat abruptly. | For M1: a roll (or a third point) in `Pose` if overhead swings show it. | `models::along` | open |
| R5.12 | finding | Villagers can differ by kind alone. | One kind, one look. | `Dress::variant` (0 to 3: a farmer with a straw hat, a woman in a headscarf and shawl, an old man with a beard and cane, a woman with a bun and a basket); `figure` draws variant 0. M5 picks a variant per villager. | `models::VILLAGER_VARIANTS` | open |
| R5.13 | finding | Twenty figures at about 2 500 to 3 200 triangles each fit the software renderer. | Each figure is 35 to 50 parts and draws its triangles twice (solid and outline): about 5 000 to 6 400 a figure. M0 measured SwiftShader at 23 fps with 17 568 ground triangles (R2.17). | Not measured with a crowd yet. Proposal for M7: a far figure (no marks, fewer parts) chosen by distance, like M2's detail levels. | `crates/core/tests/model_counts.rs` | open |
| R5.14 | finding | The cart is a prop. | It also rolls with the merchant (M5). | `Prop::Cart` bakes a parked cart; `prop_shapes(Prop::Cart, …)` draws it as 70 moving shapes; its wheels do not turn yet. `donkey` swings its legs with `step_phase`; the donkey stands `DONKEY_TO_CART` (3.2 m) ahead of the cart's base. | `models::DONKEY_TO_CART` | open |
| R5.15 | worklist | | The colours in `data/models.json`, the four villager looks and each character's identifying prop (nightcap, tusks and pauldron, hood and scarf, spyglass and feather, mantle and caster's orbs, big hat and apron, duelist's coat and eye patch) are my choices. | For the person to judge against Protection of the Spheres in the lineup shots. | | person |

## Strings written by the agent

Each reader-visible string the agent wrote, with its key, for the person to read.

| Key | String |
| --- | --- |
| `title` | CHUMWORLD (the working name, CW-Q1) |
| `loading` | Loading the world |
| `no_webgl` | This game draws with WebGL 2, which this browser has turned off or does not have. Turn on hardware acceleration in the browser's settings, or open the page in a current Chrome, Firefox or Safari. |
| `play` | Play |
| `paused` | Paused |
| `resume` | Resume |
| `controls` | Controls |
| `key_move`, `control_move` | W A S D · Walk |
| `key_run`, `control_run` | Shift · Run |
| `key_look`, `control_look` | Mouse · Look |
| `key_pause`, `control_pause` | Esc · Pause |
| `hint_pause` | Esc pauses |
| core: `look.speed` refusal | The look speed is a whole percentage, such as 100, not "…". |
| `GLOSSARY.md` | Look, Pause, Run, Walk, WebGL 2 (definitions there) |

### M4

The menus, the HUD, dialogue, the shop and settings (task U). Item routes and mechanics are ids in `data/items.json`, not reader text.

| Key | String |
| --- | --- |
| `key_menu` | Tab |
| `control_menu` | Menu |
| `key_map` | M |
| `control_map` | Map |
| `key_quick` | 1 2 3 |
| `control_quick` | Quick items |
| `select_item` | Select Item |
| `equipment` | Equipment |
| `map` | Map |
| `quest` | Quest |
| `key_q` | Q |
| `key_e` | E |
| `key_tab` | Tab |
| `key_space` | Space |
| `key_1` | 1 |
| `key_2` | 2 |
| `key_3` | 3 |
| `close` | Close |
| `next` | Next |
| `assign_keys` | Put it on a key |
| `item_short_sword` | Short sword |
| `item_short_sword_about` | Your own sword: {short_sword.blade} cm of blade, light and quick. It gives no bonus. |
| `item_sword` | Sword |
| `item_sword_about` | A full {sword.blade} cm blade. It is heavy: a hard hit on a foe's blade can knock their weapon out of their hands. |
| `item_boomerang` | Boomerang |
| `item_boomerang_about` | Thrown from the free hand, it flies out in an arc as far as {boomerang.reach} metres and comes back to you. |
| `item_rock` | Rock |
| `item_rock_about` | Thrown from the free hand. A rock that lands on the merchant's cart stops the cart. |
| `item_knife` | Throwing knife |
| `item_knife_about` | A small blade, thrown from the free hand. |
| `item_bandage` | Bandage |
| `item_bandage_about` | Wraps a wound and gives back {bandage.heals} health. |
| `item_chest_key` | Chest key |
| `item_chest_key_about` | Opens the chest at the bandit camp. |
| `item_hood` | Speedster's hood |
| `item_hood_about` | One of {speedster.pieces} pieces of the speedster's armour, worn on the head. Each piece worn adds {speedster.speed_pct}% speed and makes your footing icier. |
| `item_jacket` | Speedster's jacket |
| `item_jacket_about` | One of {speedster.pieces} pieces of the speedster's armour, worn on the body. Each piece worn adds {speedster.speed_pct}% speed and makes your footing icier. |
| `item_gloves` | Speedster's gloves |
| `item_gloves_about` | One of {speedster.pieces} pieces of the speedster's armour, worn on the hands. Each piece worn adds {speedster.speed_pct}% speed and makes your footing icier. |
| `item_trousers` | Speedster's trousers |
| `item_trousers_about` | One of {speedster.pieces} pieces of the speedster's armour, worn on the legs. Each piece worn adds {speedster.speed_pct}% speed and makes your footing icier. |
| `item_boots` | Speedster's boots |
| `item_boots_about` | One of {speedster.pieces} pieces of the speedster's armour, worn on the feet. Each piece worn adds {speedster.speed_pct}% speed and makes your footing icier. |
| `carry_of` | You carry {n} of {most}. |
| `on_key` | On key {key}. |
| `assign_hint` | Press 1, 2 or 3 to put it on that key. |
| `no_key` | It goes on no key. |
| `refuse_key` | The {item} goes on no key. |
| `refuse_wear` | The {item} is not worn: it is in Select Item. |
| `refuse_missing` | You do not carry the {item}. |
| `refuse_key_number` | The quick keys are 1, 2 and 3. |
| `slot_head` | Head |
| `slot_body` | Body |
| `slot_hands` | Hands |
| `slot_legs` | Legs |
| `slot_feet` | Feet |
| `sword_hand` | Sword hand |
| `in_hand` | In your sword hand. |
| `hold_hint` | Press Enter to hold it in your sword hand. |
| `worn_hint` | Worn. Press Enter to take it off. |
| `wear_hint` | Press Enter to wear it. |
| `not_found` | Not found yet. |
| `speedster_armour` | Speedster's armour |
| `pieces_worn` | {n} of {of} pieces worn |
| `speed` | Speed |
| `footing` | Footing |
| `percent` | {n}% |
| `you` | You |
| `map_legend` | The map fills in where you walk. |
| `walked` | {n}% of the map walked |
| `place_village` | The village |
| `place_camp` | The bandit camp |
| `place_hut` | The trainer's hut |
| `place_shrine` | The river shrine |
| `place_bridge` | The bridge |
| `place_tree` | The hollow tree |
| `place_scarecrow` | The scarecrow |
| `place_chest` | The chest |
| `imbue` | Imbue |
| `imbue_known` | Learned. Q imbues your sword for {imbue.seconds} seconds, so it can deflect magic. It is ready again {imbue.cooldown} seconds after the cast. |
| `imbue_unknown` | Not learned yet. The trainer in the hut on the hill teaches it. |
| `coins` | Coins |
| `task_heading` | What to do |
| `quest_opening` | Get out of the village. The orcs have taken it. |
| `health` | Health |
| `health_of` | {now} / {most} |
| `imbue_ready` | Ready |
| `seconds` | {n} s |
| `received` | Received: {item} |
| `item_times` | {item} × {n} |
| `who_trainer` | The trainer |
| `who_merchant` | The merchant |
| `who_villager` | A villager |
| `who_orc` | Orc |
| `who_bandit` | Bandit |
| `who_leader` | The bandit leader |
| `who_lookout` | The bandit lookout |
| `shop` | The merchant's cart |
| `for_sale` | For sale |
| `to_sell` | Yours to sell |
| `nothing_to_sell` | You have no items to sell. |
| `price` | {n} coins |
| `buy` | Buy |
| `sell` | Sell |
| `leave` | Leave |
| `refuse_not_sold` | The merchant has no {item} for sale. |
| `refuse_coins` | You need {n} more coins for the {item}. |
| `refuse_full` | You carry as many as you can hold: {n}. |
| `bought` | Bought: {item}. |
| `sold` | Sold: {item}, for {n} coins. |
| `refuse_keep` | The merchant does not buy the {item}. |
| `settings` | Settings |
| `look_speed` | Look speed |
| `mouse_sensitivity` | Mouse sensitivity |
| `invert_y` | Invert Y |
| `volume` | Volume |
| `on` | On |
| `off` | Off |
| `settings_note` | Settings last until you close the page. No sound plays yet. |
| `back` | Back |
| `sample_line_1` | You ran from the village with a short sword and no plan. Good. The running part, anyway. |
| `sample_line_2` | Before you cross blades with the bandits, learn to turn their magic aside. Stand there and watch the pattern. |
| `price_one` | {n} coin |
| `refuse_coins_one` | You need {n} more coin for the {item}. |
| `sold_one` | Sold: {item}, for {n} coin. |
| `GLOSSARY.md` | Bandage, Boomerang, Chest key, Coins, Cooldown, Equipment, Footing, Free hand, Health, Imbue, Invert Y, Look speed, Map, Menu, Mouse sensitivity, Quest, Quick item, Rock, Select Item, Settings, Shop, Short sword, Speedster's armour and its five pieces, Sword, Sword hand, Throwing knife, Volume |
### M3

| Key | String |
| --- | --- |
| core: `models.lineup` refusal | The lineup has no view "…". |
| core: `models.lineup` refusal | The lineup's turn is whole degrees, such as 40, not "…". |
| core: `models.*` refusal | Unknown action "models.…". |
| `GLOSSARY.md` | Lineup (definition there) |
