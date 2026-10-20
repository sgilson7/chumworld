//! `health`: owned by M1 Duel (task D) in PLAN.md. Empty until then.
//!
//! Health taken by blade speed at contact, and a hit arm's servo weakened for
//! a while (CW-16). Steps after `blade` at 4 in `World::step`; its bar goes
//! on the screen through `World::hud`.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.
