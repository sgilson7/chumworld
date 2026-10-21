//! `models`: owned by M3 Models (task A) in PLAN.md.
//!
//! Every character, held item and prop, built in code from the faceted
//! primitives (`prim.rs`, `mesh.rs`) in the style of Protection of the
//! Spheres: balls of flat faces, dot eyes with a catch-light, blush, and one
//! thing on each character that says who they are. Heads are a quarter of
//! the height (CW-17, `pose::build`), and arms long enough to carry a sword
//! over the head. Colours and outline widths are presentation numbers in
//! `data/models.json`; the shapes are here.
//!
//! # The doors (set by the coordinator so M1 to M4 could run apart)
//!
//! - `figure` draws a person from a `pose::Pose`: limbs between the joints,
//!   the torso from pelvis to chest, the head at `pose.head` facing
//!   `pose.facing`. M1 calls it for every body it simulates.
//!   `figure_dressed` is the same with worn speedster pieces and a
//!   villager's variant (`Dress`), proposed beside it (R5.4).
//! - `held` draws what a hand holds from its grip to its tip, the hilt at
//!   the grip. `grip_to_tip` says how long each item is drawn, so M1's tip
//!   can sit where the drawn blade ends.
//! - `prop` adds a thing that never moves to a static `Group`: one mesh per
//!   kind of prop (`MeshTable::add`, a colour per vertex), built once and
//!   reused, its base at `at`, turned by `yaw`. `prop_radius` gives M2 the
//!   radius its group must hold.
//! - `donkey` draws the merchant's donkey mid-stride for M5, `pickup` an
//!   item lying in the world, `prop_shapes` a prop as separate shapes (a
//!   cart that rolls, a chest that opens).
//! - `rest_pose` stands any kind at its guard, for the lineup and for
//!   anyone who needs a figure before M1's arms move it.
//!
//! # Flicker (CW-18)
//!
//! No two parts of a model share a plane facing one way, the flicker scan
//! says so for every figure and prop (`flicker_scan_*` tests), and every
//! mark laid on a surface (eyes, blush, buttons, belts, stripes) stands off
//! it by `DECAL_LIFT`, more than the depth buffer needs at `FIGURE_FAR`
//! (`decal_lift_needed`). A prop's own details stand off by `PROP_GAP`, which
//! holds to `PROP_FAR`.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`. The lineup (`models.lineup`) is
//! the module's state, draw and act.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, Affine, V3};
use crate::look::{rgb, Look, Rgb};
use crate::pose::{Held, Kind, Pose};
use crate::prim::{Mesh, MeshId, MeshTable, VERTEX_VALUES};
use crate::render::{self, Group, Pass, Shape, Shapes};

mod figure;
mod held;
mod lineup;
mod props;

pub use figure::{rest_pose, Dress, BOOTS, GLOVES, HOOD, JACKET, SPEEDSTER_ALL, TROUSERS};
pub use held::{float, grip_to_tip, kind_held, pickup, pickup_at, ring, Pickup, PHASE_TICKS};
pub use lineup::{act, draw, Lineup, Show};
pub use props::{cart_model, donkey, prop_radius, prop_shapes, DONKEY_TO_CART};
// M7 travel: the woken waystone's glow, drawn over the baked waystone
pub use props::waystone_glow;
// M7 roadside: how tall a prop stands, for the landmarks seen from the
// boundary, and the numbers its climbed props are built to
pub use props::prop_top;
pub mod props_numbers {
    pub use super::props::{
        TOR_EDGE, TOR_FOOT, TOR_RAMP_HALF, TOR_TOP, WATCH_FOOT, WATCH_HALF, WATCH_STAIR_HALF, WATCH_TOP,
        WATCH_Z,
    };
}

/// The parts each model is drawn from, for counting and testing.
pub use figure::model as figure_model;
pub use held::{model as held_model, pickup_model};
pub use props::{donkey_model, model as prop_model};

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
    // M7 travel (P6, T11): the carved waystone at each travel point
    Waystone,
    // M7 save (P10, T16): the lantern post at a save point
    SavePost,
    // M7 roadside (P7, T12): what stands beside the ways out of the village
    Wreck,
    WreckBurnt,
    Watchtower,
    Tor,
    LoneTree,
    // M7 rumours (P11, T17): the woodpile each teller of rumours stands by
    Logs,
}

