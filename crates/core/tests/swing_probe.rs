//! A probe, not a check: what swings come out of scripted input (T10).
//! The player draws, then holds the left button and moves the mouse one
//! way for a while, at three speeds; the probe prints the plane the
//! blade turned in (degrees from horizontal), the arc it swept, the tip's
//! peak speed and how long the swing took. Numbers for MEASUREMENTS.md.
//!
//! ```bash
//! cargo test --release -p tool_core --test swing_probe -- --ignored --nocapture
//! ```

use tool_core::arm;
use tool_core::fx::ONE;
use tool_core::input::{self, Input};
use tool_core::player::Fighter;
use tool_core::pose::{Held, Kind};
use tool_core::world::World;

fn ready(held: Held) -> World {
    let mut w = World::new(3);
    if held != Held::ShortSword {
        let (p, y) = (w.player.pos, w.player.facing);
        w.me = Fighter::new(Kind::Player, held, 0, p, y, false);
    }
    w.step(&Input {
        keys: input::DRAW,
        ..Default::default()
    });
    for _ in 0..90 {
        w.step(&Input::default());
    }
    w
}

fn f(v: i32) -> f64 {
    v as f64 / ONE as f64
}

type P = [f64; 3];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: P) -> P {
    let l = dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// Plane tilt from horizontal, arc swept (both degrees), peak tip speed
/// (m/s), ticks from the first move to the end of the sweep.
fn measure(held: Held, dx: i32, dy: i32, wheel: i32, ticks: usize) -> (f64, f64, f64, usize) {
    let mut w = ready(held);
    let mut dirs = Vec::new();
    let mut speeds = Vec::new();
    let mut striking = Vec::new();
    let mut done = None;
    let mut ahead = Vec::new();
    for t in 0..90 {
        let moving = t < ticks;
        w.step(&Input {
            buttons: if t < ticks + 30 { input::SWING } else { 0 },
            dx: if moving { dx } else { 0 },
            dy: if moving { dy } else { 0 },
            wheel: if moving { wheel } else { 0 },
            ..Default::default()
        });
        let a = &w.me.arm;
        use tool_core::swing::Phase;
        let phase = a.swing.cut.map(|c| c.phase);
        striking.push(matches!(phase, Some(Phase::Strike) | Some(Phase::Follow)));
        if done.is_none() && t > 0 && phase.is_none() && striking.iter().any(|&s| s) {
            done = Some(t);
        }
        let tip = a.pts[arm::TIP];
        let fwd = tool_core::geom::facing(w.player.facing, 0);
        ahead.push(f(tip.p.sub(w.player.pos).dot(fwd)));
        let g = a.grip_point();
        let p = |v: tool_core::geom::V3| [f(v.x), f(v.y), f(v.z)];
        dirs.push(norm(sub(p(tip.p), p(g))));
        let v = tip.p.sub(tip.q);
        speeds.push(dot(p(v), p(v)).sqrt() * 60.0);
    }
    let peak = speeds.iter().cloned().fold(0.0, f64::max);
    // the sweep: from the first tick the tip passes 1.5 m/s to the last
    // (after T10: the strike and the follow-through, the wind-up left out)
    let (first, last) = if striking.iter().any(|&s| s) {
        (
            striking.iter().position(|&s| s).unwrap(),
            striking.iter().rposition(|&s| s).unwrap(),
        )
    } else {
        let first = speeds.iter().position(|&s| s > 1.5).unwrap_or(0);
        (first, speeds.iter().rposition(|&s| s > 1.5).unwrap_or(first))
    };
    let peak = speeds[first..=last]
        .iter()
        .cloned()
        .fold(0.0, f64::max)
        .max(if done.is_none() { peak } else { 0.0 });
    let mut axis = [0.0; 3];
    for k in first..last.max(first + 1).min(dirs.len() - 1) {
        let c = cross(dirs[k], dirs[k + 1]);
        axis = [axis[0] + c[0], axis[1] + c[1], axis[2] + c[2]];
    }
    let n = norm(axis);
    let tilt = n[1].abs().clamp(0.0, 1.0).acos().to_degrees();
    let mut arc = 0.0;
    for k in first..last.min(dirs.len() - 1) {
        let c = cross(dirs[k], dirs[k + 1]);
        arc += dot(c, n).atan2(dot(dirs[k], dirs[k + 1])).to_degrees();
    }
    if wheel != 0 {
        // a thrust: how far the tip went forward, in centimetres, for the arc
        let reach = ahead[first..=last].iter().cloned().fold(f64::MIN, f64::max) - ahead[first];
        return (tilt, reach * 100.0, peak, done.unwrap_or(last + 1));
    }
    (tilt, arc.abs(), peak, done.unwrap_or(last + 1))
}

#[test]
#[ignore]
fn probe_swings_from_scripted_mouse() {
    println!("| Weapon | Mouse | Speed | Plane from horizontal | Arc | Peak tip | Ticks to the end |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    // 600 px of travel in 30, 15 or 6 ticks (1 200, 2 400, 6 000 px/s)
    let speeds = [("slow", 30), ("medium", 15), ("fast", 6)];
    let ways: [(&str, i32, i32, i32); 5] = [
        ("sideways (left)", -1, 0, 0),
        ("up", 0, -1, 0),
        ("down", 0, 1, 0),
        ("diagonal (down-left)", -1, 1, 0),
        ("20° off sideways", -94, 34, 0),
    ];
    for held in [Held::ShortSword, Held::Sword, Held::Cleaver] {
        for (way, x, y, _) in ways {
            for (speed, n) in speeds {
                if held != Held::ShortSword && speed != "medium" {
                    continue;
                }
                let per = 60_000 / n as i32;
                let len = ((x * x + y * y) as f64).sqrt();
                let dx = (x as f64 / len * per as f64) as i32;
                let dy = (y as f64 / len * per as f64) as i32;
                let (tilt, arc, peak, ticks) = measure(held, dx, dy, 0, n);
                println!("| {held:?} | {way} | {speed} | {tilt:.0}° | {arc:.0}° | {peak:.1} m/s | {ticks} |");
            }
        }
        let (_, arc, peak, ticks) = measure(held, 0, 0, -10_000, 6);
        println!("| {held:?} | wheel away (thrust) | one notch | (line) | {arc:.0} cm ahead | {peak:.1} m/s | {ticks} |");
    }
}

#[test]
#[ignore]
fn probe_trace() {
    let which: Vec<String> = std::env::var("TRACE")
        .unwrap_or("ss,-1,0,0".into())
        .split(',')
        .map(|s| s.to_string())
        .collect();
    let held = match which[0].as_str() {
        "cl" => Held::Cleaver,
        "sw" => Held::Sword,
        _ => Held::ShortSword,
    };
    let (dx, dy, wh): (i32, i32, i32) = (
        which[1].parse().unwrap(),
        which[2].parse().unwrap(),
        which[3].parse().unwrap(),
    );
    let mut w = ready(held);
    for t in 0..60 {
        w.step(&Input {
            buttons: input::SWING,
            dx: if t < 3 { dx * 3000 } else { 0 },
            dy: if t < 3 { dy * 3000 } else { 0 },
            wheel: if t < 3 { wh } else { 0 },
            ..Default::default()
        });
        let a = &w.me.arm;
        let tip = a.pts[arm::TIP];
        let v = tip.p.sub(tip.q);
        let sp = (f(v.x).powi(2) + f(v.y).powi(2) + f(v.z).powi(2)).sqrt() * 60.0;
        let d = tip.p.sub(a.grip_point());
        let el = (f(d.y) / (f(d.x).powi(2) + f(d.z).powi(2)).sqrt())
            .atan()
            .to_degrees();
        let c = a.swing.cut;
        let hand = a.grip_point().sub(w.player.pos);
        let deg = |x: i32| x as f64 * 360.0 / 65536.0;
        let now = c.map(|c| {
            deg(tool_core::swing::progress(
                &c,
                w.player.facing,
                a.frame.shoulder[1],
                a.reach(),
                a.grip_point(),
            ))
        });
        let bl = c.map(|c| {
            let (pf, ps) = tool_core::swing::plane(w.player.facing, c.travel);
            let dn = d.norm();
            deg(tool_core::fx::wrap(tool_core::fx::atan2(dn.dot(ps), dn.dot(pf)))) as i32
        });
        println!(
            "{t:2} {:?} now {:?} aim {:?} blade {:?} tip {sp:5.1} m/s elev {el:5.0} hand ({:.2},{:.2},{:.2})",
            c.map(|c| c.phase),
            now.map(|x| x as i32),
            c.map(|c| deg(c.aim) as i32),
            bl,
            f(hand.x),
            f(hand.y),
            f(hand.z)
        );
    }
}
