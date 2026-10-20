//! `motion`: the feet (M1 Duel, task D). Walking, running, footwork with the
//! sword drawn, the speedster's ice (CW-8), a short dodge-step, the facing
//! rule, and the stepping gait that fills a `Pose`'s legs. Steps at 3 in
//! `World::step`, from each fighter's `Input` record alone (CW-3).
//!
//! **Grip** is the share of the gap between the velocity a fighter has and
//! the velocity its keys ask for that its feet close each tick. Bare feet
//! close a third of it (`GRIP_BASE`): a walk starts and stops in a few
//! ticks. Each speedster piece multiplies grip by 55/100 and the speed by
//! 1 + 0.2 (journal, "The speedster's armour"; CW-8), so five pieces run at
//! twice the speed on a twentieth of the grip and slide ten metres to a
//! stop. The values for 0 to 5 pieces are pinned in the tests below, worked
//! out by hand.
//!
//! **Facing.** Sheathed, a body turns toward where it moves. Drawn, it turns
//! toward the foe it holds on (`Fighter::engaged`), or, holding on no one,
//! the way it looks.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{facing, right_of, v3, yaw_of, V3};
use crate::input::{self, Input};
use crate::player::{self, Body, Fighter};
use crate::pose::Build;
use crate::ticks::TICKS_PER_SECOND;
use crate::world::World;

const TICKS: i32 = TICKS_PER_SECOND as i32;

/// Walking and running speeds, metres a second (PLAN.md, the brief).
pub const WALK: Fx = ONE * 22 / 10;
pub const RUN: Fx = fx::int(5);
/// Footwork with the sword drawn: the walking speed, whatever Shift does.
pub const FOOTWORK: Fx = WALK;
/// Bare feet's grip: a third of the gap closed each tick.
pub const GRIP_BASE: Fx = ONE / 3;
/// Each speedster piece: grip × 55/100, speed + 20% of the base (CW-8).
pub const GRIP_PER_PIECE_PCT: i32 = 55;
pub const SPEED_PER_PIECE_PCT: i32 = 20;
/// The most pieces there are.
pub const PIECES: u8 = 5;
/// The most a body turns in a tick toward where it should face (600° a
/// second sheathed, 360° drawn: a duellist turns more deliberately).
pub const TURN_RATE: i32 = deg(10);
pub const TURN_RATE_DRAWN: i32 = deg(6);
/// The dodge-step (Space): this fast for this many ticks, then a wait.
pub const DODGE_SPEED: Fx = fx::int(6);
pub const DODGE_TICKS: u8 = 8;
pub const DODGE_WAIT: u8 = 40;
/// Two bodies keep this far apart, centre to centre, at the player's girth.
pub const PERSONAL: Fx = ONE * 55 / 100;
/// The gait: a stride this long at a walk, longer with speed; feet lift this
/// high at a walk.
pub const STRIDE: Fx = ONE * 62 / 100;
pub const STRIDE_PER_MPS: Fx = ONE * 9 / 100;
pub const LIFT: Fx = ONE * 11 / 100;

/// Settings that are not any one body's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    /// Pieces of the speedster's armour the player is taken to wear, for a
    /// test and a screenshot (`act("motion.speedster", n)`), in place of
    /// `items::speedster_worn`.
    pub forced_pieces: Option<u8>,
}

/// One body's stepping gait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gait {
    /// where in the stride cycle the feet are (angle units: a full turn is
    /// two steps)
    pub phase: i32,
    /// how far the feet swing, `ONE` at a walk, eased toward the speed
    pub swing: Fx,
    /// ticks left in a dodge, and before the next may start
    pub dodge: u8,
    pub dodge_wait: u8,
    pub dodge_way: V3,
}

/// Grip with `n` speedster pieces: `GRIP_BASE` × (55/100)ⁿ, each step
/// truncated.
pub fn grip(n: u8) -> Fx {
    let mut g = GRIP_BASE;
    for _ in 0..n.min(PIECES) {
        g = g * GRIP_PER_PIECE_PCT / 100;
    }
    g
}

/// The speed share with `n` pieces, percent: 100 + 20 n.
pub fn speed_pct(n: u8) -> i32 {
    100 + SPEED_PER_PIECE_PCT * n.min(PIECES) as i32
}

