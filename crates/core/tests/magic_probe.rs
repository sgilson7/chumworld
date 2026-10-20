//! M6's measurements (CW-11): what the leader's pattern costs the core's
//! tick and the page's draws, and what it takes from a player who stands
//! still, steps side to side, or swings through it imbued. A probe, not a
//! check: it prints and asserts the 2 ms budget alone.
//!
//!     cargo test --release -p tool_core --test magic_probe -- --ignored --nocapture

use std::time::Instant;

use tool_core::input::{Input, LEFT, RIGHT, SWING};
use tool_core::world::World;

fn leader(w: &mut World) {
    w.me.health = tool_core::health::Health::new(1_000);
    w.act("magic.leader", "").unwrap();
}

#[test]
#[ignore]
fn probe_leader_volley_tick_cost() {
    let mut w = World::new(9);
    leader(&mut w);
    tool_core::imbue::lend(&mut w, 2_000);
    w.step(&Input {
        keys: tool_core::input::DRAW,
        ..Input::default()
    });
    let (mut worst, mut most, mut ticks) = (0.0f64, 0usize, 0u32);
    let mut draws_most = 0usize;
    let look = tool_core::look::Look::parse(include_str!("../../../data/look.json")).unwrap();
    let t = Instant::now();
    while tool_core::orbs::flying(&w, 1) {
        let k = ticks;
        let i = Input {
            dx: if k % 40 < 20 { 2_000 } else { -2_000 },
            buttons: if k % 40 > 30 { SWING } else { 0 },
            ..Input::default()
        };
        let f = Instant::now();
        w.step(&i);
        worst = worst.max(f.elapsed().as_secs_f64() * 1000.0);
        ticks += 1;
        if w.orbs.flying.len() > most {
            most = w.orbs.flying.len();
            let mut s = tool_core::render::Shapes::default();
            tool_core::orbs::draw(&w, 0, &look, &mut s);
            let mut runs: Vec<_> = s.list.iter().map(|x| (x.pass, x.mesh)).collect();
            runs.sort();
            runs.dedup();
            draws_most = runs.len();
        }
    }
    let mean = t.elapsed().as_secs_f64() * 1000.0 / ticks as f64;
    println!(
        "leader volley: {ticks} ticks, {mean:.4} ms mean, {worst:.4} ms worst a tick; {most} orbs at most in flight, drawn in {draws_most} runs; turned {}",
        tool_core::orbs::deflected(&w, 1)
    );
    assert!(mean < 2.0, "a tick of the volley is over the core's 2 ms");
}

#[test]
#[ignore]
fn probe_what_the_leader_s_pattern_takes() {
    for (name, how) in [("still", 0), ("side to side", 1), ("still, imbued, swinging", 2)] {
        let mut w = World::new(9);
        leader(&mut w);
        if how == 2 {
            w.step(&Input {
                keys: tool_core::input::DRAW,
                ..Input::default()
            });
            tool_core::imbue::lend(&mut w, 2_000);
        }
        let mut k = 0u32;
        let mut thrown = 0u32;
        while tool_core::orbs::flying(&w, 1) {
            let keys = match how {
                1 if (k / 50).is_multiple_of(2) => LEFT,
                1 => RIGHT,
                _ => 0,
            };
            let i = Input {
                keys,
                dx: if how == 2 && k % 30 < 15 {
                    2_500
                } else if how == 2 {
                    -2_500
                } else {
                    0
                },
                buttons: if how == 2 && k % 30 > 22 { SWING } else { 0 },
                ..Input::default()
            };
            w.step(&i);
            thrown = thrown.max(w.orbs.next);
            k += 1;
        }
        println!(
            "{name}: {} health taken of {thrown} orbs thrown; turned {}; the leader took {}",
            1_000 - w.me.health.hp,
            tool_core::orbs::deflected(&w, 1),
            w.foes[0].fighter.health.most - w.foes[0].fighter.health.hp
        );
    }
}
