# Licences

Every file in this repository that is not code written for it: its source, its author, its terms. The code is under the repository's licence.

## Assets

| Path | Source | Terms |
| --- | --- | --- |
| `web/art/` | drawn by `tools/art/art.py` from TikZ in this repository | this repository's licence |

## Vendored code

| Path | Project and version | Author | Licence | Checksum |
| --- | --- | --- | --- | --- |

## Code taken from other repositories

| From | What | Licence |
| --- | --- | --- |
| Builder's realtime kit (`builder-setting/kits/realtime/`, copied 2026-10-20; the clone has no git history, so each file's SHA-256 is in `BUILDER-SPEC.md`) | `crates/core/src/fx.rs`, `rng.rs`, `ticks.rs`, `web/input.js`, copied whole, then changed as `BUILDER-SPEC.md`'s change table says | MIT, Sam Gilson |
| Long Time Comin' (`/Users/samgilson/Documents/LongTimeComin`, commit `a319161`, 2026-10-19), same author | the instanced WebGL 2 renderer's shape (`web/gl.js`: one draw per mesh and pass, the GL state per pass, the software-renderer test); `render.rs`'s instance layout, passes, ink hull (`inflate`) and the shader biases for ink and halos; `prim.rs`'s primitive meshes (from `mesh.rs`); `geom.rs` (vectors, affine, `Mat4::look`, `project`); `flicker.rs`'s coplanar-face scan (faces, clipping, `inside`, `Seen::apart`); `fx.rs`'s `deg`, `wrap`, `atan2`, `lerp`, `lerp_angle`, `approach`; the clock-to-alpha interpolation and the pause on a lost pointer lock (`game.rs`, `app.js`) | MIT, Sam Gilson |
| Iron Vector (`/Users/samgilson/Documents/AC6Browser`, commit `6890d66`, 2026-10-09), same author | `Mat4::perspective` in i64 for a kilometres-away far plane; the 40 cm near plane; haze into the sky's colour | MIT, Sam Gilson |
| Protection of the Spheres (`/Users/samgilson/Documents/OGDota`, commit `a231703`, 2026-10-19), same author | the lighting in `data/shaders/scene.frag` (light wrapped past the terminator, a ramp, a sky fill above and a ground bounce below; its `data/presentation.json` values as a start for `data/look.json`); every shader constant as a uniform; the plum ink colour | MIT, Sam Gilson |
| Extremely Lifelike Fishing (`/Users/samgilson/Documents/FishingGame/fishing-game`, commit `fed4867`, 2026-10-19), same author (its `LICENSE`: MIT, Sam Gilson) | M2's water (CW-19, T4): `crates/core/src/water/solve.rs`, its `river/solve.rs` copied whole (SHA-256 `43fd3a9c8d21a92c80fdf1ec2b1fadeb5599c1465f64423ace842b34134db592`), the standard step method and its tests; in `water.rs`, `River::field` after its `river/bake.rs` `field` (SHA-256 `fffd2628…5878`), the river's stations, solve and rocks after its `river.rs` (`0b39ff73…e75`), the Gerstner waves' phases, `period` and `step_for_period` after its `sea.rs` (`088ec583…39d1`), the flow map's channels after its `flowmap.rs` (`36b69488…3688`), carried per vertex instead of in a texture; `data/shaders/water.vert` and `water.frag` after its `water.vert` (`91cbb07d…cb61b`) and `water.frag` (`65a0bf24…d088`), and `data/water.json`'s look after its `data/look.json` water values | MIT, Sam Gilson |
| Protection of the Spheres (as above), same author | M2's terrain look (`terrain.rs`): triangles split along alternating diagonals, a facet's colour the mean of its corners', a seeded shade on each corner, steep facets turning to rock, a darker forest floor; the pine of `scatter.rs` after its `tree_sun` model (a trunk and three cones stepping in) | MIT, Sam Gilson |
| Long Time Comin' (as above, commit `a319161`), same author | M6's magic (`crates/core/src/orbs.rs`): `fire`'s fan (spread and sweep across the repeats), ring (spin per repeat) and curtain's moving hole (the wave here), the curl about the vertical, a held formation's hang, swell and launch at the target (`Launch::Aim`), the volley clock (delay, repeat, every) and `data/patterns.json`'s field names, ported to 3D at duel distance; the orb's look in `draw` (an inked, partly unlit body with a sphere halo in the glow pass, faint near the lens), after its `draw.rs` `orbs` | MIT, Sam Gilson |
| Long Time Comin' (`/Users/samgilson/Documents/LongTimeComin`, commit `a319161`, its `LICENSE`: MIT, Sam Gilson), same author | An idea only, nothing copied (M7 forest, P13, T19): the layout of its faceted deer, the hart (`crates/core/src/draw/fauna.rs`): an ellipsoid body over a paler belly, legs from hip to foot, a neck to a long head. Rebuilt for the deer, rabbit and boar in `crates/core/src/models/props.rs` (`beast_model`) | MIT, Sam Gilson |

A file with no licence, a likeness, a recording or any third-party picture is a worklist row in `SECOND-ORDER.md` for the person to decide, and stays out of git until they do.
