//! A probe, not a check (M7, task P3): pop-in near the player. For each
//! place (the village, the meadow, the camp's edge) and each walk (straight
//! on, and a whole turn on the spot over 8 s), it counts the frames in which
//! a lit shape near the player (within 25 m) appears or disappears where the
//! camera could see it, without handing over through the dither. Prints the
//! counts for the playthrough log and MEASUREMENTS.md; asserts nothing.
//!
//! ```bash
//! cargo test --release -p tool_core --test popin_probe -- --ignored --nocapture
//! ```
//!
//! What counts: a shape drawn in the near layer's `Solid` pass (or `Dither`
//! at full strength), keyed by its mesh and its place to 5 cm, in view in
//! both frames and within 25 m of the player's feet in both, present in one
//! frame and not the next. The story is held still (`story.still`) so that
//! only the walk moves things; the player's own figure (within 2 m) is left
//! out. A shape that moves counts as one going and one coming, so the
//! counts are an upper bound on what a player sees pop.

use std::collections::{BTreeMap, BTreeSet};

use tool_core::fx::{Fx, ONE};
use tool_core::game::Game;
use tool_core::geom::{v3, V3};
use tool_core::input::{FORWARD, RUN};
use tool_core::render::{Pass, DRAW_VALUES, DYNAMIC_BUFFER, INSTANCE_VALUES};
use tool_core::ticks::TICK_US;

const NEAR_M: i64 = 25;

fn game() -> Game {
    let mut g = Game::new(
        include_str!("../../../data/strings.json"),
        include_str!("../../../GLOSSARY.md"),
        include_str!("../../../data/look.json"),
        42,
    )
    .expect("the game starts");
    g.resize(1920, 1080, 100);
    g
}

fn frame(g: &mut Game, bits: u32, dx: i32) {
    let now = g.clock.last().unwrap_or(0) + TICK_US;
    g.advance(now, bits, dx, 0, 0);
}

type Key = (u32, i32, i32, i32);

/// One frame's lit shapes out to 40 m (key → position), with the camera and
/// the player's feet it was drawn for.
struct Seen {
    shapes: BTreeMap<Key, V3>,
    cam: tool_core::render::Camera,
    feet: V3,
    aspect: Fx,
}

impl Seen {
    /// Whether a point is near the player and in this frame's view.
    fn sees(&self, p: V3) -> bool {
        let flat = v3(p.x - self.feet.x, 0, p.z - self.feet.z).len() as i64;
        if flat > NEAR_M * ONE as i64 || flat < 2 * ONE as i64 {
            return false;
        }
        self.cam
            .screen(self.aspect, p)
            .is_some_and(|(x, y)| (0..=ONE).contains(&x) && (0..=ONE).contains(&y))
    }
}

fn seen(g: &Game) -> Seen {
    let f = &g.frame;
    let origin = g.scene.origin;
    let feet = g.world.player.pos;
    let mut shapes = BTreeMap::new();
    for d in f.draws.chunks(DRAW_VALUES) {
        let (buffer, mesh, offset, count, pass, layer) = (d[0], d[1], d[2], d[3], d[4], d[5]);
        if buffer != DYNAMIC_BUFFER || layer != 1 {
            continue;
        }
        let solid = pass == Pass::Solid as u32;
        let dither = pass == Pass::Dither as u32;
        if !solid && !dither {
            continue;
        }
        let start = offset as usize / 4;
        for k in 0..count as usize {
            let v = &f.instances[start + k * INSTANCE_VALUES..start + (k + 1) * INSTANCE_VALUES];
            if dither && v[15] < ONE {
                continue;
            }
            let p = v3(v[3], v[7], v[11]).add(origin);
            if v3(p.x - feet.x, 0, p.z - feet.z).len() > 40 * ONE {
                continue;
            }
            let q = ONE / 20;
            shapes.insert((mesh, p.x / q, p.y / q, p.z / q), p);
        }
    }
    Seen {
        shapes,
        cam: g.camera(),
        feet,
        aspect: g.aspect,
    }
}

