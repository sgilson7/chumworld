//! The sprint's tests (T14, T18). Expected values are worked by hand from
//! the constants: the bare run is 7 m/s, 7 × 65 536 / 60 = 7 645 units a
//! tick (`motion::RUN`, pinned in `motion`'s tests), so the tests that name
//! a run speed write it as `per_tick(RUN, 0)` and give the number by hand.

use super::*;
use crate::geom::V3;
use crate::input::{BACK, DODGE, DRAW, FORWARD, RUN};
use crate::motion::{per_tick, step_body, RUN as RUN_SPEED, WALK};
use crate::player::{Body, Fighter};
use crate::pose::{Held, Kind};

/// One body on flat ground outside the opening, stepped as `sprint::step`
/// and `motion::step` step it.
struct Runner {
    b: Body,
    f: Fighter,
    n: u8,
}

impl Runner {
    fn new(n: u8) -> Runner {
        Runner {
            b: Body::new(V3::ZERO, 0),
            f: Fighter::new(Kind::Player, Held::ShortSword, 0, V3::ZERO, 0, false),
            n,
        }
    }

    fn tick(&mut self, input: Input, yaw: i32) {
        let (b, f) = (&mut self.b, &mut self.f);
        let way = motion::wanted(&input, yaw, false, 0);
        let k = Facts {
            moving: way != V3::ZERO,
            goal: if way != V3::ZERO {
                yaw_of(way)
            } else {
                f.gait.sprint.heading
            },
            drawn: f.drawn() || f.pressed(&input, DRAW),
            engaged: f.engaged.is_some(),
            stagger: f.gait.stagger > 0,
            hp: f.health.hp,
            dodge: f.pressed(&input, DODGE),
            down: f.health.down(),
            speed: b.vel.len_xz(),
            run: per_tick(RUN_SPEED, self.n),
            grade: 0,
            opening: false,
        };
        think(&mut f.gait.sprint, input.key(RUN), &k);
        step_body(b, f, &input, yaw, self.n, None, 0);
        f.last = input;
    }

    fn hold(&mut self, keys: u32, ticks: u32) {
        for _ in 0..ticks {
            self.tick(keys_of(keys), 0);
        }
    }

    /// Walk 20 ticks, tap Shift (down 3 ticks), and walk on: sprinting on
    /// the sixth tick after Shift lets go (`SETTLE`), its ticks at 0.
    fn start(&mut self) {
        self.hold(FORWARD, 20);
        self.hold(FORWARD | RUN, 3);
        self.hold(FORWARD, 5);
        assert!(!self.f.gait.sprint.on, "not before Shift has settled");
        self.hold(FORWARD, 1);
        assert!(self.f.gait.sprint.on, "the tap set a sprint");
    }

    fn sprint(&self) -> &Sprint {
        &self.f.gait.sprint
    }
}

fn keys_of(keys: u32) -> Input {
    Input {
        keys,
        ..Input::default()
    }
}

#[test]
fn a_tap_of_shift_while_moving_sets_a_sprint_and_another_ends_it() {
    let mut r = Runner::new(0);
    r.start();
    r.hold(FORWARD, 30);
    assert!(r.sprint().on, "it stays on with only W held");
    // with Shift let go it still runs: 5 461 a tick (5 m/s; 7 645 at 7 m/s)
    assert_eq!(r.b.vel.len_xz(), per_tick(RUN_SPEED, 0));
    r.hold(FORWARD | RUN, 2);
    r.hold(FORWARD, SETTLE as u32);
    assert!(!r.sprint().on);
    assert_eq!(r.sprint().ended, End::Tap);
    // holding Shift to run is not a tap: down 60 ticks, then let go
    let mut held = Runner::new(0);
    held.hold(FORWARD, 20);
    held.hold(FORWARD | RUN, 60);
    held.hold(FORWARD, 10);
    assert!(!held.sprint().on, "a held run is no tap");
    // a tap standing still sets nothing
    let mut still = Runner::new(0);
    still.hold(0, 20);
    still.hold(RUN, 3);
    still.hold(0, 8);
    assert!(!still.sprint().on, "a sprint needs a movement key");
    // a tap straight after Shift was let go (no quiet spell) sets nothing
    let mut quick = Runner::new(0);
    quick.hold(FORWARD | RUN, 40);
    quick.hold(FORWARD, 5);
    quick.hold(FORWARD | RUN, 3);
    quick.hold(FORWARD, 8);
    assert!(!quick.sprint().on, "Shift up 5 ticks is no quiet spell");
    // a short press and another soon after is running, not a tap
    let mut twice = Runner::new(0);
    twice.hold(FORWARD, 20);
    twice.hold(FORWARD | RUN, 3);
    twice.hold(FORWARD, 2);
    twice.hold(FORWARD | RUN, 3);
    twice.hold(FORWARD, 2);
    assert!(!twice.sprint().on, "Shift up 2 ticks between presses");
}

