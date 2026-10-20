//! `feel` (M1 Duel, the polish pass D2): what makes a blow land. A hard
//! blow or a hard block stops the world for a few ticks (the hit-stop), the
//! camera takes a small knock that rings out, blades that meet throw a
//! spark, and a body that is struck staggers back (`motion::Gait::stagger`).
//! Everything here is in the core and runs on the tick (CW-1, CW-4), and
//! everything decays to exactly nothing, so a still view still draws the
//! same pixels frame after frame (CW-18).
//!
//! It also keeps the tick's events for the story (M5): who went down this
//! tick (`State::downed`), and `World::player_down`.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, Affine, V3};
use crate::input::Input;
use crate::look::{rgb, Look};
use crate::player;
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::world::World;

/// A blow or a block at least this fast (metres a second) is hard: it stops
/// the world for `STOP_MIN` ticks, a tick more for each `STOP_PER` faster,
/// at most `STOP_MAX`.
pub const HARD: Fx = fx::int(5);
pub const STOP_MIN: u8 = 2;
pub const STOP_PER: Fx = fx::int(4);
pub const STOP_MAX: u8 = 4;
/// The camera's knock: 6 mm for each metre a second, at most 5 cm, along
/// the blow; each tick it swings back through the middle and keeps
/// `RING_PCT` of itself, and under `QUIET` it is nothing.
pub const KNOCK_PER_MPS: Fx = ONE * 6 / 1000;
pub const KNOCK_MAX: Fx = ONE * 5 / 100;
pub const RING_PCT: i32 = 55;
pub const QUIET: Fx = ONE / 2000;
/// Blades meeting faster than this throw a spark that burns `SPARK_TICKS`.
pub const SPARK_MIN: Fx = ONE * 3 / 2;
pub const SPARK_TICKS: u8 = 10;
/// A struck body staggers for `STAGGER_TICKS`, shoved back at 4 cm/s for
/// each point of health taken, at most `SHOVE_MAX`.
pub const STAGGER_TICKS: u8 = 14;
pub const SHOVE_PER_POINT: Fx = ONE * 4 / 100;
pub const SHOVE_MAX: Fx = ONE * 18 / 10;

/// A spark where two blades met: where, how old, how bright it began.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spark {
    pub at: V3,
    pub age: u8,
    pub size: Fx,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    /// ticks of hit-stop left
    pub stop: u8,
    /// the camera's knock this tick and last (metres, drawn between)
    pub knock: V3,
    pub prev_knock: V3,
    pub sparks: Vec<Spark>,
    /// the fighters who went down this tick, by number (the story's hook)
    pub downed: Vec<u8>,
}

/// How long a blow or block at `speed` stops the world.
pub fn stop_ticks(speed: Fx) -> u8 {
    if speed < HARD {
        return 0;
    }
    (STOP_MIN as i32 + (speed - HARD) / STOP_PER).min(STOP_MAX as i32) as u8
}

/// The camera's knock for a blow at `speed` along `way`.
pub fn knock(speed: Fx, way: V3) -> V3 {
    way.norm().scale(fx::mul(KNOCK_PER_MPS, speed).min(KNOCK_MAX))
}

/// The knock a tick on: back through the middle, `RING_PCT` of it kept,
/// nothing once it is under `QUIET`.
pub fn ring(k: V3) -> V3 {
    let n = v3(
        -k.x * RING_PCT / 100,
        -k.y * RING_PCT / 100,
        -k.z * RING_PCT / 100,
    );
    if n.len() < QUIET {
        V3::ZERO
    } else {
        n
    }
}

/// Step 4, after health: this tick's blows and blocks stop the world, knock
/// the camera, throw sparks and stagger the struck; what was set going
/// before rings down.
pub fn step(w: &mut World) {
    let mut stop = 0u8;
    let mut knocked: Option<V3> = None;
    for c in w.blades.clashes.clone() {
        if c.speed >= SPARK_MIN {
            w.feel.sparks.push(Spark {
                at: c.at,
                age: 0,
                size: (ONE / 30 + fx::mul(c.speed, ONE / 120)).min(ONE / 6),
            });
        }
        stop = stop.max(stop_ticks(c.speed));
        if (c.a == 0 || c.b == 0) && c.speed >= HARD {
            let other = if c.a == 0 { c.b } else { c.a } as usize;
            let way = player::body(w, other).pos.sub(w.player.pos);
            knocked = Some(knock(c.speed, v3(way.x, 0, way.z)));
        }
    }
    for h in w.blades.hits.clone() {
        let taken = crate::health::damage(h.speed, h.part);
        if taken <= 0 {
            continue;
        }
        let (from, to) = (
            player::body(w, h.by as usize).pos,
            player::body(w, h.on as usize).pos,
        );
        let flat = v3(to.x - from.x, 0, to.z - from.z);
        let way = if flat == V3::ZERO {
            v3(0, 0, ONE)
        } else {
            flat.norm()
        };
        let (b, f) = player::parts_mut(w, h.on as usize);
        f.gait.stagger = STAGGER_TICKS;
        f.gait.stagger_way = way;
        let shove = (SHOVE_PER_POINT * taken).min(SHOVE_MAX) / crate::ticks::TICKS_PER_SECOND as i32;
        b.vel = b.vel.add(way.scale(shove));
        stop = stop.max(stop_ticks(h.speed));
        if h.on == 0 || h.by == 0 {
            knocked = Some(knock(h.speed, way));
        }
    }
    w.feel.stop = w.feel.stop.max(stop);
    ring_down(w);
    if let Some(k) = knocked {
        w.feel.knock = k;
    }
}

