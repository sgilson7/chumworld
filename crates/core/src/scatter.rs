//! `scatter`: trees, rocks, grass and flowers, placed from the land's seed
//! (M2 World, task W), drawn by distance through `lod.rs`.
//!
//! The land is cut into chunks (`chunk_m`, `grass_chunk_m` for grass); each
//! chunk's things come from its own seeded generator, so a chunk built
//! again, or on another machine, holds the same things (CW-4). A jittered
//! grid of cells: in the forest most cells hold a pine, on the plains a few
//! hold a leafy tree where the clumping noise is high, rocks lie anywhere,
//! grass tufts and flowers carpet the meadows near the eye. None stands on a
//! road, in water, on a footprint, in a clearing, in the scarecrow's field or
//! on a steep slope.
//!
//! Detail levels, Protection of the Spheres' faceted trees first:
//!
//! - **whole** (to `full_m`): a trunk and three cones (a pine) or two
//!   faceted crowns (a leafy tree), with the ink outline;
//! - **one cone** (to `cone_m`): a single cone a tree, no ink;
//! - **a cluster** (forest only, to `cluster_m`): one rounded crown for every few
//!   trees, so the forest still stands above the plain from kilometres off;
//! - then the forest floor's colour in the ground (`terrain.rs`).
//!
//! Rocks are whole and inked to `rock_m`, grass and flowers to `grass_m`.
//! Each level hands over to the next through the dither between its two
//! distances, both measured to the chunk's middle, so the pair draws every
//! pixel once (R2.12). Thousands of things go in a few meshes the core builds
//! (R2.16): a chunk's level is one mesh, its ink another.

use serde::Deserialize;

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, Affine, V3};
use crate::look::{rgb, shade, Rgb};
use crate::prim::{self, Mesh};
use crate::render::{inflate, Pass};
use crate::terrain::{facet, hash, local_normal, unit, Land};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ScatterSpec {
    pub chunk_m: i32,
    pub grass_chunk_m: i32,
    pub full_m: [i32; 2],
    pub cone_m: [i32; 2],
    pub cluster_m: [i32; 2],
    pub rock_m: [i32; 2],
    pub grass_m: [i32; 2],
    /// the tufts: coming in over the first two distances, going over the last two
    pub tuft_m: [i32; 4],
    pub tuft_chunk_m: i32,
    pub tuft_cell_cm: i32,
    pub tuft_pct: i32,
    pub road_m: [i32; 2],
    pub road_chunk_m: i32,
    pub meadow_m: [i32; 4],
    pub meadow_chunk_m: i32,
    pub meadow_cell_cm: i32,
    pub meadow_pct: i32,
    pub extent_m: [i32; 2],
    pub forest_cell_m: i32,
    pub forest_pct: i32,
    pub plains_cell_m: i32,
    pub plains_pct: i32,
    pub clump_m: i32,
    pub cluster_cell_m: i32,
    pub rock_cell_m: i32,
    pub rock_pct: i32,
    pub grass_cell_cm: i32,
    pub grass_pct: i32,
    pub flower_pct: i32,
    pub steep_pct: i32,
    pub pine: Vec<[i32; 3]>,
    pub leafy: Vec<[i32; 3]>,
    pub trunk: [i32; 3],
    pub rock: Vec<[i32; 3]>,
    pub blade: Vec<[i32; 3]>,
    pub flower: Vec<[i32; 3]>,
    pub tree_size_pct: [i32; 2],
    pub plains_size_pct: [i32; 2],
}

/// The parts of a chunk, each its own mesh (`lod::Key::level`).
pub const WHOLE: u8 = 0;
pub const WHOLE_INK: u8 = 1;
pub const CONE: u8 = 2;
pub const CLUSTER: u8 = 3;
pub const ROCK: u8 = 4;
pub const ROCK_INK: u8 = 5;
pub const GRASS: u8 = 6;
/// M7: chunky tufts of grass, to the middle distance (R9.44)
pub const TUFT: u8 = 7;
/// M7: the roads' surface, with its ruts, crown and soft edges (R9.46)
pub const ROAD: u8 = 8;
/// M7 (R9.53): the meadow's far clumps, the middle tier of grass, out to
/// a few hundred metres
pub const MEADOW: u8 = 9;
pub const PARTS: [u8; 10] = [
    WHOLE, WHOLE_INK, CONE, CLUSTER, ROCK, ROCK_INK, GRASS, TUFT, ROAD, MEADOW,
];

/// Never: a distance past any the land reaches, metres.
const NEVER: i32 = 1 << 20;

/// A part's band, metres: it comes in from the first distance to the
/// second and goes from the third to the fourth, each triangle by its own
/// distance from the eye (the shader's `band`, after `band_fade`). A part
/// whole from the eye out comes in from −1 m.
pub fn band(sp: &ScatterSpec, part: u8) -> [i32; 4] {
    let near = |go: [i32; 2]| [-1, -1, go[0], go[1]];
    let between = |come: [i32; 2], go: [i32; 2]| [come[0], come[1], go[0], go[1]];
    match part {
        WHOLE | WHOLE_INK => near(sp.full_m),
        CONE => between(sp.full_m, sp.cone_m),
        CLUSTER => between(sp.cone_m, sp.cluster_m),
        ROCK | ROCK_INK => near(sp.rock_m),
        GRASS => near(sp.grass_m),
        TUFT => sp.tuft_m,
        ROAD => near(sp.road_m),
        MEADOW => sp.meadow_m,
        _ => [NEVER, NEVER, NEVER, NEVER],
    }
}

/// Which of the shader's bands a part takes (`uBands`, then `uBands2`).
pub fn band_slot(part: u8) -> usize {
    match part {
        WHOLE | WHOLE_INK => 0,
        CONE => 1,
        CLUSTER => 2,
        ROCK | ROCK_INK => 3,
        GRASS => 4,
        TUFT => 5,
        ROAD => 6,
        _ => 7,
    }
}

/// How many bands the shader holds: two matrices of four.
pub const BANDS: usize = 8;

/// The value a part's shapes are drawn with: 2 + its band's slot, which the
/// shader reads as "fade each triangle by its own distance through this
/// band" (a hand-over fade is never above ONE).
pub fn band_value(part: u8) -> Fx {
    (2 + band_slot(part) as i32) * ONE
}

