//! `boom` (M7 Polish, task P1 "feel"): the camera's boom, kept out of
//! solid things, and the player's house cut away round the player.
//!
//! The camera stays over the right shoulder (CW-15, `camera.rs`). Its
//! *boom* is the line from the head to where the eye would be; when a
//! building, a tent, a fence or a tree's trunk or crown stands in the way,
//! the eye comes in along that line to stand clear of it. Two clearances:
//! each tick the boom's length eases toward the length that keeps the eye
//! `SOFT` clear (in quickly, out slowly), so the view comes in before a
//! wall arrives rather than when it does; and each frame the eye is held
//! at least `HARD` clear of anything, whatever the easing says, so it is
//! never inside a solid (`clip`). A solid the head is inside (a pine's
//! lowest boughs over the player) is no wall to the boom.
//!
//! The player's house is drawn cut away (its roof and the tops of its walls
//! left off) while the player is in or at it, or the eye is inside it. The
//! change is a dithered hand-over over `CUT_TICKS`, as the detail levels
//! hand over (`look::hand_over`), so the roof never pops (CW-18): `cut`
//! counts up while the house should be cut and down while it should not,
//! one tick at a time. While any of it is cut, the boom meets the low walls
//! only; whole, it meets the whole house.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, V3};
use crate::models::Prop;
use crate::places;
use crate::scatter::Thing;
use crate::solid::{self, Wall};
use crate::world::World;

/// The eye keeps this clear of a solid as the boom eases (each tick), and
/// at least this clear always (each frame): the near plane is 0.4 m out
/// (`render::NEAR`), so at `HARD` a wall is not cut open by it straight
/// ahead.
pub const SOFT: Fx = ONE * 80 / 100;
pub const HARD: Fx = ONE * 45 / 100;
/// The boom comes in by at most this a tick (15 m/s: a wall is met at a
/// run without the eye reaching it), and goes back out by this share of
/// the way a tick, at least `OUT_MIN`.
pub const IN_STEP: Fx = ONE / 4;
pub const OUT_SHARE: Fx = ONE / 10;
pub const OUT_MIN: Fx = ONE / 100;
/// The march along the boom, this far a step, then halved this many times.
pub const MARCH: Fx = ONE / 20;
pub const REFINE: u32 = 6;
/// The house's cut away hand-over takes this many ticks (0.4 s).
pub const CUT_TICKS: u8 = 24;
/// The house is cut while the eye is within this of its footprint.
pub const EYE_NEAR: Fx = ONE * 3 / 10;
/// Trees are looked up again once the player is this far from where they
/// were last looked up, and kept within `TREES_NEAR` of that point.
pub const TREES_AGAIN: Fx = fx::int(8);
pub const TREES_NEAR: Fx = fx::int(20);
/// A fence stands this high.
pub const FENCE_TOP: Fx = ONE * 11 / 10;

/// A thing the boom stops at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Solid {
    /// a footprint turned as its building, from `y0` to `y1`
    Block { wall: Wall, y0: Fx, y1: Fx },
    /// an upright post (a trunk): its foot's middle, radius, top
    Post { at: V3, r: Fx, y1: Fx },
    /// an upright cone (a pine's boughs): the middle of its base, its
    /// radius there, its base's and its point's heights
    Cone { at: V3, r: Fx, y0: Fx, y1: Fx },
    /// a ball pressed flat or long (a leafy crown): its middle and radii
    Ball { c: V3, r: V3 },
}

/// How far a point at `out` beyond each of up to three faces is from the
/// shape they bound: outside, the distance to the nearest corner, edge or
/// face; inside, minus the distance to the nearest face.
fn combine(parts: &[Fx]) -> Fx {
    let outside: Vec<Fx> = parts.iter().copied().filter(|d| *d > 0).collect();
    match outside.len() {
        0 => parts.iter().copied().max().unwrap_or(0),
        1 => outside[0],
        _ => {
            let sq: i64 = outside.iter().map(|d| *d as i64 * *d as i64).sum();
            (fx::isqrt_u64(sq as u64) as i64) as Fx
        }
    }
}

