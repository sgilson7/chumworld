//! `swing` (M7, T10): how the mouse's path becomes a cut.
//!
//! The person: "the combat should be made easier to decide between
//! horizontal and vertical swings, right now all i do are these like limp
//! diagonal slashes". Before this module the left button only raised the
//! servos' caps while the hand followed the mouse at 0.15° a pixel, so a
//! cut was as long and as fast as the mouse's own stroke, and its plane was
//! whatever slant the stroke had (R9.160).
//!
//! **Choosing.** While the left button is held the mouse no longer moves
//! the hand: its travel is added up (`Swing::gesture`). Once it passes
//! `COMMIT_PX` the direction of travel picks the cut, as a Blade Symphony
//! stance does: within `SNAP` of an axis it is snapped to that axis
//! (sideways a level cut, down a chop, up a rising cut); between, a
//! diagonal cut at 45°. The wheel turned away (`THRUST_WHEEL`) picks a
//! thrust. A level cut's height comes from where the hand was (`Band`).
//! The choice is read from the input record alone, so a pilot cuts the
//! same way (CW-3).
//!
//! **Cutting.** A cut is a plane through the body and an arc in it: the
//! blade from `start` on the side opposite the travel, through straight
//! ahead, to `end` on the side of the travel. Nothing is animated: the
//! hand's servo is aimed at a point on the arc a little ahead of where the
//! hand now is (`LEAD`), as Vagrancy's joint motor drives a limb round its
//! joint at a capped speed, and the blade's servo at the line the arc asks
//! for there. A wind-up first pulls the hand back past the start
//! (`WIND`), the strike drives it through, and a follow-through carries it
//! `FOLLOW` past the end. A heavy weapon's heft takes from the caps, so a
//! cleaver comes round later and slower; a blade that meets a guard stops
//! there (`blade.rs`), and a cut that stops gaining ground for
//! `STALL_TICKS` is over.
//!
//! Every rule here is in the core and in Q16.16 (CW-1, CW-2).

use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{facing, right_of, V3};
use crate::input::{self, Input};
use crate::look::{rgb, Look};
use crate::player::{self, Sheath};
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::world::World;

/// Travel that picks a cut: 40 CSS pixels (hundredths).
pub const COMMIT_PX: i32 = 4_000;
/// Travel from which the cue shows the cut it would pick: 12 pixels.
pub const CUE_PX: i32 = 1_200;
/// Within this of an axis the travel snaps to it; farther, a diagonal.
pub const SNAP: i32 = deg(30);
/// The wheel turned this far away from the reader picks a thrust (a notch).
pub const THRUST_WHEEL: i32 = 100;
/// The hand's angle on its arc is this share of the blade's.
pub const HAND_PCT: i32 = 80;
/// The wind-up pulls the blade this far back past the start; the strike's
/// target leads the hand by `LEAD` along the arc; the follow-through runs
/// this far past the end.
pub const WIND: i32 = deg(20);
pub const LEAD: i32 = deg(40);
pub const FOLLOW: i32 = deg(20);
/// The wind-up is over once the hand is this near its point, or after
/// `WIND_TICKS`; the follow-through lasts `FOLLOW_TICKS`.
pub const WIND_NEAR: Fx = ONE * 8 / 100;
pub const WIND_TICKS: u8 = 16;
pub const FOLLOW_TICKS: u8 = 8;
/// A strike that gains less than `STALL_GAIN` in `STALL_TICKS` has met
/// something it cannot pass: the cut is over. No cut lasts past `MAX_TICKS`.
pub const STALL_TICKS: u8 = 5;
pub const STALL_GAIN: i32 = deg(3);
pub const MAX_TICKS: u8 = 60;
/// A heavy weapon's wind-up takes a while to turn round: no stall is
/// counted in a strike's first ticks.
pub const STRIKE_GRACE: u8 = 10;
/// The wind-up drives the arm at this share of its strength.
pub const WIND_PCT: i32 = 80;
/// The heights a level cut takes from the hand's pitch: above `HIGH_FROM`
/// high, below `LOW_FROM` low, else the middle; the plane's height under
/// or over the sword shoulder, percent of the arm.
pub const HIGH_FROM: i32 = deg(20);
pub const LOW_FROM: i32 = -deg(30);
pub const HIGH_PCT: i32 = 20;
pub const MIDDLE_PCT: i32 = -25;
pub const LOW_PCT: i32 = -45;
/// A chop or a rising cut falls this far (percent of the arm) toward the
/// body's middle from the sword shoulder, so it comes down in front of the
/// chest.
pub const INWARD_PCT: i32 = 15;
/// The hand's arc keeps this share of the arm from the shoulder.
pub const ARC_PCT: i32 = 92;
/// A thrust: drawn in to `CHAMBER_PCT` of the arm, then out to `OUT_PCT`,
/// held `FOLLOW_TICKS`, along this pitch.
pub const CHAMBER_PCT: i32 = 45;
pub const OUT_PCT: i32 = 97;
pub const THRUST_PITCH: i32 = -deg(8);

/// How high a level cut runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    High,
    Middle,
    Low,
}

impl Band {
    pub fn of(pitch: i32) -> Band {
        if pitch > HIGH_FROM {
            Band::High
        } else if pitch < LOW_FROM {
            Band::Low
        } else {
            Band::Middle
        }
    }

