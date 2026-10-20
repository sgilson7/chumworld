//! tool_core: every rule of the game. No input or output.
//!
//! Builder requirement: this crate performs no input or output. It takes
//! values and returns values, so `cargo test` reaches every rule without a
//! browser. `tests/boundary.rs` enforces that, along with the dependency
//! allowlist and the reference ban on floats, hash maps and clocks.
//!
//! Where to start reading: `api.rs` is the door the shim calls through,
//! `game.rs` the screens and the clock, `world.rs` the tick and the
//! extension contract every later milestone follows, `render.rs` what the
//! page draws. The modules after `// stubs` are empty, each naming the
//! milestone in PLAN.md that fills it.

pub mod api;
pub mod camera;
pub mod flicker;
pub mod fx;
pub mod game;
pub mod geom;
pub mod glossary;
pub mod input;
pub mod look;
pub mod page;
pub mod player;
pub mod pose;
pub mod prim;
pub mod render;
pub mod rng;
pub mod ticks;
pub mod world;

// stubs: M1 Duel
pub mod arm;
pub mod blade;
pub mod feel;
pub mod health;
pub mod motion;
pub mod pilot;
pub mod throwables;
// stubs: M2 World
pub mod lod;
pub mod places;
pub mod scatter;
pub mod sky;
pub mod terrain;
pub mod water;
// stubs: M3 Models
pub mod mesh;
pub mod models;
// stubs: M4 Menus
pub mod inventory;
pub mod items;
pub mod ui;
// stubs: M5 Places and story
pub mod boomerang;
pub mod camp;
pub mod merchant;
pub mod solid;
pub mod story;
pub mod trainer;
// stubs: M6 Magic
pub mod imbue;
pub mod orbs;
// M7 roadside (T12)
pub mod roadside;
