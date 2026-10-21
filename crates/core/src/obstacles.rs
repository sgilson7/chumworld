//! `obstacles` (M7 Polish, task P2 "land"): what a thing in flight meets in
//! the world. A thrown rock or knife, the boomerang and the orbs stop on
//! (or glance off) every prop and every tree and rock of the scatter, where
//! before they met only the ground, bodies and the cart (SECOND-ORDER.md
//! R8.13, R7.4, R9.45).
//!
//! Each volume is a convex solid: the half-spaces whose meeting is its
//! inside (`Volume::planes`), a sphere round it to pass it by quickly. They
//! come from the models themselves, so the solid matches what is drawn:
//!
//! - every part of every prop's model (M3's `models::prop_model`) of any
//!   size becomes a box over its own axes (`Mesh::Cube` and the like), an
//!   eight-sided prism (a cylinder, a crown, a faceted rock) or an
//!   eight-sided pyramid (a cone), placed as the prop is placed;
//! - the player's house is hollow: its walls are M5's (`solid::house_walls`)
//!   raised to the eaves, so a rock thrown indoors meets the wall, not a
//!   solid house round the bed;
//! - a tree is its trunk and crowns, a rock its stone (`scatter::tree_parts`),
//!   worked out a 128 m chunk at a time the first time anything flies
//!   there, and kept (`Map::trees`).
//!
//! `first` answers where along a path a sphere first meets any of them,
//! and which way the face it meets looks, so a rock can glance off it.
//! Everything is whole Q16 numbers (CW-2), the same for any run (CW-4).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, Affine, V3};
use crate::prim::Mesh;
use crate::world::World;

/// A part smaller than this every way is passed over (a door's knob, a
/// flower in a window box): a rock would not stop on it.
pub const SMALLEST: Fx = ONE * 15 / 100;

/// A convex solid: inside where `n · p <= d` for every plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    /// unit normals pointing out, and their offsets (Q16 metres, i64)
    pub planes: Vec<(V3, i64)>,
    /// a sphere holding it
    pub centre: V3,
    pub radius: Fx,
}

/// The unit shape's half-spaces in its own frame, before it is placed: a
/// box, an eight-sided prism or an eight-sided pyramid, inside the unit
/// cube (±½) as the primitives are (`prim.rs`).
fn local_planes(mesh: Mesh) -> Vec<([i64; 3], i64)> {
    const H: i64 = ONE as i64 / 2;
    // the eight directions round the vertical, as (cos, sin) in Q16
    let round = |out: &mut Vec<([i64; 3], i64)>, y: i64, d: i64| {
        for k in 0..8 {
            let a = fx::TURN * k / 8;
            out.push(([fx::cos(a) as i64, y, fx::sin(a) as i64], d));
        }
    };
    let mut out = vec![([0, -(ONE as i64), 0], H)];
    match mesh {
        Mesh::Cone => {
            // the sides through the base's rim (radius ½ at y = −½) and the
            // apex (y = ½): cos·x + sin·z + ½ y <= ¼, as unit normals
            // (1, ½) / √(5/4): 0.894 and 0.447, ¼ / 1.118 = 0.2236
            for k in 0..8 {
                let a = fx::TURN * k / 8;
                let (c, s) = (fx::cos(a) as i64, fx::sin(a) as i64);
                out.push((
                    [c * 894 / 1000, ONE as i64 * 447 / 1000, s * 894 / 1000],
                    ONE as i64 * 2236 / 10_000,
                ));
            }
        }
        Mesh::Cyl | Mesh::Blob | Mesh::Sphere | Mesh::Capsule | Mesh::Octa | Mesh::Facet | Mesh::HalfCyl => {
            out.push(([0, ONE as i64, 0], H));
            round(&mut out, 0, H);
        }
        _ => {
            out.push(([0, ONE as i64, 0], H));
            for (x, z) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                out.push(([x * ONE as i64, 0, z * ONE as i64], H));
            }
        }
    }
    out
}

