//! `throwables` (M1 Duel, task D): rocks and knives, thrown along the aim
//! with gravity. F winds the free hand back for `WIND_TICKS` and lets go;
//! the stone flies until it meets a body, a prop or the ground. Steps at 5
//! in `World::step`, after the blades.
//!
//! **The hook for M5.** `State::landed` lists every throwable that came to
//! a stop this tick, where, how fast, and what it met (`Impact`). A prop is
//! anything placed in `State::props` as a box; M5 puts the merchant's cart
//! there and reads `landed` for an impact on it, which stops the cart.
//!
//! The player throws what M4's inventory holds ready in the free hand
//! (`next`); a foe carries its own `Pouch` from `data/pilots.json`
//! (SECOND-ORDER.md R3.13).
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use crate::arm::GRAVITY;
use crate::blade::{self, Part, Swept};
use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{facing, v3, Affine, V3};
use crate::input::{self, Input};
use crate::items::Item;
use crate::look::{rgb, Look};
use crate::player;
use crate::pose::Held;
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::world::World;

/// Ticks from F to the throw.
pub const WIND_TICKS: u8 = 12;
/// Ticks after a throw before the next may start.
pub const RECOVER_TICKS: u8 = 18;
/// Thrown this far above the line of the aim, so a throw at a foe a few
/// metres off arcs onto them.
pub const LOB: i32 = deg(9);
/// How fast each leaves the hand, metres a second.
pub const ROCK_SPEED: Fx = fx::int(15);
pub const KNIFE_SPEED: Fx = fx::int(19);
/// A rock's radius and a knife's length.
pub const ROCK_R: Fx = ONE * 6 / 100;
pub const KNIFE_LEN: Fx = ONE * 22 / 100;
/// What a blow takes for each metre a second it lands at, hundredths of a
/// blade's (`health::PER_MPS_PCT`): a rock bruises, a knife cuts.
pub const ROCK_PCT: i32 = 25;
pub const KNIFE_PCT: i32 = 60;
/// A thrower's own body is passed by its throw for this long.
pub const SELF_TICKS: u16 = 12;
/// Stones that have come to rest stay this long, the most `RESTING` of them.
pub const REST_TICKS: u32 = 600;
pub const RESTING: usize = 24;
/// Nothing flies longer than this (a throw off into the distance).
pub const FLIGHT_TICKS: u16 = 600;

/// Rocks and knives a fighter carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pouch {
    pub rocks: u8,
    pub knives: u8,
}

impl Pouch {
    /// What F throws next: rocks while there are any, then knives.
    pub fn next(&self) -> Option<Held> {
        if self.rocks > 0 {
            Some(Held::Rock)
        } else if self.knives > 0 {
            Some(Held::Knife)
        } else {
            None
        }
    }

    pub fn any(&self) -> bool {
        self.next().is_some()
    }
}

/// A throw under way: ticks wound so far, or the wait after one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Throw {
    pub wind: u8,
    pub recover: u8,
}

impl Throw {
    pub fn winding(&self) -> bool {
        self.wind > 0
    }
}

/// One in the air.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flying {
    pub item: Held,
    pub by: u8,
    pub p: V3,
    pub prev: V3,
    /// metres a tick
    pub vel: V3,
    pub ticks: u16,
    /// M7: how many times it has glanced off a wall, a tree or a rock
    pub bounces: u8,
}

/// What a throwable met when it stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Struck {
    Body(u8),
    Prop(u16),
    Ground,
    /// M7: a prop of the places or a tree or rock of the scatter
    /// (`obstacles.rs`)
    Scenery,
}

/// A throwable come to a stop this tick (the hook M5 reads).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Impact {
    pub item: Held,
    pub by: u8,
    pub at: V3,
    /// metres a second at impact
    pub speed: Fx,
    pub struck: Struck,
}

/// A box a throwable stops against: a prop that is struck. Its half sizes
/// are along its own axes, turned by `yaw` (`Affine::rot_y`'s; M7: the
/// cart's box turns with the cart, R7.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub centre: V3,
    pub half: V3,
    pub yaw: i32,
}

