//! `sprint`: the run that holds itself and builds (M7, T14). The person,
//! 2026-10-20: "you should be able to set yourself to sprinting, and if
//! you've been sprinting for 5seconds you start to go faster and faster
//! until you are running at triple speed".
//!
//! **The key.** A tap of Shift, the run key, sets a sprint on while a
//! movement key is held, and another tap ends it (SECOND-ORDER.md R9.250).
//! A tap is Shift down for at most `TAP_MOST` ticks after it has been up for
//! at least `QUIET`, and up again for `SETTLE`: holding Shift to run is never
//! a tap, and the orcs' running, Shift held four ticks in five
//! (`story::RUN_TICKS`), never one.
//!
//! **The build.** A sprint runs at the run speed. After `BUILD_AFTER` ticks
//! (4 s) unbroken, its speed eases up over `RAMP` ticks (4 s, a smoothstep,
//! so it starts and ends without a kink) to `TOP` times the run speed. It
//! is a multiplier on `motion::RUN` and whatever a road gives, so it carries
//! over any change to those (R9.251). The person (T18): "change the sprint
//! time to 4 seconds, and make it go up to 5x, and the speedster can make
//! you go 10x": the speedster's pieces multiply the sprint, the bare run
//! times the sprint times 1 + 0.2 a piece, so the top is 5 to 10 times the
//! bare run for 0 to 5 pieces, and nothing goes past `CAP`, ten times it
//! (R9.420, replacing R9.252's sum).
//!
//! **What ends it**: another tap; no movement key (a stop); the sword drawn
//! or a foe held on; a blow (a stagger, or health lost); a dodge-step; a
//! hard turn (`HARD_TURN` from the way it runs); falling down or a leg off.
//!
//! **Feel at speed.** A sprint runs along its own heading, which turns
//! toward the way the keys ask at a rate that falls with speed (`turn_rate`):
//! 600° a second at a run, 120° at `TOP` times it. Ended at speed, a body
//! skids: it loses at most `SKID` of speed a tick until it is back at the run
//! speed. Uphill, the build gives way (`STEEP`). The camera pulls back and
//! the view widens with the build (`view`), eased in and out.
//!
//! **The opening.** While the opening runs, a sprint holds at the run speed:
//! the build does not count inside the raid, so the 250 m escape takes as
//! long as running does (R9.255).
//!
//! Every fighter's sprint steps from its own input record (CW-3): a pilot
//! sprints only by tapping Shift as the player does, and none does.

use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{facing, v3, yaw_of, V3};
use crate::input::{self, Input};
use crate::motion;
use crate::page::Strings;
use crate::player::{self, Fighter};
use crate::ticks::TICKS_PER_SECOND;
use crate::world::World;

/// A tap: Shift down this many ticks at most (0.2 s) ...
pub const TAP_MOST: u16 = 12;
/// ... after it has been up at least this many (0.2 s), and counted once it
/// has stayed up this many more (0.1 s): a press soon after is a run.
pub const QUIET: u16 = 12;
pub const SETTLE: u16 = 6;
/// A sprint runs this many ticks (4 s) at the run speed before it builds.
pub const BUILD_AFTER: u32 = 240;
/// The build takes this many ticks (4 s) from the run speed to the top.
pub const RAMP: u32 = 240;
/// The top: five times the run speed.
pub const TOP: Fx = fx::int(5);
/// Nothing sprints faster than ten times the bare run speed (70 m/s),
/// whatever is worn and whatever the road.
pub const CAP: Fx = fx::int(10);
/// The keys asking for more than this from the way the sprint runs end it.
pub const HARD_TURN: i32 = deg(100);
/// The most the sprint's heading turns a tick: this at the run speed (600°
/// a second), falling to `TURN_AT_TOP` at `TOP` times it (120° a second).
pub const TURN_AT_RUN: i32 = deg(10);
pub const TURN_AT_TOP: i32 = deg(2);
/// Skidding, a body loses at most this much speed a tick: 0.5 m/s, a
/// braking of 30 m/s².
pub const SKID: Fx = ONE / 2 / TICKS_PER_SECOND as i32;
/// Uphill, the build falls in a straight line from all of it on the flat to
/// none on a slope this steep (40%, rise over run).
pub const STEEP: Fx = ONE * 40 / 100;
/// How fast the slope's share and the view ease, a tick.
/// (The view's is rounded up, so 40 ticks take it all the way.)
pub const SLOPE_EASE: Fx = ONE / 30;
pub const VIEW_EASE: Fx = (ONE + 39) / 40;
/// At the top of the build the camera sits this much further back and the
/// view is this much wider.
pub const PULL: Fx = ONE * 8 / 10;
pub const WIDEN: i32 = deg(6);