impl Solid {
    /// How far `p` is outside the solid (minus: how deep inside), near
    /// enough for a clearance.
    pub fn gap(&self, p: V3) -> Fx {
        match *self {
            Solid::Block { wall, y0, y1 } => {
                let (lx, lz) = wall.local(p.x, p.z);
                combine(&[lx.abs() - wall.hx, lz.abs() - wall.hz, (y0 - p.y).max(p.y - y1)])
            }
            Solid::Post { at, r, y1 } => {
                let d = v3(p.x - at.x, 0, p.z - at.z).len() - r;
                combine(&[d, (at.y - p.y).max(p.y - y1)])
            }
            Solid::Cone { at, r, y0, y1 } => {
                let up = (y0 - p.y).max(p.y - y1);
                let k = fx::div((y1 - p.y).clamp(0, y1 - y0), (y1 - y0).max(1));
                let d = v3(p.x - at.x, 0, p.z - at.z).len() - fx::mul(r, k);
                combine(&[d, up])
            }
            Solid::Ball { c, r } => {
                let q = v3(
                    fx::div(p.x - c.x, r.x.max(1)),
                    fx::div(p.y - c.y, r.y.max(1)),
                    fx::div(p.z - c.z, r.z.max(1)),
                );
                fx::mul(q.len() - ONE, r.x.min(r.y).min(r.z))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    /// the boom's length this tick and last, as a share of the full boom
    /// (`ONE`: nothing in the way); not yet set before the first tick
    pub len: Fx,
    pub prev: Fx,
    pub set: bool,
    /// where this tick's eye is, for the house's cut
    pub eye: V3,
    /// the house's hand-over this tick and last, 0 (whole) to `CUT_TICKS`
    /// (cut away)
    pub cut: u8,
    pub prev_cut: u8,
    /// the trees near where they were last looked up, and that point
    pub trees: Vec<Solid>,
    pub trees_at: Option<V3>,
    /// every building, tent and fence, worked out once (`blocks`)
    pub blocks: std::rc::Rc<Vec<Solid>>,
}

/// The highest point of a prop's model over its foot.
pub fn top_of(prop: Prop) -> Fx {
    let m = crate::models::prop_model(prop);
    m.parts
        .iter()
        .map(|p| p.at.t.y + (p.at.c0.y.abs() + p.at.c1.y.abs() + p.at.c2.y.abs()) / 2)
        .max()
        .unwrap_or(0)
}

/// Every building, tent and fence in the region but the player's house
/// (`home`), as the boom meets them: footprints as `solid` keeps bodies
/// out, as high as each model stands.
pub fn blocks(w: &World) -> Vec<Solid> {
    let layout = &w.land.layout;
    let home = w.solid.home;
    let mut out = Vec::new();
    for (pk, place) in layout.places.iter().enumerate() {
        let c = places::metres(place.centre);
        for (k, q) in place.props.iter().enumerate() {
            let Some(prop) = places::prop_kind(&q.prop) else {
                continue;
            };
            if q.inside || prop == Prop::Bridge || home.is_some_and(|h| h.place == pk && h.prop == k) {
                continue;
            }
            let Some(size) = layout.footprints_m.get(&q.prop) else {
                continue;
            };
            let at = c.add(v3(q.at[0] * ONE, 0, q.at[1] * ONE));
            let wall = Wall {
                x: at.x,
                z: at.z,
                hx: size[0] * ONE / 2,
                hz: size[1] * ONE / 2,
                yaw: fx::deg(q.yaw_deg),
            };
            let g = w.ground(at.x, at.z);
            out.push(Solid::Block {
                wall,
                y0: g - ONE / 2,
                y1: g + top_of(prop),
            });
        }
        for f in places::fence_spans(w, place) {
            let wall = Wall {
                x: f.at.x,
                z: f.at.z,
                hx: places::FENCE_SPAN / 2,
                hz: solid::FENCE_THICK / 2,
                yaw: f.yaw,
            };
            out.push(Solid::Block {
                wall,
                y0: f.at.y - ONE / 2,
                y1: f.at.y + FENCE_TOP,
            });
        }
    }
    out
}

/// The player's house as the boom meets it now: whole, or (while any of
/// it is cut away) its four low walls.
pub fn home(w: &World) -> Vec<Solid> {
    let Some(h) = w.solid.home else {
        return Vec::new();
    };
    let floor = h.foot.y + solid::FLOOR;
    if w.boom.cut == 0 {
        return vec![Solid::Block {
            wall: h.wall,
            y0: h.foot.y - ONE / 2,
            y1: h.foot.y + top_of(Prop::House),
        }];
    }
    solid::house_walls(&h.wall)
        .into_iter()
        .map(|wall| Solid::Block {
            wall,
            y0: h.foot.y - ONE / 2,
            y1: floor + crate::story::CUT_WALL + ONE / 10,
        })
        .collect()
}

/// A tree as the boom meets it: its trunk and its boughs or crowns, the
/// sizes `scatter` draws it at (`tree_parts`).
pub fn tree(it: &crate::scatter::Item) -> Vec<Solid> {
    let s = |m: Fx| m * it.size / 100;
    let at = it.at;
    match it.thing {
        Thing::Pine => {
            let mut v = vec![Solid::Post {
                at: at.sub(v3(0, ONE / 2, 0)),
                r: s(ONE * 2 / 10),
                y1: at.y + s(ONE * 14 / 10),
            }];
            for (y, wd, h) in [(110, 390, 230), (230, 310, 200), (345, 215, 175)] {
                let y0 = at.y + s(y * ONE / 100);
                v.push(Solid::Cone {
                    at: v3(at.x, y0, at.z),
                    r: s(wd * ONE / 100) / 2,
                    y0,
                    y1: y0 + s(h * ONE / 100),
                });
            }
            v
        }
        Thing::Leafy => vec![
            Solid::Post {
                at: at.sub(v3(0, ONE / 2, 0)),
                r: s(ONE * 18 / 100),
                y1: at.y + s(ONE * 18 / 10),
            },
            Solid::Ball {
                c: at.add(v3(0, s(ONE * 3), 0)),
                r: v3(s(ONE * 18 / 10), s(ONE * 14 / 10), s(ONE * 17 / 10)),
            },
            Solid::Ball {
                c: at.add(v3(0, s(ONE * 41 / 10), 0)),
                r: v3(s(ONE * 12 / 10), s(ONE), s(ONE * 115 / 100)),
            },
        ],
        Thing::Rock => Vec::new(),
    }
}

/// The trees within `TREES_NEAR` of `p`.
pub fn trees_near(w: &World, p: V3) -> Vec<Solid> {
    let side = w.land.spec.scatter.chunk_m * ONE;
    let (i0, i1) = (
        (p.x - TREES_NEAR).div_euclid(side),
        (p.x + TREES_NEAR).div_euclid(side),
    );
    let (j0, j1) = (
        (p.z - TREES_NEAR).div_euclid(side),
        (p.z + TREES_NEAR).div_euclid(side),
    );
    let mut out = Vec::new();
    for i in i0..=i1 {
        for j in j0..=j1 {
            for it in crate::scatter::items(&w.land, i, j) {
                if it.at.sub(p).len_xz() <= TREES_NEAR {
                    out.extend(tree(&it));
                }
            }
        }
    }
    out
}

/// Every solid the boom may meet from the head at `from`.
pub fn solids(w: &World, from: V3) -> Vec<Solid> {
    let mut all: Vec<Solid> = w
        .boom
        .blocks
        .iter()
        .filter(|s| match s {
            Solid::Block { wall, .. } => wall.holds(from.x, from.z, fx::int(8)),
            _ => true,
        })
        .copied()
        .collect();
    all.extend(home(w));
    all.extend(w.boom.trees.iter().copied());
    all
}

/// How far along the line from `from` to `to` (a share, `ONE` the whole
/// line) a point may go and stay `margin` clear of every solid (less for a
/// solid `from` is itself nearer than that: never nearer than `from` is;
/// none for a solid `from` is inside).
pub fn reach(solids: &[Solid], from: V3, to: V3, margin: Fx) -> Fx {
    let near: Vec<(Solid, Fx)> = solids
        .iter()
        .filter_map(|s| {
            let g = s.gap(from);
            (g > 0).then_some((*s, margin.min(g * 9 / 10)))
        })
        .collect();
    if near.is_empty() {
        return ONE;
    }
    let clear = |t: Fx| {
        let p = from.lerp(to, t);
        near.iter().all(|(s, m)| s.gap(p) >= *m)
    };
    let len = to.sub(from).len().max(1);
    let step = fx::div(MARCH, len).max(1);
    let mut t = 0;
    while t < ONE {
        let next = (t + step).min(ONE);
        if !clear(next) {
            // between t (clear) and next (not): halve the step
            let (mut lo, mut hi) = (t, next);
            for _ in 0..REFINE {
                let mid = (lo + hi) / 2;
                if clear(mid) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return lo;
        }
        t = next;
    }
    ONE
}

/// The head the boom starts from, for feet at `feet`.
pub fn head(feet: V3) -> V3 {
    feet.add(v3(0, crate::camera::HEAD, 0))
}

/// The eye for this frame: `eye` (the shoulder's, as `camera` places it)
/// brought in along the boom from the head to the eased length, and at
/// least `HARD` clear of everything.
pub fn clip(w: &World, alpha: Fx, feet: V3, eye: V3) -> V3 {
    let from = head(feet);
    let all = solids(w, from);
    let hard = reach(&all, from, eye, HARD);
    let eased = if w.boom.set {
        fx::lerp(w.boom.prev, w.boom.len, alpha)
    } else {
        ONE
    };
    from.lerp(eye, hard.min(eased))
}

/// Step 9b, last in the tick: the trees near the player looked up again
/// when they have walked on; the boom eased; the house's hand-over.
pub fn step(w: &mut World) {
    let p = w.player.pos;
    if w.boom.trees_at.is_none_or(|at| at.sub(p).len_xz() > TREES_AGAIN) {
        w.boom.trees = trees_near(w, p);
        w.boom.trees_at = Some(p);
    }
    let cam = crate::camera::view_from(p, w.aim, crate::camera::BACK, |x, z| w.ground(x, z));
    let from = head(p);
    let all = solids(w, from);
    let want = reach(&all, from, cam.eye, SOFT);
    let b = &mut w.boom;
    b.prev = if b.set { b.len } else { want };
    b.len = if !b.set {
        want
    } else if want < b.len {
        (b.len - IN_STEP).max(want)
    } else {
        (b.len + fx::mul(want - b.len, OUT_SHARE).max(OUT_MIN)).min(want)
    };
    b.set = true;
    b.eye = from.lerp(cam.eye, b.len);
    // the house: cut while the player is at it or the eye is in it
    let cut = w.solid.home.is_some_and(|h| {
        h.wall.holds(p.x, p.z, crate::story::CUT_NEAR + solid::RADIUS)
            || h.wall.holds(w.boom.eye.x, w.boom.eye.z, EYE_NEAR)
    });
    let b = &mut w.boom;
    b.prev_cut = b.cut;
    b.cut = if cut {
        (b.cut + 1).min(CUT_TICKS)
    } else {
        b.cut.saturating_sub(1)
    };
}

/// The house cut away at once (the opening, in bed).
pub fn cut_now(w: &mut World) {
    w.boom.cut = CUT_TICKS;
    w.boom.prev_cut = CUT_TICKS;
}

/// The house's hand-over now, `alpha` of the way from last tick: 0 whole,
/// `ONE` cut away.
pub fn cut_share(w: &World, alpha: Fx) -> Fx {
    let a = w.boom.prev_cut as i32 * ONE;
    let b = w.boom.cut as i32 * ONE;
    fx::lerp(a, b, alpha) / CUT_TICKS as i32
}

#[cfg(test)]
mod tests;