/// A rock glances off a wall with this share of its speed into the wall
/// turned back, and this share of its speed along it kept.
pub const BOUNCE_PCT: i32 = 35;
pub const GLANCE_PCT: i32 = 70;
/// It glances off at most this many times, and only while it is faster
/// than `BOUNCE_MIN`; then it drops where it is.
pub const BOUNCES: u8 = 3;
pub const BOUNCE_MIN: Fx = ONE * 3 / 2;

/// One come to rest on the ground, still drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resting {
    pub item: Held,
    pub at: V3,
    pub since: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    pub flying: Vec<Flying>,
    /// this tick's impacts, cleared at the start of the next
    pub landed: Vec<Impact>,
    /// boxes that stop a throwable (M5: the merchant's cart)
    pub props: Vec<Block>,
    pub resting: Vec<Resting>,
    /// M7: this tick's glances off the scenery (a rock that flies on), for
    /// the sound of a knock (P1)
    pub knocks: Vec<Impact>,
}

/// Step 5: throws begun and let go, everything in the air moved one tick
/// and stopped where it meets something.
pub fn step(w: &mut World, input: &Input, foes: &[Input]) {
    w.throwables.landed.clear();
    w.throwables.knocks.clear();
    for id in 0..player::count(w) {
        let inp = if id == 0 { *input } else { foes[id - 1] };
        wind(w, id, &inp);
    }
    let bodies: Vec<_> = (0..player::count(w))
        .map(|id| {
            let f = player::fighter(w, id);
            (f.health.down(), blade::capsules(f.kind, &f.prev_pose, &f.pose))
        })
        .collect();
    let mut still = Vec::new();
    for mut t in std::mem::take(&mut w.throwables.flying) {
        t.prev = t.p;
        t.vel.y -= GRAVITY;
        t.p = t.p.add(t.vel);
        t.ticks += 1;
        let r = if t.item == Held::Knife {
            ONE * 3 / 100
        } else {
            ROCK_R
        };
        let path = Swept {
            a0: t.prev,
            b0: t.prev,
            a1: t.p,
            b1: t.p,
        };
        // the earliest of a body, a prop and the ground along this tick
        let mut best: Option<(i64, Struck, V3)> = None;
        let mut consider = |share: i64, s: Struck, at: V3| {
            if best.is_none_or(|(b, _, _)| share < b) {
                best = Some((share, s, at));
            }
        };
        for (id, (down, caps)) in bodies.iter().enumerate() {
            if *down || (id == t.by as usize && t.ticks <= SELF_TICKS) {
                continue;
            }
            for (_, cap, cr) in caps {
                if let Some(c) = blade::sweep(&path, cap, cr + r, 16, 0) {
                    consider(
                        c.k as i64 * ONE as i64 / c.s as i64,
                        Struck::Body(id as u8),
                        c.on_a,
                    );
                }
            }
        }
        for (k, b) in w.throwables.props.iter().enumerate() {
            if let Some(share) = through_box(t.prev, t.p, b, r) {
                consider(share, Struck::Prop(k as u16), t.prev.lerp(t.p, share as Fx));
            }
        }
        // M7: the props, trees and rocks of the world
        let scenery = w.obstacles.first(w, t.prev, t.p, r);
        if let Some(h) = scenery {
            consider(h.share, Struck::Scenery, t.prev.lerp(t.p, h.share as Fx));
        }
        let ground = w.ground(t.p.x, t.p.z);
        if t.p.y <= ground + r {
            let drop = (t.prev.y - t.p.y).max(1) as i64;
            let share = ((t.prev.y - ground - r).max(0) as i64 * ONE as i64 / drop).min(ONE as i64);
            let mut at = t.prev.lerp(t.p, share as Fx);
            at.y = ground + r;
            consider(share, Struck::Ground, at);
        }
        // a rock glances off the scenery and flies on
        if let (Some((_, Struck::Scenery, at)), Some(h)) = (best, scenery) {
            let speed = player::mps(t.vel.len());
            if t.item == Held::Rock && t.bounces < BOUNCES && speed > BOUNCE_MIN {
                w.throwables.knocks.push(Impact {
                    item: t.item,
                    by: t.by,
                    at,
                    speed,
                    struck: Struck::Scenery,
                });
                t.vel = glance(t.vel, h.normal);
                // where it met, a millimetre out of the face
                t.p = at.add(h.normal.scale(ONE / 1000));
                t.bounces += 1;
                still.push(t);
                continue;
            }
        }
        match best {
            Some((_, struck, at)) => {
                let speed = player::mps(t.vel.len());
                if let Struck::Body(id) = struck {
                    blow(w, &t, id as usize, speed, at);
                }
                w.throwables.landed.push(Impact {
                    item: t.item,
                    by: t.by,
                    at,
                    speed,
                    struck,
                });
                let mut rest = at;
                // a knife sticks in the wall or the tree it meets (M7); the
                // rest lie on the ground under where they stopped
                if !(t.item == Held::Knife && struck == Struck::Scenery) {
                    rest.y = w.ground(at.x, at.z) + if t.item == Held::Rock { ROCK_R } else { ONE / 100 };
                }
                w.throwables.resting.push(Resting {
                    item: t.item,
                    at: rest,
                    since: w.tick,
                });
            }
            None if t.ticks < FLIGHT_TICKS => still.push(t),
            None => {}
        }
    }
    w.throwables.flying = still;
    let tick = w.tick;
    w.throwables
        .resting
        .retain(|r| tick - r.since < REST_TICKS as u64);
    let extra = w.throwables.resting.len().saturating_sub(RESTING);
    w.throwables.resting.drain(..extra);
}