/// The orcs run with Shift held four ticks in five (`story::RUN_TICKS`):
/// down 4, up 1. The first press comes after a quiet walk and is short,
/// but Shift never stays up the 6 ticks after it that a tap needs.
#[test]
fn the_orcs_running_never_sprints() {
    let mut r = Runner::new(0);
    r.hold(FORWARD, 20);
    for t in 0..900u32 {
        let run = if t % 5 < 4 { RUN } else { 0 };
        r.tick(keys_of(FORWARD | run), 0);
        assert!(!r.sprint().on, "tick {t}");
    }
}

/// share (T18): 0 to 240 ticks; at 360, half the ramp, t = ½: 3/4 − 2/8 =
/// ½ exactly (32 768 × 32 768 / 65 536 = 16 384; × (196 608 − 65 536) / 65
/// 536 = 32 768); at 480 and after, ONE.
#[test]
fn four_seconds_of_sprint_go_at_the_run_speed_then_it_builds() {
    assert_eq!(BUILD_AFTER, 240, "4 s at 60 ticks a second");
    assert_eq!(share(0), 0);
    assert_eq!(share(240), 0);
    assert_eq!(share(360), ONE / 2);
    assert_eq!(share(480), ONE);
    assert_eq!(share(10_000), ONE);
    let mut r = Runner::new(0);
    r.start();
    // the start's tick is 0; 240 more is the 240th
    r.hold(FORWARD, 240);
    assert_eq!(r.sprint().ticks, 240);
    assert_eq!(mult(r.sprint()), ONE, "still the run speed at 4 s");
    assert_eq!(r.b.vel.len_xz(), 7_645);
    // halfway up the ramp: 1 + 4 × ½, three times the run (22 935 a tick,
    // 21 m/s), the feet a little behind (the want climbs at most 191 a
    // tick; a third of the gap closed each tick leaves twice that, 382,
    // behind)
    r.hold(FORWARD, 120);
    assert_eq!(mult(r.sprint()), 3 * ONE);
    let v = r.b.vel.len_xz();
    let want = 22_935;
    assert!(v < want && v > want - 450, "{v} against {want}");
}

/// mult at the top (T18): ONE + 4 × share 1 × slope 1 = 5 ONE; the speed 5
/// × 7 645 = 38 225 a tick, 35 m/s.
#[test]
fn at_the_top_a_sprint_runs_five_times_the_run_speed() {
    let mut r = Runner::new(0);
    r.start();
    r.hold(FORWARD, 700);
    assert_eq!(mult(r.sprint()), 5 * ONE);
    assert_eq!(speed(0, 5 * ONE, per_tick(WALK, 0), per_tick(WALK, 0)), 38_225);
    let v = r.b.vel.len_xz();
    // facing(0) × 38 225 is (0, 0, −38 225) to the unit
    assert!((v - 38_225).abs() <= 2, "{v}");
}