/// Why the last sprint ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum End {
    #[default]
    None,
    Tap,
    Stop,
    Drawn,
    Hit,
    Dodge,
    Turn,
    Down,
}

/// One body's sprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sprint {
    pub on: bool,
    /// ticks of unbroken sprinting counted toward the build
    pub ticks: u32,
    /// the way the sprint runs
    pub heading: i32,
    /// the share of the build the slope ahead allows, eased (`ONE` flat)
    pub slope: Fx,
    /// ended at speed: skidding down to the run speed
    pub skid: bool,
    pub ended: End,
    /// Shift: ticks it has been down, ticks it has been up (to `QUIET`),
    /// whether this press came after a quiet spell, and a short press let
    /// go that counts as a tap once Shift stays up `SETTLE`
    pub down: u16,
    pub up: u16,
    pub quiet: bool,
    pub tapping: bool,
    /// health last tick, to see a blow
    pub hp: i32,
    /// how far the view has eased toward the top's (`ONE`), now and last tick
    pub view: Fx,
    pub prev_view: Fx,
}

/// What a body's tick shows the sprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Facts {
    /// a movement key is held, and the heading it asks for
    pub moving: bool,
    pub goal: i32,
    pub drawn: bool,
    pub engaged: bool,
    pub stagger: bool,
    pub hp: i32,
    pub dodge: bool,
    pub down: bool,
    /// the body's speed across the ground, and its run speed, a tick
    pub speed: Fx,
    pub run: Fx,
    /// rise over run of the ground one metre ahead along the heading
    pub grade: Fx,
    pub opening: bool,
}

/// The build's share at `ticks` into a sprint: 0 until `BUILD_AFTER`, then a
/// smoothstep, 3t² − 2t³, over `RAMP` to `ONE`.
pub fn share(ticks: u32) -> Fx {
    if ticks <= BUILD_AFTER {
        return 0;
    }
    let t = ((ticks - BUILD_AFTER).min(RAMP) as i64 * ONE as i64 / RAMP as i64) as Fx;
    fx::mul(fx::mul(t, t), 3 * ONE - 2 * t)
}

/// The sprint's multiplier on the run speed: `ONE` off, to `TOP` at the top
/// on the flat.
pub fn mult(s: &Sprint) -> Fx {
    if !s.on {
        return ONE;
    }
    ONE + fx::mul(TOP - ONE, fx::mul(share(s.ticks), s.slope))
}

/// The speed a sprint asks for, a tick: the run speed with `n` speedster
/// pieces (the bare run times 1 + 0.2 n, CW-8) times `mult`, times the
/// road's share (`want_len` over `asked`, the speed the keys asked for
/// before the road), held under `CAP` times the bare run. At `mult` =
/// `ONE` with Shift held this is `want_len` exactly.
pub fn speed(n: u8, mult: Fx, want_len: Fx, asked: Fx) -> Fx {
    let bare = motion::per_tick(motion::RUN, 0);
    let sprint = fx::mul(motion::per_tick(motion::RUN, n), mult);
    let road = (want_len as i64 * sprint as i64 / asked.max(1) as i64) as Fx;
    road.min(fx::mul(bare, CAP))
}

/// The most the heading turns a tick at `speed` (a tick): `TURN_AT_RUN` at
/// the bare run speed or slower, `TURN_AT_TOP` at `TOP` times it or faster,
/// in a straight line between.
pub fn turn_rate(speed: Fx) -> i32 {
    let bare = motion::per_tick(motion::RUN, 0);
    let q = fx::div(speed, bare.max(1));
    let past = fx::div((q - ONE).max(0), TOP - ONE).clamp(0, ONE);
    TURN_AT_RUN - fx::mul(TURN_AT_RUN - TURN_AT_TOP, past)
}

