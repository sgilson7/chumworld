//! A probe, not a check: what one display frame costs the core, and how much
//! it sends the page. It prints numbers for MEASUREMENTS.md and asserts only
//! loose bounds. Run it on demand, in a release build:
//!
//! ```bash
//! cargo test --release -p tool_core --test frame_probe -- --ignored --nocapture
//! ```
//!
//! Native timings; the WebAssembly build in a browser runs slower (the page's
//! frame rate is measured by `tools/shoot.py --measure`).

use std::time::Instant;

use tool_core::game::Game;
use tool_core::render::{DRAW_VALUES, INSTANCE_VALUES};

const FRAME_US: i64 = 16_667;

fn game() -> Game {
    let mut g = Game::new(
        include_str!("../../../data/strings.json"),
        include_str!("../../../GLOSSARY.md"),
        include_str!("../../../data/look.json"),
        7,
    )
    .expect("the game starts");
    g.resize(1920, 1080, 100);
    g
}

#[test]
#[ignore]
fn probe_frame_cost() {
    let t = Instant::now();
    let mut g = game();
    let start_ms = t.elapsed().as_secs_f64() * 1000.0;
    g.act("screen.show", "play").unwrap();
    let mut now = 0i64;
    // warm up, then 1 200 frames running and turning in a wide circle
    for _ in 0..60 {
        now += FRAME_US;
        g.advance(now, 0, 0, 0, 0);
    }
    let bits = 1 | (1 << 4);
    let mut worst = 0.0f64;
    let t = Instant::now();
    let frames = 1200;
    for _ in 0..frames {
        now += FRAME_US;
        let f = Instant::now();
        g.advance(now, bits, 300, 0, 0);
        worst = worst.max(f.elapsed().as_secs_f64() * 1000.0);
    }
    let per_frame_ms = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
    // a frame that rebuilds the static array (the origin moved)
    let t = Instant::now();
    let mut table = tool_core::prim::MeshTable::default();
    // M2: the places' groups are drawn by the detail levels, not the static array
    let mut detail = tool_core::lod::Detail::new(&g.world.land.spec.lod, 0);
    let groups = detail.take_faded(g.world.scene(&g.look, &mut table));
    let rebuild = tool_core::render::Scene::build(&groups, g.scene.origin, 99);
    let rebuild_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("start (parse, lay out the field, first static array): {start_ms:.1} ms");
    println!("one frame (ticks, camera, instance array, draw list): {per_frame_ms:.3} ms mean, {worst:.3} ms worst");
    println!("static array rebuilt on an origin move: {rebuild_ms:.1} ms");
    println!(
        "static instances: {}, {} KB; moving instances a frame: {}; draws a frame: {}",
        g.scene.instance_count(),
        g.scene.values.len() * 4 / 1024,
        g.frame.instances.len() / INSTANCE_VALUES,
        g.frame.draws.len() / DRAW_VALUES
    );
    println!(
        "static rebuilds while running 1 200 frames: {}",
        g.scene.version - 1
    );
    assert_eq!(rebuild.instance_count(), g.scene.instance_count());
    assert!(
        per_frame_ms < 16.0,
        "a frame costs the core more than the whole budget"
    );
}

/// M1 (CW-11): the core's tick with eight fighters, the raid's size: the
/// player and seven foes, three orcs, three bandits and a leader, all
/// duelling or ringing, over ten seconds. Native release.
#[test]
#[ignore]
fn probe_raid_tick_cost() {
    use tool_core::input::Input;
    use tool_core::world::World;
    let mut w = World::new(9);
    for kind in ["orc", "orc", "orc", "bandit", "bandit", "bandit", "leader"] {
        w.act("duel.spawn", kind).unwrap();
    }
    // a player who stands through it, its sword drawn, swinging
    w.me.health = tool_core::health::Health::new(1_000_000);
    w.step(&Input {
        keys: tool_core::input::DRAW,
        ..Input::default()
    });
    let mut worst = 0.0f64;
    let ticks = 600;
    let t = Instant::now();
    for k in 0..ticks {
        let i = Input {
            dx: if k % 40 < 20 { 2_000 } else { -2_000 },
            buttons: if k % 40 > 30 { tool_core::input::SWING } else { 0 },
            ..Input::default()
        };
        let f = Instant::now();
        w.step(&i);
        worst = worst.max(f.elapsed().as_secs_f64() * 1000.0);
    }
    let mean = t.elapsed().as_secs_f64() * 1000.0 / ticks as f64;
    let h = Instant::now();
    let _ = w.hash();
    let hash_ms = h.elapsed().as_secs_f64() * 1000.0;
    println!("raid tick, 8 fighters: {mean:.3} ms mean, {worst:.3} ms worst; World::hash {hash_ms:.3} ms ({} bytes of debug text)", format!("{w:?}").len());
    println!(
        "blows {} and clashes {} in the last tick; foes standing {}",
        w.blades.hits.len(),
        w.blades.clashes.len(),
        w.foes.iter().filter(|f| !f.fighter.health.down()).count()
    );
    assert!(mean < 2.0, "the raid's tick is over the core's 2 ms");
}