impl Volume {
    /// The unit shape `mesh` placed by `at` (columns at right angles, as
    /// every model part's are): a plane n·p <= d in its frame is, in the
    /// world, (Σ n_k c_k / |c_k|²) · (p − t) <= d, made unit.
    pub fn placed(mesh: Mesh, at: &Affine) -> Volume {
        let cols = [at.c0, at.c1, at.c2];
        let len2: Vec<i64> = cols
            .iter()
            .map(|c| {
                ((c.x as i64 * c.x as i64 + c.y as i64 * c.y as i64 + c.z as i64 * c.z as i64) >> 16).max(1)
            })
            .collect();
        let mut planes = Vec::new();
        for (n, d) in local_planes(mesh) {
            // in 1/65536ths of a metre⁻¹: n_k / |c_k| per column, summed
            let mut w = [0i64; 3];
            for k in 0..3 {
                let f = n[k] * ONE as i64 / len2[k];
                w[0] += (cols[k].x as i64 * f) >> 16;
                w[1] += (cols[k].y as i64 * f) >> 16;
                w[2] += (cols[k].z as i64 * f) >> 16;
            }
            let l = crate::terrain::root((w[0] * w[0] + w[1] * w[1] + w[2] * w[2]) as u64) as i64;
            if l == 0 {
                continue;
            }
            let u = v3(
                (w[0] * ONE as i64 / l) as Fx,
                (w[1] * ONE as i64 / l) as Fx,
                (w[2] * ONE as i64 / l) as Fx,
            );
            // d in the world: d / |w| (|w| in 1/ONE metre⁻¹), and the shift by t
            let dw = d * ONE as i64 / l + dot(u, at.t);
            planes.push((u, dw));
        }
        let radius = cols.iter().map(|c| c.len() / 2).sum::<Fx>();
        Volume {
            planes,
            centre: at.t,
            radius,
        }
    }

    /// An upright box `half` across each way round `centre`, turned by `yaw`
    /// (`Affine::rot_y`'s).
    pub fn upright(centre: V3, half: V3, yaw: i32) -> Volume {
        let at = Affine::translate(centre)
            .then(&Affine::rot_y(yaw))
            .then(&Affine::scale(v3(half.x * 2, half.y * 2, half.z * 2)));
        Volume::placed(Mesh::Cube, &at)
    }

    /// Where along `a` to `b` (a share, 0 to ONE) a sphere of radius `r`
    /// first enters the volume, grown by `r`, and the outward normal of the
    /// face it enters by; none if it does not. A path that starts inside
    /// meets it at once, pushed out by the nearest face (Cyrus and Beck's
    /// clipping).
    pub fn through(&self, a: V3, b: V3, r: Fx) -> Option<(i64, V3)> {
        let one = ONE as i64;
        let (mut lo, mut hi) = (0i64, one);
        let mut face = None;
        let mut nearest: Option<(i64, V3)> = None;
        for &(n, d) in &self.planes {
            let d = d + r as i64;
            let pa = dot(n, a) - d;
            let pb = dot(n, b) - d;
            if pa > 0 && pb > 0 {
                return None;
            }
            if pa <= 0 && nearest.is_none_or(|(m, _)| pa > m) {
                nearest = Some((pa, n));
            }
            if pa > 0 {
                // entering: from outside to in at pa / (pa − pb)
                let t = pa * one / (pa - pb).max(1);
                if t > lo || face.is_none() {
                    lo = lo.max(t);
                    face = Some(n);
                }
            } else if pb > 0 {
                hi = hi.min((-pa) * one / (pb - pa).max(1));
            }
            if lo > hi {
                return None;
            }
        }
        match face {
            Some(n) => Some((lo, n)),
            // inside from the start: out through the nearest face
            None => nearest.map(|(_, n)| (0, n)),
        }
    }
}

fn dot(a: V3, b: V3) -> i64 {
    (a.x as i64 * b.x as i64 + a.y as i64 * b.y as i64 + a.z as i64 * b.z as i64) >> 16
}

/// One prop's volumes and a sphere round them all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thing {
    pub prop: crate::models::Prop,
    pub at: V3,
    pub centre: V3,
    pub radius: Fx,
    pub parts: Vec<Volume>,
}

fn thing(prop: crate::models::Prop, at: V3, parts: Vec<Volume>) -> Thing {
    let mut c = at;
    if let Some(top) = parts.iter().map(|p| p.centre.y + p.radius).max() {
        c.y = (at.y + top) / 2;
    }
    let radius = parts
        .iter()
        .map(|p| p.centre.dist(c) + p.radius)
        .max()
        .unwrap_or(0);
    Thing {
        prop,
        at,
        centre: c,
        radius,
        parts,
    }
}

/// The volumes of one prop placed at `at` turned by `yaw`: its model's
/// parts of any size.
pub fn prop_volumes(prop: crate::models::Prop, at: V3, yaw: i32) -> Vec<Volume> {
    let place = Affine::translate(at).then(&Affine::rot_y(yaw));
    crate::models::prop_model(prop)
        .parts
        .iter()
        .filter(|p| {
            let size = [p.at.c0.len(), p.at.c1.len(), p.at.c2.len()];
            size.iter().copied().max().unwrap_or(0) >= SMALLEST
        })
        .map(|p| Volume::placed(p.mesh, &place.then(&p.at)))
        .collect()
}

