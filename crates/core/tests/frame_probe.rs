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

/// M5 (CW-11): the opening's own tick: the game's start (`story::opening`),
/// six orcs on their rounds, four villagers fleeing, the merchant on the
/// road, the walls and floors; the player gets up and stands on the green
/// so the orcs come, draw and fight (a player who does not fall), over 20 s.
/// Then the camp: five more foes in a fight. Native release.
#[test]
#[ignore]
fn probe_opening_tick_cost() {
    use tool_core::input::Input;
    let mut w = tool_core::story::opening(9);
    w.me.health = tool_core::health::Health::new(1_000_000);
    let measure = |w: &mut tool_core::world::World, ticks: u32| {
        let mut worst = 0.0f64;
        let t = Instant::now();
        for k in 0..ticks {
            let i = Input {
                keys: if k == 200 { tool_core::input::DRAW } else { 0 },
                dx: if k % 40 < 20 { 2_000 } else { -2_000 },
                buttons: if k % 40 > 30 { tool_core::input::SWING } else { 0 },
                ..Input::default()
            };
            let f = Instant::now();
            w.step(&i);
            worst = worst.max(f.elapsed().as_secs_f64() * 1000.0);
        }
        (t.elapsed().as_secs_f64() * 1000.0 / ticks as f64, worst)
    };
    // up, then out of the house onto the green by `places.go`
    measure(&mut w, 130);
    w.act("places.go", "1990,2805").unwrap();
    w.me.health = tool_core::health::Health::new(1_000_000);
    let (mean, worst) = measure(&mut w, 1200);
    let near = w
        .foes
        .iter()
        .filter(|f| f.body.pos.sub(w.player.pos).len_xz() < tool_core::fx::int(15))
        .count();
    println!(
        "opening tick: {mean:.3} ms mean, {worst:.3} ms worst; {} fighters, {near} foes within 15 m",
        tool_core::player::count(&w)
    );
    // the camp, alarmed, with the raid still in the world
    w.act("places.go", "3196,1016").unwrap();
    w.me.health = tool_core::health::Health::new(1_000_000);
    let (cmean, cworst) = measure(&mut w, 600);
    println!(
        "camp tick, raid kept: {cmean:.3} ms mean, {cworst:.3} ms worst; {} fighters",
        tool_core::player::count(&w)
    );
    assert!(mean < 2.0 && cmean < 2.0, "over the core's 2 ms");
}

/// M7 travel (CW-11): travel's own share of a tick: the five waystones'
/// nearness, the gift's check and a journey, called alone over 60 000 ticks
/// on a world past the opening with every waystone woken; then the woken
/// waystones' light and the traveller drawn. Native release.
#[test]
#[ignore]
fn probe_travel_tick_cost() {
    use tool_core::world::World;
    let mut w = tool_core::story::opening(9);
    tool_core::story::step_aside(&mut w);
    // the dearest case first: nothing yet reached, so every tick measures
    // the player's distance to all five waystones
    let ticks = 60_000;
    let t = Instant::now();
    for _ in 0..ticks {
        tool_core::travel::step(&mut w);
    }
    let unreached_us = t.elapsed().as_secs_f64() * 1_000_000.0 / ticks as f64;
    for k in 0..5 {
        tool_core::travel::wake(&mut w, k);
    }
    let t = Instant::now();
    for k in 0..ticks {
        if k % 1000 == 0 {
            w.travel.warp = Some(tool_core::travel::Warp {
                to: (k / 1000) % 5,
                ticks: 0,
                ride: false,
            });
        }
        tool_core::travel::step(&mut w);
    }
    let step_us = t.elapsed().as_secs_f64() * 1_000_000.0 / ticks as f64;
    let look = tool_core::look::Look::parse(include_str!("../../../data/look.json")).unwrap();
    let mut shapes = tool_core::render::Shapes::default();
    let t = Instant::now();
    let frames = 2_000;
    for _ in 0..frames {
        shapes.list.clear();
        tool_core::travel::draw(&w, 0, &look, &mut shapes);
    }
    let draw_us = t.elapsed().as_secs_f64() * 1_000_000.0 / frames as f64;
    let _: &World = &w;
    println!(
        "travel: step {unreached_us:.3} µs a tick with nothing reached, {step_us:.3} µs with every waystone woken and a journey each 1 000 ticks; draw {draw_us:.1} µs a frame, {} shapes (five woken waystones and the traveller)",
        shapes.list.len()
    );
    assert!(
        unreached_us.max(step_us) < 20.0,
        "travel's tick is not negligible"
    );
}

