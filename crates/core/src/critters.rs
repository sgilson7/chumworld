//! `critters`: the small things that crawl the dungeon's floors (T21, phase
//! A; the journal's hub: "small enemies on the floor annoy you if you fall").
//!
//! An **ant** keeps to its room's floor. While the player stands on that
//! floor within `SENSE` it crawls toward them at `SPEED`, and from within
//! `BITE_REACH` nips for `BITE` health, once every `BITE_EVERY` ticks. A
//! blade swung through one (its edge passing within `KILL_GAP` of the ant,
//! moving at least `KILL_MOVE` that tick) kills it. Otherwise it wanders a
//! slow circle about where it hatched. Up on a ledge or a pillar the player
//! is out of its reach: it waits below. Ants are a nuisance, not progress:
//! they come back whenever the player comes into their room (`reset`), and
//! a save keeps none of them.
//!
//! An ant is not a fighter: it moves itself, as M7's animals do, and the
//! player's blade is the only thing that hurts it. Its rules are the core's
//! (CW-1), the same for every ant and every run (CW-4).

use crate::fx::{self, Fx, ONE};
use crate::geom::{facing, v3, yaw_of, Affine, V3};
use crate::look::Look;
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::world::World;

/// Crawling speed, 1.6 m/s, a tick.
pub const SPEED: Fx = ONE * 16 / 600;
/// How near the player an ant notices them, on its floor.
pub const SENSE: Fx = ONE * 7;
/// How near it nips from, how much, and how often.
pub const BITE_REACH: Fx = ONE * 7 / 10;
pub const BITE: i32 = 3;
pub const BITE_EVERY: u16 = 50;
/// A blade's edge this near an ant, moving this far in the tick, kills it.
pub const KILL_GAP: Fx = ONE * 4 / 10;
pub const KILL_MOVE: Fx = ONE * 3 / 100;
/// A player's feet this near the floor are on it.
pub const ON_FLOOR: Fx = ONE * 3 / 10;
/// It wanders a circle this wide about its home, a turn in this many ticks.
pub const WANDER: Fx = ONE * 2;
pub const WANDER_TICKS: i32 = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ant {
    pub room: usize,
    pub home: V3,
    pub pos: V3,
    pub prev: V3,
    pub yaw: i32,
    /// ticks before it may nip again
    pub wait: u16,
    pub dead: bool,
    /// its own phase on the wander circle and in its legs' stepping
    pub phase: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    pub ants: Vec<Ant>,
    pub placed: bool,
}

/// Every room's ants, at home.
fn place() -> Vec<Ant> {
    let mut out = Vec::new();
    for (k, r) in crate::dungeon::spec().rooms.iter().enumerate() {
        let c = crate::dungeon::centre(r);
        for (j, a) in r.ants.iter().enumerate() {
            let home = c.add(v3(a[0] * (ONE / 100), 0, a[1] * (ONE / 100)));
            out.push(Ant {
                room: k,
                home,
                pos: home,
                prev: home,
                yaw: 0,
                wait: 0,
                dead: false,
                phase: (j as i32 * 7919 + k as i32 * 104_729).rem_euclid(WANDER_TICKS),
            });
        }
    }
    out
}

/// Room `room`'s ants back at home and alive: the player has come in.
pub fn reset(w: &mut World, room: usize) {
    if !w.critters.placed {
        w.critters.ants = place();
        w.critters.placed = true;
    }
    for a in w.critters.ants.iter_mut().filter(|a| a.room == room) {
        a.dead = false;
        a.pos = a.home;
        a.prev = a.home;
        a.wait = 0;
    }
}