/// The scatter's trees and rocks by chunk, as worked out so far.
type Trees = BTreeMap<(i32, i32), Rc<Vec<Thing>>>;

/// Every prop's volumes, worked out once (`World::new`), and the trees' and
/// rocks' by chunk as things fly near them.
#[derive(Default)]
pub struct Map {
    pub things: Vec<Thing>,
    trees: RefCell<Trees>,
}

impl std::fmt::Debug for Map {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // the trees worked out so far are a cache: two worlds alike are
        // alike whatever has flown past which chunk
        write!(f, "obstacles::Map({} props)", self.things.len())
    }
}

impl PartialEq for Map {
    fn eq(&self, o: &Map) -> bool {
        self.things == o.things
    }
}
impl Eq for Map {}

/// What a path met: how far along (a share of ONE), the face's outward
/// normal, and the prop's kind (`None` for a tree or a rock of the scatter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub share: i64,
    pub normal: V3,
    pub prop: Option<crate::models::Prop>,
}

impl Map {
    pub fn build(w: &World) -> Map {
        use crate::models::Prop;
        let mut things = Vec::new();
        let home = w.solid.home;
        for (pk, place) in w.land.layout.places.iter().enumerate() {
            let spots = crate::places::spots(w, place);
            for (k, s) in spots.iter().enumerate() {
                let parts = match home {
                    Some(h) if h.place == pk && h.prop == k => home_volumes(w, s),
                    _ => prop_volumes(s.prop, s.at, s.yaw),
                };
                things.push(thing(s.prop, s.at, parts));
            }
            for f in crate::places::fence_spans(w, place) {
                things.push(thing(Prop::Fence, f.at, prop_volumes(Prop::Fence, f.at, f.yaw)));
            }
        }
        Map {
            things,
            trees: RefCell::new(BTreeMap::new()),
        }
    }

    /// The trees and rocks of scatter chunk (i, j), worked out once.
    fn trees(&self, w: &World, i: i32, j: i32) -> Rc<Vec<Thing>> {
        if let Some(t) = self.trees.borrow().get(&(i, j)) {
            return t.clone();
        }
        let sp = &w.land.spec.scatter;
        let list: Vec<Thing> = crate::scatter::items(&w.land, i, j)
            .iter()
            .map(|it| {
                let parts = crate::scatter::tree_parts(sp, it)
                    .into_iter()
                    .map(|(m, a, _)| Volume::placed(m, &a))
                    .collect();
                thing(crate::models::Prop::HollowTree, it.at, parts)
            })
            .collect();
        let rc = Rc::new(list);
        self.trees.borrow_mut().insert((i, j), rc.clone());
        rc
    }

    /// The first volume a sphere of radius `r` meets along `a` to `b`.
    pub fn first(&self, w: &World, a: V3, b: V3, r: Fx) -> Option<Hit> {
        let mid = a.lerp(b, ONE / 2);
        let reach = a.dist(b) / 2 + r;
        let mut best: Option<Hit> = None;
        let try_thing = |t: &Thing, prop: Option<crate::models::Prop>, best: &mut Option<Hit>| {
            if t.centre.dist(mid) > t.radius + reach {
                return;
            }
            for v in &t.parts {
                if v.centre.dist(mid) > v.radius + reach {
                    continue;
                }
                if let Some((share, normal)) = v.through(a, b, r) {
                    if best.is_none_or(|h| share < h.share) {
                        *best = Some(Hit { share, normal, prop });
                    }
                }
            }
        };
        for t in &self.things {
            try_thing(t, Some(t.prop), &mut best);
        }
        // the scatter's chunks the path's box touches, grown by a crown
        let side = w.land.spec.scatter.chunk_m * ONE;
        let grow = 4 * ONE + r;
        let (x0, x1) = (a.x.min(b.x) - grow, a.x.max(b.x) + grow);
        let (z0, z1) = (a.z.min(b.z) - grow, a.z.max(b.z) + grow);
        for j in z0.div_euclid(side)..=z1.div_euclid(side) {
            for i in x0.div_euclid(side)..=x1.div_euclid(side) {
                for t in self.trees(w, i, j).iter() {
                    try_thing(t, None, &mut best);
                }
            }
        }
        best
    }
}