/// A speed in metres a second as metres a tick, with `n` pieces.
pub fn per_tick(speed: Fx, n: u8) -> Fx {
    (speed as i64 * speed_pct(n) as i64 / 100) as Fx / TICKS
}

/// How many pieces the player wears: the test setting if one is forced,
/// otherwise the inventory's (M4, `items::speedster_worn`).
pub fn pieces(w: &World) -> u8 {
    w.motion
        .forced_pieces
        .unwrap_or_else(|| crate::items::speedster_worn(w))
        .min(PIECES)
}

/// `vel` moved `grip` of the way toward `want`, at least one unit a tick
/// on each axis that is not there yet, so it always arrives.
pub fn approach(vel: V3, want: V3, grip: Fx) -> V3 {
    let one = |v: i32, w: i32| {
        let d = w - v;
        let c = fx::mul(d, grip);
        v + if c == 0 { d.signum() } else { c }
    };
    v3(one(vel.x, want.x), one(vel.y, want.y), one(vel.z, want.z))
}

/// The velocity a body's keys ask for: the movement keys turned by `yaw`
/// (the camera's heading for the player, the way it looks for a foe).
pub fn wanted(input: &Input, yaw: i32, drawn: bool, n: u8) -> V3 {
    let (x, z) = input.walk_axes();
    let way = right_of(yaw)
        .scale(x * ONE)
        .sub(facing(yaw, 0).scale(z * ONE))
        .norm();
    let speed = if drawn {
        FOOTWORK
    } else if input.key(input::RUN) {
        RUN
    } else {
        WALK
    };
    way.scale(per_tick(speed, n))
}

/// One tick of every body's feet.
pub fn step(w: &mut World, input: &Input, foes: &[Input]) {
    let n_player = pieces(w);
    for id in 0..player::count(w) {
        let inp = if id == 0 { *input } else { foes[id - 1] };
        let yaw = player::look_yaw(w, id);
        let n = if id == 0 { n_player } else { 0 };
        let toward = player::engaged_at(w, id);
        let ground = {
            let b = player::body(w, id);
            w.ground(b.pos.x, b.pos.z)
        };
        let (b, f) = player::parts_mut(w, id);
        step_body(b, f, &inp, yaw, n, toward, ground);
    }
    separate(w);
}

/// One body's tick: speed, grip, the dodge, facing and the gait.
pub fn step_body(
    b: &mut Body,
    f: &mut Fighter,
    input: &Input,
    yaw: i32,
    n: u8,
    toward: Option<V3>,
    ground: Fx,
) {
    b.prev_pos = b.pos;
    b.prev_facing = b.facing;
    b.prev_vel = b.vel;
    let drawn = f.drawn();
    let down = f.health.down();
    let mut want = if down {
        V3::ZERO
    } else {
        wanted(input, yaw, drawn, n)
    };
    // the dodge-step: Space with a direction, once the last has worn off
    let dodge = !down && f.pressed(input, input::DODGE);
    let g = &mut f.gait;
    g.dodge_wait = g.dodge_wait.saturating_sub(1);
    if dodge && g.dodge_wait == 0 && want != V3::ZERO {
        g.dodge = DODGE_TICKS;
        g.dodge_wait = DODGE_WAIT;
        g.dodge_way = want.norm();
    }
    if g.dodge > 0 {
        g.dodge -= 1;
        want = g.dodge_way.scale(per_tick(DODGE_SPEED, n));
    }
    b.vel = approach(b.vel, want, grip(n));
    if b.vel.len() <= 1 {
        b.vel = V3::ZERO;
    }
    b.pos = b.pos.add(b.vel);
    b.pos.y = ground;
    // facing
    let goal = if down {
        None
    } else if drawn {
        Some(match toward {
            Some(t) if t.sub(b.pos).len_xz() > ONE / 10 => yaw_of(t.sub(b.pos)),
            _ => yaw,
        })
    } else if b.vel != V3::ZERO && want != V3::ZERO {
        Some(yaw_of(b.vel))
    } else {
        None
    };
    if let Some(goal) = goal {
        let rate = if drawn { TURN_RATE_DRAWN } else { TURN_RATE };
        let turn = fx::wrap(goal - b.facing).clamp(-rate, rate);
        b.facing = fx::wrap(b.facing + turn);
    }
    // feet stride as fast as they are asked to go, at most as fast as the
    // body goes: a body sliding on ice with nothing held glides on still feet
    let asked = if want.len() < b.vel.len() { want } else { b.vel };
    stride(&mut f.gait, asked);
}

