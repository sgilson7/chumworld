//! `imbue`: owned by M6 Magic (task M) in PLAN.md. Empty until then.
//!
//! The imbue on Q (`input::IMBUE`): 600 ticks on, 1,200 ticks of cooldown
//! counted from the cast, usable once the trainer's practice is passed (CW-9,
//! T2). Steps at 6 in `World::step`.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

// --- Doors, set by the coordinator so M5 (story) and M6 (magic) can run in
// parallel. M6 fills the bodies; the signatures stay. M5's trainer calls
// `learn` when the practice is passed.

use crate::world::World;

/// The trainer has taught the imbue (T2): Q works from now on.
pub fn learn(w: &mut World) {
    w.inventory.knows_imbue = true;
}

/// Whether the imbue has been taught.
pub fn learned(w: &World) -> bool {
    w.inventory.knows_imbue
}

/// Whether the player's blade is imbued this tick (turns orbs, CW-9).
/// Placeholder: never; M6 fills it.
pub fn on(w: &World) -> bool {
    // M5: the trainer lends the imbue through the practice (T2); M6 adds
    // the player's own imbue to this
    crate::trainer::lending(w)
}