/// A throwable's blow on a body: health by its speed, on the part nearest.
fn blow(w: &mut World, t: &Flying, id: usize, speed: Fx, at: V3) {
    let pct = if t.item == Held::Knife {
        KNIFE_PCT
    } else {
        ROCK_PCT
    };
    let f = player::fighter(w, id);
    let part = if at.dist(f.pose.head) < f.build().head * 6 / 10 {
        Part::Head
    } else {
        Part::Torso
    };
    let scaled = (speed as i64 * pct as i64 / 100) as Fx + crate::health::MIN_HIT;
    player::parts_mut(w, id).1.health.take(scaled, part);
    w.blades.hits.push(blade::Hit {
        by: t.by,
        on: id as u8,
        part,
        // M7 wounds: the head's capsule or the torso's, and the throw's travel
        seg: if part == Part::Head { 0 } else { 1 },
        vel: t.p.sub(t.prev),
        speed,
        at,
    });
}

/// A velocity off a face whose outward normal is `n`: the part into the
/// face turned back at `BOUNCE_PCT`, the part along it kept at `GLANCE_PCT`.
pub fn glance(v: V3, n: V3) -> V3 {
    let into = v.dot(n);
    if into >= 0 {
        return v;
    }
    let normal = n.scale(into);
    let along = v.sub(normal);
    along
        .scale(ONE * GLANCE_PCT / 100)
        .sub(normal.scale(ONE * BOUNCE_PCT / 100))
}

