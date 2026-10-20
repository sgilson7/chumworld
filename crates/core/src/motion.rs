//! `motion`: owned by M1 Duel (task D) in PLAN.md. Empty until then.
//!
//! Feet, grip and ice: takes over `player::Body::step`'s walking rule, with
//! grip × 55/100 for each speedster piece (CW-8). Steps at 3 in
//! `World::step`, from the `Input` record alone (CW-3).
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.