    fn pct(self) -> i32 {
        match self {
            Band::High => HIGH_PCT,
            Band::Middle => MIDDLE_PCT,
            Band::Low => LOW_PCT,
        }
    }
}

/// The cut chosen, by name (the cue's and the pilots' words).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// sideways: `true` from right to left (the mouse moved left)
    Level(bool),
    Chop,
    Rising,
    /// the angle of travel on the screen, a diagonal
    Diagonal(i32),
    Thrust,
}

/// Where a cut is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    WindUp,
    Strike,
    Follow,
}

/// A cut under way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cut {
    pub way: Way,
    /// the direction of travel on the screen (0 right, a quarter turn up),
    /// snapped
    pub travel: i32,
    pub band: Band,
    pub phase: Phase,
    /// ticks in this phase, and in the cut
    pub t: u8,
    pub age: u8,
    /// the hand's target on its arc (angle), and the best the hand has
    /// reached, with the tick it got there
    pub aim: i32,
    pub best: (i32, u8),
}

/// A fighter's swing: the travel added up while the button is held, and
/// the cut under way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Swing {
    /// mouse right, down, and wheel since the button went down or the last
    /// cut ended (hundredths of a pixel; wheel units)
    pub gesture: (i32, i32, i32),
    pub cut: Option<Cut>,
}

/// The cut a travel picks, if it is far enough: right and down positive,
/// the wheel toward the reader positive.
pub fn choose(dx: i32, dy: i32, wheel: i32) -> Option<(Way, i32)> {
    if wheel <= -THRUST_WHEEL {
        return Some((Way::Thrust, deg(90)));
    }
    let far = fx::length(dx, dy);
    if far < COMMIT_PX {
        return None;
    }
    Some(snap(dx, dy))
}

/// The travel's direction, snapped to an axis within `SNAP` of one and to a
/// diagonal otherwise, and the cut it names.
pub fn snap(dx: i32, dy: i32) -> (Way, i32) {
    let a = fx::atan2(-dy, dx);
    let q = deg(90);
    let axis = (a + q / 2).div_euclid(q) * q;
    let off = fx::wrap(a - axis);
    let travel = if off.abs() <= SNAP {
        fx::wrap(axis)
    } else {
        let d = deg(45);
        fx::wrap((a + d / 2).div_euclid(d) * d)
    };
    let way = match travel {
        0 => Way::Level(false),
        t if t == -fx::HALF_TURN => Way::Level(true),
        t if t == deg(90) => Way::Rising,
        t if t == -deg(90) => Way::Chop,
        t => Way::Diagonal(t),
    };
    (way, travel)
}

/// The blade's angles on its arc: where it starts (the side away from the
/// travel) and how far past straight ahead it ends. A low level cut stops
/// shorter across the body, where the hips are (`LOW_ACROSS`).
pub fn extent(way: Way, band: Band) -> (i32, i32) {
    let low = band == Band::Low;
    match way {
        Way::Level(true) => (deg(95), if low { LOW_ACROSS } else { deg(70) }),
        Way::Level(false) => (if low { LOW_ACROSS } else { deg(75) }, deg(90)),
        Way::Chop => (deg(115), deg(50)),
        Way::Rising => (deg(50), deg(105)),
        Way::Diagonal(_) => (deg(105), deg(60)),
        Way::Thrust => (0, 0),
    }
}

/// How far across the body a low level cut goes, past straight ahead.
pub const LOW_ACROSS: i32 = deg(45);

/// A cut's plane for a body facing `yaw`: forward, and the side the cut
/// starts on (opposite the travel), both unit; a direction at angle φ on
/// the arc is `forward cos φ + start sin φ`.
pub fn plane(yaw: i32, travel: i32) -> (V3, V3) {
    let (r, u) = (right_of(yaw), V3::UP);
    let start = r.scale(-fx::cos(travel)).add(u.scale(-fx::sin(travel)));
    (facing(yaw, 0), start.norm())
}

fn on_arc(f: V3, s: V3, phi: i32) -> V3 {
    f.scale(fx::cos(phi)).add(s.scale(fx::sin(phi)))
}

/// The centre of the hand's arc and its radius, for a sword shoulder at
/// `shoulder` and an arm `arm` long.
pub fn pivot(cut: &Cut, yaw: i32, shoulder: V3, arm: Fx) -> (V3, Fx) {
    let level = fx::cos(cut.travel).abs();
    let up = (arm as i64 * cut.band.pct() as i64 / 100 * level as i64 / ONE as i64) as Fx;
    let inward = (arm as i64 * INWARD_PCT as i64 / 100 * (ONE - level) as i64 / ONE as i64) as Fx;
    let off = V3::UP.scale(up).add(right_of(yaw).scale(-inward));
    let most = arm * ARC_PCT / 100;
    let r = fx::sqrt((fx::mul(most, most) - fx::mul(off.len(), off.len())).max(ONE / 400));
    (shoulder.add(off), r)
}

