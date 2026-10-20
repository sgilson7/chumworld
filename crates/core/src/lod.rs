//! `lod`: the choice of detail level by distance, and the dithered hand-over
//! between levels (CW-1, CW-18), owned by M2 World (task W).
//!
//! # Terrain tiles
//!
//! The terrain is a quadtree of square tiles of `tile_cells` cells a side.
//! Level 0 has 2 m cells; each level up has cells twice as coarse, to 128 m
//! at the top (`render::rings`, PLAN.md). The top level's tiles cover the
//! land from `extent_m[0]` to `extent_m[1]` each way, so the camera anywhere
//! in the walkable square sees land to the 16 km horizon.
//!
//! Each frame the tree is walked from the top. A tile of level L is split
//! into its four children of level L − 1 while the camera is inside the ring
//! of level L − 1 (128 cells of that level, `ring`), measured flat to the
//! tile's middle. Over the outer `band_pct` of the ring the tile and its
//! children are both drawn, the children coming in with dither fade t and
//! the tile going out with t − ONE (`look::hand_over`), so every pixel is
//! drawn by exactly one of them and a change of level never pops.
//!
//! At most two levels meet anywhere. A child's middle is 0.71 of a child
//! tile (22.6 of its cells, for 32-cell tiles) from its parent's, so while
//! the parent is in its band (85% to 100% of 128 cells) the child is at
//! least 86 of its cells off, outside its own ring of 64; and the parent's
//! parent, at most 87 of its own cells off, is split whole (inside 85% of
//! 128). The argument holds for tiles of up to 63 cells; measured to the
//! nearest point instead it would hold for any size, at 1.6 times the
//! triangles (SECOND-ORDER.md R4.3).
//!
//! # Building without a hitch
//!
//! A tile's mesh is built once and kept while it may be needed: the walk
//! asks for the children of every tile within `prefetch_pct` of its ring,
//! and the nearest `builds_per_frame` of those are built each frame, ahead
//! of need, so a moving camera never waits (R2.5). A tile needed now and not
//! yet built (after a jump: the game's start, a teleport) is built at once,
//! and counted (`Stats::sync_builds`). Tiles beyond `evict_pct` of their
//! ring are forgotten and their mesh slot reused.
//!
//! The page uploads each new mesh once (`uploads`): its id, its vertex
//! count, then `mesh(id)` while the page holds it. The core keeps no copy of
//! a mesh after handing it over.
//!
//! # Culling
//!
//! A moving shape is drawn in the layers its affine's reach can touch
//! (`render::Camera::layers`), and the reach assumes a mesh inside the unit
//! cube. So a tile's mesh is built about its middle with x and z divided by
//! a power of two `k` at least the tile's radius (exact on the cells'
//! lattice), y in metres, and drawn with the affine scale(k, 1, k).

use std::collections::BTreeMap;

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, Affine, V3};
use crate::look;
use crate::prim::MeshId;
use crate::render::{Group, Pass, Shape, Shapes, FAR};
use crate::terrain::{Land, LodSpec};

/// What a slot holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    /// 0: a terrain tile; other kinds are scatter's (`scatter.rs`)
    pub kind: u8,
    pub level: u8,
    pub i: i32,
    pub j: i32,
    /// which of a patch's meshes (a solid and its ink)
    pub part: u8,
}

pub const TERRAIN: u8 = 0;

/// A mesh the page holds, drawn as one shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub key: Key,
    /// the affine it is drawn with (its reach holds the mesh)
    pub at: Affine,
    pub pass: Pass,
    pub rgb: look::Rgb,
    pub vertices: usize,
}

/// A mesh built: its vertices and how it is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub vertices: Vec<i32>,
    pub at: Affine,
    pub pass: Pass,
}

/// Counts for the measurements and the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    /// meshes built this frame, and of those, built because they were
    /// needed at once
    pub builds: usize,
    pub sync_builds: usize,
    /// since the game began
    pub total_builds: usize,
    pub total_sync_builds: usize,
    pub evictions: usize,
    /// shapes drawn this frame, and their triangles
    pub drawn: usize,
    pub triangles: usize,
    pub resident: usize,
    pub resident_triangles: usize,
    /// the last thing built because it was needed at once
    pub last_sync: Option<Key>,
}

/// A shape the walk drew this frame: the key, and its dither fade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drawn {
    pub key: Key,
    pub fade: Fx,
}