/// M7 P4 (CW-11): the raid of `probe_raid_tick_cost` with wounds: every
/// foe cut on a part every 20 ticks, a part taken off every 60, so the drops
/// stay near their cap of 160, the splats at their 64 and the pieces at
/// their 12. The tick, and drawing what moves (`World::draw`), native release.
#[test]
#[ignore]
fn probe_raid_wounds_cost() {
    use tool_core::input::Input;
    use tool_core::world::World;
    let mut w = World::new(9);
    for kind in ["orc", "orc", "orc", "bandit", "bandit", "bandit", "leader"] {
        w.act("duel.spawn", kind).unwrap();
    }
    w.me.health = tool_core::health::Health::new(1_000_000);
    w.step(&Input {
        keys: tool_core::input::DRAW,
        ..Input::default()
    });
    let parts = [
        "forearm_l",
        "shin_r",
        "upper_arm_l",
        "thigh_l",
        "forearm_r",
        "shin_l",
    ];
    let look = tool_core::look::Look::parse(include_str!("../../../data/look.json")).unwrap();
    let (mut worst, mut total, mut draw_worst, mut draw_total) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut most_drops, mut most_splats, mut most_pieces, mut most_shapes) = (0, 0, 0, 0);
    let ticks = 600;
    for k in 0..ticks {
        if k % 20 == 0 {
            w.act("wounds.cut", "torso,14").unwrap();
            for f in &mut w.foes {
                f.fighter.health.hp = f.fighter.health.most;
                f.fighter.health.down_ticks = 0;
            }
        }
        if k % 60 == 0 {
            // a fresh part each time: the foes made whole first
            for f in &mut w.foes {
                f.fighter.health.limbs = Default::default();
            }
            w.act("wounds.take_off", parts[(k / 60) % parts.len()]).unwrap();
        }
        let i = Input {
            dx: if k % 40 < 20 { 2_000 } else { -2_000 },
            buttons: if k % 40 > 30 { tool_core::input::SWING } else { 0 },
            ..Input::default()
        };
        let f = Instant::now();
        w.step(&i);
        let ms = f.elapsed().as_secs_f64() * 1000.0;
        worst = worst.max(ms);
        total += ms;
        if ms > 3.0 {
            println!(
                "  tick {k}: {ms:.3} ms, {} drops, {} splats",
                w.wounds.drops.len(),
                w.wounds.splats.len()
            );
        }
        let d = Instant::now();
        let mut out = tool_core::render::Shapes::default();
        w.draw(0, &look, &mut out);
        let dms = d.elapsed().as_secs_f64() * 1000.0;
        draw_worst = draw_worst.max(dms);
        draw_total += dms;
        most_drops = most_drops.max(w.wounds.drops.len());
        most_splats = most_splats.max(w.wounds.splats.len());
        most_pieces = most_pieces.max(w.wounds.pieces.len());
        most_shapes = most_shapes.max(out.len());
    }
    let mean = total / ticks as f64;
    println!(
        "raid with wounds, 8 fighters: tick {mean:.3} ms mean, {worst:.3} ms worst; draw {:.3} ms mean, {draw_worst:.3} ms worst; at most {most_drops} drops, {most_splats} splats, {most_pieces} pieces, {most_shapes} shapes drawn",
        draw_total / ticks as f64
    );
    assert!(mean < 2.0, "the raid's tick with wounds is over the core's 2 ms");
}

