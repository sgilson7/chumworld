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
use crate::pose::Pose;
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

/// A weapon's whole length swept over the tick (pommel to tip).
fn weapon_swept(a: &Arm) -> Swept {
    let (p, t) = (&a.pts[arm::POMMEL], &a.pts[arm::TIP]);
    Swept {
        a0: p.q,
        b0: t.q,
        a1: p.p,
        b1: t.p,
    }
}

/// The edge alone, from the guard to the tip, swept over the tick.
fn edge_swept(a: &Arm) -> Swept {
    let w = weapon_swept(a);
    let at = a.weapon.hilt as i64;
    let len = a.weapon.len.max(1) as i64;
    Swept {
        a0: part_way(w.a0, w.b0, at, len),
        b0: w.b0,
        a1: part_way(w.a1, w.b1, at, len),
        b1: w.b1,
    }
}

/// A fighter's capsules: a part, its spine last tick and this, and its
/// radius.
pub fn capsules(prev: &Pose, now: &Pose, girth_pct: i32, head: Fx) -> Vec<(Part, Swept, Fx)> {
    let seg = |a0: V3, b0: V3, a1: V3, b1: V3| Swept { a0, b0, a1, b1 };
    let torso = (ONE as i64 * 16 / 100 * girth_pct as i64 / 100) as Fx;
    let mut out = vec![
        (
            Part::Head,
            seg(prev.head, prev.head, now.head, now.head),
            head * 40 / 100,
        ),
        (
            Part::Torso,
            seg(prev.pelvis, prev.chest, now.pelvis, now.chest),
            torso,
        ),
    ];
    for s in 0..2 {
        out.push((
            Part::Arm(s),
            seg(prev.shoulder[s], prev.hand[s], now.shoulder[s], now.hand[s]),
            ONE * 55 / 1000,
        ));
        out.push((
            Part::Leg(s),
            seg(prev.hip[s], prev.foot[s], now.hip[s], now.foot[s]),
            ONE * 7 / 100,
        ));
    }
    out
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
    let w = weapon_swept(a);
    bounds(&[w.a0, w.b0, w.a1, w.b1], pad)
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
            capsules(
                &f.pose,
                &player::pose_of(w, id),
                f.build().girth_pct,
                f.build().head,
            )
        })
        .collect();
    for i in 0..n {
        for (j, body) in bodies.iter().enumerate() {
            if i == j || player::fighter(w, i).team == player::fighter(w, j).team {
                continue;
            }
            if !armed(w, i) || player::fighter(w, j).health.down() {
                set(&mut w.blades.touching, (i as u8, j as u8), false);
                continue;
            }
            strike(w, i, j, body);
        }
    }
}

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

/// The nearest points of two weapons now, as they stand.
fn nearest_now(a: &Arm, b: &Arm) -> (V3, V3) {
    let (_, _, pa, pb) = closest(
        a.pts[arm::POMMEL].p,
        a.pts[arm::TIP].p,
        b.pts[arm::POMMEL].p,
        b.pts[arm::TIP].p,
    );
    (pa, pb)
}