/// The gait follows the distance the feet cover: half a turn of phase per
/// stride, the swing eased toward how fast the body goes.
pub fn stride(g: &mut Gait, vel: V3) {
    let speed = vel.len_xz() as i64 * TICKS as i64;
    let len = STRIDE + fx::mul(speed as Fx, STRIDE_PER_MPS);
    g.phase = fx::wrap(g.phase + (vel.len_xz() as i64 * fx::HALF_TURN as i64 / len as i64) as i32);
    let goal = (speed * ONE as i64 / WALK as i64).min(ONE as i64 * 3 / 2) as Fx;
    g.swing = fx::approach(g.swing, goal, ONE / 8);
}

/// Bodies closer than their personal space are pushed apart, half each,
/// across the ground.
fn separate(w: &mut World) {
    let n = player::count(w);
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (player::body(w, i).pos, player::body(w, j).pos);
            let gi = player::fighter(w, i).build().girth_pct;
            let gj = player::fighter(w, j).build().girth_pct;
            let room = (PERSONAL as i64 * (gi + gj) as i64 / 200) as Fx;
            let d = v3(b.x - a.x, 0, b.z - a.z);
            let l = d.len();
            if l >= room {
                continue;
            }
            let push = if l == 0 {
                v3(ONE, 0, 0)
            } else {
                d.scale(fx::div(ONE, l))
            }
            .scale((room - l) / 2);
            player::parts_mut(w, i).0.pos = a.sub(push);
            player::parts_mut(w, j).0.pos = b.add(push);
        }
    }
}

/// Where each foot is: under its hip, swung along the way the body moves
/// and lifted in the half of the cycle it is off the ground.
pub fn feet(b: &Body, g: &Gait, build: &Build, feet_at: V3) -> [V3; 2] {
    let right = right_of(b.facing);
    let way = if b.vel.len_xz() > 0 {
        v3(b.vel.x, 0, b.vel.z).norm()
    } else {
        facing(b.facing, 0)
    };
    let half = fx::mul(STRIDE / 2, g.swing);
    let mut out = [V3::ZERO; 2];
    for (s, foot) in out.iter_mut().enumerate() {
        let side = if s == 0 { -1 } else { 1 };
        let ph = g.phase.wrapping_add(if s == 0 { 0 } else { fx::HALF_TURN });
        let along = fx::mul(fx::sin(ph), half);
        let lift = fx::mul(fx::cos(ph).max(0), fx::mul(LIFT, g.swing.min(ONE)));
        *foot = feet_at
            .add(right.scale(side * build.hip_x))
            .add(way.scale(along))
            .add(v3(0, lift, 0));
    }
    out
}

/// The knee between a hip and a foot, bent toward `fwd`: the two-bone
/// answer, the thigh `a` and shin `c` long.
pub fn knee(hip: V3, foot: V3, a: Fx, c: Fx, fwd: V3) -> V3 {
    let span = foot.sub(hip);
    let d = span.len().clamp(ONE / 20, a + c - ONE / 1000);
    let u = span.norm();
    // along the hip-to-foot line: (a² − c² + d²) / 2d
    let x = fx::div(fx::mul(a, a) - fx::mul(c, c) + fx::mul(d, d), 2 * d);
    let h = fx::sqrt(fx::mul(a, a) - fx::mul(x, x));
    let bend = fwd.sub(u.scale(fwd.dot(u))).norm();
    hip.add(u.scale(x)).add(bend.scale(h))
}