/// The detail kept round the camera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detail {
    pub spec: LodSpec,
    /// the first mesh id a slot takes (after the static mesh table)
    pub first: u32,
    slots: Vec<Option<Slot>>,
    free: Vec<usize>,
    by_key: BTreeMap<Key, usize>,
    /// built since the page last asked
    uploads: Vec<(u32, Vec<i32>)>,
    /// what the page is reading now
    lent: Vec<(u32, Vec<i32>)>,
    pub stats: Stats,
    /// what was drawn this frame, in order
    pub drawn: Vec<Drawn>,
    /// kept from frame to frame: the frame number each key was last wanted
    wanted: BTreeMap<Key, u64>,
    frame: u64,
    /// what is drawn whole near and handed over by distance: the places'
    /// props and the water's pieces
    pub faded: Vec<Faded>,
    water_built: bool,
    /// under test, every mesh is kept too, for the flicker scan
    kept: BTreeMap<u32, Vec<i32>>,
    /// the scatter's chunks with nothing in a part, and each tree chunk's things
    empty: std::collections::BTreeSet<Key>,
    things: BTreeMap<(i32, i32), Vec<crate::scatter::Item>>,
}

/// A group drawn by distance: whole inside `fade.0` of its centre, handed
/// over through the dither to `fade.1`; a far one the other way, coming in
/// as its near twin goes (both measured to one centre, so between them every
/// pixel is drawn once).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Faded {
    pub name: String,
    pub centre: V3,
    pub shapes: Vec<Shape>,
    pub fade: (Fx, Fx),
    pub far: bool,
}

pub const WATER: u8 = 1;
pub const SCATTER: u8 = 2;
/// a faded group's entry in `drawn`: its place in `faded`
pub const FADED: u8 = 3;

/// A level's cell, Q16 metres.
pub fn cell(level: u32) -> Fx {
    ONE << (level + 1)
}

/// The radius out to which a level is drawn: 128 of its cells (256 m for
/// level 0's 2 m cells, doubling each level, to 16 384 m).
pub fn ring(level: u32) -> i64 {
    (cell(level) as i64) * 128
}

/// The power of two a mesh's x and z are divided by: at least `radius`.
pub fn unit_for(radius: i64) -> Fx {
    let mut k: i64 = ONE as i64;
    while k < radius {
        k *= 2;
    }
    k.min(1 << 30) as Fx
}

/// The flat distance from (x, z) to the square from (x0, z0), `side` wide.
fn to_square(x: i64, z: i64, x0: i64, z0: i64, side: i64) -> i64 {
    let dx = (x0 - x).max(x - (x0 + side)).max(0);
    let dz = (z0 - z).max(z - (z0 + side)).max(0);
    fx::isqrt_u64((dx * dx + dz * dz) as u64) as i64
}

impl Detail {
    pub fn new(spec: &LodSpec, first: u32) -> Detail {
        Detail {
            spec: spec.clone(),
            first,
            slots: Vec::new(),
            free: Vec::new(),
            by_key: BTreeMap::new(),
            uploads: Vec::new(),
            lent: Vec::new(),
            stats: Stats::default(),
            drawn: Vec::new(),
            wanted: BTreeMap::new(),
            frame: 0,
            faded: Vec::new(),
            water_built: false,
            kept: BTreeMap::new(),
            empty: std::collections::BTreeSet::new(),
            things: BTreeMap::new(),
        }
    }

    /// Builds a scatter chunk's part, or notes it has nothing.
    fn build_scatter(&mut self, land: &Land, key: Key, ink: look::Rgb) {
        let (i, j) = (key.i, key.j);
        let things = if key.level == crate::scatter::GRASS || key.level == crate::scatter::CLUSTER {
            Vec::new()
        } else {
            self.things
                .entry((i, j))
                .or_insert_with(|| crate::scatter::items(land, i, j))
                .clone()
        };
        match crate::scatter::build(land, key.level, i, j, &things, ink) {
            Some(b) => self.put(key, b),
            None => {
                self.empty.insert(key);
            }
        }
        self.stats.builds += 1;
        self.stats.total_builds += 1;
    }

