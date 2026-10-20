//! `boomerang` (M5 Places and story, task S): thrown from the free hand, an
//! arc out and back that always comes home.
//!
//! The brief: "a boomerang which flies in an arc when thrown and always come
//! back". With the boomerang ready on a quick key (M4), F winds the free
//! hand back as for a rock (M1's `Throw`, `WIND_TICKS`) and lets it go. It
//! flies a loop flat over the ground, `FLIGHT_TICKS` (150) long: out along
//! the aim to `REACH` (18 m, `data/items.json`) at the loop's far end, round
//! to the left `WIDTH` across, and back. Its path is worked out from the
//! tick of the flight alone, so it is the same every throw; only where it
//! comes home moves: the loop's start slides from where the hand let go to
//! where the hand is now, eased in over the flight, so at tick 150 it is in
//! the hand, wherever the thrower has walked.
//!
//! On the way it strikes what it meets: a body takes a light blow
//! (`BLOW_SPEED`, as a blade at that speed would) once a throw, and the
//! merchant's cart is struck (an `Impact` on the cart's box, as a landed
//! rock is, which stops the cart). It flies through them and comes home.
//!
//! Steps at 5 in `World::step`, after M1's throwables.

use crate::fx::{self, Fx, ONE};
use crate::geom::{facing, right_of, v3, Affine, V3};
use crate::input::{self, Input};
use crate::items::Item;
use crate::look::Look;
use crate::player;
use crate::pose::Held;
use crate::render::Shapes;
use crate::throwables::{self, Impact, Struck};
use crate::world::World;

/// The flight's length, ticks, and its reach, metres (`data/items.json`).
pub fn flight_ticks() -> u32 {
    Item::Boomerang.def().flight_ticks as u32
}
pub fn reach() -> Fx {
    fx::int(Item::Boomerang.def().reach_m)
}
/// How wide the loop is, at its widest (a third of the way out).
pub const WIDTH: Fx = fx::int(7);
/// How high over the throw's height it rises at its far end.
pub const RISE: Fx = ONE * 8 / 10;
/// It spins this much a tick (about 4 turns a second).
pub const SPIN: i32 = fx::TURN / 15;
/// A body it strikes takes a blow as from a blade at this speed: (4 − 1.5)
/// × 5 = 12 health on the torso (M1's `health::damage`).
pub const BLOW_SPEED: Fx = fx::int(4);
/// It strikes a body whose chest is this near its path, or head this near.
pub const BODY_R: Fx = ONE * 40 / 100;
pub const HEAD_R: Fx = ONE * 25 / 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flight {
    /// ticks flown
    pub t: u32,
    /// the hand where it was let go, and the heading thrown
    pub from: V3,
    pub yaw: i32,
    pub p: V3,
    pub prev: V3,
    /// bodies struck this throw (fighter ids), and whether the cart was
    pub struck: Vec<u8>,
    pub cart: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    pub flight: Option<Flight>,
    /// F pressed: the hand winds back; it leaves when the wind ends
    pub winding: bool,
    /// throws so far, and catches
    pub thrown: u32,
    pub caught: u32,
}

/// Where the loop is `t` ticks into a flight thrown along `yaw` from the
/// origin: out along the heading by `REACH (1 − cos φ) / 2`, to the left by
/// `WIDTH sin φ / 2`, up by `RISE sin(φ/2)`, where φ goes once round over
/// the flight. At 0 and at the flight's end it is at the origin.
pub fn offset(t: u32, yaw: i32) -> V3 {
    let n = flight_ticks().max(1);
    let phi = (t.min(n) as i64 * fx::TURN as i64 / n as i64) as i32;
    let out = fx::mul(reach(), ONE - fx::cos(phi)) / 2;
    let side = fx::mul(WIDTH / 2, fx::sin(phi));
    let up = fx::mul(RISE, fx::sin(phi / 2));
    let fwd = facing(yaw, 0);
    let left = right_of(yaw).neg();
    fwd.scale(out).add(left.scale(side)).add(v3(0, up, 0))
}

/// The free hand of the player, where the boomerang leaves and comes home.
pub fn hand(w: &World) -> V3 {
    w.me.arm.pts[crate::arm::HAND[0]].p
}

/// Whether the boomerang is out (thrown, or winding to be).
pub fn out(w: &World) -> bool {
    w.boomerang.flight.is_some() || w.boomerang.winding
}