#[test]
fn a_stop_a_draw_a_blow_a_dodge_a_hold_on_or_a_hard_turn_ends_a_sprint() {
    let ended = |r: &Runner| (r.sprint().on, r.sprint().ended);
    // a stop: no movement key
    let mut r = Runner::new(0);
    r.start();
    r.hold(0, 1);
    assert_eq!(ended(&r), (false, End::Stop));
    // X pressed
    let mut r = Runner::new(0);
    r.start();
    r.hold(FORWARD | DRAW, 1);
    assert_eq!(ended(&r), (false, End::Drawn));
    // a blow: staggered, or health lost
    let mut r = Runner::new(0);
    r.start();
    r.f.gait.stagger = 10;
    r.hold(FORWARD, 1);
    assert_eq!(ended(&r), (false, End::Hit));
    let mut r = Runner::new(0);
    r.start();
    r.f.health.hp -= 5;
    r.hold(FORWARD, 1);
    assert_eq!(ended(&r), (false, End::Hit));
    // Space
    let mut r = Runner::new(0);
    r.start();
    r.hold(FORWARD | DODGE, 1);
    assert_eq!(ended(&r), (false, End::Dodge));
    // held on a foe
    let mut r = Runner::new(0);
    r.start();
    r.f.engaged = Some(1);
    r.hold(FORWARD, 1);
    assert_eq!(ended(&r), (false, End::Drawn));
    // S: the keys ask for the way back, 180° from the run
    let mut r = Runner::new(0);
    r.start();
    r.hold(BACK, 1);
    assert_eq!(ended(&r), (false, End::Turn));
    // the view turned 120° in one go ends it; 90° does not
    let mut r = Runner::new(0);
    r.start();
    r.tick(keys_of(FORWARD), deg(120));
    assert_eq!(ended(&r), (false, End::Turn));
    let mut r = Runner::new(0);
    r.start();
    r.tick(keys_of(FORWARD), deg(90));
    assert!(r.sprint().on, "a quarter turn steers");
}

/// The ramp's steepest tick (T18): the smoothstep's slope is 3/2 at its
/// middle, so the want climbs at most (5 − 1) × 7 645 × 3/2 / 240 = 191
/// units a tick (0.175 m/s a tick, 10.5 m/s²; R9.421); with a unit or two
/// of rounding the feet never gain more. The bound is that number, written
/// out, not worked from the constants.
#[test]
fn the_build_never_jumps_from_one_tick_to_the_next() {
    let mut r = Runner::new(0);
    r.start();
    r.hold(FORWARD, 30);
    let bound = 191 + 3;
    let mut last = r.b.vel.len_xz();
    let mut most = 0;
    for _ in 0..700 {
        r.hold(FORWARD, 1);
        let v = r.b.vel.len_xz();
        most = most.max((v - last).abs());
        last = v;
    }
    assert!(most <= bound, "a tick gained {most}, bound {bound}");
    assert!(most > bound / 2, "the ramp did climb: {most}");
}

/// T18: the speedster's pieces multiply the sprint, the bare run times the
/// sprint times 1 + 0.2 a piece, under the cap of ten times the bare run.
/// By hand: a piece count's share is (65 536 × (100 + 20 n) + 50) / 100
/// (65 536, 78 643, 91 750, 104 858, 117 965, 131 072), its run 7 m/s times
/// that over 60 ticks (7 645, 9 175, 10 704, 12 233, 13 762, 15 291 a tick),
/// and five of those at the top: 38 225, 45 875, 53 520, 61 165, 68 810 and
/// 76 455, which the cap, 10 × 7 645 = 76 450 (70 m/s), holds to 76 450.
#[test]
fn speedster_pieces_multiply_the_sprint_and_the_cap_of_ten_holds() {
    let run = |n| per_tick(RUN_SPEED, n);
    let runs = [7_645, 9_175, 10_704, 12_233, 13_762, 15_291];
    let tops = [38_225, 45_875, 53_520, 61_165, 68_810, 76_450];
    for n in 0..=5u8 {
        let r = runs[n as usize];
        // at the run speed a sprint is the run, pieces and all (CW-8's pins)
        assert_eq!(speed(n, ONE, r, r), r, "{n} pieces at a run");
        assert_eq!(speed(n, 5 * ONE, r, r), tops[n as usize], "{n} pieces at the top");
    }
    // part way up the build the pieces multiply too: five pieces at twice
    // the run, 2 × 15 291 = 30 582 (where adding them gave 2 × 7 645 + 7 646)
    assert_eq!(speed(5, 2 * ONE, 15_291, 15_291), 30_582);
    // and on a road 18% faster than five pieces' run, still the cap
    let road = 15_291 + 15_291 * 18 / 100;
    assert_eq!(speed(5, 5 * ONE, road, run(5)), 76_450);
    // a road with no pieces carries through: 18% more asked (7 645 + 1 376
    // = 9 021) over 7 645, times five runs: five times the road's run,
    // 45 105 (41.3 m/s), under the cap
    assert_eq!(speed(0, 5 * ONE, 9_021, 7_645), 45_105);
    // without Shift held a sprint still runs: a walk asked, a run given
    assert_eq!(speed(0, ONE, per_tick(WALK, 0), per_tick(WALK, 0)), 7_645);
}