/// The hand's target and the blade's line for the arm's servos this tick,
/// for a hand at `hand`: in the strike the target is pushed `DRIVE_PCT` of
/// the way on past the point on the arc, so the servo drives the hand round
/// at its cap rather than easing in to the point (a motor, not a spring).
pub fn targets(cut: &Cut, yaw: i32, shoulder: V3, arm: Fx, hand: V3) -> (V3, V3) {
    let (to, line) = aim(cut, yaw, shoulder, arm);
    if cut.phase == Phase::Strike {
        (hand.add(to.sub(hand).scale(ONE * DRIVE_PCT / 100)), line)
    } else {
        (to, line)
    }
}

/// How hard the strike drives the hand round its arc: the servo is aimed
/// this share of the way to the point on the arc ahead.
pub const DRIVE_PCT: i32 = 250;
/// The blade's line is lifted by this much of the world's up, against the
/// sag of the tip under its weight: a level cut stays level.
pub const LIFT: Fx = ONE / 16;

/// The line a cut's blade servo turns toward: the line asked for, or,
/// while the blade points more than a quarter turn from it, the line
/// halfway, which lies in the cut's plane when both do.
pub fn toward(a: &crate::arm::Arm, want: V3) -> V3 {
    let now = a.pts[crate::arm::TIP].p.sub(a.grip_point()).norm();
    if now.dot(want) >= 0 {
        return want;
    }
    let mid = now.add(want);
    if mid.len() < ONE / 20 {
        want
    } else {
        mid.norm()
    }
}

/// A cut wider than the free hand can follow on a two-handed hilt: a
/// level cut, or a diagonal (more than half level).
pub fn wide(cut: &Cut) -> bool {
    cut.way != Way::Thrust && fx::cos(cut.travel).abs() > ONE / 2
}

/// The share of the arm's strength a cut drives with: the wind-up a
/// gathering, `WIND_PCT`, the strike and the follow-through all of it.
pub fn strength_pct(cut: &Cut) -> i32 {
    if cut.phase == Phase::WindUp {
        WIND_PCT
    } else {
        100
    }
}

/// The hand's target and the blade's line this tick for a cut, the body
/// facing `yaw`.
pub fn aim(cut: &Cut, yaw: i32, shoulder: V3, arm: Fx) -> (V3, V3) {
    if cut.way == Way::Thrust {
        let pct = if cut.phase == Phase::WindUp {
            CHAMBER_PCT
        } else {
            OUT_PCT
        };
        let line = facing(yaw + deg(4), THRUST_PITCH);
        return (shoulder.add(line.scale(arm * pct / 100)), line);
    }
    let (f, s) = plane(yaw, cut.travel);
    let (p, r) = pivot(cut, yaw, shoulder, arm);
    let (start, end) = extent(cut.way, cut.band);
    let blade = (cut.aim as i64 * 100 / HAND_PCT as i64) as i32;
    let blade = blade.clamp(-end - FOLLOW, start + WIND);
    let line = on_arc(f, s, blade).add(V3::UP.scale(LIFT)).norm();
    (p.add(on_arc(f, s, cut.aim).scale(r)), line)
}

/// The motor that turns the weapon through a cut's strike and lets it run
/// down through the follow-through (`arm::Turn`); none in the wind-up or a
/// thrust, which the servos drive.
pub fn turn(cut: &Cut, yaw: i32, shoulder: V3, arm: Fx, hand: V3) -> Option<crate::arm::Turn> {
    if cut.phase == Phase::WindUp || cut.way == Way::Thrust {
        return None;
    }
    let (f, s) = plane(yaw, cut.travel);
    let (p, r) = pivot(cut, yaw, shoulder, arm);
    let now = progress(cut, yaw, shoulder, arm, hand);
    let (start, end) = extent(cut.way, cut.band);
    let blade = ((now as i64 * 100 / HAND_PCT as i64) as i32).clamp(-end - FOLLOW, start + WIND);
    Some(crate::arm::Turn {
        pivot: p,
        axis: s.cross(f).norm(),
        most: if cut.phase == Phase::Strike { TURN_MOST } else { 0 },
        gain: TURN_GAIN,
        on: p.add(on_arc(f, s, now).scale(r)),
        line: on_arc(f, s, blade).add(V3::UP.scale(LIFT)).norm(),
    })
}

/// The strike's turn: at most 0.18 radians a tick (10°, 11 rad/s: a short
/// sword's tip at about 15 m/s), less for a heavy weapon (`arm::Arm::motor`),
/// gathered by at most 0.05 radians a tick each tick, times the weapon's
/// heft and the arm's strength.
pub const TURN_MOST: Fx = ONE * 18 / 100;
pub const TURN_GAIN: Fx = ONE * 5 / 100;

/// The hand's angle on the cut's arc, from where it is.
pub fn progress(cut: &Cut, yaw: i32, shoulder: V3, arm: Fx, hand: V3) -> i32 {
    let (f, s) = plane(yaw, cut.travel);
    let (p, _) = pivot(cut, yaw, shoulder, arm);
    let d = hand.sub(p);
    fx::wrap(fx::atan2(d.dot(s), d.dot(f)))
}

/// The blade's angle on the cut's arc, pointing along `d`.
pub fn blade_at(cut: &Cut, yaw: i32, d: V3) -> i32 {
    let (f, s) = plane(yaw, cut.travel);
    fx::wrap(fx::atan2(d.dot(s), d.dot(f)))
}

fn hand_of(a: i32) -> i32 {
    a * HAND_PCT / 100
}

