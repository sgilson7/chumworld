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
//! - **a cluster** (forest only, to `cluster_m`): one big cone for every few
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
pub const PARTS: [u8; 7] = [WHOLE, WHOLE_INK, CONE, CLUSTER, ROCK, ROCK_INK, GRASS];

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

/// The trees and rocks of chunk (i, j).
pub fn items(land: &Land, i: i32, j: i32) -> Vec<Item> {
    let sp = &land.spec.scatter;
    let side = sp.chunk_m * ONE;
    let (x0, z0) = (i * side, j * side);
    let [lo, hi] = sp.extent_m.map(|v| v * ONE);
    if x0 + side < lo || x0 > hi || z0 + side < lo || z0 > hi {
        return Vec::new();
    }
    let sc = land.scope(x0 - 8 * ONE, z0 - 8 * ONE, x0 + side + 8 * ONE, z0 + side + 8 * ONE);
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
                let [s0, s1] = if forest { sp.tree_size_pct } else { sp.plains_size_pct };
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
                size: if big { r.between(140, 260) } else { r.between(35, 90) },
                yaw: r.below(fx::TURN as u32) as i32,
                tone: r.below(sp.rock.len().max(1) as u32) as usize,
            });
        }
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
            .map(|c| ((c.x as i64 * c.x as i64 + c.y as i64 * c.y as i64 + c.z as i64 * c.z as i64) >> 16).max(1))
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
                w[0] += c.x as i64 * f >> 16;
                w[1] += c.y as i64 * f >> 16;
                w[2] += c.z as i64 * f >> 16;
            }
            let nl = local_normal(unit(w), self.k);
            let lp = [self.local(p[0]), self.local(p[1]), self.local(p[2])];
            facet(&mut self.out, lp, nl, c);
        }
    }

    /// A thin blade from `foot` to `tip`, three narrow faces round it.
    fn blade(&mut self, foot: V3, tip: V3, width: Fx, c: Rgb) {
        let corners: Vec<V3> = (0..3)
            .map(|q| {
                let a = fx::TURN * q / 3;
                foot.add(v3(fx::mul(fx::cos(a), width), 0, fx::mul(fx::sin(a), width)))
            })
            .collect();
        for q in 0..3 {
            let (a, b) = (corners[q], corners[(q + 1) % 3]);
            // counter-clockwise seen from outside
            let n = crate::terrain::facet_normal(b, a, tip);
            let tri = [self.local(b), self.local(a), self.local(tip)];
            facet(&mut self.out, tri, local_normal(n, self.k), shade(c, 92 + 6 * q as i32));
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
fn tree_parts(sp: &ScatterSpec, it: &Item) -> Vec<(Mesh, Affine, Rgb)> {
    let s = |m: Fx| m * it.size / 100;
    let at = it.at;
    let turn = Affine::rot_y(it.yaw);
    let place = |y: Fx, w: Fx, h: Fx, d: Fx| Affine::translate(at.add(v3(0, y, 0))).then(&turn).then(&Affine::scale(v3(w, h, d)));
    let trunk = rgb(sp.trunk);
    let mut out = Vec::new();
    match it.thing {
        Thing::Pine => {
            let leaf = rgb(sp.pine[it.tone % sp.pine.len()]);
            // Protection of the Spheres' pine: a trunk, three cones stepping in
            let th = s(ONE * 14 / 10);
            out.push((Mesh::Cyl, place(th / 2 - ONE / 4, s(ONE * 4 / 10), th + ONE / 2, s(ONE * 4 / 10)), trunk));
            for (k, (y, w, h)) in [(110, 390, 230), (230, 310, 200), (345, 215, 175)].into_iter().enumerate() {
                let c = shade(leaf, 100 + 7 * k as i32);
                out.push((Mesh::Cone, place(s(y * ONE / 100) + s(h * ONE / 100) / 2, s(w * ONE / 100), s(h * ONE / 100), s(w * ONE / 100)), c));
            }
        }
        Thing::Leafy => {
            let leaf = rgb(sp.leafy[it.tone % sp.leafy.len()]);
            let th = s(ONE * 18 / 10);
            out.push((Mesh::Cyl, place(th / 2 - ONE / 4, s(ONE * 36 / 100), th + ONE / 2, s(ONE * 36 / 100)), trunk));
            out.push((Mesh::Blob, place(s(ONE * 30 / 10), s(ONE * 36 / 10), s(ONE * 28 / 10), s(ONE * 34 / 10)), leaf));
            out.push((Mesh::Blob, place(s(ONE * 41 / 10), s(ONE * 24 / 10), s(ONE * 20 / 10), s(ONE * 23 / 10)), shade(leaf, 108)));
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
    let side = if part == GRASS { sp.grass_chunk_m } else { sp.chunk_m } * ONE;
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
                    Thing::Pine => (s(ONE * 36 / 10), s(ONE * 47 / 10), rgb(sp.pine[it.tone % sp.pine.len()])),
                    _ => (s(ONE * 36 / 10), s(ONE * 42 / 10), rgb(sp.leafy[it.tone % sp.leafy.len()])),
                };
                let a = Affine::translate(it.at.add(v3(0, s(ONE * 8 / 10) + h / 2, 0)))
                    .then(&Affine::rot_y(it.yaw))
                    .then(&Affine::scale(v3(w, h, w)));
                b.shape(Mesh::Cone, &a, c);
            }
        }
        CLUSTER => {
            // a big cone for a cluster cell of the forest, in its mean green
            let cell = sp.cluster_cell_m.max(1) * ONE;
            let n = side / cell;
            let sc = land.scope(i * side, j * side, (i + 1) * side, (j + 1) * side);
            let mean = sp.pine.iter().fold([0; 3], |acc, c| [acc[0] + c[0], acc[1] + c[1], acc[2] + c[2]]);
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
                    let size = r.between(80, 125);
                    let (w, h) = (ONE * 9 * size / 100, ONE * 12 * size / 100);
                    let a = Affine::translate(v3(x, land.height(x, z) + h / 2 + ONE, z))
                        .then(&Affine::rot_y(r.below(fx::TURN as u32) as i32))
                        .then(&Affine::scale(v3(w, h, w)));
                    b.shape(Mesh::Cone, &a, shade(mean, 88 + r.between(0, 12)));
                }
            }
        }
        GRASS => {
            let cell = (sp.grass_cell_cm.max(10) * ONE / 100) as Fx;
            let n = side / cell;
            let sc = land.scope(i * side - ONE, j * side - ONE, (i + 1) * side + ONE, (j + 1) * side + ONE);
            for q in 0..n {
                for p in 0..n {
                    let (ci, cj) = (i * n + p, j * n + q);
                    let mut r = cell_rng(land.spec.seed, 0x9a55, ci, cj);
                    let x = i * side + p * cell + r.below(cell as u32) as Fx;
                    let z = j * side + q * cell + r.below(cell as u32) as Fx;
                    if !r.chance(sp.grass_pct.max(0) as u32, 100) || !room(land, &sc, x, z, ONE / 3) {
                        continue;
                    }
                    // fewer in the forest's shade
                    if land.forest(x, z, &sc) > ONE as i64 / 2 && r.chance(2, 3) {
                        continue;
                    }
                    let y = land.ground(x, z) - ONE / 20;
                    let foot = v3(x, y, z);
                    let c = rgb(sp.blade[r.below(sp.blade.len() as u32) as usize]);
                    let flower = r.chance(sp.flower_pct.max(0) as u32, 100);
                    for blade in 0..4 {
                        let a = r.below(fx::TURN as u32) as i32;
                        let lean = ONE * r.between(6, 16) / 100;
                        let tall = ONE * r.between(22, 42) / 100;
                        let tip = foot.add(v3(fx::mul(fx::cos(a), lean), tall, fx::mul(fx::sin(a), lean)));
                        b.blade(foot.add(v3(blade * ONE / 30, 0, 0)), tip, ONE * 4 / 100, c);
                    }
                    if flower {
                        let fc = rgb(sp.flower[r.below(sp.flower.len() as u32) as usize]);
                        let top = foot.add(v3(0, ONE * r.between(30, 44) / 100, 0));
                        b.blade(foot, top, ONE * 2 / 100, shade(c, 90));
                        let head = Affine::translate(top).then(&Affine::scale(v3(ONE * 9 / 100, ONE * 7 / 100, ONE * 9 / 100)));
                        b.shape(Mesh::Octa, &head, fc);
                    }
                }
            }
        }
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
    let sp = &land.spec.scatter;
    let side = if part == GRASS { sp.grass_chunk_m } else { sp.chunk_m } * ONE;
    (i * side + side / 2, j * side + side / 2, side)
}