/// Where along `a`→`b` (a share, `0..=ONE`) a sphere of radius `r` first
/// enters box `k`, if it does: the slab test on the box grown by `r`, in
/// the box's own turned frame.
pub fn through_box(a: V3, b: V3, k: &Block, r: Fx) -> Option<i64> {
    let one = ONE as i64;
    let (mut lo, mut hi) = (0i64, one);
    // into the box's frame: undo its turn about its centre (as
    // `solid::Wall::local`)
    let (s, c) = (fx::sin(k.yaw), fx::cos(k.yaw));
    let local = |p: V3| {
        let (dx, dz) = (p.x - k.centre.x, p.z - k.centre.z);
        v3(
            fx::mul(dx, c) - fx::mul(dz, s),
            p.y - k.centre.y,
            fx::mul(dx, s) + fx::mul(dz, c),
        )
    };
    let (a, b) = (local(a), local(b));
    let axes = [
        (a.x, b.x, 0, k.half.x),
        (a.y, b.y, 0, k.half.y),
        (a.z, b.z, 0, k.half.z),
    ];
    for (p, q, c, h) in axes {
        let (min, max) = (c as i64 - h as i64 - r as i64, c as i64 + h as i64 + r as i64);
        let d = q as i64 - p as i64;
        if d == 0 {
            if (p as i64) < min || (p as i64) > max {
                return None;
            }
            continue;
        }
        let (mut t0, mut t1) = ((min - p as i64) * one / d, (max - p as i64) * one / d);
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        lo = lo.max(t0);
        hi = hi.min(t1);
        if lo > hi {
            return None;
        }
    }
    Some(lo)
}

/// What fighter `id` would throw: a foe from its pouch; the player what the
/// free hand holds ready (M4's quick items, `inventory::ready`), or with
/// nothing ready the rocks it carries, then its knives.
pub fn next(w: &World, id: usize) -> Option<Held> {
    if id > 0 {
        return player::fighter(w, id).pouch.next();
    }
    let has = |i: Item| w.inventory.count(i) > 0;
    match crate::inventory::ready(w) {
        Some(Item::Rock) if has(Item::Rock) => Some(Held::Rock),
        Some(Item::Knife) if has(Item::Knife) => Some(Held::Knife),
        Some(_) => None,
        None if has(Item::Rock) => Some(Held::Rock),
        None if has(Item::Knife) => Some(Held::Knife),
        None => None,
    }
}

/// A fighter's throw: F begins the wind while it has something to throw;
/// at its end the stone leaves the free hand along the aim.
fn wind(w: &mut World, id: usize, input: &Input) {
    let f = player::fighter(w, id);
    if f.health.down() {
        return;
    }
    let mut th = f.throw;
    th.recover = th.recover.saturating_sub(1);
    let start = f.pressed(input, input::THROW) && th.wind == 0 && th.recover == 0 && next(w, id).is_some();
    if start {
        th.wind = 1;
    } else if th.wind > 0 {
        th.wind += 1;
    }
    let release = th.wind > WIND_TICKS;
    if release {
        th.wind = 0;
        th.recover = RECOVER_TICKS;
    }
    player::parts_mut(w, id).1.throw = th;
    if !release {
        return;
    }
    let (yaw, pitch) = if id == 0 {
        (w.aim.yaw, w.aim.pitch)
    } else {
        let f = player::fighter(w, id);
        (f.look_yaw, f.look_pitch)
    };
    let f = player::fighter(w, id);
    let Some(item) = next(w, id) else {
        return;
    };
    let from = f.arm.pts[crate::arm::HAND[0]].p;
    let speed = if item == Held::Knife {
        KNIFE_SPEED
    } else {
        ROCK_SPEED
    };
    let vel = facing(yaw, pitch + LOB).scale(speed / crate::ticks::TICKS_PER_SECOND as i32);
    if id == 0 {
        let carried = if item == Held::Knife {
            Item::Knife
        } else {
            Item::Rock
        };
        crate::inventory::take(w, carried, 1);
    } else {
        let f = player::parts_mut(w, id).1;
        match item {
            Held::Knife => f.pouch.knives -= 1,
            _ => f.pouch.rocks -= 1,
        }
    }
    w.throwables.flying.push(Flying {
        item,
        by: id as u8,
        p: from,
        prev: from,
        vel,
        ticks: 0,
        bounces: 0,
    });
}

/// Everything in the air and at rest, through the models' door: a rock as a
/// stone from its centre, a knife along its flight.
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    for t in &w.throwables.flying {
        let at = t.prev.lerp(t.p, alpha);
        let along = t.vel.norm().scale(if t.item == Held::Knife {
            KNIFE_LEN
        } else {
            ROCK_R * 2
        });
        crate::models::held(t.item, at, at.add(along), look, out);
    }
    for r in &w.throwables.resting {
        let along = v3(ROCK_R * 2, 0, 0);
        crate::models::held(r.item, r.at, r.at.add(along), look, out);
    }
}