/// The dither fade of a triangle `d` from the eye (Q16 metres) in `b`:
/// coming in, the share of the way through the first two distances; going
/// out, that share of the last two less ONE, so the level going out and the
/// one coming in at the same distance keep every pixel once
/// (`look::hand_over`).
pub fn band_fade(b: [i32; 4], d: i64) -> Fx {
    let share = |a: i32, z: i32| {
        let (a, z) = (a as i64 * ONE as i64, z as i64 * ONE as i64);
        if z <= a {
            if d >= a {
                ONE
            } else {
                0
            }
        } else {
            ((d - a) * ONE as i64 / (z - a)).clamp(0, ONE as i64) as Fx
        }
    };
    let (come, go) = (share(b[0], b[1]), share(b[2], b[3]));
    if go > 0 {
        go - ONE
    } else {
        come
    }
}

/// The share of pixels a fade keeps (`look::keeps`).
pub fn kept(f: Fx) -> Fx {
    if f >= 0 {
        f
    } else {
        -f
    }
}

/// The shader's bands as two matrices' columns, metres in Q16: each part's
/// band at its slot, the slots no part takes never drawn; then how deep
/// grass sinks. Slots 0 to 3 dither (trees, rocks: a tree sunk would read
/// as one shrinking); 4 to 7 sink into the ground as they fade, so a band
/// of grass leaves no stipple to crawl as the view pans (R9.44).
pub fn uniform_values(sp: &ScatterSpec) -> Vec<i32> {
    let mut out = vec![fx::int(NEVER); BANDS * 4];
    for part in PARTS {
        let k = band_slot(part);
        for (q, v) in band(sp, part).into_iter().enumerate() {
            out[k * 4 + q] = fx::int(v);
        }
    }
    out.push(SINK);
    out.extend([band_value(WHOLE), 4 * ONE, 6 * ONE, 7 * ONE]);
    // a unit normal is 1 long, an indexed facet's 2 (to 3 where grass
    // bends, `Bake::bend_from`), the ground's 4 (`lod::index_flat`)
    out.extend([ONE * 3 / 2, ONE * 7 / 2, 2 * ONE]);
    // grass 78% as bright 6 m off, its own colour by 18 m: dark enough near
    // to read against the ground, matched to it by the time a blade is thin
    // enough to crawl as the view pans (R9.44)
    out.extend([ONE * 78 / 100, 6 * ONE, 18 * ONE]);
    out
}

/// The uniforms they go in, after the sky's.
pub const UNIFORMS: &[(&str, &str, usize)] = &[
    ("uBands", "uniformMatrix4fv", 16),
    ("uBands2", "uniformMatrix4fv", 16),
    ("uSink", "uniform1fv", 1),
    ("uBandKey", "uniform4fv", 4),
    ("uFlatKey", "uniform3fv", 3),
    ("uGrassNear", "uniform3fv", 3),
];

/// How deep grass and tufts sink as they fade (the second matrix's bands):
/// deeper than the tallest reed stands, 1.2 m.
pub const SINK: Fx = ONE * 120 / 100;

/// How far a part's shapes reach past its chunk's square, Q16 metres (a
/// crown's radius, a blade's lean).
pub fn grown(part: u8) -> i64 {
    let m: i64 = match part {
        WHOLE | WHOLE_INK | CONE => 5,
        CLUSTER => 16,
        ROCK | ROCK_INK | ROAD => 4,
        _ => 1,
    };
    m * ONE as i64
}

/// What a chunk holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thing {
    Pine,
    Leafy,
    Rock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    pub thing: Thing,
    /// its foot on the ground
    pub at: V3,
    /// percent of the model's size
    pub size: i32,
    pub yaw: i32,
    pub tone: usize,
}

/// A generator for a cell of a grid, seeded by the cell and what it holds.
fn cell_rng(seed: u32, salt: u32, i: i32, j: i32) -> crate::rng::Rng {
    crate::rng::Rng::new(((hash(i, j, seed ^ salt) as u64) << 32) | hash(j, i, seed ^ salt ^ 0x55aa) as u64)
}

/// Whether a thing may stand at (x, z): not on a road, in water, on a
/// footprint, in a clearing or the scarecrow's field, nor on a slope steeper
/// than `steep_pct`. `clear` is how much room it needs round it.
fn room(land: &Land, sc: &crate::terrain::Scope, x: Fx, z: Fx, clear: Fx) -> bool {
    let sp = &land.spec.scatter;
    let (rd, rh) = land.road_distance(x, z, sc);
    if rh > 0 && rd < rh + clear {
        return false;
    }
    if land.footprint_distance(x, z, sc) < clear + ONE {
        return false;
    }
    if land.cleared(x, z, sc) || land.in_field(x, z, sc) {
        return false;
    }
    if let Some(n) = land.near(x, z, sc) {
        if n.d < n.half + clear + 3 * ONE {
            return false;
        }
    }
    if land.in_pond(x, z, clear) {
        return false;
    }
    let (hx0, hx1) = (land.height_in(x - ONE, z, sc), land.height_in(x + ONE, z, sc));
    let (hz0, hz1) = (land.height_in(x, z - ONE, sc), land.height_in(x, z + ONE, sc));
    let slope = ((hx1 - hx0).abs()).max((hz1 - hz0).abs()) / 2;
    slope * 100 < ONE * sp.steep_pct
}

/// Whether a tuft of grass may grow at (x, z): not on a road, a footprint,
/// in water, a clearing or the field (no slope check: grass grows on banks).
fn room_grass(land: &Land, sc: &crate::terrain::Scope, x: Fx, z: Fx) -> bool {
    let (rd, rh) = land.road_distance(x, z, sc);
    if rh > 0 && rd < rh + ONE / 3 {
        return false;
    }
    if land.footprint_distance(x, z, sc) < ONE + ONE / 3 {
        return false;
    }
    if land.cleared(x, z, sc) || land.in_field(x, z, sc) {
        return false;
    }
    if let Some(n) = land.near(x, z, sc) {
        if n.d < n.half + ONE / 2 {
            return false;
        }
    }
    !land.in_pond(x, z, ONE / 3)
}

