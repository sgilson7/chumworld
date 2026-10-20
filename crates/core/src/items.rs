//! `items`: owned by M4 Menus (task U) in PLAN.md. Empty until then.
//!
//! `data/items.json`: every item, its one way to be acquired and its one
//! mechanic (CW-6), the starting gear changing nothing (CW-7), the speedster
//! pieces' values pinned for 0 to 5 (CW-8).
//!
//! # The doors other milestones read (set by the coordinator)
//!
//! M1's `motion.rs` reads `speedster_worn` for its speed and grip, and
//! `held` for the blade length; M4 replaces both bodies once the inventory
//! exists. The signatures stay.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use crate::pose::Held;
use crate::world::World;

/// How many speedster pieces the player wears, 0 to 5. Placeholder: none.
pub fn speedster_worn(w: &World) -> u8 {
    let _ = w;
    0
}

/// What the player's sword hand holds. Placeholder: the starting short sword.
pub fn held(w: &World) -> Held {
    let _ = w;
    Held::ShortSword
}