/// The dither fade a part of a chunk is drawn with `d` from the eye
/// (measured to its chunk's middle), or none when it is not drawn.
pub fn fade(sp: &ScatterSpec, part: u8, d: i64) -> Option<Fx> {
    // the share of the way through a band from `a` to `b` metres
    let t = |[a, b]: [i32; 2]| {
        let (a, b) = (a as i64 * ONE as i64, b as i64 * ONE as i64);
        ((d - a) * ONE as i64 / (b - a).max(1)).clamp(0, ONE as i64) as Fx
    };
    // the level going out over a band: whole before, gone after
    let going = |band: [i32; 2]| {
        let s = t(band);
        match s {
            0 => Some(ONE),
            ONE => None,
            _ => Some(crate::look::hand_over(s).1),
        }
    };
    // the level coming in over one band and going out over the next
    let between = |come: [i32; 2], go: [i32; 2]| {
        let (a, b) = (t(come), t(go));
        if a == 0 || b == ONE {
            None
        } else if b > 0 {
            Some(crate::look::hand_over(b).1)
        } else if a < ONE {
            Some(crate::look::hand_over(a).0)
        } else {
            Some(ONE)
        }
    };
    match part {
        WHOLE | WHOLE_INK => going(sp.full_m),
        CONE => between(sp.full_m, sp.cone_m),
        CLUSTER => between(sp.cone_m, sp.cluster_m),
        ROCK | ROCK_INK => going(sp.rock_m),
        GRASS => going(sp.grass_m),
        _ => None,
    }
}