/// Frames in which something near popped, and how many pops, over `n`
/// frames of `bits` and `dx`: a shape in one frame and not the next, where
/// both frames see its place.
fn walk(g: &mut Game, n: usize, bits: u32, dx: i32) -> (usize, usize, BTreeMap<u32, usize>) {
    let mut before = seen(g);
    let (mut frames, mut pops) = (0, 0);
    let mut by_mesh: BTreeMap<u32, usize> = BTreeMap::new();
    for i in 0..n {
        frame(g, bits, dx);
        let now = seen(g);
        let a: BTreeSet<&Key> = before.shapes.keys().collect();
        let b: BTreeSet<&Key> = now.shapes.keys().collect();
        let mut popped = Vec::new();
        let dist = |p: V3| v3(p.x - now.feet.x, 0, p.z - now.feet.z).len() as f64 / ONE as f64;
        for k in b.difference(&a) {
            let p = now.shapes[*k];
            if now.sees(p) && before.sees(p) {
                popped.push(k.0);
                println!("    frame {i}: mesh {} came, {:.1} m off", k.0, dist(p));
            }
        }
        for k in a.difference(&b) {
            let p = before.shapes[*k];
            if now.sees(p) && before.sees(p) {
                popped.push(k.0);
                println!("    frame {i}: mesh {} went, {:.1} m off", k.0, dist(p));
            }
        }
        if !popped.is_empty() {
            frames += 1;
            pops += popped.len();
            for m in popped {
                *by_mesh.entry(m).or_default() += 1;
            }
        }
        before = now;
    }
    (frames, pops, by_mesh)
}

#[test]
#[ignore]
fn probe_pop_in_near_the_player() {
    // how far one frame of mouse movement turns the view, to turn a whole
    // way round in 480 frames
    let mut g = game();
    g.act("story.skip", "").unwrap();
    g.act("screen.show", "play").unwrap();
    for _ in 0..60 {
        frame(&mut g, 0, 0);
    }
    let y0 = g.world.aim.yaw;
    frame(&mut g, 0, 10_000);
    let per = (g.world.aim.yaw - y0).rem_euclid(tool_core::fx::TURN).max(1) as i64;
    let dx = (10_000i64 * tool_core::fx::TURN as i64 / 480 / per) as i32;
    println!("turning: {dx} hundredths of a pixel a frame for a turn in 480 frames");

    for (place, go) in [
        ("the village", "1996,2838,18,-7"),
        ("the meadow", "1500,2650,80,-3"),
        ("the camp's edge", "3150,1060,300,-6"),
    ] {
        // T18 (R9.423): and sprinting at its top, 35 m/s, and at the top
        // with five speedster pieces, 70 m/s (the sprint set at its top;
        // five pieces' grip takes a few seconds to reach it)
        let sprint: &[(&str, &str)] = &[("sprint.set", "480")];
        let fast: &[(&str, &str)] = &[("motion.speedster", "5"), ("sprint.set", "480")];
        for (what, bits, turn, n, acts) in [
            ("walking straight", FORWARD, 0, 480, &[][..]),
            ("running straight", FORWARD | RUN, 0, 480, &[][..]),
            ("turning a whole way round", 0, dx, 480, &[][..]),
            ("sprinting straight at 35 m/s", FORWARD, 0, 480, sprint),
            ("sprinting straight at 70 m/s", FORWARD, 0, 480, fast),
        ] {
            let mut g = game();
            g.act("story.skip", "").unwrap();
            g.act("places.go", go).unwrap();
            g.act("story.still", "on").unwrap();
            g.act("screen.show", "play").unwrap();
            for _ in 0..180 {
                frame(&mut g, 0, 0);
            }
            for (name, value) in acts {
                g.act(name, value).unwrap();
            }
            let from: Fx = g.world.player.pos.x;
            let (frames, pops, by_mesh) = walk(&mut g, n, bits, turn);
            let moved = (g.world.player.pos.x - from) as f64 / ONE as f64;
            let speed = g.world.player.vel.len_xz() as f64 * 60.0 / ONE as f64;
            println!(
                "{place}, {what}: {frames} of {n} frames with a pop within {NEAR_M} m, {pops} pops (by mesh {by_mesh:?}); moved {moved:.1} m east, at {speed:.1} m/s at the end"
            );
        }
    }
}