/// `act("motion.<name>", value)`: `speedster` takes the player to wear 0 to
/// 5 pieces, for a test or a screenshot; `worn` hands it back to the
/// inventory.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    match name {
        "speedster" => {
            let n: u8 = value
                .trim()
                .parse()
                .ok()
                .filter(|n| *n <= PIECES)
                .ok_or_else(|| {
                    format!("The speedster pieces are a whole number from 0 to {PIECES}, not \"{value}\".")
                })?;
            w.motion.forced_pieces = Some(n);
            Ok(())
        }
        "worn" => {
            w.motion.forced_pieces = None;
            Ok(())
        }
        _ => Err(format!("Unknown action \"motion.{name}\".")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{FORWARD, RUN as RUN_KEY};
    use crate::player::Fighter;
    use crate::pose::Kind;

    fn hold(keys: u32) -> Input {
        Input {
            keys,
            ..Input::default()
        }
    }

    fn fighter() -> Fighter {
        Fighter::new(Kind::Player, crate::pose::Held::ShortSword, 0, V3::ZERO, 0, false)
    }

    #[test]
    fn grip_and_speed_are_pinned_for_zero_to_five_pieces() {
        // CW-8. Grip: 65 536 / 3 = 21 845 (21 845.3 truncated), then × 55 /
        // 100 truncated each time: 12 014 (12 014.75), 6 607 (6 607.7),
        // 3 633 (3 633.85), 1 998 (1 998.15), 1 098 (1 098.9).
        assert_eq!(
            [0, 1, 2, 3, 4, 5].map(grip),
            [21_845, 12_014, 6_607, 3_633, 1_998, 1_098]
        );
        // as shares of a tick's gap: 33%, 18%, 10%, 5.5%, 3.0%, 1.7%
        // Speed: 5 m/s is 327 680; × (100 + 20 n) / 100, then / 60 a tick:
        // 5 461 (5 461.3), 6 553 (6 553.6), 7 645 (7 645.9), 8 738 (8 738.1),
        // 9 830 (9 830.4), 10 922 (10 922.7): 5, 6, 7, 8, 9, 10 m/s.
        assert_eq!(
            [0, 1, 2, 3, 4, 5].map(|n| per_tick(RUN, n)),
            [5_461, 6_553, 7_645, 8_738, 9_830, 10_922]
        );
        assert_eq!(grip(9), grip(5), "there are five pieces");
    }

    /// From a full run, let go: each tick keeps (1 − g) of the speed, so the
    /// slide is v (1 − g) / g. Bare feet: 5 461 × (1 − ⅓) / ⅓ = 10 922 units,
    /// 0.17 m. Five pieces: 10 922 × (1 − 0.016 75) / 0.016 75 = 641 000
    /// units, 9.78 m. Truncation shortens each a little.
    #[test]
    fn five_speedster_pieces_slide_ten_metres_to_a_stop_and_bare_feet_a_hand() {
        for (n, want, tol) in [(0u8, 10_922, 80), (5, 641_000, 641_000 / 50)] {
            let mut b = Body::new(V3::ZERO, 0);
            let mut f = fighter();
            for _ in 0..1200 {
                step_body(&mut b, &mut f, &hold(FORWARD | RUN_KEY), 0, n, None, 0);
            }
            assert_eq!(b.vel.z, -per_tick(RUN, n), "{n} pieces: at full speed");
            let from = b.pos.z;
            for _ in 0..1200 {
                step_body(&mut b, &mut f, &Input::default(), 0, n, None, 0);
            }
            let slide = from - b.pos.z;
            assert!(
                (slide - want).abs() <= tol,
                "{n} pieces: slid {slide}, by hand {want}"
            );
            assert_eq!(b.vel, V3::ZERO, "{n} pieces: stopped");
        }
    }

    #[test]
    fn a_walk_starts_in_a_few_ticks_and_holding_w_walks_north() {
        let mut b = Body::new(V3::ZERO, 0);
        let mut f = fighter();
        // 2.2 m/s is 2 402 a tick (144 179 / 60); the gap shrinks to ⅔ a
        // tick: after 10 ticks (⅔)¹⁰ = 1.7% is left, 41 units
        for _ in 0..10 {
            step_body(&mut b, &mut f, &hold(FORWARD), 0, 0, None, 0);
        }
        assert!((b.vel.z + 2402).abs() <= 45, "{}", b.vel.z);
        assert_eq!(b.vel.x, 0);
        for _ in 0..30 {
            step_body(&mut b, &mut f, &hold(FORWARD), 0, 0, None, 0);
        }
        assert_eq!(b.vel.z, -2402);
    }

    #[test]
    fn shift_runs_only_while_the_sword_is_sheathed() {
        let n = 0;
        assert_eq!(wanted(&hold(FORWARD | RUN_KEY), 0, false, n).z, -5461);
        assert_eq!(
            wanted(&hold(FORWARD | RUN_KEY), 0, true, n).z,
            -2402,
            "drawn: footwork"
        );
        // the speedster's 20% a piece reaches footwork too: 2.2 × 2 = 4.4
        // m/s, 288 358 / 60 = 4 805
        assert_eq!(wanted(&hold(FORWARD), 0, true, 5).z, -4805);
    }

    #[test]
    fn movement_is_relative_to_the_heading_given() {
        // heading a quarter turn left faces west: W walks west (−x)
        let v = wanted(&hold(FORWARD), fx::QUARTER_TURN, false, 0);
        assert!(v.x < -2300 && v.z.abs() <= 2, "{v:?}");
    }

    #[test]
    fn sheathed_a_body_turns_to_its_way_and_drawn_it_turns_to_its_foe() {
        let mut b = Body::new(V3::ZERO, 0);
        let mut f = fighter();
        for _ in 0..40 {
            step_body(&mut b, &mut f, &hold(crate::input::LEFT), 0, 0, None, 0);
        }
        assert_eq!(b.facing, fx::QUARTER_TURN, "walking west faces west");
        // drawn, a foe due east: it turns there at 6° a tick, walking or not
        f.sheath = crate::player::Sheath::Drawn;
        let east = v3(fx::int(5), 0, 0);
        for _ in 0..40 {
            step_body(&mut b, &mut f, &Input::default(), 0, 0, Some(east), 0);
        }
        let want = yaw_of(east.sub(b.pos));
        assert!(fx::wrap(b.facing - want).abs() <= deg(1), "{} {want}", b.facing);
    }

    #[test]
    fn a_dodge_steps_aside_once_then_waits() {
        let mut b = Body::new(V3::ZERO, 0);
        let mut f = fighter();
        let side = Input {
            keys: crate::input::RIGHT | crate::input::DODGE,
            ..Input::default()
        };
        step_body(&mut b, &mut f, &side, 0, 0, None, 0);
        f.last = side;
        for _ in 0..DODGE_TICKS {
            step_body(&mut b, &mut f, &hold(crate::input::RIGHT), 0, 0, None, 0);
        }
        // 6 m/s for 8 ticks with a third of the gap closed each tick: more
        // than half a metre east, against 2.2 m/s walking's 0.25 m
        assert!(b.pos.x > ONE / 2, "{}", b.pos.x);
        // set to 40 on the tick it starts, then one less each tick after
        assert_eq!(f.gait.dodge_wait, DODGE_WAIT - DODGE_TICKS);
    }

    #[test]
    fn the_knee_meets_both_bones_and_bends_forward() {
        // the foot 0.66 m from the hip, inside the 0.72 m both bones reach
        let (hip, foot) = (v3(0, fx::int(1), 0), v3(0, ONE * 35 / 100, -ONE / 10));
        let (a, c) = (ONE * 37 / 100, ONE * 35 / 100);
        let k = knee(hip, foot, a, c, v3(0, 0, -ONE));
        assert!((k.dist(hip) - a).abs() < ONE / 200, "{}", k.dist(hip));
        assert!((k.dist(foot) - c).abs() < ONE / 200, "{}", k.dist(foot));
        assert!(k.z < foot.z.min(hip.z) + ONE / 100, "the knee is forward: {k:?}");
    }

    #[test]
    fn the_speedster_setting_is_refused_out_of_range() {
        let mut w = World::new(1);
        assert_eq!(
            act(&mut w, "speedster", "6"),
            Err("The speedster pieces are a whole number from 0 to 5, not \"6\".".into())
        );
        act(&mut w, "speedster", "5").unwrap();
        assert_eq!(pieces(&w), 5);
        act(&mut w, "worn", "").unwrap();
        assert_eq!(pieces(&w), 0);
    }
}
