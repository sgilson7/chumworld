//! `mesh`: owned by M3 Models (task A) in PLAN.md. Empty until then.
//!
//! Faceted primitives beyond `prim.rs`'s basic set. A new shape is a variant
//! appended to `prim::Mesh` and `Mesh::ALL`, with an arm in `prim::vertices`
//! that calls a builder here.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.