/// 10° is 1 820 units, 6° 1 092 and 2° 364 (65 536 × d / 360, truncated).
/// T18: the rate falls over the run to the top, five runs, so twice the
/// run is a quarter of the way down (1 820 − 1 456 / 4 = 1 456), three
/// times it half way (1 092), five times the top (364), and ten times it,
/// the cap, no lower.
#[test]
fn the_turn_rate_falls_with_speed() {
    let bare = per_tick(RUN_SPEED, 0);
    assert_eq!(turn_rate(0), 1_820);
    assert_eq!(turn_rate(bare), 1_820);
    assert_eq!(turn_rate(2 * bare), 1_456);
    assert_eq!(turn_rate(3 * bare), 1_092);
    assert_eq!(turn_rate(5 * bare), 364);
    assert_eq!(turn_rate(10 * bare), 364);
    // a body: at the run, the keys a quarter turn left turn the sprint 10°
    // in a tick; at the top, 2° (its speed a unit or two under five runs,
    // so a unit more than 364)
    let mut r = Runner::new(0);
    r.start();
    r.hold(FORWARD, 20);
    let before = r.sprint().heading;
    r.tick(keys_of(FORWARD), deg(90));
    assert_eq!(fx::wrap(r.sprint().heading - before), 1_820);
    let mut r = Runner::new(0);
    r.start();
    r.hold(FORWARD, 700);
    let before = r.sprint().heading;
    r.tick(keys_of(FORWARD), deg(90));
    let turned = fx::wrap(r.sprint().heading - before);
    assert!((364..=366).contains(&turned), "{turned}");
    // and the body follows: after half a second of the quarter turn it has
    // turned about 2° × 30 = 60°, not 90° (a little more, as the body's
    // speed sags across the turn and the rate rises with it)
    for _ in 0..29 {
        r.tick(keys_of(FORWARD), deg(90));
    }
    let gone = fx::wrap(r.sprint().heading - before);
    assert!((deg(59)..=deg(62)).contains(&gone), "{gone}");
}

/// From the top (T18), 38 225 a tick, let go: the skid takes off 546 a
/// tick (0.5 m/s) until the run speed, (38 225 − 7 645) / 546 = 56.0, so 57
/// ticks to pass under it, covering 57 × 38 225 − 546 × (1 + … + 57) = 1 276
/// 287 units; the bare feet's grip then stops the last 7 103 in twice that,
/// 14 206: 19.7 m in all. A run never sprinted stops as before.
#[test]
fn a_stop_from_the_top_skids_and_a_plain_run_stops_as_before() {
    let mut r = Runner::new(0);
    r.start();
    r.hold(FORWARD, 700);
    let from = r.b.pos.z;
    let mut ticks = 0;
    let mut last = r.b.vel.len_xz();
    while r.b.vel.len_xz() > per_tick(RUN_SPEED, 0) {
        r.hold(0, 1);
        let v = r.b.vel.len_xz();
        assert!(last - v <= SKID + 1, "lost {} in a tick", last - v);
        last = v;
        ticks += 1;
    }
    let run = 7_645;
    let n = 57;
    assert!((n..=n + 1).contains(&ticks), "{ticks} ticks of skid, {n} by hand");
    assert!(!r.sprint().skid, "the skid ends at the run speed");
    r.hold(0, 120);
    let slid = from - r.b.pos.z;
    let want = 1_276_287 + 14_206;
    assert!((slid - want).abs() < want / 50, "slid {slid}, by hand {want}");
    assert_eq!(r.b.vel, V3::ZERO);
    // a run with Shift held, let go: twice the run, 15 290 units, as M1
    // pinned
    let mut plain = Runner::new(0);
    plain.hold(FORWARD | RUN, 120);
    let from = plain.b.pos.z;
    plain.hold(0, 120);
    assert!(((from - plain.b.pos.z) - 2 * run).abs() <= 80);
}