/// The tufts of grass a grass chunk (`GRASS`, `grass_chunk_m` square) or a
/// tuft chunk (`TUFT`) holds: each tuft's foot on the ground and its
/// generator, seeded by its cell, so the mesh and the tests see the same.
pub fn tufts(land: &Land, part: u8, i: i32, j: i32) -> Vec<(V3, crate::rng::Rng)> {
    let sp = &land.spec.scatter;
    let (side, cell_cm, pct, salt) = match part {
        TUFT => (sp.tuft_chunk_m, sp.tuft_cell_cm, sp.tuft_pct, 0x7f7f),
        MEADOW => (sp.meadow_chunk_m, sp.meadow_cell_cm, sp.meadow_pct, 0x6e6e),
        _ => (sp.grass_chunk_m, sp.grass_cell_cm, sp.grass_pct, 0x9a55),
    };
    let side = side * ONE;
    let cell = (cell_cm.max(10) * ONE / 100) as Fx;
    let n = side / cell;
    let sc = land.scope(
        i * side - ONE,
        j * side - ONE,
        (i + 1) * side + ONE,
        (j + 1) * side + ONE,
    );
    let cells = side / (2 * ONE);
    let ground = land.patch(i * cells, j * cells, cells);
    let mut out = Vec::new();
    for q in 0..n {
        for p in 0..n {
            let (ci, cj) = (i * n + p, j * n + q);
            let mut r = cell_rng(land.spec.seed, salt, ci, cj);
            let x = i * side + p * cell + r.below(cell as u32) as Fx;
            let z = j * side + q * cell + r.below(cell as u32) as Fx;
            if !r.chance(pct.max(0) as u32, 100) || !room_grass(land, &sc, x, z) {
                continue;
            }
            // fewer in the forest's shade
            if land.forest(x, z, &sc) > ONE as i64 / 2 && r.chance(2, 3) {
                continue;
            }
            let y = ground.at(x, z).unwrap_or_else(|| land.ground(x, z));
            out.push((v3(x, y - ONE / 20, z), r));
        }
    }
    out
}

/// The trees and rocks of chunk (i, j).
pub fn items(land: &Land, i: i32, j: i32) -> Vec<Item> {
    let sp = &land.spec.scatter;
    let side = sp.chunk_m * ONE;
    let (x0, z0) = (i * side, j * side);
    let [lo, hi] = sp.extent_m.map(|v| v * ONE);
    if x0 + side < lo || x0 > hi || z0 + side < lo || z0 > hi {
        return Vec::new();
    }
    let sc = land.scope(
        x0 - 8 * ONE,
        z0 - 8 * ONE,
        x0 + side + 8 * ONE,
        z0 + side + 8 * ONE,
    );
    let seed = land.spec.seed;
    let mut out = Vec::new();
    // the forest's pines and the plains' leafy trees, each on its own grid
    for (cell_m, forest) in [(sp.forest_cell_m, true), (sp.plains_cell_m, false)] {
        let cell = cell_m.max(1) * ONE;
        let n = side / cell;
        for b in 0..n {
            for a in 0..n {
                let (ci, cj) = (i * n + a, j * n + b);
                let mut r = cell_rng(seed, if forest { 0x7ee } else { 0x1ea }, ci, cj);
                let x = x0 + a * cell + r.below(cell as u32) as Fx;
                let z = z0 + b * cell + r.below(cell as u32) as Fx;
                let w = land.forest(x, z, &sc);
                let chance = if forest {
                    (w * sp.forest_pct as i64 / 100) as i32
                } else {
                    if w > ONE as i64 / 5 {
                        continue;
                    }
                    let clump = crate::terrain::noise(x, z, sp.clump_m, seed ^ 0xc1c1);
                    let k = ((clump - ONE as i64 / 4) * 2).clamp(0, ONE as i64);
                    (k * sp.plains_pct as i64 / 100) as i32
                };
                if (r.below(ONE as u32) as i32) >= chance {
                    continue;
                }
                if !room(land, &sc, x, z, 2 * ONE) {
                    continue;
                }
                let [s0, s1] = if forest {
                    sp.tree_size_pct
                } else {
                    sp.plains_size_pct
                };
                let (thing, tones) = if forest {
                    (Thing::Pine, sp.pine.len())
                } else {
                    (Thing::Leafy, sp.leafy.len())
                };
                out.push(Item {
                    thing,
                    at: v3(x, land.ground(x, z), z),
                    size: r.between(s0, s1),
                    yaw: r.below(fx::TURN as u32) as i32,
                    tone: r.below(tones.max(1) as u32) as usize,
                });
            }
        }
    }
    // rocks, a few big ones among many small
    let cell = sp.rock_cell_m.max(1) * ONE;
    let n = side / cell;
    for b in 0..n {
        for a in 0..n {
            let (ci, cj) = (i * n + a, j * n + b);
            let mut r = cell_rng(seed, 0x0c4, ci, cj);
            let x = x0 + a * cell + r.below(cell as u32) as Fx;
            let z = z0 + b * cell + r.below(cell as u32) as Fx;
            if !r.chance(sp.rock_pct.max(0) as u32, 100) || !room(land, &sc, x, z, ONE) {
                continue;
            }
            let big = r.chance(1, 8);
            out.push(Item {
                thing: Thing::Rock,
                at: v3(x, land.ground(x, z), z),
                size: if big {
                    r.between(140, 260)
                } else {
                    r.between(35, 90)
                },
                yaw: r.below(fx::TURN as u32) as i32,
                tone: r.below(sp.rock.len().max(1) as u32) as usize,
            });
        }
    }
    // M7: the river's boulders in this chunk, standing out of the water
    // (`water::River::boulders`): a stone 2r across, its top where the river
    // says (a stone's top stands 0.55 of its width over its foot)
    for (k, b) in land.river.rocks.iter().enumerate().filter(|(_, b)| b.boulder) {
        let inside = b.at.x >= x0 && b.at.x < x0 + side && b.at.z >= z0 && b.at.z < z0 + side;
        if !inside {
            continue;
        }
        let h = hash(k as i32, 7, seed ^ 0xb01d);
        out.push(Item {
            thing: Thing::Rock,
            at: v3(b.at.x, b.top - fx::mul(b.r, ONE * 11 / 10), b.at.z),
            size: (b.r as i64 * 500 / 3 / ONE as i64) as i32,
            yaw: (h % fx::TURN as u32) as i32,
            tone: (h >> 16) as usize % sp.rock.len().max(1),
        });
    }
    out
}

/// A mesh being built about a chunk's middle, x and z divided by `k`.
struct Bake {
    out: Vec<i32>,
    centre: V3,
    k: Fx,
}

impl Bake {
    fn new(centre: V3, k: Fx) -> Bake {
        Bake {
            out: Vec::new(),
            centre,
            k,
        }
    }

