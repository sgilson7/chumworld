//! Probes, not checks (M2): what the world's terrain and detail cost to
//! build and to walk a frame, natively, what stays resident, and the
//! top-down overview of the land's colours. They print numbers for
//! MEASUREMENTS.md and assert nothing.
//!
//! ```bash
//! cargo test --release -p tool_core --test m2_probe -- --ignored --nocapture
//! ```

use std::time::Instant;

use tool_core::fx::{int, ONE};
use tool_core::geom::v3;
use tool_core::lod::Detail;
use tool_core::render::Shapes;
use tool_core::terrain::Land;

#[test]
#[ignore]
fn probe_terrain_cost() {
    let t = Instant::now();
    let land = Land::shipped();
    println!(
        "land (parse, river, solve): {:.1} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );
    let t = Instant::now();
    let mut n = 0;
    for j in 0..4 {
        for i in 0..4 {
            let tile = land.tile(0, 30 + i, 40 + j);
            n += tile.vertices.len();
        }
    }
    println!(
        "16 level-0 tiles on the plains: {:.2} ms each, {} values",
        t.elapsed().as_secs_f64() * 1000.0 / 16.0,
        n / 16
    );
    let t = Instant::now();
    for k in 0..8 {
        let _ = land.tile(0, 9 + k % 2, 35 + k / 2);
    }
    println!(
        "8 level-0 tiles at the river and bridge: {:.2} ms each",
        t.elapsed().as_secs_f64() * 1000.0 / 8.0
    );
    let t = Instant::now();
    let mut d = Detail::new(&land.spec.lod, 20);
    let mut out = Shapes::default();
    d.terrain(&land, v3(int(2000), int(2), int(2800)), &mut out);
    println!(
        "first frame at the village: {:.0} ms, {} tiles built, {} drawn, {} triangles drawn, {} resident triangles",
        t.elapsed().as_secs_f64() * 1000.0,
        d.stats.builds,
        d.stats.drawn,
        d.stats.triangles,
        d.stats.resident_triangles
    );
    let mut eye = v3(int(2000), int(2), int(2800));
    let mut worst = 0.0f64;
    let t = Instant::now();
    for _ in 0..3600 {
        eye.z -= ONE / 10;
        let f = Instant::now();
        out = Shapes::default();
        d.terrain(&land, eye, &mut out);
        let _ = d.uploads();
        worst = worst.max(f.elapsed().as_secs_f64() * 1000.0);
    }
    println!(
        "a minute running north at 6 m/s: {:.3} ms a frame mean, {:.2} ms worst, {} builds, {} at once",
        t.elapsed().as_secs_f64() * 1000.0 / 3600.0,
        worst,
        d.stats.total_builds,
        d.stats.total_sync_builds
    );
}

#[test]
#[ignore]
fn probe_parts() {
    let land = Land::shipped();
    let sc = land.scope(int(1900), int(2400), int(2100), int(2600));
    let t = Instant::now();
    let mut acc = 0i64;
    for k in 0..10_000 {
        acc += land.height_in(int(1900) + k * 1000, int(2500), &sc) as i64;
    }
    println!(
        "height on the plains: {:.2} us",
        t.elapsed().as_secs_f64() * 1e6 / 10_000.0
    );
    let t = Instant::now();
    for k in 0..10_000 {
        let h = land.height_in(int(1900) + k * 1000, int(2500), &sc);
        acc += land.colour_in(int(1900) + k * 1000, int(2500), h, 0, 2 * ONE, &sc, None)[0] as i64;
    }
    println!(
        "height + colour on the plains: {:.2} us",
        t.elapsed().as_secs_f64() * 1e6 / 10_000.0
    );
    let t = Instant::now();
    for k in 0..10_000 {
        acc += land
            .river
            .near(int(600) + k * 10, int(2000), land.reach)
            .map(|n| n.d)
            .unwrap_or(0) as i64;
    }
    println!("river near: {:.2} us", t.elapsed().as_secs_f64() * 1e6 / 10_000.0);
    let t = Instant::now();
    for k in 0..100_000u64 {
        acc += tool_core::fx::isqrt_u64(k * 123_456_789_011) as i64;
    }
    println!(
        "fx::isqrt_u64 of a big number: {:.3} us {acc}",
        t.elapsed().as_secs_f64() * 1e6 / 100_000.0
    );
}

/// A top-down picture of the land's colours, lit from the sun's side: the
/// overview for the journal. Writes a PPM; `sips` turns it into a PNG.
///
/// M2_MAP="x0 z0 size px path" picks the square (metres) and the file.
#[test]
#[ignore]
fn probe_map() {
    let land = Land::shipped();
    let spec = std::env::var("M2_MAP").unwrap_or_else(|_| "-200 -200 4400 1100 shots/m2/overview.ppm".into());
    let p: Vec<&str> = spec.split_whitespace().collect();
    let (x0, z0, size, px) = (
        p[0].parse::<i32>().unwrap(),
        p[1].parse::<i32>().unwrap(),
        p[2].parse::<i32>().unwrap(),
        p[3].parse::<i32>().unwrap(),
    );
    let step = (size as i64 * ONE as i64 / px as i64) as i32;
    let mut out = format!("P6 {px} {px} 255\n").into_bytes();
    let cell = step.max(2 * ONE);
    for j in 0..px {
        for i in 0..px {
            let (x, z) = (int(x0) + i * step, int(z0) + j * step);
            let sc = land.scope(x - cell, z - cell, x + cell, z + cell);
            let h = land.height_in(x, z, &sc);
            let near = land.near(x, z, &sc);
            let mut c = land.colour_in(x, z, h, 0, cell, &sc, near);
            if land.is_water(x, z) {
                c = [ONE / 4, ONE * 2 / 5, ONE * 9 / 20];
            }
            // light from the west-south-west: brighter where the land faces it
            let e = land.height_in(x - step, z + step / 2, &sc) as i64;
            let slope =
                ((h as i64 - e) * 4 * ONE as i64 / step as i64).clamp(-(ONE as i64) / 2, ONE as i64 / 2);
            for k in 0..3 {
                let v = (c[k] as i64 * (ONE as i64 + slope) / ONE as i64 * 255 / ONE as i64).clamp(0, 255);
                out.push(v as u8);
            }
        }
    }
    std::fs::write(format!("../../{}", p[4]), out).unwrap();
}

#[test]
#[ignore]
fn probe_frame_parts() {
    let mut g = tool_core::game::Game::new(
        include_str!("../../../data/strings.json"),
        include_str!("../../../GLOSSARY.md"),
        include_str!("../../../data/look.json"),
        7,
    )
    .unwrap();
    g.resize(1920, 1080, 100);
    g.act("screen.show", "play").unwrap();
    let (w, look) = (g.world.clone(), g.look.clone());
    let mut eye = v3(int(2000), int(2), int(2800));
    let (mut tt, mut ts, mut tf) = (0.0, 0.0, 0.0);
    let still = std::env::var("M2_STILL").is_ok();
    for _ in 0..3000 {
        let mut out = Shapes::default();
        g.detail.frame(&w, &look, eye, &mut out);
        let _ = g.detail.uploads();
    }
    println!(
        "built after 3000 still frames: {}, resident triangles {}",
        g.detail.stats.total_builds, g.detail.stats.resident_triangles
    );
    for _ in 0..600 {
        if !still {
            eye.z -= ONE / 10;
        }
        let mut out = Shapes::default();
        let t = Instant::now();
        g.detail.terrain(&w.land, eye, &mut out);
        tt += t.elapsed().as_secs_f64();
        let t = Instant::now();
        g.detail.scatter_frame(&w.land, eye, look.ink(), &mut out);
        ts += t.elapsed().as_secs_f64();
        let t = Instant::now();
        g.detail.faded_frame(eye, &mut out);
        tf += t.elapsed().as_secs_f64();
        let _ = g.detail.uploads();
    }
    println!(
        "resident (kind, level) -> (meshes, triangles): {:?}",
        g.detail.resident_by()
    );
    println!(
        "per frame: terrain {:.3} ms, scatter {:.3} ms, faded {:.3} ms",
        tt / 0.6,
        ts / 0.6,
        tf / 0.6
    );
}