/// The farthest a part is drawn, metres.
pub fn reach(sp: &ScatterSpec, part: u8) -> i32 {
    match part {
        WHOLE | WHOLE_INK => sp.full_m[1],
        CONE => sp.cone_m[1],
        CLUSTER => sp.cluster_m[1],
        ROCK | ROCK_INK => sp.rock_m[1],
        _ => sp.grass_m[1],
    }
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
        let chunks = [(4, 17), (5, 17), (4, 7), (5, 7), (15, 21), (15, 22), (16, 21), (22, 14), (23, 12)];
        let all: Vec<Item> = chunks.iter().flat_map(|&(i, j)| items(&l, i, j)).collect();
        assert!(all.len() > 100);
        for it in &all {
            assert!(!l.is_water(it.at.x, it.at.z), "{:?} in water", it.at);
            assert_eq!(it.at.y, l.ground(it.at.x, it.at.z));
            let sc = l.scope_all();
            let (rd, rh) = l.road_distance(it.at.x, it.at.z, &sc);
            assert!(rd >= rh, "{:?} on a road", it.at);
        }
    }

    #[test]
    fn each_level_hands_over_to_the_next_through_the_dither() {
        let sp = &land().spec.scatter;
        let m = |v: i32| int(v) as i64;
        // near: whole alone; 139 m, half way through 128 to 150: the whole
        // tree going out at fade −½ and the cone coming in at ½
        assert_eq!(fade(sp, WHOLE, m(100)), Some(ONE));
        assert_eq!(fade(sp, CONE, m(100)), None);
        let (w, c) = (fade(sp, WHOLE, m(139)).unwrap(), fade(sp, CONE, m(139)).unwrap());
        assert!((w + ONE / 2).abs() <= 2 && (c - ONE / 2).abs() <= 2, "{w} {c}");
        // between them they keep every pixel once: ½ and ½
        assert_eq!(c - w, ONE);
        // far: the cone alone, then the cluster, then nothing
        assert_eq!(fade(sp, CONE, m(800)), Some(ONE));
        assert_eq!(fade(sp, CLUSTER, m(800)), None);
        assert_eq!(fade(sp, CLUSTER, m(2000)), Some(ONE));
        assert_eq!(fade(sp, CLUSTER, m(4300)), None);
        assert_eq!(fade(sp, GRASS, m(70)), None);
    }
}
