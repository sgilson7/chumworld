//! `places`: owned by M2 World (task W) in PLAN.md. Empty until then.
//!
//! `data/places.json`: the village, the camp, the hut, the shrine, the road
//! and the bridge, as static groups in `World::scene`. Names stay descriptive
//! until the person gives them (CW-20).
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use crate::geom::V3;
use crate::world::World;

/// A place worth a mark on the map menu (M4 draws them): its kind as a
/// string key into `data/strings.json` (`place_<kind>`) and where it is.
/// Set by the coordinator so M4 can build the map before M2 merges;
/// M2 replaces the body with every place it lays out. The signature stays.
pub fn markers(w: &World) -> Vec<(&'static str, V3)> {
    let _ = w;
    vec![("village", crate::world::START)]
}
