//! `arm`: owned by M1 Duel (task D) in PLAN.md. Empty until then.
//!
//! The 3D Verlet arm: shoulder, elbow, hand, both hands pinned to the hilt,
//! eight relaxation passes a tick, a capped servo pulling the hand toward the
//! mouse's target on the reach shell (journal, "The arm"). Steps at 4 in
//! `World::step`; draws in `World::draw`.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.