    fn local(&self, p: V3) -> V3 {
        v3(
            ((p.x - self.centre.x) as i64 * ONE as i64 / self.k as i64) as Fx,
            p.y - self.centre.y,
            ((p.z - self.centre.z) as i64 * ONE as i64 / self.k as i64) as Fx,
        )
    }

    /// A primitive placed by `at`, in one colour: each face's normal turned
    /// as the shader turns an instance's (the inverse scale), then for the
    /// mesh's own x and z division.
    fn shape(&mut self, mesh: Mesh, at: &Affine, c: Rgb) {
        let vs = prim::vertices(mesh);
        let cols = [at.c0, at.c1, at.c2];
        let l2: Vec<i64> = cols
            .iter()
            .map(|c| {
                ((c.x as i64 * c.x as i64 + c.y as i64 * c.y as i64 + c.z as i64 * c.z as i64) >> 16).max(1)
            })
            .collect();
        for t in vs.chunks(3 * prim::VERTEX_VALUES) {
            let mut p = [V3::ZERO; 3];
            for (q, pt) in p.iter_mut().enumerate() {
                let v = &t[q * prim::VERTEX_VALUES..];
                *pt = at.apply(v3(v[0], v[1], v[2]));
            }
            // the face's normal: the first vertex's, through the inverse scale
            let n = &t[3..6];
            let mut w = [0i64; 3];
            for (k, c) in cols.iter().enumerate() {
                let f = n[k] as i64 * ONE as i64 / l2[k];
                w[0] += (c.x as i64 * f) >> 16;
                w[1] += (c.y as i64 * f) >> 16;
                w[2] += (c.z as i64 * f) >> 16;
            }
            let nl = local_normal(unit(w), self.k);
            let lp = [self.local(p[0]), self.local(p[1]), self.local(p[2])];
            facet(&mut self.out, lp, nl, c);
        }
    }

    /// A thin blade from `foot` to `tip`, three narrow faces round it.
    /// A blade from `foot` to `tip`: three faces round it, narrowing to a
    /// blunt top half as wide as its foot, so no part of it is thinner than
    /// half its width (a point would blink as the view pans, R4.25).
    fn blade(&mut self, foot: V3, tip: V3, width: Fx, c: Rgb) {
        self.blade_lit(foot, tip, width, c, 0);
    }

    /// M7 (R9.53): how far the wind bends each vertex pushed since `from`,
    /// written into its normal's length (a unit normal stretched by 1 plus
    /// half its bend, 0 at the foot `foot_y` to ONE 50 cm up), which the
    /// shader reads back (`scene.vert`, `bend`): the foot stays put, the
    /// tip swings.
    fn bend_from(&mut self, from: usize, foot_y: Fx) {
        let vv = prim::VERTEX_VALUES;
        let mut k = from;
        while k + vv <= self.out.len() {
            let y = self.out[k + 1] + self.centre.y;
            let bend = ((y - foot_y).max(0) as i64 * ONE as i64 / (ONE as i64 / 2)).min(ONE as i64);
            let s = ONE as i64 + bend / 2;
            for q in 3..6 {
                self.out[k + q] = (self.out[k + q] as i64 * s / ONE as i64) as i32;
            }
            k += vv;
        }
    }

    /// The same, each face's normal turned `up` of the way toward the sky:
    /// a far tuft lit as the ground round it is, so it reads as the
    /// meadow's texture and not as a dark spot (R9.44).
    fn blade_lit(&mut self, foot: V3, tip: V3, width: Fx, c: Rgb, up: Fx) {
        let ring = |at: V3, r: Fx| -> Vec<V3> {
            (0..3)
                .map(|q| {
                    let a = fx::TURN * q / 3;
                    at.add(v3(fx::mul(fx::cos(a), r), 0, fx::mul(fx::sin(a), r)))
                })
                .collect()
        };
        let (low, mut high) = (ring(foot, width), ring(tip, width * 4 / 5));
        // the top cut on a slant, so it lies in no plane the ground's does
        // (a level top a few centimetres over level ground is too near it
        // for the depth buffer far off, the flicker scan's measure)
        for (q, h) in high.iter_mut().enumerate() {
            h.y += width * q as i32 / 2;
        }
        let mut side = None;
        for q in 0..3 {
            let (a, b) = (low[q], low[(q + 1) % 3]);
            let (a2, b2) = (high[q], high[(q + 1) % 3]);
            // counter-clockwise seen from outside
            let n = crate::terrain::facet_normal(b, a, a2);
            let n = unit([
                fx::lerp(n.x, 0, up) as i64,
                fx::lerp(n.y, ONE, up) as i64,
                fx::lerp(n.z, 0, up) as i64,
            ]);
            let nl = local_normal(n, self.k);
            side.get_or_insert(nl);
            // lit nearly as the ground, the faces need little shade of their
            // own; a face seen edge-on is a sliver, and a sliver of another
            // shade blinks as the view pans (R9.44)
            let c = if up > ONE / 2 {
                c
            } else {
                shade(c, 92 + 6 * q as i32)
            };
            let (lb, la, la2, lb2) = (self.local(b), self.local(a), self.local(a2), self.local(b2));
            facet(&mut self.out, [lb, la, la2], nl, c);
            facet(&mut self.out, [lb, la2, lb2], nl, c);
        }
        // the blunt top
        let n = crate::terrain::facet_normal(high[0], high[2], high[1]);
        let top = [self.local(high[0]), self.local(high[2]), self.local(high[1])];
        // lit as the faces are: the top seen edge-on from a standing eye is
        // a sliver, and lit brighter it would blink as the view pans
        let nl = if up > 0 {
            side.unwrap_or(nl_up(n, self.k))
        } else {
            local_normal(n, self.k)
        };
        facet(
            &mut self.out,
            top,
            nl,
            if up > ONE / 2 { c } else { shade(c, 104) },
        );
    }
}

fn nl_up(n: V3, k: Fx) -> V3 {
    local_normal(n, k)
}

/// A road's surface across, from verge to verge: each point's share of the
/// half width out from the middle (in hundredths), how
/// far it stands over the ground (mm), and what it is: 0 the verge (the
/// ground's own colour), 1 the edge (half dirt, half ground), 2 dirt, 3 a
/// rut (dark, and sunk), 4 the crown (dirt with a little grass).
const ACROSS: [(i32, i32, u8); 11] = [
    (-100, 40, 0),
    (-80, 40, 1),
    (-62, 40, 2),
    (-50, 20, 3),
    (-38, 40, 2),
    (0, 40, 4),
    (38, 40, 2),
    (50, 20, 3),
    (62, 40, 2),
    (80, 40, 1),
    (100, 40, 0),
];