    /// The scatter's share of a frame: each part of each chunk in reach,
    /// at its fade; the parts soon in reach built ahead, a few a frame; the
    /// far ones forgotten.
    pub fn scatter_frame(&mut self, land: &Land, eye: V3, ink: look::Rgb, out: &mut Shapes) {
        let sp = land.spec.scatter.clone();
        let mut ahead: Vec<(i64, Key)> = Vec::new();
        for part in crate::scatter::PARTS {
            let (_, _, side) = crate::scatter::chunk_middle(land, part, 0, 0);
            let (lo, hi) = crate::scatter::interval(&sp, part);
            let (lo, hi) = (lo as i64 * ONE as i64, hi as i64 * ONE as i64);
            let margin = crate::scatter::margin(&sp, part);
            let soon = hi + margin;
            let s = side as i64;
            // within the land the scatter covers
            let [elo, ehi] = sp.extent_m.map(|v| v as i64 * ONE as i64);
            let (c0, c1) = (elo.div_euclid(s), ehi.div_euclid(s));
            let (i0, i1) = ((eye.x as i64 - soon).div_euclid(s).max(c0), (eye.x as i64 + soon).div_euclid(s).min(c1));
            let (j0, j1) = ((eye.z as i64 - soon).div_euclid(s).max(c0), (eye.z as i64 + soon).div_euclid(s).min(c1));
            // in 256ths of a metre, so squares fit
            let (ex, ez) = (eye.x as i64 >> 8, eye.z as i64 >> 8);
            let (near2, far2) = (((lo - margin).max(0) >> 8).pow(2), (soon >> 8).pow(2));
            for j in j0..=j1 {
                let cz = (j * s + s / 2) >> 8;
                for i in i0..=i1 {
                    let cx = (i * s + s / 2) >> 8;
                    let q = (cx - ex) * (cx - ex) + (cz - ez) * (cz - ez);
                    // neither drawn nor soon: too far, or well inside where
                    // the level before it is drawn
                    if q > far2 || q < near2 {
                        continue;
                    }
                    let d = (crate::terrain::root(q as u64) as i64) << 8;
                    let key = Key {
                        kind: SCATTER,
                        level: part,
                        i: i as i32,
                        j: j as i32,
                        part: 0,
                    };
                    match crate::scatter::fade(&sp, part, d) {
                        Some(f) => {
                            if !self.has(&key) && !self.empty.contains(&key) {
                                self.build_scatter(land, key, ink);
                                self.stats.last_sync = Some(key);
                                self.stats.sync_builds += 1;
                                self.stats.total_sync_builds += 1;
                            }
                            self.draw(key, f, out);
                        }
                        // not drawn now, but soon: built ahead, the soonest first
                        None => {
                            let slack = if d < lo { lo - d } else { d - hi };
                            if slack < margin && !self.has(&key) && !self.empty.contains(&key) {
                                ahead.push((slack, key));
                            }
                        }
                    }
                }
            }
        }
        ahead.sort();
        let mut budget = self.spec.builds_per_frame;
        for (_, key) in ahead {
            if budget == 0 {
                break;
            }
            self.build_scatter(land, key, ink);
            budget -= 1;
        }
        // forget what is well out of reach
        let far: Vec<Key> = self
            .by_key
            .keys()
            .chain(self.empty.iter())
            .filter(|k| k.kind == SCATTER)
            .filter(|k| {
                let (cx, cz, _) = crate::scatter::chunk_middle(land, k.level, k.i, k.j);
                let d = to_square(eye.x as i64, eye.z as i64, cx as i64, cz as i64, 0);
                let (lo, hi) = crate::scatter::interval(&sp, k.level);
                let m = 2 * crate::scatter::margin(&sp, k.level);
                d > hi as i64 * ONE as i64 + m || d < lo as i64 * ONE as i64 - m
            })
            .copied()
            .collect();
        for k in far {
            self.forget(k);
            self.empty.remove(&k);
        }
        // a chunk's things are kept while any of its tree parts is
        let keep: std::collections::BTreeSet<(i32, i32)> = self
            .by_key
            .keys()
            .filter(|k| k.kind == SCATTER && k.level != crate::scatter::GRASS)
            .map(|k| (k.i, k.j))
            .collect();
        self.things.retain(|ij, _| keep.contains(ij));
    }

    /// The mesh table with every detail mesh kept so far after it, at its
    /// id: what the flicker scan reads (only under test, where meshes are kept).
    pub fn scan_table(&self, base: &crate::prim::MeshTable) -> crate::prim::MeshTable {
        let mut t = base.clone();
        let top = self.kept.keys().next_back().copied().unwrap_or(0);
        for id in self.first..=top {
            t.world.push(self.kept.get(&id).cloned().unwrap_or_default());
        }
        t
    }

