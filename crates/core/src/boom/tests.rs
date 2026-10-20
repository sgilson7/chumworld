use super::*;
use crate::camera::BACK;
use crate::input::{Input, Mouse, FORWARD};
use crate::player::Body;

fn block() -> Solid {
    // 6 m by 5 m, 4 m high, square to the axes at the origin
    Solid::Block {
        wall: Wall {
            x: 0,
            z: 0,
            hx: fx::int(3),
            hz: fx::int(5) / 2,
            yaw: 0,
        },
        y0: 0,
        y1: fx::int(4),
    }
}

/// By hand: 2 m east of the east face is 2 m out; 1 m past the east face
/// and 1 m over the top is √2 out (1.414); in the middle 2 m up, 2 m in
/// (the top and the bottom are 2 m off, the faces 3 and 2.5).
#[test]
fn a_block_s_gap_is_its_distance_out_or_in() {
    let b = block();
    assert_eq!(b.gap(v3(fx::int(5), ONE * 2, 0)), fx::int(2));
    let corner = b.gap(v3(fx::int(4), fx::int(5), 0));
    assert!((corner - 92_682).abs() <= 2, "{corner}");
    assert_eq!(b.gap(v3(0, fx::int(2), 0)), -fx::int(2));
    // a 0.3 m trunk 1 m off: 0.7 m; a crown 2 m by 1 m by 2 m, 2 m over
    // its middle: one radius (1 m) out
    let post = Solid::Post {
        at: V3::ZERO,
        r: ONE * 3 / 10,
        y1: fx::int(5),
    };
    assert_eq!(post.gap(v3(ONE, ONE, 0)), ONE * 7 / 10 + 1);
    let ball = Solid::Ball {
        c: V3::ZERO,
        r: v3(fx::int(2), ONE, fx::int(2)),
    };
    assert_eq!(ball.gap(v3(0, fx::int(2), 0)), ONE);
}

/// By hand: from 1.5 m up at the origin to 10 m south, a wall whose north
/// face is 5.5 m south: kept 0.5 m clear, the eye may go 5 m, half way.
/// From inside a solid, that solid is no wall.
#[test]
fn the_boom_stops_short_of_a_wall_by_its_clearance() {
    let wall = Solid::Block {
        wall: Wall {
            x: 0,
            z: fx::int(6),
            hx: fx::int(4),
            hz: ONE / 2,
            yaw: 0,
        },
        y0: 0,
        y1: fx::int(4),
    };
    let from = v3(0, ONE * 3 / 2, 0);
    let to = v3(0, ONE * 3 / 2, fx::int(10));
    let t = reach(&[wall], from, to, ONE / 2);
    assert!((t - ONE / 2).abs() <= 8, "{t}");
    assert_eq!(
        reach(&[wall], from, v3(0, ONE * 3 / 2, fx::int(4)), ONE / 2),
        ONE,
        "short of it"
    );
    let inside = Solid::Ball {
        c: from,
        r: v3(ONE, ONE, ONE),
    };
    assert_eq!(reach(&[inside], from, to, ONE / 2), ONE);
}