impl Prop {
    pub const ALL: [Prop; 23] = [
        Prop::House,
        Prop::Barn,
        Prop::Well,
        Prop::Fence,
        Prop::Tent,
        Prop::Campfire,
        Prop::Chest,
        Prop::Scarecrow,
        Prop::Shrine,
        Prop::Hut,
        Prop::Bridge,
        Prop::Cart,
        Prop::Bed,
        Prop::Signpost,
        Prop::HollowTree,
        Prop::Waystone,
        Prop::SavePost,
        Prop::Wreck,
        Prop::WreckBurnt,
        Prop::Watchtower,
        Prop::Tor,
        Prop::LoneTree,
        Prop::Logs,
    ];
}

/// Every kind of figure, in the lineup's order.
pub const KINDS: [Kind; 8] = [
    Kind::Player,
    Kind::Orc,
    Kind::Bandit,
    Kind::Lookout,
    Kind::Leader,
    Kind::Merchant,
    Kind::Trainer,
    Kind::Villager,
];

/// How many looks a villager has (`Dress::variant`).
pub const VILLAGER_VARIANTS: u8 = 4;

/// Draws a person from a `Pose`, wearing nothing extra. M1 calls it for
/// every body it simulates.
pub fn figure(kind: Kind, pose: &Pose, look: &Look, out: &mut Shapes) {
    figure_dressed(kind, pose, Dress::default(), look, out);
}

/// The same, wearing the speedster pieces in `dress.worn` (`HOOD`, `JACKET`,
/// … bits) and, for a villager, the look `dress.variant`.
pub fn figure_dressed(kind: Kind, pose: &Pose, dress: Dress, look: &Look, out: &mut Shapes) {
    figure::model(kind, pose, dress).emit(look.ink(), out);
    figure::shadow(kind, pose, look, out);
}

/// The soft shadow `figure_dressed` lays under a figure, alone (M7's
/// `wounds::figure` draws the figure's parts itself).
pub fn figure_shadow(kind: Kind, pose: &Pose, look: &Look, out: &mut Shapes) {
    figure::shadow(kind, pose, look, out);
}

/// Draws what a hand holds from its grip (the middle of the hilt, where
/// the hands close) to its tip.
pub fn held(item: Held, grip: V3, tip: V3, look: &Look, out: &mut Shapes) {
    held::model(item, grip, tip).emit(look.ink(), out);
}

/// Adds a prop that never moves to `group`: its kind's mesh (built once,
/// reused by every later call with the same table), its base at `at`,
/// turned by `yaw`, and its ink and glowing parts as shapes of their own.
pub fn prop(kind: Prop, at: V3, yaw: i32, look: &Look, meshes: &mut MeshTable, group: &mut Group) {
    let baked = props::baked(kind);
    let place = Affine::translate(at).then(&Affine::rot_y(yaw));
    let body = find_or_add(meshes, &baked.body);
    group.shapes.solid(body, place, [ONE; 3], 0);
    if !baked.hull.is_empty() {
        let hull = find_or_add(meshes, &baked.hull);
        group.shapes.push(Shape {
            mesh: hull,
            at: place,
            rgb: look.ink(),
            value: ONE,
            pass: Pass::Ink,
        });
    }
    let lit = Model {
        parts: baked.loose.clone(),
        decals: Vec::new(),
    };
    lit.placed(&place).emit(look.ink(), &mut group.shapes);
}

/// A world mesh already in the table with these values, or a new one.
fn find_or_add(meshes: &mut MeshTable, values: &[i32]) -> MeshId {
    match meshes.world.iter().position(|m| m.as_slice() == values) {
        Some(k) => MeshId((Mesh::ALL.len() + k) as u32),
        None => meshes.add(values.to_vec()),
    }
}

// ---- the table: data/models.json ----

#[derive(Debug, Deserialize)]
struct TableFile {
    ink_figure_mm: i32,
    ink_item_mm: i32,
    ink_prop_mm: i32,
    colours: BTreeMap<String, [i32; 3]>,
}

/// `data/models.json`, read once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// the outline's width on a figure, a held item and a prop
    pub ink_figure: Fx,
    pub ink_item: Fx,
    pub ink_prop: Fx,
    colours: BTreeMap<String, Rgb>,
}