/// The roads' surfaces in road chunk (i, j) (R9.46): a strip along each
/// road, lane and trail a metre at a time, as wide as the ground is
/// flattened level across under it, laid 4 cm over it (2 cm in the ruts),
/// whose edge takes the ground's own colour so it is soft; two ruts where the wheels
/// run, dark and sunk, a crown between them with a little grass in it.
/// A strip's metre belongs to the chunk its start lies in, so the chunks
/// meet with no gap and no overlap. None over the river: the bridge spans it.
fn road_surface(land: &Land, i: i32, j: i32, b: &mut Bake) {
    let side = side_of(&land.spec.scatter, ROAD) as i64;
    let (x0, z0) = (i as i64 * side, j as i64 * side);
    let inside = |p: (i64, i64)| p.0 >= x0 && p.0 < x0 + side && p.1 >= z0 && p.1 < z0 + side;
    let (dirt, dark) = land.road_paint();
    let o = ONE as i64;
    let sc = land.scope(
        (x0 - 8 * o) as Fx,
        (z0 - 8 * o) as Fx,
        (x0 + side + 8 * o) as Fx,
        (z0 + side + 8 * o) as Fx,
    );
    for road in &land.layout.roads {
        let half = road.half_cm as i64 * o / 100;
        // the line as points a metre apart along it
        let mut pts: Vec<(i64, i64)> = Vec::new();
        for k in 0..road.line.len().saturating_sub(1) {
            let (a, c) = (road.line[k], road.line[k + 1]);
            let (dx, dz) = ((c[0] - a[0]) as i64, (c[1] - a[1]) as i64);
            let len = crate::terrain::root((dx * dx + dz * dz) as u64) as i64;
            for m in 0..len {
                pts.push((
                    a[0] as i64 * o + dx * o * m / len,
                    a[1] as i64 * o + dz * o * m / len,
                ));
            }
        }
        if let Some(l) = road.line.last() {
            pts.push((l[0] as i64 * o, l[1] as i64 * o));
        }
        // the way across at each point: square to the line two metres
        // either side, so a bend turns the strip smoothly
        let across = |k: usize| {
            let (a, c) = (pts[k.saturating_sub(2)], pts[(k + 2).min(pts.len() - 1)]);
            let d = v3((c.0 - a.0) as Fx, 0, (c.1 - a.1) as Fx).norm();
            v3(-d.z, 0, d.x)
        };
        let row = |k: usize| -> Vec<(V3, Rgb)> {
            let (cx, cz) = pts[k];
            let side = across(k);
            ACROSS
                .iter()
                .map(|&(share, lift_mm, what)| {
                    let off = half * share as i64 / 100;
                    let x = (cx + side.x as i64 * off / o) as Fx;
                    let z = (cz + side.z as i64 * off / o) as Fx;
                    let g = land.ground(x, z);
                    let ground = land.colour_in(x, z, g, 0, 2 * ONE, &sc, land.near(x, z, &sc));
                    let patch = crate::terrain::noise(x, z, 9, land.spec.seed ^ 0x5d5d);
                    let soil = crate::look::mix(dirt, dark, ((patch + ONE as i64) * 3 / 10) as Fx);
                    let c = match what {
                        0 => ground,
                        1 => crate::look::mix(ground, soil, ONE / 2),
                        3 => crate::look::mix(soil, dark, ONE * 6 / 10),
                        4 => crate::look::mix(soil, ground, ONE / 4),
                        _ => soil,
                    };
                    (v3(x, g + lift_mm * ONE / 1000, z), c)
                })
                .collect()
        };
        for k in 0..pts.len().saturating_sub(1) {
            if !inside(pts[k])
                || land.over_river(pts[k].0, pts[k].1)
                || land.over_river(pts[k + 1].0, pts[k + 1].1)
            {
                continue;
            }
            let (r0, r1) = (row(k), row(k + 1));
            for q in 0..ACROSS.len() - 1 {
                let (a, c0) = r0[q];
                let (bq, c1) = r0[q + 1];
                let (cq, c2) = r1[q + 1];
                let (d, c3) = r1[q];
                let mean = |cs: [Rgb; 3]| [0, 1, 2].map(|k| (cs[0][k] + cs[1][k] + cs[2][k]) / 3);
                // facing up, counter-clockwise from above, whichever way the
                // road runs: the turn from one edge to the next decides
                for (p, cs) in [([a, bq, cq], [c0, c1, c2]), ([a, cq, d], [c0, c2, c3])] {
                    let n = crate::terrain::facet_normal(p[0], p[1], p[2]);
                    let (p, n) = if n.y < 0 {
                        ([p[0], p[2], p[1]], n.neg())
                    } else {
                        (p, n)
                    };
                    let nl = local_normal(n, b.k);
                    let lp = [b.local(p[0]), b.local(p[1]), b.local(p[2])];
                    facet(&mut b.out, lp, nl, mean(cs));
                }
            }
        }
    }
}

/// A chunk's middle, at the height of the ground there, and the power of two
/// its meshes' x and z are divided by.
fn frame_of(land: &Land, x0: Fx, z0: Fx, side: Fx) -> (V3, Fx) {
    let c = v3(x0 + side / 2, 0, z0 + side / 2);
    let y = land.height(c.x, c.z);
    let radius = side as i64 * 3 / 4 + 40 * ONE as i64;
    (v3(c.x, y, c.z), crate::lod::unit_for(radius))
}