    /// Takes the places' groups (`places::FADED`) out of the static ones to
    /// draw by distance; returns the rest.
    pub fn take_faded(&mut self, groups: Vec<Group>) -> Vec<Group> {
        let mut rest = Vec::new();
        for g in groups {
            if g.name.starts_with(crate::places::FADED) {
                self.faded.push(Faded {
                    name: g.name.clone(),
                    centre: g.centre,
                    shapes: g.shapes.list,
                    fade: crate::places::PROPS_TO,
                    far: false,
                });
            } else {
                rest.push(g);
            }
        }
        rest
    }

    /// The water's pieces, built once into slots of their own.
    fn build_water(&mut self, land: &Land) {
        for (n, p) in crate::water::pieces(land).into_iter().enumerate() {
            let key = Key {
                kind: WATER,
                level: 0,
                i: n as i32,
                j: 0,
                part: 0,
            };
            self.put(
                key,
                Built {
                    vertices: p.vertices,
                    at: p.at,
                    pass: Pass::Water,
                },
            );
            let id = self.id_of(&key).unwrap_or(0);
            self.faded.push(Faded {
                name: p.name,
                centre: p.centre,
                shapes: vec![Shape {
                    mesh: MeshId(id),
                    at: p.at,
                    rgb: [ONE; 3],
                    value: ONE,
                    pass: Pass::Water,
                }],
                fade: p.fade,
                far: p.far,
            });
        }
        self.water_built = true;
    }

    /// The faded groups' share of a frame.
    pub fn faded_frame(&mut self, eye: V3, out: &mut Shapes) {
        for (n, f) in self.faded.iter().enumerate() {
            let d = to_square(eye.x as i64, eye.z as i64, f.centre.x as i64, f.centre.z as i64, 0);
            let (a, b) = (f.fade.0 as i64, f.fade.1 as i64);
            // t: the share of the way from near to far
            let t = ((d - a) * ONE as i64 / (b - a).max(1)).clamp(0, ONE as i64) as Fx;
            let (coming, going) = look::hand_over(t);
            let fade = if f.far { coming } else if t == 0 { ONE } else { going };
            // nothing to draw: a far piece not yet coming in, a near one gone
            if (f.far && t == 0) || (!f.far && t == ONE) || (f.far && d > FAR as i64) {
                continue;
            }
            self.drawn.push(Drawn {
                key: Key {
                    kind: FADED,
                    level: 0,
                    i: n as i32,
                    j: 0,
                    part: 0,
                },
                fade,
            });
            for s in &f.shapes {
                let mut s = *s;
                if fade != ONE {
                    // the share of pixels kept: a fade f ≥ 0 keeps f, below 0 keeps −f
                    let keep = if fade >= 0 { fade } else { -fade };
                    s = match s.pass {
                        Pass::Solid => Shape {
                            pass: Pass::Dither,
                            value: fade,
                            ..s
                        },
                        Pass::Dither | Pass::Ink | Pass::Water => Shape { value: fade, ..s },
                        Pass::Shade | Pass::Glow => Shape {
                            value: fx::mul(s.value, keep),
                            ..s
                        },
                        Pass::Sky => s,
                    };
                }
                out.push(s);
                self.stats.drawn += 1;
            }
        }
    }

    pub fn tile_side(&self, level: u32) -> i64 {
        cell(level) as i64 * self.spec.tile_cells as i64
    }

    pub fn top(&self) -> u32 {
        self.spec.levels - 1
    }

    /// The share of the way a tile of `level` (above 0) is split into its
    /// children, when the camera is `d` from it: none outside the children's
    /// ring, all inside its band.
    pub fn split(&self, level: u32, d: i64) -> Fx {
        let r = ring(level - 1);
        let inner = r * (100 - self.spec.band_pct as i64) / 100;
        if d <= inner {
            ONE
        } else if d >= r {
            0
        } else {
            ((r - d) * ONE as i64 / (r - inner)) as Fx
        }
    }

    fn put(&mut self, key: Key, b: Built) {
        let slot = Slot {
            key,
            at: b.at,
            pass: b.pass,
            rgb: [ONE; 3],
            vertices: b.vertices.len() / crate::prim::VERTEX_VALUES,
        };
        self.stats.resident_triangles += slot.vertices / 3;
        let index = match self.free.pop() {
            Some(k) => {
                self.slots[k] = Some(slot);
                k
            }
            None => {
                self.slots.push(Some(slot));
                self.slots.len() - 1
            }
        };
        self.by_key.insert(key, index);
        let id = self.first + index as u32;
        if cfg!(test) {
            self.kept.insert(id, b.vertices.clone());
        }
        // a slot taken again replaces what the page still had for it
        self.uploads.retain(|u| u.0 != id);
        self.uploads.push((id, b.vertices));
    }