/// Step 5: F with the boomerang ready winds the free hand; at the wind's
/// end it flies; in flight it moves one tick, strikes, and at the flight's
/// end is caught.
pub fn step(w: &mut World, input: &Input) {
    let ready = crate::inventory::ready(w) == Some(Item::Boomerang);
    let f = &w.me;
    let press = f.pressed(input, input::THROW);
    if ready && press && !out(w) && !f.health.down() && f.throw.wind == 0 && f.throw.recover == 0 {
        // M1's throw winds the hand from here; it lets go of nothing (its
        // `next` throws no boomerang), and this module sends it
        w.me.throw.wind = 1;
        w.boomerang.winding = true;
        return;
    }
    if w.boomerang.winding && w.me.throw.wind == 0 {
        w.boomerang.winding = false;
        if w.me.throw.recover > 0 {
            let from = hand(w);
            w.boomerang.thrown += 1;
            w.boomerang.flight = Some(Flight {
                t: 0,
                from,
                yaw: w.aim.yaw,
                p: from,
                prev: from,
                struck: Vec::new(),
                cart: false,
            });
            // it leaves the hand this tick; tick 1 of its flight is the next
            return;
        }
    }
    let Some(mut fl) = w.boomerang.flight.take() else {
        return;
    };
    fl.t += 1;
    fl.prev = fl.p;
    let n = flight_ticks();
    let share = (fl.t as i64 * ONE as i64 / n as i64) as Fx;
    let eased = fx::mul(share, share);
    let base = fl.from.lerp(hand(w), eased);
    fl.p = base.add(offset(fl.t, fl.yaw));
    strike(w, &mut fl);
    if fl.t >= n {
        w.boomerang.caught += 1;
        return;
    }
    w.boomerang.flight = Some(fl);
}

/// The nearest point of segment `a`–`b` to `p`, and how far.
fn near(a: V3, b: V3, p: V3) -> Fx {
    crate::arm::nearest_on(a, b, p).dist(p)
}

fn strike(w: &mut World, fl: &mut Flight) {
    for id in 1..player::count(w) {
        if fl.struck.contains(&(id as u8)) {
            continue;
        }
        let f = player::fighter(w, id);
        if f.health.down() || f.team == 0 {
            continue;
        }
        let (chest, head) = (f.pose.chest, f.pose.head);
        let part = if near(fl.prev, fl.p, head) < HEAD_R {
            Some(crate::blade::Part::Head)
        } else if near(fl.prev, fl.p, chest) < BODY_R {
            Some(crate::blade::Part::Torso)
        } else {
            None
        };
        if let Some(part) = part {
            fl.struck.push(id as u8);
            player::parts_mut(w, id).1.health.take(BLOW_SPEED, part);
        }
    }
    if !fl.cart {
        if let Some(k) = w.merchant.prop {
            if let Some(b) = w.throwables.props.get(k) {
                if let Some(share) = throwables::through_box(fl.prev, fl.p, b, ONE * 15 / 100) {
                    fl.cart = true;
                    let speed = player::mps(fl.p.sub(fl.prev).len());
                    w.throwables.landed.push(Impact {
                        item: Held::Boomerang,
                        by: 0,
                        at: fl.prev.lerp(fl.p, share as Fx),
                        speed,
                        struck: Struck::Prop(k as u16),
                    });
                }
            }
        }
    }
}