/// What a colour the file does not have is drawn in, so a gap shows on
/// screen; `every_model_draws_in_the_file_s_colours` finds it first.
pub const MISSING: Rgb = [ONE, 0, ONE];

impl Table {
    pub fn parse(json: &str) -> Result<Table, String> {
        let f: TableFile =
            serde_json::from_str(json).map_err(|e| format!("The models file could not be read: {e}"))?;
        let mm = |v: i32| (v as i64 * ONE as i64 / 1000) as Fx;
        Ok(Table {
            ink_figure: mm(f.ink_figure_mm),
            ink_item: mm(f.ink_item_mm),
            ink_prop: mm(f.ink_prop_mm),
            colours: f.colours.into_iter().map(|(k, c)| (k, rgb(c))).collect(),
        })
    }

    /// A colour by name.
    pub fn c(&self, name: &str) -> Rgb {
        self.colours.get(name).copied().unwrap_or(MISSING)
    }
}

static TABLE: OnceLock<Table> = OnceLock::new();

/// The models file, compiled into the core and parsed on first use (R5.3).
pub fn table() -> &'static Table {
    TABLE.get_or_init(|| Table::parse(include_str!("../../../data/models.json")).expect("data/models.json"))
}

// ---- distances the flicker rules hold to ----

/// The farthest a figure, a held item or a pickup is drawn with all its
/// marks: past it a person is a pixel. M1 and M5 should draw no figure
/// farther (R5.6).
pub const FIGURE_FAR: Fx = fx::int(1000);
/// The farthest a prop is drawn with its details. M2 dithers a prop out, or
/// swaps it for a plainer one, before this (R5.6).
pub const PROP_FAR: Fx = fx::int(3000);
/// How far every mark on a figure stands off the surface under it: a
/// centimetre, over the 9.7 mm the depth buffer needs out to `FIGURE_FAR`.
pub const DECAL_LIFT: Fx = ONE / 100;
/// How far a prop's details stand off the surfaces under them: 2.5 cm, over
/// the 19 mm the depth buffer needs out to `PROP_FAR`.
pub const PROP_GAP: Fx = ONE / 40;

/// How far apart two surfaces must be to be told apart everywhere from the
/// eye out to `far`, `render::DEPTH_STEPS` steps of the depth buffer: the
/// worst of the near layer's step at its far plane and the far layer's at
/// `far` (each grows with distance inside its layer).
pub fn decal_lift_needed(far: Fx) -> Fx {
    let near_worst = render::layered_depth_step(far.min(render::NEAR_LAYER_FAR));
    let far_worst = if far > render::NEAR_LAYER_FAR {
        render::layered_depth_step(far)
    } else {
        0
    };
    render::DEPTH_STEPS * near_worst.max(far_worst)
}

// ---- parts and models ----

/// One shape of a model before it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part {
    pub mesh: Mesh,
    /// the unit mesh placed, sized and turned; its columns at right angles
    pub at: Affine,
    pub rgb: Rgb,
    /// the outline's width; 0 draws none
    pub ink: Fx,
    /// 0 lit; `ONE` its colour unlit (flames, gems)
    pub glow: Fx,
    /// a soft halo round it this strong; 0 none
    pub halo: Fx,
}

/// A model: its parts in draw order, and which parts are marks laid on
/// another (a decal and the part under it), for the lift test.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Model {
    pub parts: Vec<Part>,
    pub decals: Vec<(usize, usize)>,
}

impl Model {
    /// Adds a part with an outline `ink` wide (0 for none), returning its index.
    pub fn add(&mut self, mesh: Mesh, at: Affine, rgb: Rgb, ink: Fx) -> usize {
        self.parts.push(Part {
            mesh,
            at,
            rgb,
            ink,
            glow: 0,
            halo: 0,
        });
        self.parts.len() - 1
    }

    /// Adds a glowing part, unlit, with a halo `halo` strong.
    pub fn add_glow(&mut self, mesh: Mesh, at: Affine, rgb: Rgb, halo: Fx) -> usize {
        self.parts.push(Part {
            mesh,
            at,
            rgb,
            ink: 0,
            glow: ONE,
            halo,
        });
        self.parts.len() - 1
    }

    /// The same model with every part placed by `place` first.
    pub fn placed(&self, place: &Affine) -> Model {
        Model {
            parts: self
                .parts
                .iter()
                .map(|p| Part {
                    at: place.then(&p.at),
                    ..*p
                })
                .collect(),
            decals: self.decals.clone(),
        }
    }

