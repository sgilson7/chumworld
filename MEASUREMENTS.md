# Measurements

Stage 4, recorded by the agent before building and again at handoff. `B2 Require step-by-step justification`.

The state measured rather than recalled, so later changes can be compared with it. A probe prints numbers and asserts loose bounds at most; record its output here with the command that produced it.

The machine: Sam's Mac, Apple M2 Max, macOS (Darwin 25.3). Frame budget (journal, `PLAN.md`): 16.7 ms a frame at 60 fps, the core's share 2 ms or less (CW-11).

| What | Command | Result | When |
| --- | --- | --- | --- |
| Toolchain | `rustc --version; wasm-bindgen --version` | rustc 1.99.0 (b940084d7 2026-09-28); wasm-bindgen 0.2.127 | 2026-10-20, M0 |
| Core tests | `./scripts/count-tests.sh` | template: 38 passed; M0: 127 passed, 0 failed, 1 ignored (the frame probe) | 2026-10-20, M0 |
| Full check | `./scripts/check.sh` | template: 9 steps passed in 48 s; M0: 9 steps passed in 45 s, three engines | 2026-10-20, M0 |
| WebAssembly size | printed by `./scripts/build.sh` | 343 KB (351 266 bytes; 100 080 gzipped at level 9); `dist/` 432 KB in all. Release profile unchanged (D11, `opt-level = "s"`) | 2026-10-20, M0 |
| Breaks caught | `python3 scripts/break_tests.py` | M0: 99 of 99 caught | 2026-10-20, M0 |
| Load to the title, Chromium on Metal, served from this machine | `.venv/bin/python tools/shoot.py OUT --measure` (`load_s`: from the navigation's first byte to `body[data-ready]`) | 0.07 s | 2026-10-20, M0 |
| Frame rate, headless Chromium on Metal (`ANGLE Metal Renderer: Apple M2 Max`), 1920 × 1080 | `tools/shoot.py OUT --measure` | 60.0 fps on the title, standing, and running in a circle; frame intervals median 16.7 ms, 95th percentile 16.8 ms, worst 16.8 ms. The display's 60 Hz caps it, so the headroom under 16.7 ms is not measured | 2026-10-20, M0 |
| Frame rate, headless Chromium on SwiftShader, 1920 × 1080 window (drawn at 960 × 540, `SOFTWARE_SCALE_PCT`) | `tools/shoot.py OUT --measure --soft` | 23 fps standing and running, 20 on the title; median 50 ms. Before the ground became four meshes (R2.16): 3.3 fps | 2026-10-20, M0 |
| Frame rate, SwiftShader, by window size | scratch script over `tools/shoot.py`'s helpers | 640 × 360: 40 fps; 960 × 540: 35; 1920 × 1080: 23 | 2026-10-20, M0 |
| What headless Firefox and WebKit draw with (the browser check's engines) | `getParameter(UNMASKED_RENDERER_WEBGL)` in each | Firefox: "Apple M1, or similar" (hardware); WebKit: "Apple GPU" (hardware); Chromium without flags: SwiftShader | 2026-10-20, M0 |
| The core's cost of one frame, native release | `cargo test --release -p tool_core --test frame_probe -- --ignored --nocapture` | 0.005 ms mean, 0.007 ms worst (ticks, camera, instance array, draw list), against the core's 2 ms | 2026-10-20, M0 |
| Start, native release | same probe | 4.6 ms (parse the data, lay out the field, build its meshes and the first static array) | 2026-10-20, M0 |
| Laying out the field and packing the static array again, native release | same probe | 4.4 ms (an origin move repacks the groups only; it does not rebuild the meshes) | 2026-10-20, M0 |
| What a frame sends | same probe | static array: 444 instances, 27 KB, uploaded once a 256 m origin cell; ground: 4 meshes, 17 568 triangles, uploaded once; moving instances a frame: 6; draws a frame: 46 | 2026-10-20, M0 |
| Flicker: still frames | `tests/browser/check.py` | two consecutive frames identical pixel for pixel in Chromium (640 × 360, software), Firefox and WebKit (1280 × 720) | 2026-10-20, M0 |
| Flicker: a slow pan, pixels that flip and flip back across three frames | `tests/browser/check.py` (look speed 25%, a fifth of a pixel a frame) | Chromium 7 of 230 400; Firefox 22 of 921 600; WebKit 19 of 921 600 (bound: 0.01%) | 2026-10-20, M0 |
| Flicker: the same pan at full look speed (0.84 px a frame at 640 wide) | scratch script, A-B-A count | before camera-relative drawing (R2.8): 1 064 of 921 600 on Metal; after: 112 on Metal, 172 of 230 400 on SwiftShader, on cone tips, far trunks and a pole's top (R2.9) | 2026-10-20, M0 |
| Depth precision | `cargo test depth_precision_holds_at_every_ring_distance` | worst step (24-bit, two layers): 2.4 mm at 128 m (near layer), 0.14 m at 16 384 m (far layer). One layer from 0.4 m would be 40 m at the horizon | 2026-10-20, M0 |

## Not measured

- The headroom under 16.7 ms on Metal: requestAnimationFrame is held to the display's 60 Hz, and the page keeps no timer of its own. A GPU timer query (`EXT_disjoint_timer_query_webgl2`) in a probe would give it.
- The WebAssembly build's own cost a frame (only the native release build's); the browser's frame rate covers it at 60 fps.
- Any machine but this Mac; Safari and Firefox with their own windows rather than headless; a phone (out of scope, `BRIEF.md`).
- Load over a network: GitHub Pages serves the same 432 KB.