/// A cut begun: the hand's target at the wind-up's point.
pub fn begin(way: Way, travel: i32, pitch: i32) -> Cut {
    let band = Band::of(pitch);
    let (start, _) = extent(way, band);
    Cut {
        way,
        travel,
        band,
        phase: Phase::WindUp,
        t: 0,
        age: 0,
        aim: hand_of(start + WIND),
        best: (hand_of(start + WIND), 0),
    }
}

/// One tick of a fighter's swing from its record: the travel added up and
/// a cut chosen while the button is held, a cut under way moved on. Returns
/// whether the mouse is taken by the swing this tick (the hand's target on
/// the shell then holds still).
pub fn step(w: &mut World, id: usize, inp: &Input) -> bool {
    let f = player::fighter(w, id);
    let yaw = player::body(w, id).facing;
    let arm = f.arm.reach();
    let shoulder = f.arm.frame.shoulder[1];
    let hand = f.arm.grip_point();
    let tip = f.arm.pts[crate::arm::TIP].p;
    let pitch = f.arm.shell.pitch;
    let held = inp.button(input::SWING);
    let mut sw = f.arm.swing;
    let mut ended = false;
    if let Some(mut c) = sw.cut {
        c.age = c.age.saturating_add(1);
        c.t = c.t.saturating_add(1);
        let now = progress(&c, yaw, shoulder, arm, hand);
        let (_, end) = extent(c.way, c.band);
        match c.phase {
            Phase::WindUp if c.way != Way::Thrust => {
                // the blade drawn back to the start: the strike turns it round
                let (start, _) = extent(c.way, c.band);
                if blade_at(&c, yaw, tip.sub(hand)) >= start || c.t >= WIND_TICKS {
                    c.phase = Phase::Strike;
                    c.t = 0;
                    c.best = (now, c.age);
                }
            }
            Phase::WindUp => {
                let (to, _) = aim(&c, yaw, shoulder, arm);
                if hand.dist(to) < WIND_NEAR || c.t >= WIND_TICKS {
                    c.phase = Phase::Strike;
                    c.t = 0;
                }
            }
            Phase::Strike if c.way == Way::Thrust => {
                // out to the arm's length (or as far as it gets)
                let (to, _) = aim(&c, yaw, shoulder, arm);
                if hand.dist(to) < WIND_NEAR / 4 || c.t >= WIND_TICKS / 2 {
                    c.phase = Phase::Follow;
                    c.t = 0;
                }
            }
            Phase::Strike => {
                // ground gained, or the wind-up still carrying it back:
                // the stall is counted from here
                if now < c.best.0 - STALL_GAIN || now > c.best.0 {
                    c.best = (now, c.age);
                }
                c.aim = (now - LEAD).max(hand_of(-end - FOLLOW)).min(c.aim);
                if now <= hand_of(-end) {
                    c.phase = Phase::Follow;
                    c.t = 0;
                    c.aim = hand_of(-end - FOLLOW);
                } else if c.t > STRIKE_GRACE && c.age - c.best.1 >= STALL_TICKS {
                    ended = true;
                }
            }
            Phase::Follow => {
                if c.t >= FOLLOW_TICKS {
                    ended = true;
                }
            }
        }
        // a blow landed or a block met last tick: the cut is spent on it
        // (or the blade lies on a body, or against a blade: pressed on, the
        // turn would drive it through)
        let met = w.blades.hits.iter().any(|h| h.by as usize == id)
            || w.blades
                .clashes
                .iter()
                .any(|k| k.a as usize == id || k.b as usize == id)
            || w.blades
                .touching
                .iter()
                .any(|&(by, on)| by as usize == id && on as usize != id)
            || w.blades
                .clashing
                .iter()
                .any(|&(a, b)| a as usize == id || b as usize == id);
        if c.age >= MAX_TICKS || (met && c.phase != Phase::WindUp) {
            ended = true;
        }
        sw.cut = if ended { None } else { Some(c) };
        if ended {
            sw.gesture = (0, 0, 0);
        }
    } else if held {
        let g = sw.gesture;
        sw.gesture = (
            g.0.saturating_add(inp.dx),
            g.1.saturating_add(inp.dy),
            g.2.saturating_add(inp.wheel),
        );
        let g = sw.gesture;
        if let Some((way, travel)) = choose(g.0, g.1, g.2) {
            sw.cut = Some(begin(way, travel, pitch));
            sw.gesture = (0, 0, 0);
        }
    } else {
        sw.gesture = (0, 0, 0);
    }
    let a = &mut player::parts_mut(w, id).1.arm;
    a.swing = sw;
    if ended {
        // the hand's target on the shell is where the cut left the hand,
        // so the mouse goes on from there
        a.shell = shell_at(&a.shell, yaw, shoulder, hand, arm);
    }
    held || sw.cut.is_some() || ended
}