    /// Pushes every part, in order: its solid, then its outline, then its halo.
    pub fn emit(&self, ink: Rgb, out: &mut Shapes) {
        for p in &self.parts {
            out.solid(p.mesh, p.at, p.rgb, p.glow);
            if p.ink > 0 {
                out.outline_by(p, ink);
            }
            if p.halo > 0 {
                out.glow(Mesh::Dot, halo_of(&p.at), p.rgb, p.halo);
            }
        }
    }

    /// M7: every part on the pixels the ordered dither keeps at `fade`
    /// (`Shapes::dithered`, `look::hand_over`), its outline the same, its
    /// halo as strong as the share of pixels kept.
    pub fn emit_faded(&self, ink: Rgb, fade: Fx, out: &mut Shapes) {
        let kept = fade.abs().min(ONE);
        for p in &self.parts {
            out.dithered(p.mesh, p.at, p.rgb, fade);
            if p.ink > 0 {
                out.push(crate::render::Shape {
                    mesh: p.mesh.into(),
                    at: render::inflate_by(&p.at, ink_width(&p.at, p.ink)),
                    rgb: ink,
                    value: fade,
                    pass: Pass::Ink,
                });
            }
            if p.halo > 0 {
                out.glow(Mesh::Dot, halo_of(&p.at), p.rgb, fx::mul(p.halo, kept));
            }
        }
    }

    /// Triangles drawn, solids only (an outline draws as many again).
    pub fn triangles(&self) -> usize {
        self.parts
            .iter()
            .map(|p| crate::prim::vertices(p.mesh).len() / (3 * VERTEX_VALUES))
            .sum()
    }

    /// Instances pushed by `emit`.
    pub fn instances(&self) -> usize {
        self.parts
            .iter()
            .map(|p| 1 + (p.ink > 0) as usize + (p.halo > 0) as usize)
            .sum()
    }
}

/// A halo round a glowing part: the part's size grown to twice, at least
/// 12 cm across.
fn halo_of(a: &Affine) -> Affine {
    let grow = |c: V3| {
        let l = c.len().max(1);
        c.scale(fx::div((2 * l).max(ONE * 12 / 100), l))
    };
    Affine {
        c0: grow(a.c0),
        c1: grow(a.c1),
        c2: grow(a.c2),
        t: a.t,
    }
}

/// An outline drawn `width` wide, held to a sixth of the part's thinnest
/// side so a blade's or a tusk's line stays in proportion.
pub fn ink_width(at: &Affine, width: Fx) -> Fx {
    let thinnest = [at.c0.len(), at.c1.len(), at.c2.len()]
        .into_iter()
        .filter(|&l| l > 0)
        .min()
        .unwrap_or(0);
    width.min(thinnest / 6).max(ONE / 500)
}

trait Outline {
    fn outline_by(&mut self, p: &Part, ink: Rgb);
}

impl Outline for Shapes {
    fn outline_by(&mut self, p: &Part, ink: Rgb) {
        self.push(Shape {
            mesh: p.mesh.into(),
            at: render::inflate_by(&p.at, ink_width(&p.at, p.ink)),
            rgb: ink,
            value: ONE,
            pass: Pass::Ink,
        });
    }
}

// ---- placing parts ----

/// Millimetres to `fx` metres.
pub const fn mm(v: i32) -> Fx {
    (v as i64 * ONE as i64 / 1000) as Fx
}

/// A frame: three unit columns at right angles (right, up, back) and an
/// origin, as an `Affine`.
pub fn frame(right: V3, up: V3, back: V3, origin: V3) -> Affine {
    Affine {
        c0: right,
        c1: up,
        c2: back,
        t: origin,
    }
}

/// A unit mesh `size` across (each axis), centred at `local` in `f`.
pub fn put(f: &Affine, local: V3, size: V3) -> Affine {
    f.then(&Affine::translate(local)).then(&Affine::scale(size))
}

/// The same, turned by `turn` about its centre first.
pub fn put_turned(f: &Affine, local: V3, turn: &Affine, size: V3) -> Affine {
    f.then(&Affine::translate(local))
        .then(turn)
        .then(&Affine::scale(size))
}