/// Two fighters' weapons this tick (Vagrancy's `clash_pair`).
fn clash_pair(w: &mut World, i: usize, j: usize) {
    let gap2 = BLADE_GAP as i64 * BLADE_GAP as i64;
    let (sa, sb) = (
        weapon_swept(&player::fighter(w, i).arm),
        weapon_swept(&player::fighter(w, j).arm),
    );
    // begun the tick touching (within half again the gap): free to slide
    let (_, _, a0, b0) = closest(sa.a0, sa.b0, sb.a0, sb.b0);
    let start = a0.sub(b0);
    let resting = dot(start, start) <= gap2 * 9 / 4;
    let (k, s, at, speed) = if resting {
        let (pa, pb) = {
            let (a, b) = (&player::fighter(w, i).arm, &player::fighter(w, j).arm);
            nearest_now(a, b)
        };
        let end = pa.sub(pb);
        let through = dot(end, start) <= 0 || dot(end, end) < gap2 / 4;
        if !through {
            let near = dot(end, end) <= gap2 * 9 / 4;
            set(&mut w.blades.clashing, (i as u8, j as u8), near);
            return;
        }
        (0i64, 1i64, a0.lerp(b0, ONE / 2), 0)
    } else {
        let Some(c) = sweep(&sa, &sb, BLADE_GAP, S_MAX, 1) else {
            set(&mut w.blades.clashing, (i as u8, j as u8), false);
            return;
        };
        // how fast the two met: the speeds of the touching points, apart
        let va = part_way(sa.a1.sub(sa.a0), sa.b1.sub(sa.b0), c.g as i64, ONE as i64);
        let vb = part_way(sb.a1.sub(sb.a0), sb.b1.sub(sb.b0), c.f as i64, ONE as i64);
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
    let (pa, pb) = nearest_now(a, b);
    let d = pa.sub(pb);
    if dot(d, d) < gap2 {
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

/// Fighter `i`'s edge against fighter `j`'s body.
fn strike(w: &mut World, i: usize, j: usize, body: &[(Part, Swept, Fx)]) {
    let edge = edge_swept(&player::fighter(w, i).arm);
    let pts: Vec<V3> = body
        .iter()
        .flat_map(|(_, s, _)| [s.a0, s.b0, s.a1, s.b1])
        .collect();
    let reach = body.iter().map(|(_, _, r)| *r).max().unwrap_or(0);
    let near = overlap(
        bounds(&[edge.a0, edge.b0, edge.a1, edge.b1], 0),
        bounds(&pts, reach),
    );
    let key = (i as u8, j as u8);
    let touching = w.blades.touching.contains(&key);
    if !near {
        set(&mut w.blades.touching, key, false);
        return;
    }
    if touching {
        // resting on the body: it may slide over it, never more than half
        // its radius in (then it goes back to where it began the tick), and
        // comes away when it parts
        let gap = |s: &Swept| {
            let (_, _, pa, pb) = closest(edge.a1, edge.b1, s.a1, s.b1);
            let d = pa.sub(pb);
            dot(d, d)
        };
        let deep = body.iter().any(|(_, s, r)| gap(s) < (*r as i64 / 2) * (*r as i64 / 2));
        if deep {
            let arm = &mut player::parts_mut(w, i).1.arm;
            rewind(arm, 0, 1);
            for _ in 0..REWIND_PASSES {
                arm.relax();
            }
        }
        let still = deep
            || body.iter().any(|(_, s, r)| {
                let r = (*r + COME_AWAY) as i64;
                gap(s) <= r * r
            });
        set(&mut w.blades.touching, key, still);
        return;
    }
    // the earliest contact with any part
    let mut first: Option<(Part, Contact)> = None;
    for (part, s, r) in body {
        if let Some(c) = sweep(&edge, s, *r, S_MAX, 1) {
            let earlier = first
                .as_ref()
                .is_none_or(|(_, f)| (c.k as u64) * (f.s as u64) < (f.k as u64) * (c.s as u64));
            if earlier {
                first = Some((*part, c));
            }
        }
    }
    let Some((part, c)) = first else {
        return;
    };
    // the blow's speed: the edge's at the touching point, less the body's
    let s = body
        .iter()
        .find(|(p, _, _)| *p == part)
        .map(|(_, s, _)| *s)
        .unwrap_or(edge);
    let ve = part_way(edge.a1.sub(edge.a0), edge.b1.sub(edge.b0), c.g as i64, ONE as i64);
    let vb = part_way(s.a1.sub(s.a0), s.b1.sub(s.b0), c.f as i64, ONE as i64);
    let speed = player::mps(ve.sub(vb).len());
    let arm = &mut player::parts_mut(w, i).1.arm;
    rewind(arm, c.k.saturating_sub(1) as i64, c.s as i64);
    for _ in 0..REWIND_PASSES {
        arm.relax();
    }
    w.blades.hits.push(Hit {
        by: i as u8,
        on: j as u8,
        part,
        speed,
        at: c.on_b,
    });
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
        let mut w = duel(85, -30, true);
        let most = w.foes[0].fighter.health.hp;
        let mut blows = Vec::new();
        let mut after = None;
        for t in 0..30 {
            w.step(&sweep_left());
            blows.extend(w.blades.hits.iter().copied());
            if !w.blades.hits.is_empty() {
                after = Some(t);
            }
            if after == Some(t - 1) {
                // the tick after the blow the tip has all but stopped
                assert!(w.me.arm.pts[arm::TIP].vel().len() * 60 < 2 * ONE, "the blade went on");
            }
            // pressed on, the edge never sinks more than half into the torso
            let torso = (ONE as i64 * 16 / 100 * 135 / 100) as Fx;
            let f = &w.foes[0].fighter.pose;
            let a = &w.me.arm;
            let (_, _, p, q) = closest(a.pts[arm::POMMEL].p, a.pts[arm::TIP].p, f.pelvis, f.chest);
            assert!(p.dist(q) >= torso / 2 - ONE / 100, "tick {t}: {} into the torso", torso - p.dist(q));
        }
        assert_eq!(blows.len(), 1, "one blow a contact: {blows:?}");
        let h = blows[0];
        assert_eq!((h.by, h.on, h.part), (0, 1, Part::Torso));
        // the tip ran at 6 to 7 m/s across the shell before it met the orc
        assert!(h.speed > ONE * 5 && h.speed < ONE * 9, "{}", h.speed);
        // (speed − 1.5 m/s) × 5, worked by hand from the speed recorded
        let taken = ((h.speed - ONE * 3 / 2) as i64 * 5 / ONE as i64) as i32;
        assert_eq!(most - w.foes[0].fighter.health.hp, taken);
    }

    #[test]
    fn a_block_stops_a_swing_and_the_blades_do_not_pass_through() {
        let mut w = duel(115, 10, false);
        let (mut met, mut before, mut normal) = (None, 0, None);
        for t in 0..30 {
            let speed = w.me.arm.pts[arm::TIP].vel().len();
            w.step(&sweep_left());
            let (a, b) = (&w.me.arm, &w.foes[0].fighter.arm);
            let (_, _, p, q) = closest(a.pts[arm::POMMEL].p, a.pts[arm::TIP].p, b.pts[arm::POMMEL].p, b.pts[arm::TIP].p);
            if met.is_none() && !w.blades.clashes.is_empty() {
                met = Some(t);
                before = speed;
                normal = Some(p.sub(q));
            }
            if met.is_some_and(|m| t == m + 1) {
                // within a tick of the contact the swing has stopped: under
                // half the speed it had
                assert!(a.pts[arm::TIP].vel().len() < before / 2, "{} of {before}", a.pts[arm::TIP].vel().len());
            }
            if let Some(n) = normal {
                // from the contact on, my blade stays on its own side of theirs
                let d = p.sub(q);
                assert!(dot(d, n) >= 0 || p.dist(q) > BLADE_GAP * 2, "tick {t}: the blades passed through");
            }
        }
        assert!(met.is_some(), "the blades met");
        assert!(before > ONE * 8 / 60, "the swing was under way: {before}");
        assert_eq!(w.foes[0].fighter.health.hp, w.foes[0].fighter.health.most, "a block takes nothing");
    }

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