/// The physics view's throwables: a stone, a knife's line.
pub fn draw_sticks(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    let ink = look.ink();
    let stone = rgb(look.palette.rock[0]);
    let mut one = |item: Held, at: V3, dir: V3| {
        if item == Held::Knife {
            player::bar(
                out,
                at,
                at.add(dir.scale(KNIFE_LEN)),
                ONE * 3 / 100,
                rgb(look.palette.visor),
                ink,
            );
        } else {
            let r = ROCK_R * 2;
            out.inked(
                Mesh::Blob,
                Affine::translate(at).then(&Affine::scale(v3(r, r * 8 / 10, r))),
                stone,
                ink,
            );
        }
    };
    for t in &w.throwables.flying {
        one(t.item, t.prev.lerp(t.p, alpha), t.vel.norm());
    }
    for r in &w.throwables.resting {
        one(r.item, r.at, v3(ONE, 0, 0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::THROW;

    fn armed(seed: u64) -> World {
        let mut w = World::new(seed);
        crate::inventory::give(&mut w, Item::Rock, 5);
        w
    }

    fn throw(w: &mut World) {
        w.step(&Input {
            keys: THROW,
            ..Input::default()
        });
        for _ in 0..WIND_TICKS {
            w.step(&Input::default());
        }
    }

    #[test]
    fn f_winds_for_twelve_ticks_then_a_rock_leaves_along_the_aim() {
        let mut w = armed(1);
        w.aim.pitch = 0;
        w.step(&Input {
            keys: THROW,
            ..Input::default()
        });
        assert!(w.me.throw.winding());
        for _ in 0..WIND_TICKS - 1 {
            w.step(&Input::default());
        }
        assert!(w.throwables.flying.is_empty(), "still winding");
        w.step(&Input::default());
        assert_eq!(w.throwables.flying.len(), 1);
        let t = w.throwables.flying[0];
        assert_eq!(t.item, Held::Rock);
        assert_eq!(
            w.inventory.count(Item::Rock),
            4,
            "one rock taken from the inventory"
        );
        // 15 m/s at 9° above level: 15 × cos 9° = 14.815 m/s ahead and
        // 2.347 m/s up; / 60 a tick, 16 182 and 2 563 units; and it has
        // flown its first tick, gravity taking 178 off the climb
        let ahead = t.vel.len_xz();
        assert!((ahead - 16_182).abs() <= 8, "{ahead}");
        assert!((t.vel.y - (2_563 - 178)).abs() <= 8, "{}", t.vel.y);
    }

    #[test]
    fn a_rock_arcs_down_and_lands_with_an_impact_on_the_ground() {
        let mut w = armed(1);
        w.aim.pitch = 0;
        throw(&mut w);
        let mut landed = None;
        for _ in 0..200 {
            w.step(&Input::default());
            if let Some(i) = w.throwables.landed.first() {
                landed = Some(*i);
                break;
            }
        }
        let i = landed.expect("it comes down");
        assert_eq!(i.struck, Struck::Ground);
        // up at 2.35 m/s from about 1.3 m, falling at 9.8 m/s²: down after
        // (2.35 + √(2.35² + 2 × 9.8 × 1.3)) / 9.8 = 0.79 s, 11.7 m ahead
        let d = i.at.sub(w.player.pos).len_xz();
        assert!(d > fx::int(9) && d < fx::int(14), "{}", fx::decimal(d, 2));
        assert!(w.throwables.flying.is_empty());
    }

    #[test]
    fn a_rock_stops_against_a_prop_and_says_which() {
        let mut w = armed(1);
        w.aim.pitch = 0;
        let ahead = facing(w.aim.yaw, 0);
        w.throwables.props.push(Block {
            centre: w.player.pos.add(ahead.scale(fx::int(4))).add(v3(0, ONE, 0)),
            half: v3(ONE, ONE, ONE),
            yaw: 0,
        });
        throw(&mut w);
        let mut hit = None;
        for _ in 0..60 {
            w.step(&Input::default());
            if let Some(i) = w.throwables.landed.first() {
                hit = Some(*i);
                break;
            }
        }
        assert_eq!(hit.map(|i| i.struck), Some(Struck::Prop(0)));
    }

    #[test]
    fn a_rock_that_meets_a_foe_takes_health() {
        let mut w = armed(1);
        crate::player::act(&mut w, "spawn", "orc").unwrap();
        w.foes[0].brain.control = crate::pilot::Control::Still;
        // straight at its chest: a throw from 6 m arrives in 0.4 s
        w.aim.pitch = -deg(3);
        let most = w.foes[0].fighter.health.hp;
        throw(&mut w);
        let mut struck = None;
        for _ in 0..60 {
            w.step(&Input::default());
            if let Some(i) = w.throwables.landed.first() {
                struck = Some(*i);
                break;
            }
        }
        assert_eq!(struck.map(|i| i.struck), Some(Struck::Body(1)));
        assert!(w.foes[0].fighter.health.hp < most);
    }

    /// R7.11: the cart's box turns with the cart. A box 2 m long (local x),
    /// 1 m wide (local z), turned 45°: a path along its length 0.7 m to its
    /// side misses it (on the world's axes the box would be 1 cos 45° + 0.5
    /// sin 45° = 1.06 m half across each way, and the path, its nearest
    /// 0.7 m from the middle, would count as a hit); 0.3 m to its side, it
    /// meets the box.
    #[test]
    fn a_turned_box_lets_a_near_miss_on_a_diagonal_by() {
        let b = Block {
            centre: V3::ZERO,
            half: v3(ONE, ONE * 8 / 10, ONE / 2),
            yaw: fx::deg(45),
        };
        let turn = Affine::rot_y(fx::deg(45));
        let (along, side) = (turn.apply_dir(v3(ONE, 0, 0)), turn.apply_dir(v3(0, 0, ONE)));
        let path = |off: Fx| {
            let o = side.scale(off);
            through_box(
                o.sub(along.scale(fx::int(5))),
                o.add(along.scale(fx::int(5))),
                &b,
                0,
            )
        };
        assert_eq!(path(ONE * 7 / 10), None, "a near miss to its side");
        assert!(path(ONE * 3 / 10).is_some(), "a hit");
        // the path at 0.3 m enters the 2 m long box 4 m along its 10: 40%
        let mid = path(ONE * 3 / 10).unwrap();
        assert!((mid - ONE as i64 * 4 / 10).abs() <= 16, "{mid}");
    }

    #[test]
    fn a_rock_glances_off_a_wall_keeping_most_of_its_speed_along_it() {
        // 10 along and 4 into a wall facing +x: back out at 35% (1.4), along
        // at 70% (7)
        let v = glance(v3(-fx::int(4), 0, fx::int(10)), v3(ONE, 0, 0));
        assert!(
            (v.x - ONE * 14 / 10).abs() <= 4 && (v.z - fx::int(7)).abs() <= 4,
            "{v:?}"
        );
        // leaving the wall already, it is not turned
        assert_eq!(glance(v3(ONE, 0, 0), v3(ONE, 0, 0)), v3(ONE, 0, 0));
    }

    #[test]
    fn the_slab_test_finds_where_a_path_enters_a_box() {
        let b = Block {
            centre: v3(fx::int(5), 0, 0),
            half: v3(ONE, ONE, ONE),
            yaw: 0,
        };
        // from x = 0 to 10 along the axis: enters at x = 4, 40% of the way
        assert_eq!(
            through_box(V3::ZERO, v3(fx::int(10), 0, 0), &b, 0),
            Some(ONE as i64 * 4 / 10)
        );
        assert_eq!(
            through_box(v3(0, fx::int(3), 0), v3(fx::int(10), fx::int(3), 0), &b, 0),
            None
        );
    }
}
