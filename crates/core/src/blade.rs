//! `blade` (M1 Duel, task D): blades are solid to each other and to bodies.
//! Ported from Vagrancy's `contact.rs` and `fight.rs::clash_pair`, in 3D.
//!
//! A fast blade can jump another blade, or a limb, between two ticks, so
//! contact is never tested only where things ended up. Each segment is
//! swept from where it began the tick to where it ended it, in `S`
//! substeps, `S` the largest relative travel of an end over the gap,
//! rounded up and capped at `S_MAX`; the first substep at which the two
//! come within the gap is the contact.
//!
//! **A block stops a swing.** At the contact, both weapons go back to the
//! last safe substep, the one before it, and their arms relax twice from
//! there; if that brings them back into contact, they go back again. What
//! each kept of its tick's travel is all the speed it has left. Blades that
//! begin a tick already touching may slide along each other and part
//! freely; they are only stopped if they would pass through (the line
//! between their nearest points turns over) or press deeper than half the
//! gap. Without that they would glue: every tick would find contact at its
//! first substep and go back to where it began (Vagrancy, the same).
//!
//! **A hit.** A blade's edge meeting another fighter's body stops there the
//! same way, and the blow is recorded with its speed at contact: the
//! blade's own speed at that point less the body's (`Hit`), which `health`
//! takes. One blow per blade per body per contact: a blade resting on a
//! body takes nothing more until it has come away.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use crate::arm::{self, Arm};
use crate::fx::{Fx, ONE};
use crate::geom::{v3, V3};
use crate::player;
use crate::pose::{self, Build, Kind, Pose};
use crate::world::World;

/// Two blades touch within this (each 2 cm thick).
pub const BLADE_GAP: Fx = ONE * 4 / 100;
/// A blade resting on a body has come away once it is this far off it: the
/// next contact is a new blow.
pub const COME_AWAY: Fx = ONE / 10;
/// The most substeps a pair is swept in.
pub const S_MAX: u32 = 32;
/// The arms relax this many passes after a rewind (Vagrancy: two).
pub const REWIND_PASSES: usize = 2;

/// Where on a body a blow lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Part {
    Head,
    Torso,
    Arm(usize),
    Leg(usize),
}

/// A blow this tick: who struck whom, where, how fast (metres a second).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub by: u8,
    pub on: u8,
    pub part: Part,
    pub speed: Fx,
    pub at: V3,
}

/// Two blades meeting this tick, and how fast they met.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clash {
    pub a: u8,
    pub b: u8,
    pub at: V3,
    pub speed: Fx,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    /// weapons, by their fighters, in contact at the end of last tick
    pub clashing: Vec<(u8, u8)>,
    /// a weapon resting on a body: (whose weapon, whose body)
    pub touching: Vec<(u8, u8)>,
    pub hits: Vec<Hit>,
    pub clashes: Vec<Clash>,
}

/// A segment at the start and at the end of a tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Swept {
    pub a0: V3,
    pub b0: V3,
    pub a1: V3,
    pub b1: V3,
}

impl Swept {
    fn at(&self, k: i64, s: i64) -> (V3, V3) {
        (part_way(self.a0, self.a1, k, s), part_way(self.b0, self.b1, k, s))
    }
}

/// `k / s` of the way from `a` to `b`.
fn part_way(a: V3, b: V3, k: i64, s: i64) -> V3 {
    let f = |x: i32, y: i32| x + ((y as i64 - x as i64) * k / s) as i32;
    v3(f(a.x, b.x), f(a.y, b.y), f(a.z, b.z))
}

fn dot(a: V3, b: V3) -> i64 {
    a.x as i64 * b.x as i64 + a.y as i64 * b.y as i64 + a.z as i64 * b.z as i64
}

/// Where a swept contact happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Contact {
    /// the substep, `0..=s`, and how many there were
    pub k: u32,
    pub s: u32,
    /// the fraction along the first segment, and along the second
    pub g: Fx,
    pub f: Fx,
    /// the nearest points, at the substep of contact
    pub on_a: V3,
    pub on_b: V3,
}

