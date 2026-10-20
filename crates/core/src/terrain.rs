//! `terrain`: owned by M2 World (task W) in PLAN.md. Empty until then.
//!
//! Terrain heights from a seed: answers `World::ground`, and draws the rings
//! of `render::rings()` round the camera. Replaces `field.rs`'s flat ground
//! in `World::scene`.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.