/// On a 20% grade up (13 107 over STEEP's 26 214) half the build is left;
/// at 40% and over, none; downhill and on the flat, all of it.
#[test]
fn uphill_the_build_gives_way() {
    assert_eq!(slope_share(0), ONE);
    assert_eq!(slope_share(-ONE), ONE);
    assert_eq!(slope_share(ONE * 20 / 100), ONE / 2);
    assert_eq!(slope_share(ONE / 2), 0);
    let mut s = Sprint {
        on: true,
        ticks: 600,
        slope: ONE,
        ..Sprint::default()
    };
    let k = Facts {
        moving: true,
        grade: ONE / 2,
        hp: 0,
        ..Facts::default()
    };
    // eased: a thirtieth a tick, so 30 ticks from all of it to none
    for _ in 0..15 {
        think(&mut s, false, &k);
    }
    assert_eq!(s.slope, ONE - 15 * SLOPE_EASE);
    for _ in 0..20 {
        think(&mut s, false, &k);
    }
    assert_eq!((s.slope, mult(&s)), (0, ONE), "a steep climb runs at the run");
}

/// While the opening runs the build does not count: a sprint in the raid
/// is a held run.
#[test]
fn the_opening_holds_a_sprint_at_the_run_speed() {
    let mut s = Sprint {
        on: true,
        slope: ONE,
        ..Sprint::default()
    };
    let raid = Facts {
        moving: true,
        opening: true,
        ..Facts::default()
    };
    for _ in 0..900 {
        think(&mut s, false, &raid);
    }
    assert_eq!((s.on, s.ticks, mult(&s)), (true, 0, ONE));
    let free = Facts {
        opening: false,
        ..raid
    };
    for _ in 0..BUILD_AFTER + 1 {
        think(&mut s, false, &free);
    }
    assert!(mult(&s) > ONE, "past the boundary it builds");
    // in the world: the story's raid holds it, a world without one does not
    let mut w = World::new(3);
    w.story.phase = crate::story::Phase::Raid;
    act(&mut w, "set", "0").unwrap();
    for _ in 0..60 {
        w.step(&keys_of(FORWARD));
    }
    assert_eq!((w.me.gait.sprint.on, w.me.gait.sprint.ticks), (true, 0));
    let mut w = World::new(3);
    act(&mut w, "set", "0").unwrap();
    for _ in 0..60 {
        w.step(&keys_of(FORWARD));
    }
    assert_eq!((w.me.gait.sprint.on, w.me.gait.sprint.ticks), (true, 60));
}

/// In the world, through `sprint::step`: a sprint holds while W is held and
/// ends when X is pressed or the keys let go.
#[test]
fn in_the_world_a_sprint_ends_at_the_draw_and_at_a_stop() {
    let mut w = World::new(3);
    act(&mut w, "set", "0").unwrap();
    for _ in 0..30 {
        w.step(&keys_of(FORWARD));
    }
    assert!(w.me.gait.sprint.on);
    w.step(&keys_of(FORWARD | DRAW));
    assert_eq!((w.me.gait.sprint.on, w.me.gait.sprint.ended), (false, End::Drawn));
    let mut w = World::new(3);
    act(&mut w, "set", "0").unwrap();
    for _ in 0..30 {
        w.step(&keys_of(FORWARD));
    }
    w.step(&Input::default());
    assert_eq!((w.me.gait.sprint.on, w.me.gait.sprint.ended), (false, End::Stop));
}

