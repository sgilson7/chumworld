//! A probe, not a check: what the world's terrain and detail cost to build,
//! natively. Prints numbers for MEASUREMENTS.md.
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
    println!("land (parse, river, solve): {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    let t = Instant::now();
    let mut n = 0;
    for j in 0..4 {
        for i in 0..4 {
            let tile = land.tile(0, 30 + i, 40 + j);
            n += tile.vertices.len();
        }
    }
    println!("16 level-0 tiles on the plains: {:.2} ms each, {} values", t.elapsed().as_secs_f64() * 1000.0 / 16.0, n / 16);
    let t = Instant::now();
    for k in 0..8 {
        let _ = land.tile(0, 9 + k % 2, 35 + k / 2);
    }
    println!("8 level-0 tiles at the river and bridge: {:.2} ms each", t.elapsed().as_secs_f64() * 1000.0 / 8.0);
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
    println!("height on the plains: {:.2} us", t.elapsed().as_secs_f64() * 1e6 / 10_000.0);
    let t = Instant::now();
    for k in 0..10_000 {
        let h = land.height_in(int(1900) + k * 1000, int(2500), &sc);
        acc += land.colour_in(int(1900) + k * 1000, int(2500), h, 0, 2 * ONE, &sc, None)[0] as i64;
    }
    println!("height + colour on the plains: {:.2} us", t.elapsed().as_secs_f64() * 1e6 / 10_000.0);
    let t = Instant::now();
    for k in 0..10_000 {
        acc += land.river.near(int(600) + k * 10, int(2000), land.reach).map(|n| n.d).unwrap_or(0) as i64;
    }
    println!("river near: {:.2} us", t.elapsed().as_secs_f64() * 1e6 / 10_000.0);
    let t = Instant::now();
    for k in 0..100_000u64 {
        acc += tool_core::fx::isqrt_u64(k * 123_456_789_011) as i64;
    }
    println!("fx::isqrt_u64 of a big number: {:.3} us {acc}", t.elapsed().as_secs_f64() * 1e6 / 100_000.0);
}

#[test]
#[ignore]
fn probe_colour_line() {
    let land = Land::shipped();
    for z in [2700, 2650] {
        for x in (1960..=2060).step_by(10) {
            let (px, pz) = (int(x), int(z));
            let sc = land.scope(px - int(64), pz - int(64), px + int(64), pz + int(64));
            let h = land.height_in(px, pz, &sc);
            let c = land.colour_in(px, pz, h, 0, 2 * ONE, &sc, None);
            let (rd, rh) = land.road_distance(px, pz, &sc);
            println!("({x}, {z}) h {:.2} rgb {:?} road {:.1}/{:.1} foot {}", h as f64 / 65536.0, c.map(|v| v * 255 / ONE), rd as f64 / 65536.0, rh as f64/65536.0, land.footprint_distance(px, pz, &sc) / ONE);
        }
    }
}

#[test]
#[ignore]
fn probe_cover() {
    let land = Land::shipped();
    let mut d = Detail::new(&land.spec.lod, 20);
    let mut out = Shapes::default();
    let eye = v3(int(2002), int(4), int(2808));
    d.terrain(&land, eye, &mut out);
    let mut bad = 0;
    for zz in (-300..300).step_by(7) {
        for xx in (-300..300).step_by(7) {
            let (px, pz) = (int(2002 + xx) as i64, int(2808 + zz) as i64);
            let mut sum = 0i64;
            let mut what = vec![];
            for dr in &d.drawn {
                let side = d.tile_side(dr.key.level as u32);
                let (x0, z0) = (dr.key.i as i64 * side, dr.key.j as i64 * side);
                if px >= x0 && px < x0 + side && pz >= z0 && pz < z0 + side {
                    sum += if dr.fade >= 0 { dr.fade as i64 } else { -(dr.fade as i64) };
                    what.push((dr.key.level, dr.key.i, dr.key.j, dr.fade));
                }
            }
            if sum != ONE as i64 && bad < 10 {
                bad += 1;
                println!("({}, {}): {sum} {:?}", 2002 + xx, 2808 + zz, what);
            }
        }
    }
    let mut levels = [0usize; 8];
    for dr in &d.drawn {
        levels[dr.key.level as usize] += 1;
    }
    println!("bad {bad}; tiles drawn by level {:?}", levels);
}

#[test]
#[ignore]
fn probe_draws() {
    let mut g = tool_core::game::Game::new(
        include_str!("../../../data/strings.json"),
        include_str!("../../../GLOSSARY.md"),
        include_str!("../../../data/look.json"),
        7,
    )
    .unwrap();
    g.resize(1920, 1080, 100);
    let first = g.detail.first;
    let mut by = std::collections::BTreeMap::new();
    for d in g.frame.draws.chunks(6) {
        let tile = d[1] >= first;
        *by.entry((d[0], d[4], d[5], tile)).or_insert(0) += d[3];
    }
    println!("(buffer, pass, layer, tile mesh) -> instances: {by:?}");
    println!("eye {:?}", g.camera().eye);
}