/// How many substeps a pair needs: the largest relative travel of their
/// ends over the gap, rounded up.
pub fn substeps(a: &Swept, b: &Swept, gap: Fx, s_max: u32) -> u32 {
    let travel = |p0: V3, p1: V3, q0: V3, q1: V3| p1.sub(p0).sub(q1.sub(q0)).len();
    let d = [
        travel(a.a0, a.a1, b.a0, b.a1),
        travel(a.a0, a.a1, b.b0, b.b1),
        travel(a.b0, a.b1, b.a0, b.a1),
        travel(a.b0, a.b1, b.b0, b.b1),
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    let r = gap.max(1) as i64;
    (((d as i64 + r - 1) / r).max(1) as u32).min(s_max.max(1))
}

/// The first substep, from `from` on, at which `a` comes within `gap` of `b`.
pub fn sweep(a: &Swept, b: &Swept, gap: Fx, s_max: u32, from: u32) -> Option<Contact> {
    let s = substeps(a, b, gap, s_max);
    let g2 = gap as i64 * gap as i64;
    for k in from..=s {
        let (p0, p1) = a.at(k as i64, s as i64);
        let (q0, q1) = b.at(k as i64, s as i64);
        let (g, f, pa, pb) = closest(p0, p1, q0, q1);
        let d = pa.sub(pb);
        if dot(d, d) <= g2 {
            return Some(Contact {
                k,
                s,
                g,
                f,
                on_a: pa,
                on_b: pb,
            });
        }
    }
    None
}

/// Closest points between segments `p0→p1` and `q0→q1`: the fractions along
/// each (`0..=ONE`) and the points. After Ericson, *Real-Time Collision
/// Detection* §5.1.9, in integers (Vagrancy's `contact::closest`, in 3D).
/// Parallel segments that overlap give the middle of the overlap.
pub fn closest(p0: V3, p1: V3, q0: V3, q1: V3) -> (Fx, Fx, V3, V3) {
    let d1 = p1.sub(p0);
    let d2 = q1.sub(q0);
    let r = p0.sub(q0);
    let a = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);
    let one = ONE as i64;
    let clamp01 = |num: i64, den: i64| -> i64 {
        if den == 0 {
            0
        } else {
            ((num as i128 * one as i128 / den as i128) as i64).clamp(0, one)
        }
    };
    let (s, t);
    if a == 0 && e == 0 {
        return (0, 0, p0, q0);
    }
    if a == 0 {
        s = 0;
        t = clamp01(f, e);
    } else {
        let c = dot(d1, r);
        if e == 0 {
            t = 0;
            s = clamp01(-c, a);
        } else {
            let b = dot(d1, d2);
            let denom = a as i128 * e as i128 - b as i128 * b as i128;
            if denom.abs() <= (a as i128 * e as i128) >> 24 {
                // parallel, or nearly: the middle of the overlap
                let s_q0 = clamp01(-c, a);
                let s_q1 = clamp01(dot(d1, q1.sub(p0)), a);
                s = (s_q0 + s_q1) / 2;
                let ps = p0.add(scale64(d1, s));
                t = clamp01(dot(d2, ps.sub(q0)), e);
            } else {
                let mut sv = ((b as i128 * f as i128 - c as i128 * e as i128) * one as i128 / denom) as i64;
                sv = sv.clamp(0, one);
                let tn = (b as i128 * sv as i128 / one as i128) as i64 + f;
                let mut tv = (tn as i128 * one as i128 / e as i128) as i64;
                if tv < 0 {
                    tv = 0;
                    sv = clamp01(-c, a);
                } else if tv > one {
                    tv = one;
                    sv = clamp01(b - c, a);
                }
                s = sv;
                t = tv;
            }
        }
    }
    (s as Fx, t as Fx, p0.add(scale64(d1, s)), q0.add(scale64(d2, t)))
}

fn scale64(v: V3, t: i64) -> V3 {
    let f = |c: i32| (c as i64 * t / ONE as i64) as i32;
    v3(f(v.x), f(v.y), f(v.z))
}

/// A weapon's rails (`arm::rails`) swept over the tick, each with its
/// radius: against its own body (`own`) from the guard, otherwise whole.
pub fn rails_swept(a: &Arm, own: bool) -> Vec<(Swept, Fx)> {
    arm::rails(a.weapon.item)
        .iter()
        .map(|r| {
            let (a1, b1, _) = a.rail(r, own, false);
            let (a0, b0, _) = a.rail(r, own, true);
            (Swept { a0, b0, a1, b1 }, r.r)
        })
        .collect()
}

/// One part of a body as a capsule: a segment and a radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capsule {
    pub part: Part,
    pub a: V3,
    pub b: V3,
    pub r: Fx,
}

/// The capsules' radii, millimetres on the player's 1.70 m (M3's figure,
/// `models/figure.rs`: the belly 160 mm across and 122 mm deep, the yoke
/// 188 by 118, the head 190 by 205 by 182, an orc's 222 by 200 by 205, the
/// limbs 56, 52, 76 and 63 with balls of 57 to 70 at the joints). A torso
/// capsule is round, so it is set between the belly's width and its depth.
pub const HEAD_MM: i32 = 192;
pub const ORC_HEAD_MM: i32 = 212;
pub const TORSO_MM: i32 = 145;
pub const UPPER_ARM_MM: i32 = 60;
pub const FOREARM_MM: i32 = 58;
pub const THIGH_MM: i32 = 78;
pub const SHIN_MM: i32 = 66;
/// The capsules a body has: the head, the torso, two segments an arm, two
/// a leg.
pub const PARTS: usize = 10;
/// Which of them are the trunk and legs (not the arms): what two bodies
/// keep apart (`motion::separate`) and what a fighter's own blade keeps out
/// of.
pub const TRUNK: [usize; 6] = [0, 1, 6, 7, 8, 9];