/// The shell's target at `hand`, held to the shell's edges.
pub fn shell_at(s: &crate::arm::Shell, yaw: i32, shoulder: V3, hand: V3, arm: Fx) -> crate::arm::Shell {
    use crate::arm::{PITCH_MAX, PITCH_MIN, REACH_MAX_PCT, REACH_MIN_PCT, SIDE_MAX, SIDE_MIN};
    let d = hand.sub(shoulder);
    let side = fx::wrap(yaw - crate::geom::yaw_of(d)).clamp(SIDE_MIN, SIDE_MAX);
    let pitch = fx::wrap(fx::atan2(d.y, d.len_xz().max(1))).clamp(PITCH_MIN, PITCH_MAX);
    let reach = d
        .len()
        .clamp(arm * REACH_MIN_PCT / 100, arm * REACH_MAX_PCT / 100);
    crate::arm::Shell {
        side,
        pitch,
        reach,
        rest: s.rest,
        swing: (0, 0, 0),
    }
}

/// What the cue shows for the player this tick: the cut being chosen (the
/// travel so far, once past `CUE_PX`), or the cut winding up.
pub fn cue(w: &World) -> Option<(Way, i32, Band)> {
    let f = &w.me;
    if !w.ui.settings.cue || f.sheath != Sheath::Drawn || f.health.down() || f.mouse_looks {
        return None;
    }
    let sw = f.arm.swing;
    match sw.cut {
        Some(c) if c.phase == Phase::WindUp => Some((c.way, c.travel, c.band)),
        Some(_) => None,
        None => {
            let (dx, dy, wheel) = sw.gesture;
            if wheel <= -THRUST_WHEEL {
                Some((Way::Thrust, deg(90), Band::Middle))
            } else if fx::length(dx, dy) >= CUE_PX && f.last.button(input::SWING) {
                let (way, travel) = snap(dx, dy);
                Some((way, travel, Band::of(f.arm.shell.pitch)))
            } else {
                None
            }
        }
    }
}

/// The cue's line over the game: the cut's name and its way, while the
/// cue shows (`cue`), and nothing otherwise.
pub fn hud(w: &World, s: &crate::page::Strings, out: &mut Vec<crate::world::HudItem>) {
    let c = cue(w);
    out.push(crate::world::HudItem {
        id: "swing".into(),
        text: c.map(|c| s.get(cue_key(c.0))).unwrap_or_default(),
        shown: c.is_some(),
    });
}

/// The cue's string key.
pub fn cue_key(way: Way) -> &'static str {
    match way {
        Way::Level(true) => "swing_level_left",
        Way::Level(false) => "swing_level_right",
        Way::Chop => "swing_chop",
        Way::Rising => "swing_rising",
        Way::Diagonal(t) if t > 0 && t < deg(90) => "swing_diagonal_up_right",
        Way::Diagonal(t) if t > 0 => "swing_diagonal_up_left",
        Way::Diagonal(t) if t > -deg(90) => "swing_diagonal_down_right",
        Way::Diagonal(_) => "swing_diagonal_down_left",
        Way::Thrust => "swing_thrust",
    }
}

/// The cue in the world: a faint line of beads where the tip will run,
/// from the start to past the end, the last bead larger: the way it goes.
/// Nothing while there is no cue (`cue`).
pub fn draw(w: &World, look: &Look, out: &mut Shapes) {
    let Some((way, travel, band)) = cue(w) else {
        return;
    };
    let f = &w.me;
    let yaw = w.player.facing;
    let shoulder = f.arm.frame.shoulder[1];
    let arm = f.arm.reach();
    let wpn = f.arm.weapon;
    let blade = wpn.len - wpn.grip[1];
    let colour = rgb(look.palette.lamp);
    let mut c = begin(way, travel, 0);
    c.band = band;
    let (start, end) = extent(way, band);
    let n = BEADS as i32;
    for k in 0..BEADS {
        let (hand, line) = if way == Way::Thrust {
            c.phase = if k < BEADS / 2 {
                Phase::WindUp
            } else {
                Phase::Strike
            };
            let (h, l) = aim(&c, yaw, shoulder, arm);
            let u = k as i32 * ONE / (n - 1);
            let near = shoulder.add(l.scale(arm * CHAMBER_PCT / 100));
            (near.lerp(h, u), l)
        } else {
            let phi = start - (start + end) * k as i32 / (n - 1);
            c.aim = hand_of(phi);
            aim(&c, yaw, shoulder, arm)
        };
        let at = hand.add(line.scale(blade));
        let r = if k + 1 == BEADS { BEAD_LAST } else { BEAD };
        out.glow(
            Mesh::Sphere,
            crate::geom::Affine::translate(at).then(&crate::geom::Affine::scale(crate::geom::v3(r, r, r))),
            colour,
            CUE_GLOW,
        );
    }
}