    fn forget(&mut self, key: Key) {
        if let Some(index) = self.by_key.remove(&key) {
            if let Some(s) = self.slots[index].take() {
                self.stats.resident_triangles -= s.vertices / 3;
            }
            self.free.push(index);
            self.stats.evictions += 1;
        }
    }

    pub fn has(&self, key: &Key) -> bool {
        self.by_key.contains_key(key)
    }

    fn tile_key(level: u32, i: i32, j: i32) -> Key {
        Key {
            kind: TERRAIN,
            level: level as u8,
            i,
            j,
            part: 0,
        }
    }

    fn build_tile(&mut self, land: &Land, key: Key) {
        let t = land.tile(key.level as u32, key.i, key.j);
        self.put(
            key,
            Built {
                vertices: t.vertices,
                at: t.at,
                pass: Pass::Dither,
            },
        );
        self.stats.builds += 1;
        self.stats.total_builds += 1;
    }

    /// Builds what the walk needs now, if it is not built yet.
    fn need(&mut self, land: &Land, key: Key) {
        if !self.has(&key) {
            self.build_tile(land, key);
            self.stats.last_sync = Some(key);
            self.stats.sync_builds += 1;
            self.stats.total_sync_builds += 1;
        }
        self.wanted.insert(key, self.frame);
    }

    fn draw(&mut self, key: Key, fade: Fx, out: &mut Shapes) {
        let Some(&index) = self.by_key.get(&key) else {
            return;
        };
        let Some(s) = &self.slots[index] else {
            return;
        };
        out.push(Shape {
            mesh: MeshId(self.first + index as u32),
            at: s.at,
            rgb: s.rgb,
            value: fade,
            pass: s.pass,
        });
        self.stats.drawn += 1;
        self.stats.triangles += s.vertices / 3;
        self.drawn.push(Drawn { key, fade });
    }

    /// One frame's terrain round the eye: the tiles drawn, with their fades,
    /// pushed into `out` in the walk's order; the tiles to build ahead built,
    /// a few; the far ones forgotten.
    pub fn frame(&mut self, w: &crate::world::World, look: &crate::look::Look, eye: V3, out: &mut Shapes) {
        if !self.water_built {
            self.build_water(&w.land);
        }
        self.terrain(&w.land, eye, out);
        self.scatter_frame(&w.land, eye, look.ink(), out);
        self.faded_frame(eye, out);
    }

    /// The terrain's part of a frame.
    pub fn terrain(&mut self, land: &Land, eye: V3, out: &mut Shapes) {
        self.frame += 1;
        self.stats.builds = 0;
        self.stats.sync_builds = 0;
        self.stats.drawn = 0;
        self.stats.triangles = 0;
        self.drawn.clear();
        let mut ahead: Vec<(i64, Key)> = Vec::new();
        let top = self.top();
        let side = self.tile_side(top);
        let [lo, hi] = self.spec.extent_m;
        let (i0, i1) = (
            (lo as i64 * ONE as i64).div_euclid(side) as i32,
            (hi as i64 * ONE as i64 - 1).div_euclid(side) as i32,
        );
        for j in i0..=i1 {
            for i in i0..=i1 {
                // a top tile soon in range is built ahead too: it has no parent to ask
                let d = to_square(eye.x as i64, eye.z as i64, i as i64 * side, j as i64 * side, side);
                let key = Detail::tile_key(top, i, j);
                if d > FAR as i64 && d < FAR as i64 + side && !self.has(&key) {
                    ahead.push((d - FAR as i64, key));
                }
                self.walk(land, eye, top, i, j, ONE, &mut ahead, out);
            }
        }
        // build ahead: the nearest few of the tiles asked for
        ahead.sort();
        ahead.dedup();
        let mut budget = self.spec.builds_per_frame;
        for (_, key) in &ahead {
            self.wanted.insert(*key, self.frame);
            if budget > 0 && !self.has(key) {
                self.build_tile(land, *key);
                budget -= 1;
            }
        }
        // forget what was neither drawn nor asked for this frame, once it is
        // well outside its ring
        let stale: Vec<Key> = self
            .by_key
            .keys()
            .filter(|k| k.kind == TERRAIN && self.wanted.get(k).copied() != Some(self.frame))
            .copied()
            .collect();
        for key in stale {
            if self.beyond(eye, key) {
                self.forget(key);
                self.wanted.remove(&key);
            }
        }
        self.stats.resident = self.by_key.len();
    }