/// Millimetres on the player, up a build's height.
fn tall(b: &Build, mm: i32) -> Fx {
    (mm as i64 * b.height as i64 / 1700) as Fx
}

/// The same, across, widened by its girth.
fn broad(b: &Build, mm: i32) -> Fx {
    tall(b, mm) * b.girth_pct / 100
}

/// A body's capsules on a pose, sized to the figure M3 draws on it.
pub fn body(kind: Kind, p: &Pose) -> [Capsule; PARTS] {
    let b = pose::build(kind);
    let head = if kind == Kind::Orc { ORC_HEAD_MM } else { HEAD_MM };
    let c = |part, a, bb, r| Capsule { part, a, b: bb, r };
    [
        c(Part::Head, p.head, p.head, tall(&b, head)),
        c(Part::Torso, p.pelvis, p.chest, broad(&b, TORSO_MM)),
        c(Part::Arm(0), p.shoulder[0], p.elbow[0], broad(&b, UPPER_ARM_MM)),
        c(Part::Arm(0), p.elbow[0], p.hand[0], broad(&b, FOREARM_MM)),
        c(Part::Arm(1), p.shoulder[1], p.elbow[1], broad(&b, UPPER_ARM_MM)),
        c(Part::Arm(1), p.elbow[1], p.hand[1], broad(&b, FOREARM_MM)),
        c(Part::Leg(0), p.hip[0], p.knee[0], broad(&b, THIGH_MM)),
        c(Part::Leg(0), p.knee[0], p.foot[0], broad(&b, SHIN_MM)),
        c(Part::Leg(1), p.hip[1], p.knee[1], broad(&b, THIGH_MM)),
        c(Part::Leg(1), p.knee[1], p.foot[1], broad(&b, SHIN_MM)),
    ]
}

/// How far two capsules sink into each other (0 or less: apart), and the
/// way from the first to the second at their nearest points.
pub fn sunk(x: &Capsule, y: &Capsule) -> (Fx, V3) {
    let (_, _, p, q) = closest(x.a, x.b, y.a, y.b);
    let d = q.sub(p);
    (x.r + y.r - d.len(), d)
}

/// A fighter's capsules swept over the tick: a part, its segment last tick
/// and this, and its radius.
pub fn capsules(kind: Kind, prev: &Pose, now: &Pose) -> Vec<(Part, Swept, Fx)> {
    let (a, b) = (body(kind, prev), body(kind, now));
    a.iter()
        .zip(b.iter())
        .map(|(p, n)| {
            (
                n.part,
                Swept {
                    a0: p.a,
                    b0: p.b,
                    a1: n.a,
                    b1: n.b,
                },
                n.r,
            )
        })
        .collect()
}