/// The cue's beads: how many, how big, how bright.
pub const BEADS: usize = 9;
pub const BEAD: Fx = ONE * 18 / 1000;
pub const BEAD_LAST: Fx = ONE * 32 / 1000;
pub const CUE_GLOW: Fx = ONE * 2 / 5;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arm::{self, Arm};
    use crate::fx::int;
    use crate::geom::v3;
    use crate::input::{Input, DRAW, SWING};
    use crate::pose::{Held, Kind};

    /// The player alone, facing north, the sword drawn and settled at the
    /// guard, holding `held` (the short sword unless given).
    fn ready(held: Held) -> World {
        let mut w = World::new(1);
        w.aim.yaw = 0;
        w.player.facing = 0;
        w.player.prev_facing = 0;
        if held != Held::ShortSword {
            let p = w.player.pos;
            w.me = player::Fighter::new(Kind::Player, held, 0, p, 0, false);
        }
        w.step(&Input {
            keys: DRAW,
            ..Default::default()
        });
        for _ in 0..90 {
            w.step(&Input::default());
        }
        w
    }

    /// The button held and the mouse moved `dx`, `dy` (hundredths of a
    /// pixel) and the wheel `wheel` on the first tick, then held still.
    fn gesture(dx: i32, dy: i32, wheel: i32) -> impl Fn(usize) -> Input {
        move |t| Input {
            buttons: SWING,
            dx: if t == 0 { dx } else { 0 },
            dy: if t == 0 { dy } else { 0 },
            wheel: if t == 0 { wheel } else { 0 },
            ..Default::default()
        }
    }

    /// The blade's direction each tick of a cut's strike and follow-through
    /// (grip to tip, unit), and the tip's top speed over them, m/s.
    fn strike(w: &mut World, input: impl Fn(usize) -> Input) -> (Vec<V3>, Fx) {
        let (mut dirs, mut top) = (Vec::new(), 0);
        for t in 0..90 {
            w.step(&input(t));
            let a: &Arm = &w.me.arm;
            match a.swing.cut {
                Some(c) if c.phase != Phase::WindUp => {
                    dirs.push(a.pts[arm::TIP].p.sub(a.grip_point()).norm());
                    top = top.max(player::mps(a.pts[arm::TIP].vel().len()));
                }
                None if !dirs.is_empty() => break,
                _ => {}
            }
        }
        (dirs, top)
    }

    /// The angle a direction makes on a plane of two unit axes, and the arc
    /// swept tick to tick, all wrapped.
    fn swept(dirs: &[V3], a: V3, b: V3) -> i32 {
        let at = |d: &V3| fx::atan2(d.dot(b), d.dot(a));
        dirs.windows(2).map(|p| fx::wrap(at(&p[1]) - at(&p[0]))).sum()
    }

    /// sin 10° = 0.17365: 11 380 units. The cuts' planes are held to that.
    const SIN_10: Fx = 11_380;

    /// Mouse 60 px straight left with the button: a level cut. Every tick of
    /// its strike and follow-through the blade's rise is within 10° of
    /// level (|up| ≤ sin 10°), so the plane it turns in is within 10° of
    /// horizontal; it sweeps from the right across to the left (its heading
    /// on the ground grows to the left, counter-clockwise from above) by
    /// 140° or more: the cut asks for 95° right to 70° left, 165°, and a
    /// follow-through past that.
    #[test]
    fn pure_sideways_input_gives_a_level_cut_of_a_full_arc() {
        let mut w = ready(Held::ShortSword);
        let (dirs, _) = strike(&mut w, gesture(-6_000, 0, 0));
        assert!(dirs.len() > 5, "a cut was made: {}", dirs.len());
        for d in &dirs {
            assert!(d.y.abs() <= SIN_10, "the blade rose {} (bound {SIN_10})", d.y);
        }
        // forward is −z, left is −x, for a body facing north (yaw 0)
        let arc = swept(&dirs, v3(0, 0, -ONE), v3(-ONE, 0, 0));
        assert!(arc >= deg(140), "swept {}°", arc * 360 / fx::TURN);
        assert!(arc <= deg(230), "swept {}°", arc * 360 / fx::TURN);
    }

    /// Mouse 60 px straight down: a chop. The blade keeps within 10° of the
    /// upright plane through straight ahead (|right| ≤ sin 10°) and falls
    /// from up and back over the head down through the front, 140° or
    /// more (asked for: 115° up and back to 50° below ahead).
    #[test]
    fn pure_vertical_input_gives_an_upright_chop_of_a_full_arc() {
        let mut w = ready(Held::ShortSword);
        let (dirs, _) = strike(&mut w, gesture(0, 6_000, 0));
        assert!(dirs.len() > 5, "a cut was made");
        for d in &dirs {
            assert!(d.x.abs() <= SIN_10, "the blade leaned {} out of the upright", d.x);
        }
        // on the upright plane: forward −z, up +y; a chop turns from up to down
        let arc = -swept(&dirs, v3(0, 0, -ONE), v3(0, ONE, 0));
        assert!(arc >= deg(140), "fell {}°", arc * 360 / fx::TURN);
        // and up: a rising cut, the same plane the other way
        let mut w = ready(Held::ShortSword);
        let (dirs, _) = strike(&mut w, gesture(0, -6_000, 0));
        for d in &dirs {
            assert!(d.x.abs() <= SIN_10, "the rising cut leaned {}", d.x);
        }
        let arc = swept(&dirs, v3(0, 0, -ONE), v3(0, ONE, 0));
        assert!(arc >= deg(140), "rose {}°", arc * 360 / fx::TURN);
    }

    /// The snap, worked by hand. Travel 20° below straight left (right 94,
    /// down 34: atan(34/94) = 19.9°) is within `SNAP` (30°) of the axis: a
    /// level cut from the right. 45° (right −100, down 100) is a diagonal at
    /// −135° (down and to the left). 29° snaps (tan 29° = 0.554: 1 000 and
    /// 554); 31° does not (tan 31° = 0.601: 1 000 and 601), and is a
    /// diagonal. Up 20° off straight up (right 34, up 94) is a rising cut;
    /// down is a chop; right is a level cut from the left. Under 40 px
    /// nothing is picked; the wheel turned a notch away is a thrust.
    #[test]
    fn a_travel_near_an_axis_snaps_to_it_and_a_clear_slant_is_a_diagonal() {
        assert_eq!(snap(-94, 34).0, Way::Level(true));
        assert_eq!(snap(-100, 100), (Way::Diagonal(-deg(135)), -deg(135)));
        assert_eq!(snap(-1_000, 554).0, Way::Level(true));
        assert_eq!(snap(-1_000, 601).0, Way::Diagonal(-deg(135)));
        assert_eq!(snap(34, -94).0, Way::Rising);
        assert_eq!(snap(10, 300).0, Way::Chop);
        assert_eq!(snap(300, 0).0, Way::Level(false));
        assert_eq!(snap(100, -100).0, Way::Diagonal(deg(45)));
        assert_eq!(choose(-3_999, 0, 0), None, "39.99 px picks nothing");
        assert_eq!(choose(-4_000, 0, 0).map(|c| c.0), Some(Way::Level(true)));
        assert_eq!(choose(0, 0, -100).map(|c| c.0), Some(Way::Thrust));
        assert_eq!(choose(0, 0, -99), None);
    }

    /// The same snap through the arm: 25° off straight left (tan 25° =
    /// 0.466: 5 640 and 2 630) makes a cut as level as a straight one
    /// (within 10°), and a 45° slant down and to the left makes a diagonal
    /// from high on the right to low on the left: the blade rises at least
    /// sin 35° (0.574: 37 590) at some tick, pointing right, and falls as
    /// far at another, pointing left.
    #[test]
    fn an_off_axis_travel_cuts_on_its_axis_and_a_slant_cuts_on_the_slant() {
        let mut w = ready(Held::ShortSword);
        let (dirs, _) = strike(&mut w, gesture(-5_640, 2_630, 0));
        assert!(dirs.len() > 5);
        assert!(dirs.iter().all(|d| d.y.abs() <= SIN_10), "a level cut");
        let mut w = ready(Held::ShortSword);
        let (dirs, _) = strike(&mut w, gesture(-4_300, 4_300, 0));
        let hi = *dirs.iter().max_by_key(|d| d.y).unwrap();
        let lo = *dirs.iter().min_by_key(|d| d.y).unwrap();
        assert!(hi.y >= 37_590 && hi.x > 0, "high on the right: {hi:?}");
        assert!(lo.y <= -37_590 && lo.x < 0, "low on the left: {lo:?}");
    }

    /// The short sword's tip tops 12 m/s in a committed level cut (the
    /// person's "limp" swing reached 4.7 to 11 m/s, R9.160); the cleaver,
    /// at 45% heft, comes round slower, and so does its tip, though it is
    /// farther out.
    #[test]
    fn a_short_sword_cuts_fast_and_a_cleaver_slower() {
        let mut w = ready(Held::ShortSword);
        let (_, short) = strike(&mut w, gesture(-6_000, 0, 0));
        let mut w = ready(Held::Cleaver);
        let (_, cleaver) = strike(&mut w, gesture(-6_000, 0, 0));
        assert!(short >= int(12), "short sword {} m/s", fx::decimal(short, 1));
        assert!(
            cleaver < short * 85 / 100,
            "cleaver {} against {}",
            fx::decimal(cleaver, 1),
            fx::decimal(short, 1)
        );
    }

    /// A level cut into a still orc's guard held across its way (60° to the
    /// orc's left, 30° down, 160 cm off): the blades meet, the cut is over
    /// the next tick (held still or not), and the first tick the world runs
    /// after the meeting the tip has lost more than half its speed.
    #[test]
    fn a_cut_into_a_held_guard_stops_on_it() {
        let mut w = World::new(3);
        player::act(&mut w, "pilot", "still").unwrap();
        player::act(&mut w, "spawn", "orc").unwrap();
        let ahead = facing(w.aim.yaw, 0).scale(ONE * 160 / 100);
        w.foes[0].body.pos = w.player.pos.add(ahead);
        w.foes[0].body.prev_pos = w.foes[0].body.pos;
        player::act(&mut w, "guard", "-60,-30").unwrap();
        w.step(&Input {
            keys: DRAW,
            ..Default::default()
        });
        for _ in 0..90 {
            w.step(&Input::default());
        }
        let mut met = None;
        for t in 0..40 {
            let before = player::mps(w.me.arm.pts[arm::TIP].vel().len());
            let held = w.feel.stop > 0;
            w.step(&gesture(-6_000, 0, 0)(t));
            if met.is_none() && !w.blades.clashes.is_empty() {
                met = Some(before);
                assert!(before > int(8), "the cut was under way: {before}");
                continue;
            }
            if met.is_some() {
                assert!(w.me.arm.swing.cut.is_none(), "the cut went on past the guard");
            }
            if let (Some(speed), false) = (met, held) {
                assert!(player::mps(w.me.arm.pts[arm::TIP].vel().len()) < speed / 2);
                return;
            }
        }
        panic!("the blades never met");
    }

    /// CW-3 with cuts: an orc run by its tree for 30 s makes level cuts
    /// and chops (its book weighs both), and the same duel replayed from the
    /// orc's records with its tree taken out makes the same cuts on the same
    /// ticks and ends the same: the cuts came through the record.
    #[test]
    fn a_pilot_s_level_cuts_and_chops_come_through_its_input_record() {
        let run = |script: Option<Vec<Input>>| {
            let mut w = World::new(5);
            w.me.health = crate::health::Health::new(100_000);
            player::act(&mut w, "spawn", "orc").unwrap();
            if let Some(s) = script {
                w.foes[0].brain.control = crate::pilot::Control::Script(s);
            }
            let (mut kept, mut cuts) = (Vec::new(), Vec::new());
            for t in 0..1_800 {
                let held = w.feel.stop > 0;
                w.step(&Input::default());
                if !held {
                    kept.push(w.foes[0].fighter.last);
                }
                if let Some(c) = w.foes[0].fighter.arm.swing.cut {
                    if c.age == 0 {
                        cuts.push((t, c.way));
                    }
                }
            }
            (kept, cuts, w.foes[0].fighter.clone(), w.me.clone())
        };
        let (kept, cuts, orc, me) = run(None);
        assert!(cuts.iter().any(|c| matches!(c.1, Way::Level(_))), "{cuts:?}");
        assert!(cuts.iter().any(|c| c.1 == Way::Chop), "{cuts:?}");
        let (_, again, orc2, me2) = run(Some(kept));
        assert_eq!(again, cuts);
        assert_eq!((orc2, me2), (orc, me));
    }

    /// CW-4 with cuts: the player cutting every way at an orc, recorded,
    /// hashes the same every run; one tick's travel turned from left to
    /// right (a level cut the other way) changes it.
    #[test]
    fn a_recorded_duel_of_cuts_hashes_the_same_every_run() {
        let record: Vec<Input> = (0..900)
            .map(|k| {
                let ways = [(-60, 0, 0), (0, 60, 0), (60, 0, 0), (0, -60, 0), (0, 0, -1)];
                let (dx, dy, wh) = ways[(k / 60) % ways.len()];
                Input {
                    keys: if k == 2 {
                        DRAW
                    } else if k % 120 < 20 {
                        crate::input::FORWARD
                    } else {
                        0
                    },
                    buttons: if k % 60 >= 40 { SWING } else { 0 },
                    dx: if k % 60 == 40 { dx * 100 } else { 0 },
                    dy: if k % 60 == 40 { dy * 100 } else { 0 },
                    wheel: if k % 60 == 40 { wh * 100 } else { 0 },
                }
            })
            .collect();
        let run = |rec: &[Input]| {
            let mut w = World::new(8);
            player::act(&mut w, "spawn", "orc").unwrap();
            let mut cuts = 0;
            for i in rec {
                w.step(i);
                cuts += w.me.arm.swing.cut.is_some_and(|c| c.age == 0) as u32;
            }
            (w.hash(), cuts)
        };
        let (a, cuts) = run(&record);
        assert!(cuts >= 6, "{cuts} cuts");
        assert_eq!(run(&record).0, a);
        let mut other = record.clone();
        other[100].dx = 6_000;
        assert_ne!(run(&other).0, a);
    }

    /// The cue: with the button held and 20 px of travel left, nine beads
    /// more are drawn in the glow pass than with the button up, and the
    /// line over the game reads "← Level cut"; with the cue off in the
    /// settings, the same travel draws not one more shape and shows no line.
    #[test]
    fn the_swing_cue_shows_the_cut_and_draws_nothing_when_off() {
        let look = crate::look::tests::look();
        let strings = crate::page::tests::strings();
        let shapes = |w: &World| {
            let mut out = Shapes::default();
            w.draw(ONE, &look, &mut out);
            out.list.len()
        };
        let line = |w: &World| {
            let mut out = Vec::new();
            w.hud(&strings, &mut out);
            out.into_iter().find(|h| h.id == "swing").unwrap()
        };
        for on in [true, false] {
            let mut w = ready(Held::ShortSword);
            w.ui.settings.cue = on;
            let before = shapes(&w);
            assert!(!line(&w).shown);
            w.step(&Input {
                buttons: SWING,
                dx: -2_000,
                ..Default::default()
            });
            assert!(w.me.arm.swing.cut.is_none(), "20 px picks no cut yet");
            let after = shapes(&w);
            if on {
                assert_eq!(after, before + BEADS);
                assert_eq!(line(&w).text, "← Level cut");
                assert!(line(&w).shown);
            } else {
                assert_eq!(after, before, "the cue off draws nothing");
                assert!(!line(&w).shown);
            }
        }
    }

    /// A still duel draws the same: the button held with the mouse still,
    /// two ticks draw the same shapes, and nothing of the cue.
    #[test]
    fn holding_the_button_still_draws_identical_frames() {
        let look = crate::look::tests::look();
        let mut w = ready(Held::ShortSword);
        let frame = |w: &World| {
            let mut out = Shapes::default();
            w.draw(ONE, &look, &mut out);
            format!("{:?}", out.list)
        };
        let hold = Input {
            buttons: SWING,
            ..Default::default()
        };
        w.step(&hold);
        let a = frame(&w);
        w.step(&hold);
        assert_eq!(frame(&w), a);
        assert!(cue(&w).is_none());
    }
}
