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
    let rebuild = tool_core::render::Scene::build(&g.world.scene(&g.look, &mut table), g.scene.origin, 99);
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