/// Step 7, after the story and the dungeon: every ant in the player's room.
pub fn step(w: &mut World) {
    let Some(room) = crate::dungeon::room_at(w.player.pos) else {
        return;
    };
    if !w.critters.placed {
        w.critters.ants = place();
        w.critters.placed = true;
    }
    let r = &crate::dungeon::spec().rooms[room];
    let c = crate::dungeon::centre(r);
    let (hx, hz) = (r.size_cm[0] * (ONE / 100) / 2, r.size_cm[1] * (ONE / 100) / 2);
    let me = w.player.pos;
    let on_floor = me.y - c.y <= ON_FLOOR && !w.me.health.down();
    let edge = (w.me.arm.in_hand && !w.me.health.down()).then(|| crate::orbs::edge(&w.me.arm));
    let tick = w.tick as i32;
    let held = crate::story::holding(w);
    let mut bites = 0;
    // out of the world while they move, so the floor can be asked about
    let mut ants = std::mem::take(&mut w.critters.ants);
    for a in ants.iter_mut().filter(|a| a.room == room && !a.dead) {
        a.prev = a.pos;
        a.wait = a.wait.saturating_sub(1);
        if held {
            continue;
        }
        let to_me = me.sub(a.pos);
        let goal = if on_floor && to_me.len_xz() <= SENSE {
            me
        } else {
            let t = (tick + a.phase).rem_euclid(WANDER_TICKS) * fx::TURN / WANDER_TICKS;
            a.home.add(facing(t, 0).scale(WANDER))
        };
        let way = v3(goal.x - a.pos.x, 0, goal.z - a.pos.z);
        let gap = way.len_xz();
        if gap > BITE_REACH / 2 {
            let step = way.norm().scale(SPEED.min(gap));
            let next = a.pos.add(step);
            // it keeps to the floor: never onto a ledge, a pillar or the ramp,
            // never out of the room
            let floor = crate::solid::surface(w, next.x, next.z);
            if floor - c.y <= ONE / 20 && (next.x - c.x).abs() < hx && (next.z - c.z).abs() < hz {
                a.pos = v3(next.x, c.y, next.z);
            }
            a.yaw = yaw_of(step);
        }
        // a nip
        if on_floor && to_me.len_xz() <= BITE_REACH && a.wait == 0 {
            bites += 1;
            a.wait = BITE_EVERY;
        }
        // a blade through it
        if edge.as_ref().is_some_and(|e| cut(e, a.pos)) {
            a.dead = true;
        }
    }
    w.critters.ants = ants;
    if bites > 0 {
        let h = &mut w.me.health;
        if !h.down() {
            h.hp -= BITE * bites;
        }
    }
}

/// Whether a blade's edge this tick kills an ant standing at `at`: the edge
/// within `KILL_GAP` of its body, and moving at least `KILL_MOVE` (a swing,
/// not a blade resting on it).
pub fn cut(e: &crate::blade::Swept, at: V3) -> bool {
    let moved = e.b1.sub(e.b0).len().max(e.a1.sub(e.a0).len());
    let body = at.add(v3(0, ONE * 15 / 100, 0));
    moved >= KILL_MOVE && to_segment(e.a1, e.b1, body) <= KILL_GAP
}

/// How far `p` is from the segment `a`–`b`.
fn to_segment(a: V3, b: V3, p: V3) -> Fx {
    let d = [(b.x - a.x) as i64, (b.y - a.y) as i64, (b.z - a.z) as i64];
    let q = [(p.x - a.x) as i64, (p.y - a.y) as i64, (p.z - a.z) as i64];
    let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    let t = if dd == 0 {
        0
    } else {
        ((q[0] * d[0] + q[1] * d[1] + q[2] * d[2]).clamp(0, dd) * ONE as i64) / dd
    };
    let c = v3(
        a.x + (d[0] * t / ONE as i64) as Fx,
        a.y + (d[1] * t / ONE as i64) as Fx,
        a.z + (d[2] * t / ONE as i64) as Fx,
    );
    p.dist(c)
}

