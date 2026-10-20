//! `orbs`: owned by M6 Magic (task M) in PLAN.md. Empty until then.
//!
//! Long Time Comin's bullet patterns: volleys of orbs in Q16.16, each kind
//! its own shape (CW-10), turned one at a time by an imbued blade. Steps at 6
//! in `World::step`; drawn with `render::Shapes::glow` halos.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

// --- Doors, set by the coordinator so M5 (story) and M6 (magic) can run in
// parallel. M6 fills the bodies; the signatures stay. M5 calls them.

use crate::world::World;

/// Which pattern a caster throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    /// the bandit leader's, cast at the player in the camp
    Leader,
    /// the trainer's practice round (T2): slower, fewer orbs, harmless
    Practice,
}

/// Fighter `caster` (`player::fighter` ids; 0 is the player) starts a
/// volley of `pattern` aimed at the player. Placeholder: does nothing.
pub fn cast(w: &mut World, caster: usize, pattern: Pattern) {
    let _ = (w, caster, pattern);
}

/// Orbs from `caster` that the player has turned with a blade since its
/// last `cast`. The trainer's practice passes on this. Placeholder: 0.
pub fn deflected(w: &World, caster: usize) -> u32 {
    let _ = (w, caster);
    0
}

/// Whether any orb from `caster` is still in flight.
pub fn flying(w: &World, caster: usize) -> bool {
    let _ = (w, caster);
    false
}
