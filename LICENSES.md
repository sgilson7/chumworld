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

A file with no licence, a likeness, a recording or any third-party picture is a worklist row in `SECOND-ORDER.md` for the person to decide, and stays out of git until they do.
