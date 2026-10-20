//! `models`: owned by M3 Models (task A) in PLAN.md. Empty until then.
//!
//! `data/models.json`: every character and prop built from primitives, heads
//! about a quarter of the height (CW-17). A model is drawn as shapes pushed
//! into `render::Shapes`; it replaces `player::Body::draw`'s capsule.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.
