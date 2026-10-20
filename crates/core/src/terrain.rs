//! `terrain`: owned by M2 World (task W) in PLAN.md. Empty until then.
//!
//! Terrain heights from a seed: answers `World::ground`, and draws the rings
//! of `render::rings()` round the camera. Replaces `field.rs`'s flat ground
//! in `World::scene`.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use crate::fx::Fx;
use crate::world::World;

/// Whether (x, z) is under water: the map menu (M4) colours it, and M1's
/// footing may slow in it. Set by the coordinator so M4 can build the map
/// before M2 merges; M2 replaces the body. The signature stays.
pub fn is_water(w: &World, x: Fx, z: Fx) -> bool {
    let _ = (w, x, z);
    false
}