/// Walking round every house in the village, a metre from its walls, the
/// view turned every way and pitched up, level and down: the eye is never
/// inside a solid, and is always at least `HARD` (0.45 m) clear of every
/// building, less a centimetre for the march's rounding.
#[test]
fn walking_round_every_house_the_eye_is_never_inside_a_solid() {
    let mut w = World::new(1);
    let houses = places::prop_spots(&w, Prop::House);
    assert!(houses.len() >= 9);
    let mut seen = 0;
    for h in &houses {
        let (hx, hz) = (fx::int(3) + ONE, fx::int(5) / 2 + ONE);
        let wall = Wall {
            x: h.at.x,
            z: h.at.z,
            hx,
            hz,
            yaw: h.yaw,
        };
        for k in 0..16 {
            // round the grown rectangle, corners and middles
            let (lx, lz) = match k % 8 {
                0 => (-hx, -hz),
                1 => (0, -hz),
                2 => (hx, -hz),
                3 => (hx, 0),
                4 => (hx, hz),
                5 => (0, hz),
                6 => (-hx, hz),
                _ => (-hx, 0),
            };
            let (dx, dz) = wall.world_dir(lx, lz);
            let mut p = v3(h.at.x + dx, 0, h.at.z + dz);
            p.y = solid::surface(&w, p.x, p.z);
            for turn in 0..8 {
                let yaw = fx::wrap(h.yaw + turn * fx::TURN / 8 + k * fx::deg(5));
                for pitch in [-fx::deg(40), -fx::deg(12), fx::deg(25)] {
                    w.player = Body::new(p, yaw);
                    w.aim.yaw = yaw;
                    w.aim.pitch = pitch;
                    crate::boom::step(&mut w);
                    for alpha in [0, ONE / 2, ONE] {
                        let eye = w.camera(alpha, Mouse::default(), BACK).eye;
                        for s in solids(&w, head(p)) {
                            let g = s.gap(eye);
                            assert!(g >= 0, "inside a solid at {eye:?} by {p:?}: {s:?}");
                            if matches!(s, Solid::Block { .. }) && s.gap(head(p)) > HARD * 2 {
                                assert!(g >= HARD - ONE / 100, "{g} from {s:?}");
                            }
                        }
                        seen += 1;
                    }
                }
            }
        }
    }
    assert!(seen > 3000);
}

/// No pop: backing up to a house's long wall, facing away from it, then
/// stepping sideways along it and past its corner, the eye moves no more
/// than the walk and the boom's 0.25 m a tick, together under 0.35 m a tick
/// (under 0.14 m a quarter tick here), from frame to frame; it comes in
/// by the wall and goes out again past the corner.
#[test]
fn passing_a_corner_the_eye_comes_in_and_out_smoothly() {
    let mut w = crate::story::opening(1);
    crate::story::step_aside(&mut w);
    let h = places::prop_spots(&w, Prop::House)[2];
    let frame = Wall {
        x: 0,
        z: 0,
        hx: 0,
        hz: 0,
        yaw: h.yaw,
    };
    // 6 m out from the middle of its long (local −z) side
    let (sx, sz) = frame.world_dir(0, -(fx::int(5) / 2 + fx::int(6)));
    let mut p = v3(h.at.x + sx, 0, h.at.z + sz);
    p.y = w.ground(p.x, p.z);
    let away = crate::geom::yaw_of(v3(p.x - h.at.x, 0, p.z - h.at.z));
    w.player = Body::new(p, away);
    w.me = crate::player::Fighter::new(
        crate::pose::Kind::Player,
        crate::pose::Held::ShortSword,
        0,
        p,
        away,
        false,
    );
    w.aim.yaw = away;
    let mut last: Option<V3> = None;
    let mut most = 0;
    let (mut least, mut after) = (ONE, 0);
    for t in 0..420 {
        let keys = if t < 150 {
            crate::input::BACK
        } else {
            crate::input::RIGHT
        };
        w.step(&Input {
            keys,
            ..Input::default()
        });
        for k in 0..4 {
            let eye = w.camera(ONE * k / 4, Mouse::default(), BACK).eye;
            if let Some(l) = last {
                most = most.max(eye.sub(l).len());
            }
            last = Some(eye);
        }
        least = least.min(w.boom.len);
        after = w.boom.len;
    }
    assert!(most < ONE * 14 / 100, "the eye jumped {most}");
    assert!(least < ONE / 2, "the boom came in by the wall: {least}");
    assert_eq!(after, ONE, "and out again past the corner");
}