/// A frame whose up column runs from `a` to `b`, its right column as near
/// `hint` as stays at right angles (another axis when `hint` runs along it).
pub fn along(a: V3, b: V3, hint: V3) -> Affine {
    let up = b.sub(a).norm();
    let up = if up == V3::ZERO { V3::UP } else { up };
    let mut right = hint.sub(up.scale(hint.dot(up)));
    if right.len() < ONE / 8 {
        let other = if up.y.abs() < ONE * 9 / 10 {
            V3::UP
        } else {
            v3(0, 0, ONE)
        };
        right = other.sub(up.scale(other.dot(up)));
    }
    let right = right.norm();
    let back = right.cross(up).norm();
    frame(right, up, back, a)
}

/// A unit mesh stretched from `a` to `b`, `thick` across both ways.
pub fn bar(a: V3, b: V3, thick: (Fx, Fx), hint: V3) -> Affine {
    let f = along(a, b, hint);
    let len = b.sub(a).len().max(1);
    let mid = v3((a.x + b.x) / 2, (a.y + b.y) / 2, (a.z + b.z) / 2);
    Affine { t: mid, ..f }.then(&Affine::scale(v3(thick.0, len, thick.1)))
}

/// The point on the ellipsoid of radii `r` (about the origin) in the
/// direction `d`, and the surface's outward unit normal there.
pub fn on_ellipsoid(r: V3, d: V3) -> (V3, V3) {
    // p = d / √((dx/rx)² + (dy/ry)² + (dz/rz)²), normal ∝ (px/rx², …)
    let q = |c: Fx, rc: Fx| fx::div(c, rc);
    let k = fx::mul(q(d.x, r.x), q(d.x, r.x))
        + fx::mul(q(d.y, r.y), q(d.y, r.y))
        + fx::mul(q(d.z, r.z), q(d.z, r.z));
    let s = fx::div(ONE, fx::sqrt(k).max(1));
    let p = d.scale(s);
    let n = v3(
        fx::div(q(p.x, r.x), r.x),
        fx::div(q(p.y, r.y), r.y),
        fx::div(q(p.z, r.z), r.z),
    )
    .norm();
    (p, n)
}

/// A direction `az` round from forward (−z) toward the right (+x) and `el`
/// up, both angles.
pub fn toward(az: i32, el: i32) -> V3 {
    let ce = fx::cos(el);
    v3(fx::mul(fx::sin(az), ce), fx::sin(el), -fx::mul(fx::cos(az), ce))
}

/// A mark laid on the ellipsoid part `base` (radii `r` in its frame `f`):
/// a dot `size` across (its depth `size.z`), its outer face `lift` proud
/// of the surface in the direction `d`. The dot's pole, a vertex, points
/// out along the surface's normal, so its outer face is exactly there.
/// Records it for the lift test.
#[allow(clippy::too_many_arguments)]
pub fn decal(m: &mut Model, base: usize, f: &Affine, r: V3, d: V3, size: V3, lift: Fx, rgb: Rgb) -> usize {
    let (p, n) = on_ellipsoid(r, d);
    // across as near the frame's level as it can be; then its height
    let up0 = V3::UP.sub(n.scale(n.dot(V3::UP)));
    let up = if up0.len() < ONE / 8 {
        v3(0, 0, -ONE)
    } else {
        up0.norm()
    };
    let across = up.cross(n).norm();
    let down = across.cross(n).norm();
    let centre = p.add(n.scale(lift - size.z / 2));
    let local = Affine {
        c0: across.scale(size.x),
        c1: n.scale(size.z),
        c2: down.scale(size.y),
        t: centre,
    };
    let k = m.add(Mesh::Dot, f.then(&local), rgb, 0);
    m.decals.push((k, base));
    k
}

/// A catch-light on the dot eye `eye`: up and to the side of its middle.
pub fn catch_light(m: &mut Model, eye: usize, size: V3, rgb: Rgb) -> usize {
    let a = m.parts[eye].at;
    let f = frame(a.c0.norm(), a.c1.norm(), a.c2.norm(), a.t);
    let r = v3(a.c0.len() / 2, a.c1.len() / 2, a.c2.len() / 2);
    // in the eye's own axes: across, out, down
    let d = v3(ONE * 30 / 100, ONE, -ONE * 42 / 100).norm();
    decal(m, eye, &f, r, d, size, DECAL_LIFT, rgb)
}

#[cfg(test)]
mod tests;