/// The boomerang in flight, spinning flat.
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    let Some(fl) = &w.boomerang.flight else {
        return;
    };
    let at = fl.prev.lerp(fl.p, alpha);
    let spin = (fl.t as i64 * SPIN as i64 + fx::mul(alpha, SPIN) as i64) as i32;
    let len = crate::models::grip_to_tip(Held::Boomerang);
    let d = facing(spin, 0).scale(len / 2);
    let turn = Affine::rot_y(0);
    let _ = turn;
    crate::models::held(Held::Boomerang, at.sub(d), at.add(d), look, out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::deg;

    fn thrower(yaw: i32) -> World {
        let mut w = World::new(2);
        crate::inventory::give(&mut w, Item::Boomerang, 1);
        w.inventory.assign(Item::Boomerang, 0).unwrap();
        w.aim.yaw = yaw;
        w.step(&Input {
            keys: input::ITEM_1,
            ..Input::default()
        });
        w.step(&Input::default());
        assert_eq!(crate::inventory::ready(&w), Some(Item::Boomerang));
        w
    }

    #[test]
    fn the_loop_starts_and_ends_at_the_hand_and_reaches_eighteen_metres_out() {
        assert_eq!(flight_ticks(), 150);
        for yaw in [0, deg(90), deg(200), -deg(33)] {
            assert_eq!(offset(0, yaw), V3::ZERO);
            assert_eq!(offset(150, yaw), V3::ZERO);
            // half way, φ is half a turn: out by 18 (1 − (−1)) / 2 = 18 m,
            // across by 0, up by 0.8 m
            let mid = offset(75, yaw);
            let along = facing(yaw, 0);
            assert!(
                (mid.dot(along) - fx::int(18)).abs() <= 8,
                "{} at {yaw}",
                mid.dot(along)
            );
            assert!((mid.y - RISE).abs() <= 2);
        }
        // a quarter of the way, out 9 m and 3.5 m to the left
        let q = offset(37, 0);
        assert!(q.x < -fx::int(3), "left of north is west: {}", q.x);
    }

    #[test]
    fn the_boomerang_comes_back_to_the_hand_in_exactly_150_ticks_from_any_direction() {
        for yaw in [0, deg(45), deg(135), deg(270), -deg(10)] {
            let mut w = thrower(yaw);
            w.step(&Input {
                keys: input::THROW,
                ..Input::default()
            });
            // the wind: M1's `WIND_TICKS`, then it leaves the hand
            let mut wind = 0;
            while w.boomerang.flight.is_none() {
                w.step(&Input::default());
                wind += 1;
                assert!(wind <= 20, "it leaves the hand");
            }
            assert_eq!(wind, crate::throwables::WIND_TICKS as u32);
            assert_eq!(w.boomerang.flight.as_ref().unwrap().t, 0, "let go this tick");
            // walking while it flies: it comes home to the hand all the same
            let mut far = 0;
            for t in 1..150 {
                let keys = if t < 80 { input::FORWARD } else { input::LEFT };
                w.step(&Input {
                    keys,
                    ..Input::default()
                });
                let fl = w.boomerang.flight.as_ref().expect("still out at tick {t}");
                assert_eq!(fl.t, t);
                far = far.max(fl.p.sub(fl.from).len_xz());
            }
            let last = w.boomerang.flight.clone().unwrap();
            assert_eq!(last.t, 149, "out for 149 ticks");
            w.step(&Input::default());
            assert!(
                w.boomerang.flight.is_none(),
                "caught at tick 150, thrown at {yaw}"
            );
            assert_eq!(w.boomerang.caught, 1);
            assert!(far > fx::int(16), "out to {far}");
            // its last tick was within a step of the hand
            assert!(last.p.sub(hand(&w)).len() < ONE, "{}", last.p.sub(hand(&w)).len());
            assert_eq!(w.inventory.count(Item::Boomerang), 1, "kept: it came back");
        }
    }

    #[test]
    fn the_boomerang_through_the_merchant_s_cart_stops_it_and_flies_on_home() {
        let mut w = crate::story::opening(8);
        crate::story::step_aside(&mut w);
        w.story.still = true;
        // the cart's middle where the loop is a quarter of the way out
        let m = w.merchant.at;
        let yaw = 0;
        let at = m.sub(offset(37, yaw)).sub(v3(0, ONE, 0));
        let mut at = at;
        at.y = w.ground(at.x, at.z);
        w.player = crate::player::Body::new(at, yaw);
        w.me = crate::player::Fighter::new(crate::pose::Kind::Player, Held::ShortSword, 0, at, yaw, false);
        crate::inventory::give(&mut w, Item::Boomerang, 1);
        w.inventory.assign(Item::Boomerang, 0).unwrap();
        w.aim.yaw = yaw;
        for keys in [input::ITEM_1, 0, input::THROW] {
            w.step(&Input {
                keys,
                ..Input::default()
            });
        }
        for _ in 0..180 {
            w.step(&Input::default());
        }
        assert!(w.merchant.waiting(), "the cart stopped");
        assert_eq!(w.merchant.hits, 1, "once a throw");
        assert_eq!(w.boomerang.caught, 1, "and it came home");
    }

    #[test]
    fn the_boomerang_strikes_a_foe_on_its_way_once_and_a_little() {
        let mut w = thrower(0);
        crate::player::act(&mut w, "spawn", "orc").unwrap();
        crate::player::act(&mut w, "pilot", "still").unwrap();
        // the orc 9 m ahead, where the loop passes a quarter of the way out
        let at = offset(37, w.aim.yaw).add(w.player.pos);
        let mut at = at;
        at.y = w.ground(at.x, at.z);
        w.foes[0].body = crate::player::Body::new(at, 0);
        let hp = w.foes[0].fighter.health.hp;
        w.step(&Input {
            keys: input::THROW,
            ..Input::default()
        });
        for _ in 0..200 {
            w.step(&Input::default());
        }
        let taken = hp - w.foes[0].fighter.health.hp;
        // M1's `health::damage` at `BLOW_SPEED`, on the torso or the head:
        // a blow, and a light one (a short sword at a swing's speed takes 30
        // or more)
        assert!(taken > 0 && taken < 20, "a light blow: {taken}");
    }
}