    /// Whether a tile is far enough outside the ring it is drawn in to forget.
    fn beyond(&self, eye: V3, key: Key) -> bool {
        let level = key.level as u32;
        let side = self.tile_side(level);
        let d = to_square(eye.x as i64, eye.z as i64, key.i as i64 * side, key.j as i64 * side, side);
        if level >= self.top() {
            return d > FAR as i64 * 2;
        }
        // a tile is drawn while its parent splits: inside its own ring
        d > ring(level) * self.spec.evict_pct as i64 / 100
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        &mut self,
        land: &Land,
        eye: V3,
        level: u32,
        i: i32,
        j: i32,
        fade_in: Fx,
        ahead: &mut Vec<(i64, Key)>,
        out: &mut Shapes,
    ) {
        let side = self.tile_side(level);
        let (x0, z0) = (i as i64 * side, j as i64 * side);
        if to_square(eye.x as i64, eye.z as i64, x0, z0, side) > FAR as i64 {
            return;
        }
        // splitting is measured to the tile's middle (see the module doc)
        let d = to_square(eye.x as i64, eye.z as i64, x0 + side / 2, z0 + side / 2, 0);
        let key = Detail::tile_key(level, i, j);
        self.need(land, key);
        if level == 0 {
            self.draw(key, fade_in, out);
            return;
        }
        let children = [(2 * i, 2 * j), (2 * i + 1, 2 * j), (2 * i, 2 * j + 1), (2 * i + 1, 2 * j + 1)];
        if d < ring(level - 1) * self.spec.prefetch_pct as i64 / 100 {
            for &(ci, cj) in &children {
                ahead.push((d, Detail::tile_key(level - 1, ci, cj)));
            }
        }
        // a tile coming in is never itself splitting (see the module doc);
        // should it be, it stays whole until its own fade is done
        let s = if fade_in < ONE { 0 } else { self.split(level, d) };
        if s == 0 {
            self.draw(key, fade_in, out);
            return;
        }
        let (coming, going) = look::hand_over(s);
        if s < ONE {
            self.draw(key, going, out);
        }
        for (ci, cj) in children {
            self.walk(land, eye, level - 1, ci, cj, coming, ahead, out);
        }
    }

    /// The meshes built since the page last asked, as (id, vertex count)
    /// pairs; their vertices are lent to the page (`lent`) until it asks
    /// again.
    pub fn uploads(&mut self) -> Vec<u32> {
        self.lent = std::mem::take(&mut self.uploads);
        self.lent
            .iter()
            .flat_map(|(id, v)| [*id, (v.len() / crate::prim::VERTEX_VALUES) as u32])
            .collect()
    }

    /// A mesh the page is reading.
    pub fn lent(&self, id: u32) -> Option<&Vec<i32>> {
        self.lent.iter().find(|u| u.0 == id).map(|u| &u.1)
    }

    /// A slot's numbers by its key, for the tests and the probe.
    pub fn slot(&self, key: &Key) -> Option<&Slot> {
        self.by_key.get(key).and_then(|&k| self.slots[k].as_ref())
    }

    pub fn id_of(&self, key: &Key) -> Option<u32> {
        self.by_key.get(key).map(|&k| self.first + k as u32)
    }

    /// Resident triangles and meshes by kind and level, for the probe.
    pub fn resident_by(&self) -> BTreeMap<(u8, u8), (usize, usize)> {
        let mut m = BTreeMap::new();
        for s in self.slots.iter().flatten() {
            let e = m.entry((s.key.kind, s.key.level)).or_insert((0, 0));
            e.0 += 1;
            e.1 += s.vertices / 3;
        }
        m
    }

    /// The meshes not yet handed to the page.
    pub fn pending(&self) -> usize {
        self.uploads.len()
    }
}

