//! `models`: owned by M3 Models (task A) in PLAN.md.
//!
//! `data/models.json`: every character and prop built from primitives, heads
//! about a quarter of the height (CW-17). A model is drawn as shapes pushed
//! into `render::Shapes`; it replaces `player::Body::draw`'s capsule.
//!
//! # The three doors (set by the coordinator so M1 to M4 could run apart)
//!
//! - `figure` draws a person from a `pose::Pose`. M1 calls it for every body
//!   it simulates; M3 replaces the placeholder below with the real model.
//! - `held` draws what a hand holds, from its grip to its tip. M1 calls it.
//! - `prop` adds a prop that never moves to a static `Group`. M2 calls it
//!   for every house, tent, fence and shrine it places; M3 replaces the
//!   placeholder box with the real model.
//!
//! The signatures stay; M3 may add arguments only by agreement through the
//! coordinator. Until M3 merges, these draw plain placeholders so M1 and M2
//! can see what they place.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use crate::fx::ONE;
use crate::geom::{v3, Affine, V3};
use crate::look::{rgb, Look};
use crate::pose::{Held, Kind, Pose};
use crate::prim::{Mesh, MeshTable};
use crate::render::{Group, Shapes};

/// A thing that never moves, placed by M2 (`places.rs`, `scatter.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Prop {
    House,
    Barn,
    Well,
    Fence,
    Tent,
    Campfire,
    Chest,
    Scarecrow,
    Shrine,
    Hut,
    Bridge,
    Cart,
    Bed,
    Signpost,
    HollowTree,
}

/// Placeholder: a sphere at each joint. M3 replaces the body of this
/// function, not its signature.
pub fn figure(kind: Kind, pose: &Pose, look: &Look, out: &mut Shapes) {
    let _ = kind;
    let ink = look.ink();
    let body = rgb(look.palette.body);
    let dot = |out: &mut Shapes, p: V3, r: i32| {
        out.inked(Mesh::Sphere, Affine::translate(p).then(&Affine::scale(v3(r, r, r))), body, ink);
    };
    dot(out, pose.head, ONE * 40 / 100);
    dot(out, pose.chest, ONE * 36 / 100);
    dot(out, pose.pelvis, ONE * 30 / 100);
    for s in 0..2 {
        for p in [pose.shoulder[s], pose.elbow[s], pose.hand[s], pose.hip[s], pose.knee[s], pose.foot[s]] {
            dot(out, p, ONE * 14 / 100);
        }
    }
}

/// Placeholder: nothing. M3 draws each held item along grip→tip.
pub fn held(item: Held, grip: V3, tip: V3, look: &Look, out: &mut Shapes) {
    let _ = (item, grip, tip, look, out);
}

/// Placeholder: a box of the prop's rough size. M3 replaces the body.
/// `yaw` turns it about its base, which stands at `at`.
pub fn prop(kind: Prop, at: V3, yaw: i32, look: &Look, meshes: &mut MeshTable, group: &mut Group) {
    let _ = meshes;
    let (w, h, d) = match kind {
        Prop::House | Prop::Hut => (6, 4, 5),
        Prop::Barn => (8, 5, 6),
        Prop::Tent => (3, 2, 3),
        Prop::Bridge => (3, 1, 12),
        Prop::Cart => (2, 2, 3),
        _ => (1, 1, 1),
    };
    let at = Affine::translate(at)
        .then(&Affine::rot_y(yaw))
        .then(&Affine::translate(v3(0, ONE * h / 2, 0)))
        .then(&Affine::scale(v3(ONE * w, ONE * h, ONE * d)));
    group.shapes.inked(Mesh::Cube, at, rgb(look.palette.body), look.ink());
}