/// The knock rings, sparks age and burn out.
fn ring_down(w: &mut World) {
    let s = &mut w.feel;
    s.prev_knock = s.knock;
    s.knock = ring(s.knock);
    for sp in &mut s.sparks {
        sp.age += 1;
    }
    s.sparks.retain(|sp| sp.age < SPARK_TICKS);
}

/// A tick of hit-stop in place of the world's: nothing moves, so every body
/// is drawn where it stands (each last tick set to this one), but the view
/// still turns to the mouse while it looks, the sword hand's target still
/// takes the mouse, sparks burn and the knock rings. Keys pressed now are
/// read when the world runs again.
pub fn hold(w: &mut World, input: &Input) {
    w.feel.stop -= 1;
    if w.me.mouse_looks {
        w.aim = w.aim.turned(crate::input::Mouse {
            dx: input.dx,
            dy: input.dy,
            wheel: input.wheel,
        });
    }
    crate::arm::move_shell(w, 0, input);
    for id in 0..player::count(w) {
        let (b, f) = player::parts_mut(w, id);
        b.prev_pos = b.pos;
        b.prev_facing = b.facing;
        f.prev_pose = f.pose;
        f.arm.prev_target = f.arm.target;
    }
    for t in &mut w.throwables.flying {
        t.prev = t.p;
    }
    w.blades.hits.clear();
    w.blades.clashes.clear();
    w.feel.downed.clear();
    ring_down(w);
}

/// Every spark: a hot flash that shrinks as it burns, and six motes thrown
/// out from it, all through the glow pass (`Shapes::glow`), in a fixed
/// order.
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    let hot = rgb(look.palette.lamp);
    for sp in &w.feel.sparks {
        let age = sp.age as i32 * ONE + alpha;
        let left = (SPARK_TICKS as i32 * ONE - age).max(0) / SPARK_TICKS as i32;
        let r = fx::mul(sp.size, left);
        if r <= 0 {
            continue;
        }
        out.glow(
            Mesh::Sphere,
            Affine::translate(sp.at).then(&Affine::scale(v3(r, r, r))),
            hot,
            left,
        );
        let flew = fx::mul(ONE / 8, age / SPARK_TICKS as i32 * 2).min(ONE / 4);
        let m = r / 4;
        for k in 0..6 {
            let a = fx::TURN * k / 6 + fx::deg(15);
            let d = v3(fx::cos(a), fx::sin(a * 2) / 2 + ONE / 3, fx::sin(a)).norm();
            let at = sp.at.add(d.scale(flew)).sub(v3(0, fx::mul(flew, flew), 0));
            out.glow(
                Mesh::Sphere,
                Affine::translate(at).then(&Affine::scale(v3(m, m, m))),
                hot,
                left,
            );
        }
    }
}

/// The camera's knock now, `alpha` of the way from the last tick to this.
pub fn knock_at(w: &World, alpha: Fx) -> V3 {
    w.feel.prev_knock.lerp(w.feel.knock, alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hard_blow_stops_the_world_two_to_four_ticks_by_its_speed() {
        // under 5 m/s nothing; then 2, a tick more each 4 m/s, at most 4
        assert_eq!(stop_ticks(HARD - 1), 0);
        assert_eq!(stop_ticks(HARD), 2);
        assert_eq!(stop_ticks(fx::int(9)), 3, "2 + 4 / 4");
        assert_eq!(stop_ticks(fx::int(25) / 2), 3, "2 + 7.5 / 4, down");
        assert_eq!(stop_ticks(fx::int(13)), 4, "2 + 8 / 4");
        assert_eq!(stop_ticks(fx::int(30)), 4, "at most 4");
    }

    /// The knock: 6 mm a metre a second (393 units), at most 5 cm (3 276).
    /// It rings back through the middle keeping 55%, truncated, and is
    /// nothing under half a millimetre (32 units): from 3 276, −1 801, 990,
    /// −544, 299, −164, 90, −49, then 26, under 32: nothing.
    #[test]
    fn the_camera_s_knock_rings_back_through_the_middle_to_exactly_nothing() {
        assert_eq!(knock(fx::int(5), v3(ONE, 0, 0)), v3(1965, 0, 0));
        let mut k = knock(fx::int(10), v3(ONE, 0, 0));
        assert_eq!(k, v3(3276, 0, 0), "held to 5 cm");
        let mut seen = vec![k.x];
        while k != V3::ZERO {
            k = ring(k);
            seen.push(k.x);
        }
        assert_eq!(seen, [3276, -1801, 990, -544, 299, -164, 90, -49, 0]);
    }
}