/// The player's house, hollow: M5's walls raised to the eaves (footing
/// 400 mm and walls 2 600 mm, M3's house), and every part of the model above
/// them (the gable, the roof, the chimney).
fn home_volumes(w: &World, s: &crate::places::Spot) -> Vec<Volume> {
    let eaves = ONE * 3000 / 1000;
    let Some(h) = w.solid.home else {
        return Vec::new();
    };
    let mut out: Vec<Volume> = crate::solid::house_walls(&h.wall)
        .iter()
        .map(|wall| {
            Volume::upright(
                v3(wall.x, s.at.y + eaves / 2, wall.z),
                v3(wall.hx, eaves / 2, wall.hz),
                wall.yaw,
            )
        })
        .collect();
    let place = Affine::translate(s.at).then(&Affine::rot_y(s.yaw));
    for p in crate::models::prop_model(s.prop).parts {
        let size = [p.at.c0.len(), p.at.c1.len(), p.at.c2.len()];
        let low = p.at.t.y - size[1] / 2;
        // the parts wholly over the walls' tops
        if low >= eaves - ONE / 10 && size.iter().copied().max().unwrap_or(0) >= SMALLEST {
            out.push(Volume::placed(p.mesh, &place.then(&p.at)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::int;

    #[test]
    fn a_path_enters_a_turned_box_where_its_face_is() {
        // a box 2 m each way at (5, 1, 0), turned a quarter: still ±1 m on
        // x and z. A path along x from 0 to 10 at y = 1 enters at x = 4,
        // 40% of the way, through the face looking −x; a sphere of 0.5 m
        // meets it at 3.5 m, 35%
        let v = Volume::upright(v3(int(5), ONE, 0), v3(ONE, ONE, ONE), fx::QUARTER_TURN);
        let (share, n) = v.through(v3(0, ONE, 0), v3(int(10), ONE, 0), 0).unwrap();
        assert!((share - ONE as i64 * 4 / 10).abs() <= 8, "{share}");
        assert!(n.x < -ONE + 16 && n.y.abs() < 16, "{n:?}");
        let (share, _) = v.through(v3(0, ONE, 0), v3(int(10), ONE, 0), ONE / 2).unwrap();
        assert!((share - ONE as i64 * 35 / 100).abs() <= 8, "{share}");
        // passing over its top (y = 2.5) it meets nothing
        assert_eq!(
            v.through(v3(0, int(25) / 10, 0), v3(int(10), int(25) / 10, 0), 0),
            None
        );
        // a box turned 45°: its corner reaches √2 = 1.414 m out along x,
        // so the same path enters at x = 5 − 1.414, 35.9%
        let d = Volume::upright(v3(int(5), ONE, 0), v3(ONE, ONE, ONE), fx::deg(45));
        let (share, _) = d.through(v3(0, ONE, 0), v3(int(10), ONE, 0), 0).unwrap();
        assert!((share - ONE as i64 * 3586 / 10_000).abs() <= 40, "{share}");
    }

    /// A rock thrown straight at the middle of `v` along `way` (flat,
    /// unit) from 1.5 m short of its face, at 15 m/s: 0.25 m a tick.
    /// Returns how far past the middle along `way` it was where it first
    /// met the scenery (negative: short of it), if it did.
    fn throw_at(w: &mut World, v: &Volume, way: V3) -> Option<Fx> {
        let target = v.centre;
        let far = target.sub(way.scale(v.radius + int(2)));
        let (share, _) = v.through(far, target, 0)?;
        let face = far.lerp(target, share as Fx);
        let from = face.sub(way.scale(ONE * 3 / 2));
        w.throwables.flying.push(crate::throwables::Flying {
            item: crate::pose::Held::Rock,
            by: 0,
            p: from,
            prev: from,
            vel: way.scale(ONE / 4),
            ticks: 0,
            bounces: 0,
        });
        let mut met = None;
        for _ in 0..40 {
            w.step(&crate::input::Input::default());
            let first = w
                .throwables
                .knocks
                .iter()
                .chain(&w.throwables.landed)
                .find(|i| i.struck == crate::throwables::Struck::Scenery);
            if let Some(i) = first {
                met = Some(i.at.sub(target).dot(way));
                break;
            }
            if w.throwables.flying.is_empty() {
                break;
            }
        }
        w.throwables.flying.clear();
        w.throwables.resting.clear();
        met
    }

    /// R9.45: every prop in the region (each house, the barn, the hut, the
    /// tents, the well, the shrine, the fire, the chest, the scarecrow, the
    /// signposts, the hollow tree, the bridge, every fence span, the bed)
    /// and the camp forest's trees stop a rock thrown at them: thrown at
    /// the middle of its biggest part from a few metres off, each from its
    /// own side round the compass, the rock meets the scenery short of
    /// that middle (it may glance off and fly on, as a rock does).
    #[test]
    fn every_prop_and_tree_stops_a_rock_thrown_at_it() {
        let mut w = World::new(1);
        let things = w.obstacles.things.clone();
        assert!(things.len() > 60, "{} props", things.len());
        let mut targets: Vec<(Volume, String)> = things
            .iter()
            .map(|t| {
                let v = t.parts.iter().max_by_key(|v| v.radius).unwrap().clone();
                (v, format!("{:?}", t.prop))
            })
            .collect();
        // the camp forest's trees, the nearest chunk's first ten
        let trees = w.obstacles.trees(&w, 3213 / 128, 987 / 128);
        for t in trees.iter().take(10) {
            targets.push((
                t.parts.iter().max_by_key(|v| v.radius).unwrap().clone(),
                "a tree".into(),
            ));
        }
        let mut stopped = 0;
        for (k, (v, what)) in targets.iter().enumerate() {
            let a = fx::deg(37 * k as i32);
            let way = v3(fx::cos(a), 0, fx::sin(a));
            let past = throw_at(&mut w, v, way);
            assert!(
                past.is_some(),
                "{what} {k} at {:?} let a rock by",
                (v.centre.x / ONE, v.centre.z / ONE)
            );
            let past = past.unwrap_or(0);
            assert!(
                past < 0,
                "{what} {k} at {:?}, radius {}: the rock got {past} past its middle",
                (v.centre.x / ONE, v.centre.y * 100 / ONE, v.centre.z / ONE),
                v.radius * 100 / ONE
            );
            stopped += 1;
        }
        assert_eq!(stopped, targets.len());
    }

    /// R8.13: an orb is stopped by a house's wall. The village house at
    /// (−46, −10) m from the centre, 6 × 5 m turned 70°: the line due east
    /// from its middle leaves it where it first crosses a wall, at the
    /// nearer of 3 / cos 70° = 8.77 m and 2.5 / sin 70° = 2.66 m. A sphere
    /// (13 cm round) at 0.3 m a tick from 8 m east, 1.5 m up, meets that
    /// wall after 8 − 2.66 − 0.13 = 5.21 m, 17.4 ticks: in the 18th tick. The player stands west of the house, so the
    /// orb flies toward them, not past them.
    #[test]
    fn an_orb_is_stopped_by_a_house_wall() {
        let mut w = World::new(1);
        let c = v3(int(1954), 0, int(2790));
        let g = w.ground(c.x, c.z);
        let mut me = v3(int(1930), 0, int(2790));
        me.y = w.ground(me.x, me.z);
        w.player = crate::player::Body::new(me, 0);
        w.step(&crate::input::Input::default());
        let from = v3(c.x + int(8), g + ONE * 3 / 2, c.z);
        w.orbs.flying.push(crate::orbs::Orb {
            id: 1,
            kind: crate::orbs::spec().kind("sphere").unwrap(),
            caster: 1,
            pos: from,
            prev: from,
            dir: v3(-ONE, 0, 0),
            speed: ONE * 3 / 10,
            curl: 0,
            hold: 0,
            held: 0,
            launch: crate::orbs::Launch::Along,
            age: 0,
            from,
            turned: None,
        });
        let mut ticks = 0;
        while !w.orbs.flying.is_empty() && ticks < 60 {
            w.step(&crate::input::Input::default());
            ticks += 1;
            if let Some(o) = w.orbs.flying.first() {
                assert!(o.pos.x > c.x, "the orb got into the house, to {}", o.pos.x / ONE);
            }
        }
        assert!((17..=18).contains(&ticks), "spent after {ticks} ticks");
    }

    #[test]
    fn a_cone_is_narrow_at_its_top_and_wide_at_its_foot() {
        // a cone 4 m across at its foot (y = 0) and 4 m tall: at 3 m up
        // it is 1 m across, so a path 0.8 m from its axis passes at 3 m and
        // meets it at 1 m (where it is 3 m across)
        let at = Affine::translate(v3(0, int(2), 0)).then(&Affine::scale(v3(int(4), int(4), int(4))));
        let v = Volume::placed(Mesh::Cone, &at);
        let path = |y: Fx| v.through(v3(-int(5), y, ONE * 8 / 10), v3(int(5), y, ONE * 8 / 10), 0);
        assert!(path(int(3)).is_none(), "passes over its narrow top");
        assert!(path(ONE).is_some(), "meets its wide foot");
    }
}