/// The box round a set of points, grown by `pad`.
fn bounds(pts: &[V3], pad: Fx) -> (V3, V3) {
    let mut lo = v3(i32::MAX, i32::MAX, i32::MAX);
    let mut hi = v3(i32::MIN, i32::MIN, i32::MIN);
    for p in pts {
        lo = v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    (
        v3(lo.x - pad, lo.y - pad, lo.z - pad),
        v3(hi.x + pad, hi.y + pad, hi.z + pad),
    )
}

fn overlap(a: (V3, V3), b: (V3, V3)) -> bool {
    a.0.x <= b.1.x && b.0.x <= a.1.x && a.0.y <= b.1.y && b.0.y <= a.1.y && a.0.z <= b.1.z && b.0.z <= a.1.z
}

fn weapon_box(a: &Arm, pad: Fx) -> (V3, V3) {
    let pts: Vec<V3> = rails_swept(a, false)
        .iter()
        .flat_map(|(w, _)| [w.a0, w.b0, w.a1, w.b1])
        .collect();
    bounds(&pts, pad + ONE / 20)
}

/// Whether a fighter's weapon meets anything: drawn, in hand, standing.
fn armed(w: &World, id: usize) -> bool {
    let f = player::fighter(w, id);
    f.arm.in_hand && !f.health.down()
}

/// Step 4, after the arms: blade against blade, then blade against body.
pub fn step(w: &mut World) {
    w.blades.hits.clear();
    w.blades.clashes.clear();
    let n = player::count(w);
    for i in 0..n {
        for j in i + 1..n {
            let (ti, tj) = (player::fighter(w, i).team, player::fighter(w, j).team);
            if ti == tj || !armed(w, i) || !armed(w, j) {
                set(&mut w.blades.clashing, (i as u8, j as u8), false);
                continue;
            }
            let near = overlap(
                weapon_box(&player::fighter(w, i).arm, BLADE_GAP),
                weapon_box(&player::fighter(w, j).arm, BLADE_GAP),
            );
            if !near {
                set(&mut w.blades.clashing, (i as u8, j as u8), false);
                continue;
            }
            clash_pair(w, i, j);
        }
    }
    // every body's capsules, last tick's and this tick's
    let bodies: Vec<Vec<(Part, Swept, Fx)>> = (0..n)
        .map(|id| {
            let f = player::fighter(w, id);
            capsules(f.kind, &f.pose, &player::pose_of(w, id))
        })
        .collect();
    for i in 0..n {
        for (j, body) in bodies.iter().enumerate() {
            // a weapon held in a hand is solid to every body, its own and its
            // side's too; it takes health from a foe standing
            let f = player::fighter(w, i);
            let held = f.arm.in_hand && (f.arm.hold[0] || f.arm.hold[1]);
            if !held {
                set(&mut w.blades.touching, (i as u8, j as u8), false);
                continue;
            }
            let blow = armed(w, i)
                && i != j
                && f.team != player::fighter(w, j).team
                && !player::fighter(w, j).health.down();
            strike(w, i, j, body, blow);
        }
    }
    // every arm has moved: each keeps out of the others where they now are
    let mut caps = arm::all_capsules(w);
    for id in 0..n {
        let near = arm::fences_of(w, id, &caps, true);
        let a = &mut player::parts_mut(w, id).1.arm;
        a.fences = near;
        for _ in 0..SETTLE_PASSES {
            a.relax();
        }
        caps[id] = body(player::fighter(w, id).kind, &player::pose_of(w, id));
    }
}

/// Relaxation passes each arm takes at the end of the blades' step, against
/// the bodies as every arm has left them.
pub const SETTLE_PASSES: usize = 2;

fn set(list: &mut Vec<(u8, u8)>, k: (u8, u8), on: bool) {
    list.retain(|&x| x != k);
    if on {
        list.push(k);
        list.sort();
    }
}

/// Two weapons' every point put back `k / s` of the way through the tick.
fn rewind(a: &mut Arm, k: i64, s: i64) {
    for i in [arm::POMMEL, arm::TIP] {
        let pt = &mut a.pts[i];
        pt.p = part_way(pt.q, pt.p, k, s);
    }
}

/// The two rails, one of each weapon, nearest to touching: their indices,
/// their nearest points and how far those are apart past both radii, at
/// the tick's start (`prev`) or now.
fn nearest_rails(a: &[(Swept, Fx)], b: &[(Swept, Fx)], prev: bool) -> (usize, usize, V3, V3, i64) {
    let mut best = (0, 0, V3::ZERO, V3::ZERO, i64::MAX);
    for (x, (sa, ra)) in a.iter().enumerate() {
        for (y, (sb, rb)) in b.iter().enumerate() {
            let (pa, pb) = if prev {
                let (_, _, p, q) = closest(sa.a0, sa.b0, sb.a0, sb.b0);
                (p, q)
            } else {
                let (_, _, p, q) = closest(sa.a1, sa.b1, sb.a1, sb.b1);
                (p, q)
            };
            let gap = pa.dist(pb) as i64 - (*ra + *rb) as i64;
            if gap < best.4 {
                best = (x, y, pa, pb, gap);
            }
        }
    }
    best
}

/// Two fighters' weapons this tick (Vagrancy's `clash_pair`), rail by rail.
fn clash_pair(w: &mut World, i: usize, j: usize) {
    let (sa, sb) = (
        rails_swept(&player::fighter(w, i).arm, false),
        rails_swept(&player::fighter(w, j).arm, false),
    );
    // begun the tick touching (within half again the gap): free to slide
    let (x, y, a0, b0, gap0) = nearest_rails(&sa, &sb, true);
    let room = (sa[x].1 + sb[y].1) as i64;
    let start = a0.sub(b0);
    let resting = gap0 <= room / 2;
    let (k, s, at, speed) = if resting {
        let (sx, sy) = (&sa[x].0, &sb[y].0);
        let (_, _, pa, pb) = closest(sx.a1, sx.b1, sy.a1, sy.b1);
        let end = pa.sub(pb);
        let through = dot(end, start) <= 0 || (end.len() as i64) < room / 2;
        if !through {
            let near = nearest_rails(&sa, &sb, false).4 <= room / 2;
            set(&mut w.blades.clashing, (i as u8, j as u8), near);
            return;
        }
        (0i64, 1i64, a0.lerp(b0, ONE / 2), 0)
    } else {
        let mut first: Option<(Contact, usize, usize)> = None;
        for (x, (ra, ga)) in sa.iter().enumerate() {
            for (y, (rb, gb)) in sb.iter().enumerate() {
                if let Some(c) = sweep(ra, rb, *ga + *gb, S_MAX, 1) {
                    let earlier = first
                        .as_ref()
                        .is_none_or(|(f, _, _)| (c.k as u64) * (f.s as u64) < (f.k as u64) * (c.s as u64));
                    if earlier {
                        first = Some((c, x, y));
                    }
                }
            }
        }
        let Some((c, x, y)) = first else {
            set(&mut w.blades.clashing, (i as u8, j as u8), false);
            return;
        };
        // how fast the two met: the speeds of the touching points, apart
        let (ra, rb) = (&sa[x].0, &sb[y].0);
        let va = part_way(ra.a1.sub(ra.a0), ra.b1.sub(ra.b0), c.g as i64, ONE as i64);
        let vb = part_way(rb.a1.sub(rb.a0), rb.b1.sub(rb.b0), c.f as i64, ONE as i64);
        (
            c.k.saturating_sub(1) as i64,
            c.s as i64,
            c.on_a.lerp(c.on_b, ONE / 2),
            player::mps(va.sub(vb).len()),
        )
    };
    let mut arms = player::arms_mut(w);
    let (lo, hi) = arms.split_at_mut(j);
    let (a, b) = (&mut *lo[i], &mut *hi[0]);
    rewind(a, k, s);
    rewind(b, k, s);
    let safe = (a.pts, b.pts);
    for _ in 0..REWIND_PASSES {
        a.relax();
        b.relax();
    }
    if nearest_rails(&rails_swept(a, false), &rails_swept(b, false), false).4 < 0 {
        // the relaxation pulled them back into each other: back again
        for k in [arm::POMMEL, arm::TIP] {
            a.pts[k].p = safe.0[k].p;
            b.pts[k].p = safe.1[k].p;
        }
    }
    let first = !w.blades.clashing.contains(&(i as u8, j as u8));
    if first {
        w.blades.clashes.push(Clash {
            a: i as u8,
            b: j as u8,
            at,
            speed,
        });
    }
    set(&mut w.blades.clashing, (i as u8, j as u8), true);
}

/// Fighter `i`'s weapon against fighter `j`'s body (its own when `i ==
/// j`: the head, torso and legs, the blade from the guard): swept, so a
/// fast blade cannot jump a body between ticks, and stopped at the first
/// contact as against a blade. With `blow`, the contact is a blow, taken
/// with the speed it landed at. A weapon resting on a body slides over it
/// freely, held out of it by the arm's fences (`arm::Arm::fence`), and
/// comes away once it parts.
fn strike(w: &mut World, i: usize, j: usize, body: &[(Part, Swept, Fx)], blow: bool) {
    let own = i == j;
    let rails = rails_swept(&player::fighter(w, i).arm, own);
    let parts: Vec<&(Part, Swept, Fx)> = body
        .iter()
        .enumerate()
        .filter(|(n, _)| !own || TRUNK.contains(n))
        .map(|(_, c)| c)
        .collect();
    let pts: Vec<V3> = parts
        .iter()
        .flat_map(|(_, s, _)| [s.a0, s.b0, s.a1, s.b1])
        .collect();
    let reach = parts.iter().map(|(_, _, r)| *r).max().unwrap_or(0);
    let edge: Vec<V3> = rails.iter().flat_map(|(s, _)| [s.a0, s.b0, s.a1, s.b1]).collect();
    let thick = rails.iter().map(|(_, r)| *r).max().unwrap_or(0);
    let key = (i as u8, j as u8);
    if !overlap(bounds(&edge, thick), bounds(&pts, reach)) {
        set(&mut w.blades.touching, key, false);
        return;
    }
    if w.blades.touching.contains(&key) {
        let still = rails.iter().any(|(e, re)| {
            parts.iter().any(|(_, s, r)| {
                let (_, _, pa, pb) = closest(e.a1, e.b1, s.a1, s.b1);
                pa.dist(pb) <= *re + *r + COME_AWAY
            })
        });
        set(&mut w.blades.touching, key, still);
        return;
    }
    // the earliest contact of any rail with any part
    let mut first: Option<(Part, Contact, usize, usize)> = None;
    for (x, (e, re)) in rails.iter().enumerate() {
        for (y, (part, s, r)) in parts.iter().enumerate() {
            if let Some(c) = sweep(e, s, *re + *r, S_MAX, 1) {
                let earlier = first
                    .as_ref()
                    .is_none_or(|(_, f, _, _)| (c.k as u64) * (f.s as u64) < (f.k as u64) * (c.s as u64));
                if earlier {
                    first = Some((*part, c, x, y));
                }
            }
        }
    }
    let Some((part, c, x, y)) = first else {
        return;
    };
    // the blow's speed: the edge's at the touching point, less the body's
    let (e, s) = (&rails[x].0, &parts[y].1);
    let ve = part_way(e.a1.sub(e.a0), e.b1.sub(e.b0), c.g as i64, ONE as i64);
    let vb = part_way(s.a1.sub(s.a0), s.b1.sub(s.b0), c.f as i64, ONE as i64);
    let speed = player::mps(ve.sub(vb).len());
    let arm = &mut player::parts_mut(w, i).1.arm;
    rewind(arm, c.k.saturating_sub(1) as i64, c.s as i64);
    for _ in 0..REWIND_PASSES {
        arm.relax();
    }
    if blow {
        w.blades.hits.push(Hit {
            by: i as u8,
            on: j as u8,
            part,
            speed,
            at: c.on_b,
        });
    }
    set(&mut w.blades.touching, key, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::int;
    use crate::world::World;

    /// Lengths in units of 1/64 m (about 1.6 cm), which Q16 holds exactly,
    /// so the expected points come out to the unit.
    fn cm(x: i32, y: i32, z: i32) -> V3 {
        v3(x * ONE / 64, y * ONE / 64, z * ONE / 64)
    }

    const GAP: Fx = ONE / 64;

    /// Vagrancy's H2 in 3D: a blade 10 cm long moving 20 cm along x in one
    /// tick across a limb at x = 4 cm (radius 1 cm) touches it at neither
    /// end of the tick; the sweep finds it at the middle of the limb.
    #[test]
    fn a_blade_that_jumps_a_limb_is_caught_between_ticks() {
        let blade = Swept {
            a0: cm(-10, 5, 3),
            b0: cm(0, 5, 3),
            a1: cm(10, 5, 3),
            b1: cm(20, 5, 3),
        };
        let limb = Swept {
            a0: cm(4, 0, 3),
            b0: cm(4, 10, 3),
            a1: cm(4, 0, 3),
            b1: cm(4, 10, 3),
        };
        let c = sweep(&blade, &limb, GAP, 64, 1).expect("the swept test finds the limb");
        assert_eq!(c.on_b, cm(4, 5, 3), "the middle of the limb's spine");
        assert_eq!(c.f, ONE / 2);
        // where they began and ended, they do not touch
        let (_, _, p, q) = closest(blade.a0, blade.b0, limb.a0, limb.b0);
        assert!(p.dist(q) > GAP);
        let (_, _, p, q) = closest(blade.a1, blade.b1, limb.a1, limb.b1);
        assert!(p.dist(q) > GAP);
    }

    #[test]
    fn a_blade_moving_sideways_is_not_missed_between_substeps() {
        // Vagrancy's H2b: a 4 cm blade moving 20 cm across a limb at x = 10:
        // substeps of 4 cm miss it; S = 20 / 1 = 20 finds it at (10, 5)
        let blade = Swept {
            a0: cm(0, 3, 0),
            b0: cm(0, 7, 0),
            a1: cm(20, 3, 0),
            b1: cm(20, 7, 0),
        };
        let limb = Swept {
            a0: cm(10, 0, 0),
            b0: cm(10, 10, 0),
            a1: cm(10, 0, 0),
            b1: cm(10, 10, 0),
        };
        assert_eq!(substeps(&blade, &limb, GAP, 64), 20);
        assert!(sweep(&blade, &limb, GAP, 5, 1).is_none(), "five substeps miss it");
        let c = sweep(&blade, &limb, GAP, 64, 1).expect("twenty find it");
        assert_eq!(c.on_b, cm(10, 5, 0));
    }

    /// The player, sword drawn, its hand wound out to the right at `pitch`,
    /// and a still orc `gap` centimetres ahead facing them, its own blade
    /// held at the guard or (`high`) up out of the way.
    fn duel(gap: i32, pitch: i32, high: bool) -> World {
        let mut w = World::new(3);
        player::act(&mut w, "pilot", "still").unwrap();
        player::act(&mut w, "spawn", "orc").unwrap();
        let ahead = crate::geom::facing(w.aim.yaw, 0).scale(gap * ONE / 100);
        w.foes[0].body.pos = w.player.pos.add(ahead);
        w.foes[0].body.prev_pos = w.foes[0].body.pos;
        if high {
            let s = &mut w.foes[0].fighter.arm.shell;
            s.side = arm::SIDE_MAX;
            s.pitch = arm::PITCH_MAX;
        }
        w.step(&crate::input::Input {
            keys: crate::input::DRAW,
            ..Default::default()
        });
        for _ in 0..30 {
            w.step(&Default::default());
        }
        w.me.arm.shell.side = crate::fx::deg(70);
        w.me.arm.shell.pitch = crate::fx::deg(pitch);
        for _ in 0..60 {
            w.step(&Default::default());
        }
        w
    }

    /// A committed sweep to the left: 150 px a tick with the left button.
    fn sweep_left() -> crate::input::Input {
        crate::input::Input {
            dx: -15_000,
            buttons: crate::input::SWING,
            ..Default::default()
        }
    }

    #[test]
    fn a_blade_swung_into_a_body_stops_there_and_takes_health_by_its_speed() {
        let mut w = duel(115, -10, true);
        let most = w.foes[0].fighter.health.hp;
        let mut blows = Vec::new();
        let mut after = None;
        for t in 0..30 {
            w.step(&sweep_left());
            blows.extend(w.blades.hits.iter().copied());
            if !w.blades.hits.is_empty() {
                after = Some(t);
            }
            if let Some(a) = after.filter(|&a| a + 1 == t) {
                // the tick after the blow the swing has stopped: the tip goes
                // on at under half the speed it landed at
                let landed = blows.last().map(|h: &Hit| h.speed).unwrap_or(0);
                assert!(
                    player::mps(w.me.arm.pts[arm::TIP].vel().len()) < landed / 2,
                    "the blade went on after tick {a}"
                );
            }
            // pressed on, the edge stays out of the torso (to the blade's
            // half-width and a millimetre)
            let torso = body(Kind::Orc, &w.foes[0].fighter.pose)[1];
            let a = &w.me.arm;
            let (_, _, p, q) = closest(a.pts[arm::POMMEL].p, a.pts[arm::TIP].p, torso.a, torso.b);
            assert!(
                p.dist(q) >= torso.r + arm::rails(a.weapon.item)[0].r - ONE / 1000,
                "tick {t}: {} into the torso",
                torso.r - p.dist(q)
            );
        }
        assert_eq!(blows.len(), 1, "one blow a contact: {blows:?}");
        let h = blows[0];
        assert_eq!((h.by, h.on, h.part), (0, 1, Part::Torso));
        // a committed sweep through the front (`player::through_front`) runs
        // the edge at 10 to 13 m/s where it meets the orc 1.15 m off
        assert!(h.speed > ONE * 8 && h.speed < ONE * 16, "{}", h.speed);
        // (speed − 1.5 m/s) × 5, worked by hand from the speed recorded
        let taken = ((h.speed - ONE * 3 / 2) as i64 * 5 / ONE as i64) as i32;
        assert_eq!(most - w.foes[0].fighter.health.hp, taken);
    }

    /// A blade that crosses a whole body in one tick (set by hand: a tick
    /// ago 30 cm to the orc's left at chest height, now 30 cm to its right,
    /// both clear of its torso) is caught by the sweep, stopped on the side
    /// it came from, and the blow is taken.
    #[test]
    fn a_blade_that_crosses_a_body_in_one_tick_is_stopped_on_the_near_side() {
        let mut w = duel(160, 0, true);
        let orc = w.foes[0].fighter.pose;
        let mid = orc.pelvis.lerp(orc.chest, ONE * 3 / 4);
        let across = crate::geom::right_of(w.foes[0].body.facing);
        let toward = w.player.pos.sub(w.foes[0].body.pos);
        let toward = v3(toward.x, 0, toward.z).norm();
        let a = &mut w.me.arm;
        let len = a.weapon.len;
        // the blade lies toward the orc, its middle crossing the orc's spine
        for (side, prev) in [(ONE * 3 / 10, true), (-ONE * 3 / 10, false)] {
            let at = mid.add(across.scale(side));
            let pommel = at.add(toward.scale(len / 2));
            let tip = at.sub(toward.scale(len / 2));
            if prev {
                a.pts[arm::POMMEL].q = pommel;
                a.pts[arm::TIP].q = tip;
            } else {
                a.pts[arm::POMMEL].p = pommel;
                a.pts[arm::TIP].p = tip;
            }
        }
        w.me.pose = player::pose_of(&w, 0);
        w.blades.touching.clear();
        step(&mut w);
        assert!(
            w.blades.hits.iter().any(|h| h.on == 1 && h.part == Part::Torso),
            "{:?}",
            w.blades.hits
        );
        let a = &w.me.arm;
        let (_, _, p, q) = closest(a.pts[arm::POMMEL].p, a.pts[arm::TIP].p, orc.pelvis, orc.chest);
        assert!(p.sub(q).dot(across) > 0, "the blade went through to the far side");
    }

    #[test]
    fn a_block_stops_a_swing_and_the_blades_do_not_pass_through() {
        let mut w = duel(125, 10, false);
        let (mut met, mut before, mut normal) = (None, 0, None);
        for t in 0..30 {
            let speed = w.me.arm.pts[arm::TIP].vel().len();
            w.step(&sweep_left());
            let (a, b) = (&w.me.arm, &w.foes[0].fighter.arm);
            let (_, _, p, q) = closest(
                a.pts[arm::POMMEL].p,
                a.pts[arm::TIP].p,
                b.pts[arm::POMMEL].p,
                b.pts[arm::TIP].p,
            );
            if met.is_none() && !w.blades.clashes.is_empty() {
                met = Some(t);
                before = speed;
                normal = Some(p.sub(q));
            }
            if met.is_some_and(|m| t == m + 1) {
                // within a tick of the contact the swing has stopped: under
                // half the speed it had
                assert!(
                    a.pts[arm::TIP].vel().len() < before / 2,
                    "{} of {before}",
                    a.pts[arm::TIP].vel().len()
                );
            }
            if let Some(n) = normal {
                // from the contact on, my blade stays on its own side of theirs
                let d = p.sub(q);
                assert!(
                    dot(d, n) >= 0 || p.dist(q) > BLADE_GAP * 2,
                    "tick {t}: the blades passed through"
                );
            }
        }
        assert!(met.is_some(), "the blades met");
        assert!(before > ONE * 8 / 60, "the swing was under way: {before}");
        assert_eq!(
            w.foes[0].fighter.health.hp, w.foes[0].fighter.health.most,
            "a block takes nothing"
        );
    }

    /// How deep any held weapon's rails and any hand sit in any body at
    /// the end of a tick, and where: every weapon against every body (its
    /// own: head, torso and legs, the blade from the guard), every hand
    /// against every other body and its own head.
    pub fn worst_sink(w: &World) -> (Fx, String) {
        let n = player::count(w);
        let bodies: Vec<_> = (0..n)
            .map(|id| body(player::fighter(w, id).kind, &player::pose_of(w, id)))
            .collect();
        let mut worst = (0, String::new());
        for i in 0..n {
            let a = &player::fighter(w, i).arm;
            for (j, caps) in bodies.iter().enumerate() {
                let own = i == j;
                for (k, c) in caps.iter().enumerate() {
                    if own && !TRUNK.contains(&k) {
                        continue;
                    }
                    if a.in_hand {
                        for r in arm::rails(a.weapon.item) {
                            let (x, y, _) = a.rail(r, own, false);
                            let (_, _, p, q) = closest(x, y, c.a, c.b);
                            let sink = r.r + c.r - p.dist(q);
                            if sink > worst.0 {
                                worst = (sink, format!("{i}'s weapon in {j}'s {:?}", c.part));
                            }
                        }
                    }
                    if own && c.part != Part::Head {
                        continue;
                    }
                    for h in arm::HAND {
                        let p = a.pts[h].p;
                        let q = arm::nearest_on(c.a, c.b, p);
                        let sink = a.joint_r.1 + c.r - p.dist(q);
                        if sink > worst.0 {
                            worst = (sink, format!("{i}'s hand in {j}'s {:?}", c.part));
                        }
                    }
                }
            }
        }
        worst
    }

    /// The player's records for a recorded duel: drawn, walking in and out,
    /// the hand swept side to side and up and down, swings now and then.
    pub fn recorded(ticks: usize) -> Vec<crate::input::Input> {
        use crate::input::{self, Input};
        (0..ticks)
            .map(|k| Input {
                keys: match k {
                    3 => input::DRAW,
                    _ if k % 120 < 30 => input::FORWARD,
                    _ if k % 120 < 45 => input::LEFT,
                    _ if k % 240 > 200 => input::BACK,
                    _ => 0,
                },
                buttons: if k % 90 > 70 { input::SWING } else { 0 },
                dx: if k % 90 < 60 { 450 } else { -2_400 },
                dy: if k % 50 < 25 { 300 } else { -300 },
                wheel: if k % 200 == 100 { 10_000 } else { 0 },
            })
            .collect()
    }

    /// R3.23: over recorded duels (the player against an orc's tree, a
    /// bandit's and a leader's, and against three bandits), no weapon and
    /// no hand ever sits in a body by more than `SINK_MM` at the end of a
    /// tick: blades that begin inside are pushed out, and fast ones are
    /// stopped by the sweep before they pass through.
    #[test]
    fn no_blade_or_hand_sits_inside_a_body_over_a_recorded_duel() {
        let mut worst = (0, String::new());
        for (seed, kinds) in [
            (3u64, &["orc"][..]),
            (4, &["bandit"][..]),
            (6, &["leader"][..]),
            (5, &["bandit", "bandit", "bandit"][..]),
        ] {
            let mut w = World::new(seed);
            w.me.health = crate::health::Health::new(100_000);
            for k in kinds {
                player::act(&mut w, "spawn", k).unwrap();
            }
            for (t, i) in recorded(900).iter().enumerate() {
                w.step(i);
                let s = worst_sink(&w);
                if s.0 > worst.0 {
                    worst = (s.0, format!("{kinds:?} tick {t}: {}", s.1));
                }
            }
        }
        assert!(worst.0 <= SINK, "{} units: {}", worst.0, worst.1);
    }

    /// How deep a blade or hand may sit in a body at the end of a tick:
    /// 12.5 mm. Measured over these 3 600 ticks: six ticks over 3 mm, the
    /// worst 788 units (12 mm), a bandit's sword pressed into the player's
    /// forearm held level against the torso, where the arm's way out and
    /// the torso's push against each other until the rounds run out
    /// (`arm::FENCE_ROUNDS`).
    const SINK: Fx = ONE / 80;

    #[test]
    fn closest_points_of_crossing_skew_and_apart_segments() {
        // crossing in one plane
        let (s, t, p, q) = closest(cm(0, 0, 0), cm(10, 0, 0), cm(5, -5, 0), cm(5, 5, 0));
        assert_eq!((s, t, p, q), (ONE / 2, ONE / 2, cm(5, 0, 0), cm(5, 0, 0)));
        // skew: one along x at z = 0, one along y at z = 3, crossing above
        // (5, 0): 3 cm apart
        let (_, _, p, q) = closest(cm(0, 0, 0), cm(10, 0, 0), cm(5, -5, 3), cm(5, 5, 3));
        assert_eq!((p, q), (cm(5, 0, 0), cm(5, 0, 3)));
        // apart: the nearest ends
        let (_, _, p, q) = closest(cm(0, 0, 0), cm(10, 0, 0), cm(20, 3, 0), cm(30, 3, 0));
        assert_eq!((p, q), (cm(10, 0, 0), cm(20, 3, 0)));
        let _ = int(1);
    }
}