/// CW-4: a walk, a tap, a long sprint with turns and a stop give the same
/// world twice; the tap a tick later gives another.
#[test]
fn the_same_seed_and_inputs_give_the_same_world_with_a_sprint() {
    let inputs = |tap_at: usize| -> Vec<Input> {
        (0..900)
            .map(|t| Input {
                keys: if t >= 860 {
                    0
                } else if (tap_at..tap_at + 3).contains(&t) {
                    FORWARD | RUN
                } else {
                    FORWARD
                },
                dx: if t % 120 < 20 { 150 } else { -20 },
                ..Input::default()
            })
            .collect()
    };
    let run = |inputs: &[Input]| {
        let mut w = World::new(11);
        crate::places::act(&mut w, "go", "1500,2650,80,0").unwrap();
        let mut sprinted = 0;
        for i in inputs {
            w.step(i);
            sprinted += w.me.gait.sprint.on as u32;
        }
        (w, sprinted)
    };
    let (a, sa) = run(&inputs(20));
    let (b, sb) = run(&inputs(20));
    assert!(sa > 300, "it sprinted for {sa} ticks");
    assert_eq!(sa, sb);
    assert_eq!(a.hash(), b.hash());
    assert_eq!(a, b);
    let (c, _) = run(&inputs(21));
    assert_ne!(c.hash(), a.hash(), "a tap a tick later shows");
}

/// The camera: still, exactly the shoulder view; at the top of the build,
/// 0.8 m further back and 6° wider (60° is 10 922 units, 6° 1 092); half
/// way, the eased share is a half, 0.4 m (26 214) and 546.
#[test]
fn the_camera_pulls_back_and_widens_with_the_build_and_a_still_view_is_unchanged() {
    use crate::input::Mouse;
    let w = World::new(1);
    let cam = w.camera(0, Mouse::default(), crate::camera::BACK);
    let plain = crate::camera::view_from(w.player.feet(0), w.aim, crate::camera::BACK, |x, z| {
        w.ground(x, z)
    });
    assert_eq!(cam, plain, "a still view is the view it was");
    assert_eq!(cam.fov, 10_922);
    let mut top = w.clone();
    top.me.gait.sprint.view = ONE;
    top.me.gait.sprint.prev_view = ONE;
    assert_eq!(view(&top, 0), (ONE * 8 / 10, 1_092));
    let c = top.camera(0, Mouse::default(), crate::camera::BACK);
    assert_eq!(c.fov, 12_014);
    // the eye is the shoulder view's from 3.5 + 0.8 = 4.3 m back
    let back = crate::camera::view_from(
        w.player.feet(0),
        w.aim,
        crate::camera::BACK + ONE * 8 / 10,
        |x, z| w.ground(x, z),
    );
    assert_eq!(c.eye, back.eye);
    let mut half = w.clone();
    half.me.gait.sprint.view = ONE / 2;
    half.me.gait.sprint.prev_view = ONE / 2;
    assert_eq!(view(&half, 0), (26_214, 546));
    // eased in: from a sprint's start the view climbs a fortieth a tick at
    // most, and never moves before the build
    let mut s = Sprint {
        on: true,
        slope: ONE,
        ..Sprint::default()
    };
    let k = Facts {
        moving: true,
        ..Facts::default()
    };
    let mut last = 0;
    for t in 0..700 {
        think(&mut s, false, &k);
        if s.ticks <= BUILD_AFTER {
            assert_eq!(s.view, 0, "tick {t}");
        }
        assert!(s.view - last <= VIEW_EASE);
        last = s.view;
    }
    assert_eq!(s.view, ONE);
    // and out again as it ends: 40 ticks back to none, 1 639 a tick
    let stop = Facts::default();
    think(&mut s, false, &stop);
    assert_eq!(s.view, ONE - 1_639);
    for _ in 0..39 {
        think(&mut s, false, &stop);
    }
    assert_eq!((s.on, s.view), (false, 0));
}