/// The affines of a tree's parts: its trunk, then its crowns, each with a
/// colour (a pine's cones, a leafy tree's faceted crowns).
pub fn tree_parts(sp: &ScatterSpec, it: &Item) -> Vec<(Mesh, Affine, Rgb)> {
    let s = |m: Fx| m * it.size / 100;
    let at = it.at;
    let turn = Affine::rot_y(it.yaw);
    let place = |y: Fx, w: Fx, h: Fx, d: Fx| {
        Affine::translate(at.add(v3(0, y, 0)))
            .then(&turn)
            .then(&Affine::scale(v3(w, h, d)))
    };
    let trunk = rgb(sp.trunk);
    let mut out = Vec::new();
    match it.thing {
        Thing::Pine => {
            let leaf = rgb(sp.pine[it.tone % sp.pine.len()]);
            // Protection of the Spheres' pine: a trunk, three cones stepping in
            let th = s(ONE * 14 / 10);
            out.push((
                Mesh::Cyl,
                place(th / 2 - ONE / 4, s(ONE * 4 / 10), th + ONE / 2, s(ONE * 4 / 10)),
                trunk,
            ));
            for (k, (y, w, h)) in [(110, 390, 230), (230, 310, 200), (345, 215, 175)]
                .into_iter()
                .enumerate()
            {
                let c = shade(leaf, 100 + 7 * k as i32);
                out.push((
                    Mesh::Cone,
                    place(
                        s(y * ONE / 100) + s(h * ONE / 100) / 2,
                        s(w * ONE / 100),
                        s(h * ONE / 100),
                        s(w * ONE / 100),
                    ),
                    c,
                ));
            }
        }
        Thing::Leafy => {
            let leaf = rgb(sp.leafy[it.tone % sp.leafy.len()]);
            let th = s(ONE * 18 / 10);
            out.push((
                Mesh::Cyl,
                place(
                    th / 2 - ONE / 4,
                    s(ONE * 36 / 100),
                    th + ONE / 2,
                    s(ONE * 36 / 100),
                ),
                trunk,
            ));
            out.push((
                Mesh::Blob,
                place(
                    s(ONE * 30 / 10),
                    s(ONE * 36 / 10),
                    s(ONE * 28 / 10),
                    s(ONE * 34 / 10),
                ),
                leaf,
            ));
            out.push((
                Mesh::Blob,
                place(
                    s(ONE * 41 / 10),
                    s(ONE * 24 / 10),
                    s(ONE * 20 / 10),
                    s(ONE * 23 / 10),
                ),
                shade(leaf, 108),
            ));
        }
        Thing::Rock => {
            let c = rgb(sp.rock[it.tone % sp.rock.len()]);
            let w = s(ONE * 12 / 10);
            out.push((Mesh::Blob, place(w / 5, w, w * 7 / 10, w * 85 / 100), c));
        }
    }
    out
}