/// The affine that draws a mesh built about `centre` with x and z divided
/// by `k`.
pub fn placed(centre: V3, k: Fx) -> Affine {
    Affine::translate(centre).then(&Affine::scale(v3(k, ONE, k)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::int;
    use crate::terrain::tests::land;

    fn detail() -> Detail {
        Detail::new(&land().spec.lod, 100)
    }

    #[test]
    fn the_rings_double_from_256_m_to_the_horizon() {
        // level 0's 2 m cells to 256 m, each level twice as far: 512, 1024,
        // …, level 6's 128 m cells to 16 384 m (PLAN.md, render::rings)
        assert_eq!(ring(0), int(256) as i64);
        assert_eq!(ring(6), int(16_384) as i64);
        let r: Vec<(i64, Fx)> = (0..7).map(|l| (ring(l), cell(l))).collect();
        let want: Vec<(i64, Fx)> = crate::render::rings()
            .into_iter()
            .skip(2)
            .map(|(d, c)| (d as i64, c))
            .collect();
        assert_eq!(r, want);
        assert_eq!(unit_for(int(45) as i64), int(64));
        assert_eq!(unit_for(int(64) as i64), int(64));
    }

    #[test]
    fn a_tile_splits_over_the_outer_fifteen_percent_of_its_children_s_ring() {
        let d = detail();
        // level 1 tiles split into level 0's, whose ring is 256 m: whole
        // inside 217.6 m (85%), not at all past 256 m, half at 236.8 m
        assert_eq!(d.split(1, int(200) as i64), ONE);
        assert_eq!(d.split(1, int(256) as i64), 0);
        let half = d.split(1, (int(2368) / 10) as i64);
        assert!((half - ONE / 2).abs() <= 2, "{half}");
    }

    /// Every point of the land is drawn once: the tiles drawn at fade ONE,
    /// and the hand-over pairs, cover the square round the eye exactly, a
    /// coming-in fade's share and the going-out tile's share adding to one.
    #[test]
    fn the_tiles_drawn_cover_the_land_once_at_every_point() {
        let l = land();
        let mut d = detail();
        let eye = v3(int(1234), int(3), int(2345));
        let mut out = Shapes::default();
        d.terrain(&l, eye, &mut out);
        // the coverage of a tile: a fade f ≥ 0 keeps f of the pixels, a
        // fade f < 0 keeps −f of them
        let cover = |f: Fx| if f >= 0 { f as i64 } else { -(f as i64) };
        // sample points in a 3 km square: the cover of every tile over each
        // adds to ONE
        for (x, z) in [(1234, 2345), (1500, 2300), (1000, 2100), (1460, 2345), (2600, 2900), (-300, 4100)] {
            let (px, pz) = (int(x) as i64, int(z) as i64);
            let mut sum = 0;
            for dr in &d.drawn {
                let side = d.tile_side(dr.key.level as u32);
                let (x0, z0) = (dr.key.i as i64 * side, dr.key.j as i64 * side);
                if px >= x0 && px < x0 + side && pz >= z0 && pz < z0 + side {
                    sum += cover(dr.fade);
                }
            }
            assert_eq!(sum, ONE as i64, "at ({x}, {z})");
        }
        assert_eq!(out.len(), d.drawn.len());
        assert!(d.stats.sync_builds > 0, "the first frame builds what it draws");
    }

    /// Near the eye the cells are 2 m: a level 1 tile is whole split inside
    /// 85% of level 0's ring (217.6 m) to its middle, and holds points 90.5 m
    /// from its middle (half its 128 m diagonal), so within 127 m every point
    /// is drawn by a level 0 tile alone.
    #[test]
    fn two_metre_cells_reach_out_to_the_first_ring() {
        let l = land();
        let mut d = detail();
        let eye = v3(int(2000), int(2), int(2800));
        let mut out = Shapes::default();
        d.terrain(&l, eye, &mut out);
        for (dx, dz) in [(0, 0), (126, 0), (0, -126), (-89, 89)] {
            let (px, pz) = (int(2000 + dx) as i64, int(2800 + dz) as i64);
            let levels: Vec<(u8, Fx)> = d
                .drawn
                .iter()
                .filter(|dr| {
                    let side = d.tile_side(dr.key.level as u32);
                    let (x0, z0) = (dr.key.i as i64 * side, dr.key.j as i64 * side);
                    px >= x0 && px < x0 + side && pz >= z0 && pz < z0 + side
                })
                .map(|dr| (dr.key.level, dr.fade))
                .collect();
            assert_eq!(levels, vec![(0, ONE)], "({dx}, {dz})");
        }
    }

    #[test]
    fn a_moving_camera_builds_ahead_and_never_waits() {
        let l = land();
        let mut d = detail();
        let mut out = Shapes::default();
        let mut eye = v3(int(1000), int(2), int(3000));
        d.terrain(&l, eye, &mut out);
        // 6 m/s east for a minute, a frame a tick: 0.1 m a frame
        for _ in 0..3600 {
            eye.x += ONE / 10;
            out = Shapes::default();
            d.terrain(&l, eye, &mut out);
            assert_eq!(d.stats.sync_builds, 0, "a tile was needed before it was built, at {}", eye.x / ONE);
            assert!(d.stats.builds <= d.spec.builds_per_frame);
        }
    }

    /// CW-18, PLAN.md flicker test 3: on camera paths at the fastest speeds
    /// (5 m/s running, 6 m/s with speedster pieces, a frame a tick), every
    /// surface the detail levels draw (terrain tiles, the scatter's parts,
    /// the places and the water) changes its share of the pixels by at most
    /// one cell of the 4 × 4 dither (1/16) a frame: none appears, vanishes
    /// or changes level except through the dither band, and none waits to
    /// be built.
    #[test]
    fn no_surface_changes_level_without_passing_through_the_dither_band() {
        let w = crate::world::World::new(1);
        let look = crate::look::tests::look();
        let cover = |f: Fx| if f >= 0 { f } else { -f };
        // (start, metres a tick east, metres a tick south, ticks)
        let paths = [
            // running north from the village up the lane and across the road
            ((2000, 2790), 0, -ONE / 12, 4000),
            // with speedster pieces, west over the river by the bridge
            ((900, 2300), -ONE / 10, -ONE / 100, 3000),
            // into the forest toward the camp
            ((2860, 1900), ONE / 40, -ONE / 10, 3000),
        ];
        for ((x, z), dx, dz, ticks) in paths {
            let mut d = Detail::new(&w.land.spec.lod, 100);
            let mut eye = v3(int(x), 0, int(z));
            let mut last: BTreeMap<Key, Fx> = BTreeMap::new();
            for t in 0..ticks {
                eye.x += dx;
                eye.z += dz;
                eye.y = w.ground(eye.x, eye.z) + int(2);
                let mut out = Shapes::default();
                d.frame(&w, &look, eye, &mut out);
                let now: BTreeMap<Key, Fx> = d.drawn.iter().map(|dr| (dr.key, cover(dr.fade))).collect();
                // a terrain tile wholly past the haze's edge (sky.rs: the land
                // is the sky's own colour there) shows nothing of its own
                let edge = crate::sky::SkySpec::shipped().edge_m[1] as i64 * ONE as i64;
                let hidden = |k: &Key| {
                    k.kind == TERRAIN && {
                        let side = d.tile_side(k.level as u32);
                        to_square(eye.x as i64, eye.z as i64, k.i as i64 * side, k.j as i64 * side, side) >= edge
                    }
                };
                if t > 0 {
                    assert_eq!(d.stats.sync_builds, 0, "{:?} waited to be built at tick {t}, eye {:?}", d.stats.last_sync, (eye.x / ONE, eye.z / ONE));
                    for k in last.keys().chain(now.keys()).filter(|k| !hidden(k)) {
                        let (a, b) = (last.get(k).copied().unwrap_or(0), now.get(k).copied().unwrap_or(0));
                        assert!(
                            (a - b).abs() <= ONE / 16,
                            "{k:?} went from {a} to {b} of ONE at tick {t}, eye {:?}",
                            (eye.x / ONE, eye.z / ONE)
                        );
                    }
                }
                last = now;
            }
        }
    }

    #[test]
    fn the_page_is_handed_each_new_mesh_once() {
        let l = land();
        let mut d = detail();
        let mut out = Shapes::default();
        d.terrain(&l, v3(int(2000), 0, int(2800)), &mut out);
        let ups = d.uploads();
        assert!(ups.len() > 20);
        let (id, count) = (ups[0], ups[1]);
        assert!(id >= 100, "after the static table");
        assert_eq!(d.lent(id).unwrap().len(), count as usize * crate::prim::VERTEX_VALUES);
        // the frames after build ahead, a few at a time, until all is built
        let mut frames = 0;
        loop {
            d.terrain(&l, v3(int(2000), 0, int(2800)), &mut out);
            frames += 1;
            if d.uploads().is_empty() {
                break;
            }
            assert!(d.stats.builds <= d.spec.builds_per_frame);
        }
        assert!(frames > 1);
        // nothing new: nothing handed over, and the last lot is let go
        assert!(d.lent(id).is_none());
        d.terrain(&l, v3(int(2000), 0, int(2800)), &mut out);
        assert!(d.uploads().is_empty());
    }
}