/// Every living ant in the player's room: a faceted body of three parts and
/// six legs stepping as it goes.
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    let Some(room) = crate::dungeon::room_at(w.player.pos) else {
        return;
    };
    let ink = look.ink();
    let body = [ONE * 35 / 100, ONE * 18 / 100, ONE * 16 / 100];
    let head = [ONE * 48 / 100, ONE * 24 / 100, ONE * 21 / 100];
    for a in w.critters.ants.iter().filter(|a| a.room == room && !a.dead) {
        let at = a.prev.lerp(a.pos, alpha);
        let place = Affine::translate(at).then(&Affine::rot_y(a.yaw));
        let ball = |x: Fx, y: Fx, z: Fx, s: [Fx; 3]| {
            place
                .then(&Affine::translate(v3(x, y, z)))
                .then(&Affine::scale(v3(s[0], s[1], s[2])))
        };
        out.inked(
            Mesh::Facet,
            ball(
                0,
                ONE * 14 / 100,
                ONE * 22 / 100,
                [ONE * 26 / 100, ONE * 20 / 100, ONE * 34 / 100],
            ),
            body,
            ink,
        );
        out.inked(
            Mesh::Facet,
            ball(
                0,
                ONE * 13 / 100,
                0,
                [ONE * 15 / 100, ONE * 13 / 100, ONE * 18 / 100],
            ),
            body,
            ink,
        );
        out.inked(
            Mesh::Facet,
            ball(
                0,
                ONE * 15 / 100,
                -ONE * 17 / 100,
                [ONE * 17 / 100, ONE * 15 / 100, ONE * 16 / 100],
            ),
            head,
            ink,
        );
        let moving = a.pos != a.prev;
        for k in 0..6 {
            let side = if k % 2 == 0 { ONE } else { -ONE };
            let z = (k / 2 - 1) * ONE * 9 / 100;
            let swing = if moving {
                fx::sin((w.tick as i32 * 40 + k * 120) * fx::TURN / 1000) / 10
            } else {
                0
            };
            let leg = place
                .then(&Affine::translate(v3(
                    fx::mul(side, ONE * 14 / 100),
                    ONE * 6 / 100,
                    z + swing,
                )))
                .then(&Affine::scale(v3(ONE * 16 / 100, ONE * 2 / 100, ONE * 2 / 100)));
            out.solid(Mesh::Cube, leg, body, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;

    /// A world past the opening, the player standing at `x`, `z` (cm) on
    /// the hub's floor, its ants placed.
    fn on_hub_floor(x: i32, z: i32) -> World {
        let mut w = crate::story::opening(9);
        crate::story::step_aside(&mut w);
        let r = &crate::dungeon::spec().rooms[0];
        let p = crate::dungeon::centre(r).add(v3(x * (ONE / 100), 0, z * (ONE / 100)));
        w.player = crate::player::Body::new(p, 0);
        w.me = crate::player::Fighter::new(w.me.kind, crate::items::held(&w), 0, p, 0, false);
        reset(&mut w, 0);
        w
    }

    #[test]
    fn an_ant_crawls_to_a_player_on_its_floor_and_nips_every_fifty_ticks() {
        // the first ant hatches at (-600, -700); stand 4 m east of it
        let mut w = on_hub_floor(-200, -700);
        let hp = w.me.health.hp;
        let start = w.critters.ants[0].pos;
        // 4 m at 1.6 m/s, less the 0.35 m it stops short: about 137 ticks
        for _ in 0..170 {
            w.step(&Input::default());
        }
        let a = w.critters.ants[0];
        assert!(
            a.pos.sub(w.player.pos).len_xz() <= BITE_REACH,
            "it came: {} cm off",
            a.pos.sub(w.player.pos).len_xz() * 100 / ONE
        );
        assert!(a.pos.sub(start).len_xz() > ONE * 3);
        // nipped: 3 a nip, from the first tick in reach, once every 50
        let lost = hp - w.me.health.hp;
        assert!(lost >= BITE && lost % BITE == 0, "lost {lost}");
        let before = w.me.health.hp;
        for _ in 0..BITE_EVERY {
            w.step(&Input::default());
        }
        assert_eq!(
            before - w.me.health.hp,
            BITE * w
                .critters
                .ants
                .iter()
                .filter(|a| !a.dead && a.pos.sub(w.player.pos).len_xz() <= BITE_REACH)
                .count() as i32,
            "one nip each in 50 ticks"
        );
    }

    #[test]
    fn up_on_a_pillar_the_player_is_out_of_the_ants_reach() {
        let mut w = on_hub_floor(-200, -700);
        let r = &crate::dungeon::spec().rooms[0];
        let top = crate::dungeon::centre(r).add(v3(-450 * (ONE / 100), 300 * (ONE / 100), 400 * (ONE / 100)));
        w.player = crate::player::Body::new(top, 0);
        let hp = w.me.health.hp;
        let floor = crate::dungeon::centre(r).y;
        for _ in 0..400 {
            w.step(&Input::default());
            // and none ever stands inside a pillar, a ledge or the ramp
            for a in w.critters.ants.iter().filter(|a| !a.dead) {
                assert_eq!(
                    crate::solid::surface(&w, a.pos.x, a.pos.z),
                    floor,
                    "an ant in something"
                );
            }
        }
        assert_eq!(w.me.health.hp, hp, "no ant reaches the top of a pillar");
        let floor = crate::dungeon::centre(r).y;
        assert!(
            w.critters.ants.iter().all(|a| a.pos.y == floor),
            "every ant on the floor"
        );
    }

    #[test]
    fn a_blade_swung_through_an_ant_kills_it_and_a_still_one_does_not() {
        use crate::blade::Swept;
        let at = v3(ONE * 10, 0, ONE * 10);
        let body = at.add(v3(0, ONE * 15 / 100, 0));
        let across = |up: Fx, off: Fx| Swept {
            a0: body.add(v3(-ONE / 2, up, off)),
            b0: body.add(v3(ONE / 2, up, off)),
            a1: body.add(v3(-ONE / 2, 0, off)),
            b1: body.add(v3(ONE / 2, 0, off)),
        };
        // brought down 20 cm onto it this tick: a swing through it
        assert!(cut(&across(ONE / 5, 0), at));
        // resting across it, moving 1 cm: no
        assert!(!cut(&across(ONE / 100, 0), at));
        // swung 3 cm, the least that counts, through it: yes
        assert!(cut(&across(KILL_MOVE, 0), at));
        // swung through the air 1 m to its side: no
        assert!(!cut(&across(ONE / 5, ONE), at));
        // 0.39 m to its side: within the 0.4 m gap
        assert!(cut(&across(ONE / 5, ONE * 39 / 100), at));
    }

    #[test]
    fn an_ant_crawling_to_the_player_never_crawls_through_a_pillar() {
        // the player on the floor at (-750, -50), 6.96 m from the third
        // ant's home at (-1000, 600), inside its 7 m sense: its straight line
        // crosses the pillar at (-850, 150) at about (-827, 150), inside the
        // pillar's square (x -946 to -754, z 54 to 246)
        let mut w = on_hub_floor(-750, -50);
        let r = &crate::dungeon::spec().rooms[0];
        let floor = crate::dungeon::centre(r).y;
        let start = w.critters.ants[2].pos;
        for _ in 0..600 {
            w.step(&Input::default());
            let a = w.critters.ants[2];
            assert_eq!(
                crate::solid::surface(&w, a.pos.x, a.pos.z),
                floor,
                "the ant went into the pillar"
            );
        }
        assert!(w.critters.ants[2].pos.sub(start).len_xz() > ONE, "it did come on");
    }

    #[test]
    fn coming_back_into_the_room_brings_its_ants_back() {
        let mut w = on_hub_floor(-200, -700);
        for a in w.critters.ants.iter_mut() {
            a.dead = true;
        }
        reset(&mut w, 0);
        assert!(w.critters.ants.iter().all(|a| !a.dead && a.pos == a.home));
    }
}