/// A chunk part's mesh: its vertices, the affine it is drawn with and its
/// pass; none for a part the chunk has nothing in.
pub fn build(land: &Land, part: u8, i: i32, j: i32, items: &[Item], ink: Rgb) -> Option<crate::lod::Built> {
    let sp = &land.spec.scatter;
    let side = side_of(sp, part);
    let (centre, k) = frame_of(land, i * side, j * side, side);
    let mut b = Bake::new(centre, k);
    let mut pass = Pass::Dither;
    match part {
        WHOLE | WHOLE_INK => {
            for it in items.iter().filter(|it| it.thing != Thing::Rock) {
                for (m, a, c) in tree_parts(sp, it) {
                    if part == WHOLE {
                        b.shape(m, &a, c);
                    } else {
                        b.shape(m, &inflate(&a), ink);
                    }
                }
            }
            if part == WHOLE_INK {
                pass = Pass::Ink;
            }
        }
        ROCK | ROCK_INK => {
            for it in items.iter().filter(|it| it.thing == Thing::Rock) {
                for (m, a, c) in tree_parts(sp, it) {
                    if part == ROCK {
                        b.shape(m, &a, c);
                    } else {
                        b.shape(m, &inflate(&a), ink);
                    }
                }
            }
            if part == ROCK_INK {
                pass = Pass::Ink;
            }
        }
        CONE => {
            // one cone a tree, as wide as its widest crown, from low on the trunk
            for it in items.iter().filter(|it| it.thing != Thing::Rock) {
                let s = |m: Fx| m * it.size / 100;
                let (w, h, c) = match it.thing {
                    Thing::Pine => (
                        s(ONE * 36 / 10),
                        s(ONE * 47 / 10),
                        rgb(sp.pine[it.tone % sp.pine.len()]),
                    ),
                    _ => (
                        s(ONE * 36 / 10),
                        s(ONE * 42 / 10),
                        rgb(sp.leafy[it.tone % sp.leafy.len()]),
                    ),
                };
                let a = Affine::translate(it.at.add(v3(0, s(ONE * 8 / 10) + h / 2, 0)))
                    .then(&Affine::rot_y(it.yaw))
                    .then(&Affine::scale(v3(w, h, w)));
                b.shape(Mesh::Cone, &a, c);
            }
        }
        CLUSTER => {
            // a crown for a cluster cell of the forest, in its mean green
            let cell = sp.cluster_cell_m.max(1) * ONE;
            let n = side / cell;
            let sc = land.scope(i * side, j * side, (i + 1) * side, (j + 1) * side);
            let mean = sp
                .pine
                .iter()
                .fold([0; 3], |acc, c| [acc[0] + c[0], acc[1] + c[1], acc[2] + c[2]]);
            let mean = rgb(mean.map(|v| v / sp.pine.len().max(1) as i32));
            for q in 0..n {
                for p in 0..n {
                    let (ci, cj) = (i * n + p, j * n + q);
                    let mut r = cell_rng(land.spec.seed, 0x0c1c, ci, cj);
                    let x = i * side + p * cell + r.below(cell as u32) as Fx;
                    let z = j * side + q * cell + r.below(cell as u32) as Fx;
                    if land.forest(x, z, &sc) < ONE as i64 / 2 || !room(land, &sc, x, z, 3 * ONE) {
                        continue;
                    }
                    // a rounded, faceted crown with no point: a point thinner
                    // than a pixel blinks as the view pans (R4.25)
                    let size = r.between(80, 125);
                    // M7 (R9.49): a skyline, not a strip. The crowns stand
                    // taller deep in the forest and where a slow noise
                    // lifts them (70% to 190% of 15 m), and about a third
                    // carry a narrower crown above, a spruce's top seen
                    // from kilometres off, still blunt
                    let deep = land.forest(x, z, &sc);
                    let lift = crate::terrain::noise(x, z, 90, land.spec.seed ^ 0x51e1) + ONE as i64;
                    let tall_pct =
                        70 + (lift * 60 / ONE as i64) as i32 + (deep * 60 / ONE as i64) as i32 - 30;
                    let (w, h) = (
                        ONE * 14 * size / 100,
                        ONE * 15 * size / 100 * tall_pct.clamp(70, 190) / 100,
                    );
                    let foot = land.height(x, z) + 2 * ONE;
                    let turn = Affine::rot_y(r.below(fx::TURN as u32) as i32);
                    let a = Affine::translate(v3(x, foot + h / 2, z))
                        .then(&turn)
                        .then(&Affine::scale(v3(w, h, w)));
                    let tone = shade(mean, 88 + r.between(0, 12));
                    b.shape(Mesh::Blob, &a, tone);
                    if r.chance(1, 3) {
                        let (w2, h2) = (w * 55 / 100, h * 75 / 100);
                        let top = Affine::translate(v3(x, foot + h * 85 / 100 + h2 / 2, z))
                            .then(&turn)
                            .then(&Affine::scale(v3(w2, h2, w2)));
                        b.shape(Mesh::Blob, &top, shade(tone, 94));
                    }
                }
            }
        }
        GRASS => {
            let sc = land.scope(i * side, j * side, (i + 1) * side, (j + 1) * side);
            for (foot, mut r) in tufts(land, GRASS, i, j) {
                // the blades' greens half the ground's own there, lit half as
                // the ground is: a meadow's grass, not spikes on it, and its
                // edges low enough in contrast not to crawl as the view pans
                let ground = land.meadow_in(foot.x, foot.z, 0, &sc);
                let blade = rgb(sp.blade[r.below(sp.blade.len() as u32) as usize]);
                let mut c = shade(crate::look::mix(ground, blade, ONE * 45 / 100), 96);
                // M7 (R9.47): by the water's edge, off a gravel bar, reeds:
                // three times as tall, upright, an olive green
                // in clumps where a slow noise says so, thinning away from the water
                let reed = land.near(foot.x, foot.z, &sc).is_some_and(|n| {
                    let edge = n.d - n.half;
                    let clump = crate::terrain::noise(foot.x, foot.z, 6, land.spec.seed ^ 0x2eed);
                    let reach = ONE as i64 / 2 + (clump + ONE as i64) * 2;
                    edge > ONE * 3 / 10
                        && (edge as i64) < reach
                        && clump > -(ONE as i64) / 5
                        && land.bank_shape(&n).2 < ONE as i64 / 2
                });
                if reed {
                    c = shade(crate::look::mix(blade, land.dry_colour(), ONE * 4 / 10), 84);
                }
                let flower = r.chance(sp.flower_pct.max(0) as u32, 100) && !reed;
                let from = b.out.len();
                for blade in 0..if reed { 6 } else { 4 } {
                    let a = r.below(fx::TURN as u32) as i32;
                    let lean = ONE * r.between(6, 16) / 100 * if reed { 3 } else { 2 } / 2;
                    let tall = ONE * r.between(22, 42) / 100 * if reed { 22 } else { 10 } / 10;
                    let tip = foot.add(v3(fx::mul(fx::cos(a), lean), tall, fx::mul(fx::sin(a), lean)));
                    b.blade_lit(foot.add(v3(blade * ONE / 30, 0, 0)), tip, ONE * 5 / 100, c, ONE);
                }
                if flower {
                    let fc = rgb(sp.flower[r.below(sp.flower.len() as u32) as usize]);
                    let top = foot.add(v3(0, ONE * r.between(30, 44) / 100, 0));
                    b.blade(foot, top, ONE * 2 / 100, shade(c, 90));
                    let head = Affine::translate(top).then(&Affine::scale(v3(
                        ONE * 14 / 100,
                        ONE * 11 / 100,
                        ONE * 14 / 100,
                    )));
                    b.shape(Mesh::Octa, &head, fc);
                }
                b.bend_from(from, foot.y);
            }
        }
        TUFT => {
            // a chunky tuft: a three-sided blade 28 cm across, blunt at half
            // that, leaning a little; a third have a second beside it
            let sc = land.scope(i * side, j * side, (i + 1) * side, (j + 1) * side);
            for (foot, mut r) in tufts(land, TUFT, i, j) {
                // the ground's own colour there, a little of the blades' and
                // darker: far off a tuft is the meadow's texture, not a spot
                // on it (and low contrast does not shimmer, R4.25)
                let ground = land.meadow_in(foot.x, foot.z, 0, &sc);
                let blade = rgb(sp.blade[r.below(sp.blade.len() as u32) as usize]);
                let c = shade(crate::look::mix(ground, blade, ONE * 25 / 100), 90);
                let blades = if r.chance(1, 3) { 2 } else { 1 };
                let from = b.out.len();
                for k in 0..blades {
                    let a = r.below(fx::TURN as u32) as i32;
                    let lean = ONE * r.between(4, 10) / 100;
                    let tall = ONE * r.between(24, 36) / 100;
                    let at = foot.add(v3(k * ONE / 8, 0, 0));
                    let tip = at.add(v3(fx::mul(fx::cos(a), lean), tall, fx::mul(fx::sin(a), lean)));
                    b.blade_lit(at, tip, ONE * 16 / 100, c, ONE);
                }
                b.bend_from(from, foot.y);
            }
        }
        MEADOW => {
            // the middle tier: one broad, low clump every few metres, lit
            // and coloured as the meadow is, a little darker, so far off it
            // is the grass's own texture (R9.53)
            let sc = land.scope(i * side, j * side, (i + 1) * side, (j + 1) * side);
            for (foot, mut r) in tufts(land, MEADOW, i, j) {
                let ground = land.meadow_in(foot.x, foot.z, 0, &sc);
                let blade = rgb(sp.blade[r.below(sp.blade.len() as u32) as usize]);
                let c = shade(crate::look::mix(ground, blade, ONE / 5), 93);
                let a = r.below(fx::TURN as u32) as i32;
                let lean = ONE * r.between(4, 12) / 100;
                let tall = ONE * r.between(30, 46) / 100;
                let tip = foot.add(v3(fx::mul(fx::cos(a), lean), tall, fx::mul(fx::sin(a), lean)));
                let from = b.out.len();
                b.blade_lit(foot, tip, ONE * 30 / 100, c, ONE);
                b.bend_from(from, foot.y);
            }
        }
        ROAD => road_surface(land, i, j, &mut b),
        _ => {}
    }
    if b.out.is_empty() {
        return None;
    }
    Some(crate::lod::Built {
        vertices: b.out,
        at: crate::lod::placed(centre, k),
        pass,
    })
}

/// The middle of a part's chunk and its side.
pub fn chunk_middle(land: &Land, part: u8, i: i32, j: i32) -> (Fx, Fx, Fx) {
    let side = side_of(&land.spec.scatter, part);
    (i * side + side / 2, j * side + side / 2, side)
}

