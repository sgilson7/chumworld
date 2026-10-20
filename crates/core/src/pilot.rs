//! `pilot`: owned by M1 Duel (task D) in PLAN.md. Empty until then.
//!
//! Behaviour trees read from `data/pilots.json`: each enemy's tree looks at
//! the world as it was a reaction delay ago and returns that tick's
//! `input::Input`, the record the player's keys and mouse make (CW-3). Steps
//! at 2 in `World::step`.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.