/// The slope's share of the build for a grade: all of it on the flat or
/// downhill, none at `STEEP` uphill or steeper.
pub fn slope_share(grade: Fx) -> Fx {
    if grade <= 0 {
        return ONE;
    }
    (ONE - fx::div(grade, STEEP)).clamp(0, ONE)
}

/// One tick of a sprint: Shift's taps, the end, the start, the heading, the
/// build and the view. `shift` is the run key held this tick.
pub fn think(s: &mut Sprint, shift: bool, k: &Facts) {
    // a tap: a short press after a quiet spell, counted once Shift has
    // stayed up a little after it
    let mut tap = false;
    if shift {
        if s.down == 0 {
            s.quiet = s.up >= QUIET;
        }
        s.down = s.down.saturating_add(1);
        s.up = 0;
        s.tapping = false;
    } else {
        if s.down > 0 {
            s.tapping = s.quiet && s.down <= TAP_MOST;
        }
        s.down = 0;
        s.up = s.up.saturating_add(1).min(QUIET);
        if s.tapping && s.up >= SETTLE {
            s.tapping = false;
            tap = true;
        }
    }
    let hit = k.stagger || k.hp < s.hp;
    s.hp = k.hp;
    if s.on {
        let end = if tap {
            End::Tap
        } else if !k.moving {
            End::Stop
        } else if k.drawn || k.engaged {
            End::Drawn
        } else if hit {
            End::Hit
        } else if k.dodge {
            End::Dodge
        } else if k.down {
            End::Down
        } else if fx::wrap(k.goal - s.heading).abs() > HARD_TURN {
            End::Turn
        } else {
            End::None
        };
        if end != End::None {
            s.on = false;
            s.ticks = 0;
            s.ended = end;
            s.skid = k.speed > k.run;
        } else {
            let rate = turn_rate(k.speed);
            s.heading = fx::wrap(s.heading + fx::wrap(k.goal - s.heading).clamp(-rate, rate));
            if !k.opening {
                s.ticks = s.ticks.saturating_add(1);
            }
            s.slope = fx::approach(s.slope, slope_share(k.grade), SLOPE_EASE);
        }
    } else if tap && k.moving && !k.drawn && !k.engaged && !hit && !k.down {
        s.on = true;
        s.ticks = 0;
        s.heading = k.goal;
        s.slope = slope_share(k.grade);
        s.ended = End::None;
    }
    s.prev_view = s.view;
    let goal = if s.on { fx::mul(share(s.ticks), s.slope) } else { 0 };
    s.view = fx::approach(s.view, goal, VIEW_EASE);
}

/// Step 3 of the tick, before the feet (`motion::step`): every fighter's
/// sprint from its own input record.
pub fn step(w: &mut World, input: &Input, foes: &[Input]) {
    let opening = w.story.opening();
    let n_player = motion::pieces(w);
    for id in 0..player::count(w) {
        let inp = if id == 0 { *input } else { foes[id - 1] };
        let n = if id == 0 { n_player } else { 0 };
        let way = motion::wanted(&inp, player::look_yaw(w, id), false, 0);
        let (b, f) = (player::body(w, id), player::fighter(w, id));
        let s = &f.gait.sprint;
        let ahead = b.pos.add(facing(s.heading, 0));
        let k = Facts {
            moving: way != V3::ZERO,
            goal: if way != V3::ZERO { yaw_of(way) } else { s.heading },
            drawn: f.drawn() || f.pressed(&inp, input::DRAW),
            engaged: f.engaged.is_some(),
            stagger: f.gait.stagger > 0,
            hp: f.health.hp,
            dodge: f.pressed(&inp, input::DODGE),
            down: f.health.down() || crate::wounds::legs_lost(f.health.limbs.lost) > 0,
            speed: b.vel.len_xz(),
            run: motion::per_tick(motion::RUN, n),
            grade: w.ground(ahead.x, ahead.z) - w.ground(b.pos.x, b.pos.z),
            opening,
        };
        think(
            &mut player::parts_mut(w, id).1.gait.sprint,
            inp.key(input::RUN),
            &k,
        );
    }
}

/// In `motion::step_body`, once the keys, the road, a stagger and a dodge
/// have set the velocity a body wants: sprinting, it runs along the
/// sprint's heading at the sprint's speed.
pub fn want(f: &Fighter, input: &Input, want: V3, n: u8) -> V3 {
    let s = &f.gait.sprint;
    if !s.on || want == V3::ZERO {
        return want;
    }
    let asked = motion::per_tick(
        if input.key(input::RUN) {
            motion::RUN
        } else {
            motion::WALK
        },
        n,
    );
    facing(s.heading, 0).scale(speed(n, mult(s), want.len(), asked))
}