/// The HUD bar (T18, R9.424): the seconds to the build (240 − 120 = 120
/// ticks, 2 s), then how many times the bare run (7 645 a tick) the sprint
/// asks for, pieces included, to the nearest whole; filled by that
/// multiple's share of the way from 1 to the cap, 10. By hand: no pieces,
/// half way (3 × 7 645 = 22 935): 3, 2/9 = 22%; at the top (38 225): 5,
/// 4/9 = 44%. Five pieces, half way: 3 × 15 291 = 45 873, 6.0003 times,
/// so 6, 5.0003/9 = 56%; at the top, the cap's 76 450: 10, 100%; and before
/// the build, their run (15 291, 2 times): 1/9 = 11%.
#[test]
fn the_hud_shows_the_build_as_a_bar_with_words() {
    let mut g = crate::game::tests::game();
    assert_eq!(crate::ui::hud(&g).sprint, None, "no bar until a sprint");
    g.world.act("sprint.set", "120").unwrap();
    let b = crate::ui::hud(&g).sprint.unwrap();
    assert_eq!(
        (b.label.as_str(), b.text.as_str(), b.pct),
        ("Sprint", "Builds in 2 s", 0)
    );
    g.world.act("sprint.set", "239").unwrap();
    assert_eq!(crate::ui::hud(&g).sprint.unwrap().text, "Builds in 1 s");
    let at = |g: &mut crate::game::Game, ticks: &str| {
        g.world.act("sprint.set", ticks).unwrap();
        let b = crate::ui::hud(g).sprint.unwrap();
        (b.text, b.pct)
    };
    assert_eq!(at(&mut g, "360"), ("3 × run speed".into(), 22));
    assert_eq!(at(&mut g, "480"), ("5 × run speed".into(), 44));
    g.world.act("motion.speedster", "5").unwrap();
    assert_eq!(at(&mut g, "120"), ("Builds in 2 s".into(), 11));
    assert_eq!(at(&mut g, "360"), ("6 × run speed".into(), 56));
    assert_eq!(at(&mut g, "480"), ("10 × run speed".into(), 100));
    g.world.act("sprint.off", "").unwrap();
    assert_eq!(crate::ui::hud(&g).sprint, None);
    assert_eq!(
        g.world.act("sprint.set", "soon"),
        Err("A sprint is set by a whole number of ticks, not \"soon\".".into())
    );
}

/// The controls panel names the key, and the glossary's Sprint entry is
/// filled by the core (T18): 240 ticks are 4 s, the build 4 s, the top 5.
#[test]
fn the_controls_name_the_tap_and_the_glossary_gives_the_numbers() {
    let g = crate::game::tests::game();
    let u: serde_json::Value = serde_json::from_str(&crate::api::ok(&crate::api::ui(&g))).unwrap();
    let rows = &u["ok"]["panels"][0]["rows"];
    assert_eq!(rows[1]["key"], "Shift");
    assert_eq!(rows[2]["key"], "Shift, tapped");
    assert_eq!(rows[2]["does"], "Sprint on or off, while moving");
    let ui = crate::api::ui(&g);
    let h = ui
        .hints
        .iter()
        .find(|h| h.term == "Sprint")
        .expect("Sprint is in the glossary");
    assert!(
        h.definition
            .contains("After 4 s it builds over 4 s to 5 times the run speed."),
        "{}",
        h.definition
    );
}

/// T18: at ten times the run, 1.17 m a tick, over the meadow's rolling
/// ground for 4 s (280 m east), a body neither leaves the ground at a crest
/// nor sinks into it at a hollow: every tick its feet are at the land's
/// height under them, and it has no speed up or down.
#[test]
fn at_ten_times_the_run_a_body_keeps_to_the_ground() {
    let mut w = World::new(3);
    crate::places::act(&mut w, "go", "1500,2650,80,0").unwrap();
    w.motion.forced_pieces = Some(5);
    w.player.vel = crate::geom::facing(w.aim.yaw, 0).scale(76_450);
    act(&mut w, "set", "480").unwrap();
    let from = w.player.pos;
    let (mut lowest, mut highest) = (i32::MAX, i32::MIN);
    for t in 0..240 {
        w.step(&keys_of(FORWARD));
        let p = w.player.pos;
        assert_eq!(p.y, w.ground(p.x, p.z), "tick {t}");
        assert_eq!(w.player.vel.y, 0, "tick {t}");
        lowest = lowest.min(p.y);
        highest = highest.max(p.y);
    }
    let gone = w.player.pos.sub(from).len_xz();
    assert!(gone > fx::int(270), "it ran {gone}");
    assert!(highest - lowest > ONE, "the ground rolled: {lowest} to {highest}");
}