/// M7 (P9, T15, CW-11): the wanderers' tick. Three encounters moved beside
/// the player so the cap's six foes stand and fight (more than the shipped
/// spacing ever brings together), against a player who does not fall; then
/// the same world with every encounter parked far off. Native release.
#[test]
#[ignore]
fn probe_wanderers_tick_cost() {
    use tool_core::geom::v3;
    use tool_core::input::Input;
    use tool_core::wanderers::{self, Moves};
    let mut w = tool_core::world::World::new(9);
    w.story.phase = tool_core::story::Phase::Free;
    w.story.crossed = Some(tool_core::world::START);
    wanderers::ensure(&mut w);
    let pairs: Vec<usize> = (0..w.wanderers.list.len())
        .filter(|&i| w.wanderers.list[i].moves == Moves::Roam)
        .take(3)
        .collect();
    let here = w.wanderers.list[pairs[0]].anchor;
    for (n, &i) in pairs.iter().enumerate() {
        let e = &mut w.wanderers.list[i];
        let d = here.add(v3((n as i32 * 6 - 6) * 65536, 0, 0)).sub(e.anchor);
        e.anchor = e.anchor.add(d);
        for p in &mut e.route {
            *p = p.add(d);
        }
    }
    let mut at = here.add(v3(0, 0, 12 * 65536));
    at.y = w.ground(at.x, at.z);
    w.player = tool_core::player::Body::new(at, 0);
    w.me = tool_core::player::Fighter::new(
        tool_core::pose::Kind::Player,
        tool_core::pose::Held::ShortSword,
        0,
        at,
        0,
        false,
    );
    let measure = |w: &mut tool_core::world::World, ticks: u32| {
        let mut worst = 0.0f64;
        let t = Instant::now();
        for k in 0..ticks {
            w.me.health = tool_core::health::Health::new(1_000_000);
            let i = Input {
                keys: if k == 2 { tool_core::input::DRAW } else { 0 },
                dx: if k % 40 < 20 { 2_000 } else { -2_000 },
                buttons: if k % 40 > 30 { tool_core::input::SWING } else { 0 },
                ..Input::default()
            };
            let f = Instant::now();
            w.step(&i);
            worst = worst.max(f.elapsed().as_secs_f64() * 1000.0);
        }
        (t.elapsed().as_secs_f64() * 1000.0 / ticks as f64, worst)
    };
    let (mean, worst) = measure(&mut w, 600);
    println!(
        "wanderers, {} foes standing near: {mean:.3} ms mean, {worst:.3} ms worst",
        wanderers::near_count(&w)
    );
    // every encounter parked: the player at the village centre
    let at = tool_core::world::START;
    w.player = tool_core::player::Body::new(at, 0);
    w.me = tool_core::player::Fighter::new(
        tool_core::pose::Kind::Player,
        tool_core::pose::Held::ShortSword,
        0,
        at,
        0,
        false,
    );
    measure(&mut w, 5);
    let (pm, pw) = measure(&mut w, 600);
    println!(
        "all parked ({} foes left in the world): {pm:.4} ms mean, {pw:.3} ms worst",
        w.foes.len()
    );
    let t = Instant::now();
    for _ in 0..600 {
        wanderers::step(&mut w);
    }
    println!(
        "wanderers::step alone, {} encounters parked: {:.2} us",
        w.wanderers.list.len(),
        t.elapsed().as_secs_f64() * 1e6 / 600.0
    );
    let best = |f: &dyn Fn() -> usize| {
        (0..5)
            .map(|_| {
                let t = Instant::now();
                let n = f();
                (t.elapsed().as_secs_f64() * 1000.0, n)
            })
            .fold((f64::MAX, 0), |a, b| if b.0 < a.0 { b } else { a })
    };
    let (ms, n) = best(&|| wanderers::place(&w, 9).len());
    let (rms, _) = best(&|| wanderers::runs(&w).len());
    println!("placing {n} encounters: {ms:.1} ms (best of 5), of which measuring the ways {rms:.1} ms");
}

/// M7 (T14, CW-11): what a sprint costs the core a frame. From the plains
/// west of the village, 25 s of frames: one run with Shift held, one with a
/// tap of Shift that builds to three times the run speed after 9 s, the
/// same gentle turn in both. The frames carry the ticks, the camera, the
/// detail levels streaming the land past at speed, and the draw list.
#[test]
#[ignore]
fn probe_sprint_frame_cost() {
    let run = |sprint: bool| {
        let mut g = game();
        g.act("places.go", "1500,2650,80,0").unwrap();
        g.act("screen.show", "play").unwrap();
        let mut now = 0i64;
        for _ in 0..120 {
            now += FRAME_US;
            g.advance(now, 0, 0, 0, 0);
        }
        let frames = 1500;
        let (mut worst, mut slow) = (0.0f64, 0);
        let start = g.world.player.pos;
        let t = Instant::now();
        for k in 0..frames {
            now += FRAME_US;
            let shift = if sprint { (20..23).contains(&k) } else { true };
            let bits = 1 | if shift { 1 << 4 } else { 0 };
            let f = Instant::now();
            g.advance(now, bits, 40, 0, 0);
            let ms = f.elapsed().as_secs_f64() * 1000.0;
            worst = worst.max(ms);
            slow += (ms > 8.0) as u32;
        }
        let mean = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        let gone = g.world.player.pos.sub(start).len_xz() / 65_536;
        let top = tool_core::player::mps(g.world.player.vel.len_xz());
        (mean, worst, slow, gone, top, g.scene.version - 1)
    };
    for (name, sprint) in [("run (Shift held)", false), ("sprint (Shift tapped)", true)] {
        let (mean, worst, slow, gone, top, rebuilds) = run(sprint);
        println!(
            "{name}: {mean:.3} ms mean, {worst:.3} ms worst, {slow} frames over 8 ms; {gone} m in 25 s, {} m/s at the end; static rebuilds {rebuilds}",
            tool_core::fx::decimal(top, 1)
        );
        assert!(mean < 16.0, "a frame costs the core more than the whole budget");
    }
}