/// In `motion::step_body`, once the feet have moved the velocity from
/// `before` to `after`: sprinting or skidding, it loses at most `SKID` of
/// its speed a tick. The skid ends at the run speed.
pub fn skid(f: &mut Fighter, before: V3, after: V3, n: u8) -> V3 {
    let s = &mut f.gait.sprint;
    if !s.on && !s.skid {
        return after;
    }
    let (was, now) = (before.len_xz(), after.len_xz());
    let out = if was - now > SKID {
        let way = if now > 0 { after } else { before };
        let flat = v3(way.x, 0, way.z).norm().scale(was - SKID);
        v3(flat.x, after.y, flat.z)
    } else {
        after
    };
    if !s.on && out.len_xz() <= motion::per_tick(motion::RUN, n) {
        s.skid = false;
    }
    out
}

/// The camera's pull back and the view's widening, `alpha` of the way from
/// the last tick to this one (`World::camera`).
pub fn view(w: &World, alpha: Fx) -> (Fx, i32) {
    let s = &w.me.gait.sprint;
    let v = fx::lerp(s.prev_view, s.view, alpha);
    // eased in and out again on the way, so the pull starts and stops softly
    let e = fx::mul(fx::mul(v, v), 3 * ONE - 2 * v);
    (fx::mul(PULL, e), fx::mul(WIDEN, e))
}

/// The HUD's sprint bar while the player sprints: the seconds until the
/// build starts, then how many times the bare run speed the sprint asks
/// for, the speedster's pieces included, to the nearest whole number (T18,
/// R9.424); filled by the same multiple's share of the way from the bare
/// run to `CAP`.
pub fn bar(s: &Strings, w: &World) -> Option<crate::ui::Bar> {
    let sp = &w.me.gait.sprint;
    if !sp.on {
        return None;
    }
    let n = motion::pieces(w);
    let (bare, run) = (motion::per_tick(motion::RUN, 0), motion::per_tick(motion::RUN, n));
    let times = fx::div(speed(n, mult(sp), run, run), bare.max(1));
    let text = if sp.ticks < BUILD_AFTER {
        let left = (BUILD_AFTER - sp.ticks).div_ceil(TICKS_PER_SECOND as u32);
        crate::api::fill(&s.get("sprint_builds"), &[("n", left.to_string())])
    } else {
        let whole = (times + ONE / 2) / ONE;
        crate::api::fill(&s.get("sprint_times"), &[("n", whole.to_string())])
    };
    let past = (times - ONE).max(0) as i64;
    let span = (CAP - ONE) as i64;
    Some(crate::ui::Bar {
        id: "sprint".into(),
        label: s.get("sprint"),
        text,
        pct: ((past * 100 + span / 2) / span).min(100) as i32,
    })
}

/// The numbers the glossary's Sprint entry names.
pub fn values() -> Vec<(&'static str, String)> {
    let secs = |t: u32| (t / TICKS_PER_SECOND as u32).to_string();
    vec![
        ("sprint.after", secs(BUILD_AFTER)),
        ("sprint.ramp", secs(RAMP)),
        ("sprint.top", fx::decimal(TOP, 1)),
    ]
}

/// `act("sprint.<name>", value)`: `set` puts the player `value` ticks into
/// a sprint along the camera's heading (for a test or a screenshot), `off`
/// ends it.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    match name {
        "set" => {
            let t: u32 = value
                .trim()
                .parse()
                .map_err(|_| format!("A sprint is set by a whole number of ticks, not \"{value}\"."))?;
            let yaw = w.aim.yaw;
            let hp = w.me.health.hp;
            let s = &mut w.me.gait.sprint;
            s.on = true;
            s.ticks = t;
            s.heading = yaw;
            s.slope = ONE;
            s.hp = hp;
            s.ended = End::None;
            Ok(())
        }
        "off" => {
            w.me.gait.sprint.on = false;
            Ok(())
        }
        _ => Err(format!("Unknown action \"sprint.{name}\".")),
    }
}

#[cfg(test)]
mod tests;