/// A part's chunk's side, Q16 metres.
pub fn side_of(sp: &ScatterSpec, part: u8) -> Fx {
    let m = match part {
        GRASS => sp.grass_chunk_m,
        TUFT => sp.tuft_chunk_m,
        ROAD => sp.road_chunk_m,
        MEADOW => sp.meadow_chunk_m,
        _ => sp.chunk_m,
    };
    m * ONE
}

/// The distances a part is drawn between, metres: from where it starts to
/// come in to where it is gone.
pub fn interval(sp: &ScatterSpec, part: u8) -> (i32, i32) {
    let b = band(sp, part);
    (b[0].max(0), b[3])
}

/// How far ahead of its interval a part is built (Q16 metres, i64): a
/// tenth of the interval, 40 m at the least (at 7 m/s, nearly six seconds).
pub fn margin(sp: &ScatterSpec, part: u8) -> i64 {
    let (lo, hi) = interval(sp, part);
    ((hi - lo) as i64 * ONE as i64 / 10).max(40 * ONE as i64)
}

/// The farthest a part is drawn, metres.
pub fn reach(sp: &ScatterSpec, part: u8) -> i32 {
    band(sp, part)[3]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::int;
    use crate::terrain::tests::land;

    #[test]
    fn the_same_chunk_holds_the_same_things_and_none_on_the_road_or_in_water() {
        let l = land();
        // a forest chunk, round (3100, 1000)
        let (i, j) = (3100 / 128, 1000 / 128);
        let a = items(&l, i, j);
        assert_eq!(a, items(&l, i, j));
        let pines = a.iter().filter(|x| x.thing == Thing::Pine).count();
        // 128 m / 9 m = 14 cells a side, 196 cells, about 82% deep in the forest
        assert!(pines > 100, "{pines} pines");
        // the chunks over the river and the bridge, the village, the road
        // through the forest
        let chunks = [
            (4, 17),
            (5, 17),
            (4, 7),
            (5, 7),
            (15, 21),
            (15, 22),
            (16, 21),
            (22, 14),
            (23, 12),
        ];
        let all: Vec<Item> = chunks.iter().flat_map(|&(i, j)| items(&l, i, j)).collect();
        assert!(all.len() > 100);
        for it in &all {
            // none in the water but the river's own boulders (M7)
            let boulder = l
                .river
                .rocks
                .iter()
                .any(|b| b.boulder && b.at.x == it.at.x && b.at.z == it.at.z);
            if boulder {
                continue;
            }
            assert!(!l.is_water(it.at.x, it.at.z), "{:?} in water", it.at);
            assert_eq!(it.at.y, l.ground(it.at.x, it.at.z));
            let sc = l.scope_all();
            let (rd, rh) = l.road_distance(it.at.x, it.at.z, &sc);
            assert!(rd >= rh, "{:?} on a road", it.at);
        }
    }

    /// R9.44: each triangle fades by its own distance. Whole trees go from
    /// 128 to 150 m and cones come in over the same: at 139 m, half way, the
    /// tree's fade is ½ − 1 = −½ (keeping the half of the pixels whose
    /// threshold is ½ or more) and the cone's ½ (the other half).
    #[test]
    fn each_level_hands_over_to_the_next_through_the_dither() {
        let sp = &land().spec.scatter;
        let m = |v: i32| int(v) as i64;
        let at = |part: u8, d: i64| band_fade(band(sp, part), d);
        assert_eq!(at(WHOLE, m(100)), ONE);
        assert_eq!(at(CONE, m(100)), 0);
        let (w, c) = (at(WHOLE, m(139)), at(CONE, m(139)));
        assert!((w + ONE / 2).abs() <= 2 && (c - ONE / 2).abs() <= 2, "{w} {c}");
        // between them they keep every pixel once: ½ and ½
        assert_eq!(c - w, ONE);
        assert_eq!(kept(w) + kept(c), ONE);
        // far: the cone alone, then the cluster, then nothing
        assert_eq!(at(CONE, m(600)), ONE);
        assert_eq!(at(CLUSTER, m(600)), 0);
        assert_eq!(at(CLUSTER, m(2000)), ONE);
        assert_eq!(kept(at(CLUSTER, m(4300))), 0);
        // grass whole to 28 m, a quarter gone at 30.5 m ((30.5 − 28) / 10),
        // gone at 38; tufts half in at 24 m, whole at 28, half gone at 70
        assert_eq!(at(GRASS, m(28)), ONE);
        assert_eq!(kept(at(GRASS, (int(305) / 10) as i64)), ONE * 3 / 4);
        assert_eq!(kept(at(GRASS, m(38))), 0);
        assert_eq!(at(TUFT, m(24)), ONE / 2);
        assert_eq!(at(TUFT, m(28)), ONE);
        assert_eq!(kept(at(TUFT, m(70))), ONE / 2);
        assert_eq!(kept(at(TUFT, m(78))), 0);
    }

    #[test]
    fn the_shader_fades_each_triangle_through_the_core_s_band() {
        let sp = &land().spec.scatter;
        let u = uniform_values(sp);
        assert_eq!(u.len(), BANDS * 4 + 11);
        // grass's band in slot 4: from −1 m (whole from the eye), 28 to 38 m
        assert_eq!(&u[16..20], &[-ONE, -ONE, int(28), int(38)]);
        assert_eq!(band_value(GRASS), 6 * ONE);
        let vert = include_str!("../../../data/shaders/scene.vert");
        assert!(vert.contains("return go > 0.0 ? go - 1.0 : come;"));
        assert!(vert.contains("float f = k >= 0.0 ? band(length((w - uEye).xz), k) : iD.a;"));
        // the first band's value, 2, and four bands a matrix
        assert_eq!(
            &u[BANDS * 4 + 1..BANDS * 4 + 5],
            &[2 * ONE, 4 * ONE, 6 * ONE, 7 * ONE]
        );
        // an indexed facet's normal is twice a unit's: 1.5 tells them apart
        assert_eq!(
            &u[BANDS * 4 + 5..BANDS * 4 + 8],
            &[ONE * 3 / 2, ONE * 7 / 2, 2 * ONE]
        );
        assert!(vert.contains("w.y -= (1.0 - (f >= 0.0 ? f : -f)) * uSink;"));
        // grass and tufts take the second matrix's slots, which sink
        assert!(band_slot(GRASS) >= 4 && band_slot(TUFT) >= 4 && band_slot(ROCK) < 4);
        assert_eq!(u[BANDS * 4], SINK);
    }
}