/// The roof's hand-over: walking in at the door, the cut counts up a tick at
/// a time, 0 to 24, missing none and never going back; walking out and
/// away, down again the same way. Drawn between ticks, the share only grows
/// going in.
#[test]
fn the_house_fades_out_and_in_a_tick_at_a_time() {
    let mut w = crate::story::opening(2);
    crate::story::step_aside(&mut w);
    let h = w.solid.home.unwrap();
    // 6 m out from the door (its local −z), facing in
    let (dx, dz) = h.wall.world_dir(0, -(h.wall.hz + fx::int(6)));
    let mut p = v3(h.wall.x + dx, 0, h.wall.z + dz);
    p.y = solid::surface(&w, p.x, p.z);
    let inward = crate::geom::yaw_of(v3(h.wall.x - p.x, 0, h.wall.z - p.z));
    w.player = Body::new(p, inward);
    w.me = crate::player::Fighter::new(
        crate::pose::Kind::Player,
        crate::pose::Held::ShortSword,
        0,
        p,
        inward,
        false,
    );
    w.aim.yaw = inward;
    w.aim.pitch = -fx::deg(12);
    // the opening had it cut (in bed): it comes back whole, a tick at a time
    for _ in 0..CUT_TICKS + 2 {
        w.step(&Input::default());
    }
    assert_eq!(w.boom.cut, 0, "whole from outside");
    let mut seen = vec![w.boom.cut];
    let mut shares = Vec::new();
    for _ in 0..200 {
        w.step(&Input {
            keys: FORWARD,
            ..Input::default()
        });
        if *seen.last().unwrap() != w.boom.cut {
            seen.push(w.boom.cut);
        }
        for k in 0..4 {
            shares.push(cut_share(&w, ONE * k / 4));
        }
    }
    let want: Vec<u8> = (0..=CUT_TICKS).collect();
    assert_eq!(seen, want, "in: up one a tick to cut away");
    assert!(
        shares.windows(2).all(|s| s[1] >= s[0]),
        "the share never goes back"
    );
    // turn and walk out, far enough for the eye to leave the room too
    w.aim.yaw = fx::wrap(inward + fx::HALF_TURN);
    let mut out = vec![w.boom.cut];
    for _ in 0..400 {
        w.step(&Input {
            keys: FORWARD,
            ..Input::default()
        });
        if *out.last().unwrap() != w.boom.cut {
            out.push(w.boom.cut);
        }
    }
    let down: Vec<u8> = (0..=CUT_TICKS).rev().collect();
    assert_eq!(out, down, "out: down one a tick to whole");
}

/// The hand-over draws every pixel of the walls once: the whole house's
/// fade and the cut walls' fade are a `hand_over` pair at every step.
#[test]
fn the_house_s_two_drawings_share_every_pixel_once() {
    for k in 0..=CUT_TICKS as i32 {
        let t = k * ONE / CUT_TICKS as i32;
        let (cut, whole) = crate::look::hand_over(t);
        assert_eq!(cut - whole, ONE, "step {k}");
    }
}

/// The game opens in bed with the house already cut away: no hand-over on
/// the title.
#[test]
fn the_opening_shows_the_house_cut_away_from_the_first_frame() {
    let w = crate::story::opening(3);
    assert_eq!((w.boom.cut, w.boom.prev_cut), (CUT_TICKS, CUT_TICKS));
    assert_eq!(cut_share(&w, ONE / 2), ONE);
}

/// Backed up to a wall, facing away from it, the eye rises (by 0.3 m and
/// more over the head) and stays over the right shoulder: right of the
/// head along the view's right, looking down past it.
#[test]
fn backed_against_a_wall_the_eye_rises_over_the_right_shoulder() {
    let mut w = crate::story::opening(1);
    crate::story::step_aside(&mut w);
    // 1.5 m out from the north wall of the house at (−8, 46) from the
    // village centre (turned 180°), facing north, away from it
    let mut p = v3(fx::int(1992), 0, fx::int(2842));
    p.y = w.ground(p.x, p.z);
    w.player = Body::new(p, 0);
    w.aim.yaw = 0;
    w.aim.pitch = -fx::deg(8);
    for _ in 0..60 {
        crate::boom::step(&mut w);
    }
    let c = w.camera(0, Mouse::default(), BACK);
    let h = head(p);
    assert!(c.eye.y > h.y + ONE * 3 / 10, "risen: {}", c.eye.y - h.y);
    let right = crate::geom::right_of(0);
    assert!(c.eye.sub(h).dot(right) > ONE / 2, "over the right shoulder");
    assert!(c.fwd.y < -ONE / 4, "looking down past the head: {}", c.fwd.y);
}
