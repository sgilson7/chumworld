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
    /// the vertices the page holds, and the triangles they make
    pub vertices: usize,
    pub triangles: usize,
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
    /// M7: the vertices and indices the page holds for them
    pub resident_vertices: usize,
    pub resident_indices: usize,
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
    uploads: Vec<(u32, Vec<i32>, Vec<u32>)>,
    /// what the page is reading now
    lent: Vec<(u32, Vec<i32>, Vec<u32>)>,
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
    /// scatter frames since the start, for the sweep of the far chunks
    sweep: u64,
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

/// A scatter part this near being drawn is built at once (Q16 metres, i64):
/// more than a fast run's frame (14 m/s, 0.23 m) and some.
pub const NEAR_SLACK: i64 = 2 * ONE as i64;

/// How near the player every grass tuft, tree and rock is drawn whole, Q16
/// metres (the person's "immediate viewable area", R9.44).
pub const NEAR: Fx = 25 * ONE;

/// The nearest and farthest flat distances from the eye to the square from
/// (x0, z0), `side` wide, Q16 metres.
pub fn reach_of(eye: V3, x0: i64, z0: i64, side: i64) -> (i64, i64) {
    let (x, z) = (eye.x as i64, eye.z as i64);
    let near = to_square(x, z, x0, z0, side);
    let dx = (x - x0).abs().max((x - x0 - side).abs());
    let dz = (z - z0).abs().max((z - z0 - side).abs());
    (near, crate::terrain::root((dx * dx + dz * dz) as u64) as i64)
}

/// The flat distance from (x, z) to the square from (x0, z0), `side` wide.
fn to_square(x: i64, z: i64, x0: i64, z0: i64, side: i64) -> i64 {
    let dx = (x0 - x).max(x - (x0 + side)).max(0);
    let dz = (z0 - z).max(z - (z0 + side)).max(0);
    crate::terrain::root((dx * dx + dz * dz) as u64) as i64
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
            sweep: 0,
        }
    }

    /// Builds a scatter chunk's part, or notes it has nothing.
    fn build_scatter(&mut self, land: &Land, key: Key, ink: look::Rgb) {
        let (i, j) = (key.i, key.j);
        let things = if matches!(
            key.level,
            crate::scatter::GRASS
                | crate::scatter::CLUSTER
                | crate::scatter::TUFT
                | crate::scatter::ROAD
                | crate::scatter::MEADOW
        ) {
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

    /// The scatter's share of a frame (R9.44): each part of each chunk
    /// whose shapes can be kept anywhere (its square, grown by how far its
    /// shapes reach, nearer than the band's end and farther than its start),
    /// drawn with its band's value so the shader fades each triangle by its
    /// own distance; the parts soon in reach built ahead, a few a frame, in
    /// every direction alike; the far ones forgotten.
    pub fn scatter_frame(&mut self, land: &Land, eye: V3, ink: look::Rgb, out: &mut Shapes) {
        let sp = land.spec.scatter.clone();
        let mut ahead: Vec<(i64, Key)> = Vec::new();
        for part in crate::scatter::PARTS {
            let s = crate::scatter::side_of(&sp, part) as i64;
            let b = crate::scatter::band(&sp, part);
            let (lo, hi) = (b[0] as i64 * ONE as i64, b[3] as i64 * ONE as i64);
            let margin = crate::scatter::margin(&sp, part);
            let g = crate::scatter::grown(part);
            let soon = hi + margin + g;
            // within the land the scatter covers
            let [elo, ehi] = sp.extent_m.map(|v| v as i64 * ONE as i64);
            let (c0, c1) = (elo.div_euclid(s), ehi.div_euclid(s));
            let (i0, i1) = (
                (eye.x as i64 - soon).div_euclid(s).max(c0),
                (eye.x as i64 + soon).div_euclid(s).min(c1),
            );
            let (j0, j1) = (
                (eye.z as i64 - soon).div_euclid(s).max(c0),
                (eye.z as i64 + soon).div_euclid(s).min(c1),
            );
            // a quick test first, in 256ths of a metre so squares fit: the
            // chunk's middle against the band and the margin, give or take
            // its half diagonal
            let (ex, ez) = (eye.x as i64 >> 8, eye.z as i64 >> 8);
            let half = (s * 3 / 4 + g) >> 8;
            let outer = ((hi + margin) >> 8) + half;
            let inner = ((lo - margin) >> 8) - half;
            let (outer2, inner2) = (outer * outer, if inner > 0 { inner * inner } else { -1 });
            for j in j0..=j1 {
                let cz = (j * s + s / 2) >> 8;
                for i in i0..=i1 {
                    let cx = (i * s + s / 2) >> 8;
                    let q = (cx - ex) * (cx - ex) + (cz - ez) * (cz - ez);
                    if q > outer2 || q < inner2 {
                        continue;
                    }
                    let (near, far) = reach_of(eye, i * s - g, j * s - g, s + 2 * g);
                    if near > hi + margin || far < lo - margin {
                        continue;
                    }
                    let key = Key {
                        kind: SCATTER,
                        level: part,
                        i: i as i32,
                        j: j as i32,
                        part: 0,
                    };
                    if near < hi && far > lo {
                        if !self.has(&key) && !self.empty.contains(&key) {
                            self.build_scatter(land, key, ink);
                            self.stats.last_sync = Some(key);
                            self.stats.sync_builds += 1;
                            self.stats.total_sync_builds += 1;
                        }
                        self.draw(key, crate::scatter::band_value(part), out);
                    } else if !self.has(&key) && !self.empty.contains(&key) {
                        // not drawn now, but soon: built ahead, the soonest first
                        let slack = if near >= hi { near - hi } else { lo - far };
                        ahead.push((slack, key));
                    }
                }
            }
        }
        ahead.sort();
        let mut budget = self.spec.builds_per_frame;
        for (slack, key) in ahead {
            // a part a few metres from being drawn is built whatever the
            // budget (after a jump, when there are many), so it never waits
            if budget == 0 && slack > NEAR_SLACK {
                break;
            }
            self.build_scatter(land, key, ink);
            budget = budget.saturating_sub(1);
        }
        // forget what is well out of reach, every sixteenth frame (a scan of
        // every chunk kept, the empty ones too, thousands far off)
        self.sweep += 1;
        if self.sweep % 16 != 1 {
            return;
        }
        let far: Vec<Key> = self
            .by_key
            .keys()
            .chain(self.empty.iter())
            .filter(|k| k.kind == SCATTER)
            .filter(|k| {
                let s = crate::scatter::side_of(&sp, k.level) as i64;
                let g = crate::scatter::grown(k.level);
                let (near, far) = reach_of(eye, k.i as i64 * s - g, k.j as i64 * s - g, s + 2 * g);
                let b = crate::scatter::band(&sp, k.level);
                let m = 2 * crate::scatter::margin(&sp, k.level);
                near > b[3] as i64 * ONE as i64 + m || far < b[0] as i64 * ONE as i64 - m
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
            .filter(|k| {
                k.kind == SCATTER
                    && matches!(
                        k.level,
                        crate::scatter::WHOLE
                            | crate::scatter::WHOLE_INK
                            | crate::scatter::CONE
                            | crate::scatter::ROCK
                            | crate::scatter::ROCK_INK
                    )
            })
            .map(|k| (k.i, k.j))
            .collect();
        self.things.retain(|ij, _| keep.contains(ij));
    }

    /// Of the grass, tree and rock chunks whose square comes within `r` of
    /// `feet`, how many this frame left undrawn (neither drawn nor known to
    /// hold nothing), and how many there were (the probe, `lod.probe`).
    pub fn near_missing(&self, land: &Land, feet: V3, r: Fx) -> (u32, u32) {
        let sp = &land.spec.scatter;
        let drawn: std::collections::BTreeSet<Key> = self
            .drawn
            .iter()
            .filter(|d| d.key.kind == SCATTER)
            .map(|d| d.key)
            .collect();
        let (mut missing, mut looked) = (0, 0);
        for part in [crate::scatter::GRASS, crate::scatter::WHOLE, crate::scatter::ROCK] {
            let s = crate::scatter::side_of(sp, part) as i64;
            let (x, z, r) = (feet.x as i64, feet.z as i64, r as i64);
            for j in (z - r).div_euclid(s)..=(z + r).div_euclid(s) {
                for i in (x - r).div_euclid(s)..=(x + r).div_euclid(s) {
                    if to_square(x, z, i * s, j * s, s) > r {
                        continue;
                    }
                    let key = Key {
                        kind: SCATTER,
                        level: part,
                        i: i as i32,
                        j: j as i32,
                        part: 0,
                    };
                    looked += 1;
                    if !drawn.contains(&key) && !self.empty.contains(&key) {
                        missing += 1;
                    }
                }
            }
        }
        (missing, looked)
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
            let d = to_square(
                eye.x as i64,
                eye.z as i64,
                f.centre.x as i64,
                f.centre.z as i64,
                0,
            );
            let (a, b) = (f.fade.0 as i64, f.fade.1 as i64);
            // t: the share of the way from near to far
            let t = ((d - a) * ONE as i64 / (b - a).max(1)).clamp(0, ONE as i64) as Fx;
            let (coming, going) = look::hand_over(t);
            let fade = if f.far {
                coming
            } else if t == 0 {
                ONE
            } else {
                going
            };
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
        let triangles = b.vertices.len() / crate::prim::VERTEX_VALUES / 3;
        // M7 (R9.52): every mesh is handed over indexed
        let (vertices, indices) = if key.kind == WATER {
            // the water's vertices are shared whole: each carries its own flow
            index_shared(&b.vertices)
        } else {
            // the ground's normals four times as long, the rest twice: the
            // shader shades grass on the ground (R9.54)
            index_flat(&b.vertices, if key.kind == TERRAIN { 4 } else { 2 })
        };
        let slot = Slot {
            key,
            at: b.at,
            pass: b.pass,
            rgb: [ONE; 3],
            vertices: vertices.len() / crate::prim::VERTEX_VALUES,
            triangles,
        };
        self.stats.resident_triangles += slot.triangles;
        self.stats.resident_vertices += slot.vertices;
        self.stats.resident_indices += indices.len();
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
        self.uploads.push((id, vertices, indices));
    }

    fn forget(&mut self, key: Key) {
        if let Some(index) = self.by_key.remove(&key) {
            if let Some(s) = self.slots[index].take() {
                self.stats.resident_triangles -= s.triangles;
                self.stats.resident_vertices -= s.vertices;
                self.stats.resident_indices -= s.triangles * 3;
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
        self.stats.triangles += s.triangles;
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
        let d = to_square(
            eye.x as i64,
            eye.z as i64,
            key.i as i64 * side,
            key.j as i64 * side,
            side,
        );
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
        let children = [
            (2 * i, 2 * j),
            (2 * i + 1, 2 * j),
            (2 * i, 2 * j + 1),
            (2 * i + 1, 2 * j + 1),
        ];
        if d < ring(level - 1) * self.spec.prefetch_pct as i64 / 100 {
            // T18 (R9.423): soonest needed first, by how far the tile is
            // from splitting, as the top tiles are; by `d` alone a coarse
            // tile's children, far off but about to split, waited behind
            // every near fine one, and at 70 m/s one was needed first
            for &(ci, cj) in &children {
                ahead.push((d - ring(level - 1), Detail::tile_key(level - 1, ci, cj)));
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

    /// The meshes built since the page last asked, as (id, vertex count,
    /// index count) triples, the last 0 for a mesh drawn unindexed; their
    /// vertices and indices are lent to the page (`lent`, `lent_indices`)
    /// until it asks again.
    pub fn uploads(&mut self) -> Vec<u32> {
        self.lent = std::mem::take(&mut self.uploads);
        self.lent
            .iter()
            .flat_map(|(id, v, i)| [*id, (v.len() / crate::prim::VERTEX_VALUES) as u32, i.len() as u32])
            .collect()
    }

    /// A mesh the page is reading.
    pub fn lent(&self, id: u32) -> Option<&Vec<i32>> {
        self.lent.iter().find(|u| u.0 == id).map(|u| &u.1)
    }

    /// Its indices, three a triangle (none: drawn as it is).
    pub fn lent_indices(&self, id: u32) -> Option<&Vec<u32>> {
        self.lent.iter().find(|u| u.0 == id).map(|u| &u.2)
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
            e.1 += s.triangles;
        }
        m
    }

    /// The meshes not yet handed to the page.
    pub fn pending(&self) -> usize {
        self.uploads.len()
    }
}

/// How a flat-faceted mesh (each triangle's three vertices alike in normal
/// and colour) is handed to the page indexed (R9.52): a triangle's last
/// vertex, the one WebGL takes a `flat` value from, is its own, carrying
/// the facet's normal and colour; its first two share any vertex already
/// at their place, whatever it carries. So a grid of facets holds about one
/// vertex a triangle instead of three. Each normal is stored `stretch`
/// times as long (2, or the ground's 4), which tells the shader to take the
/// facet's values flat, and the ground from the rest (`scene.vert`).
pub fn index_flat(soup: &[i32], stretch: i32) -> (Vec<i32>, Vec<u32>) {
    let vv = crate::prim::VERTEX_VALUES;
    let mut out: Vec<i32> = Vec::with_capacity(soup.len() / 2);
    let mut indices = Vec::with_capacity(soup.len() / vv);
    let mut at: BTreeMap<(i32, i32, i32), u32> = BTreeMap::new();
    let push = |out: &mut Vec<i32>, v: &[i32]| -> u32 {
        let k = (out.len() / vv) as u32;
        out.extend_from_slice(&v[..3]);
        out.extend(v[3..6].iter().map(|n| n * stretch));
        out.extend_from_slice(&v[6..vv]);
        k
    };
    for tri in soup.chunks(3 * vv) {
        for q in 0..2 {
            let v = &tri[q * vv..(q + 1) * vv];
            let key = (v[0], v[1], v[2]);
            let k = match at.get(&key) {
                Some(&k) => k,
                None => {
                    let k = push(&mut out, v);
                    at.insert(key, k);
                    k
                }
            };
            indices.push(k);
        }
        let v = &tri[2 * vv..3 * vv];
        let k = push(&mut out, v);
        at.entry((v[0], v[1], v[2])).or_insert(k);
        indices.push(k);
    }
    (out, indices)
}

/// A mesh whose triangles share whole vertices (the water's grid), handed
/// over indexed: each distinct vertex once (R9.52).
pub fn index_shared(soup: &[i32]) -> (Vec<i32>, Vec<u32>) {
    let vv = crate::prim::VERTEX_VALUES;
    let mut out: Vec<i32> = Vec::new();
    let mut indices = Vec::with_capacity(soup.len() / vv);
    let mut at: BTreeMap<Vec<i32>, u32> = BTreeMap::new();
    for v in soup.chunks(vv) {
        let k = *at.entry(v.to_vec()).or_insert_with(|| {
            out.extend_from_slice(v);
            (out.len() / vv - 1) as u32
        });
        indices.push(k);
    }
    (out, indices)
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
        // a quarter of the way in, at 227.2 m: (256 − 227.2) / 38.4 = ¾
        let most = d.split(1, (int(2272) / 10) as i64);
        assert!((most - ONE * 3 / 4).abs() <= 2, "{most}");
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
        for (x, z) in [
            (1234, 2345),
            (1500, 2300),
            (1000, 2100),
            (1460, 2345),
            (2600, 2900),
            (-300, 4100),
        ] {
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
            assert_eq!(
                d.stats.sync_builds,
                0,
                "a tile was needed before it was built, at {}",
                eye.x / ONE
            );
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
                // a scatter part fades each triangle by its own distance
                // (R9.44): its share is the most any of its shapes keeps,
                // over the distances its grown square spans
                let sp = &w.land.spec.scatter;
                let share = |dr: &Drawn| {
                    if dr.key.kind != SCATTER || dr.fade != crate::scatter::band_value(dr.key.level) {
                        return cover(dr.fade);
                    }
                    let side = crate::scatter::side_of(sp, dr.key.level) as i64;
                    let g = crate::scatter::grown(dr.key.level);
                    let (near, far) = reach_of(
                        eye,
                        dr.key.i as i64 * side - g,
                        dr.key.j as i64 * side - g,
                        side + 2 * g,
                    );
                    let b = crate::scatter::band(sp, dr.key.level);
                    let at = |d: i64| crate::scatter::kept(crate::scatter::band_fade(b, d));
                    if far < b[1] as i64 * ONE as i64 {
                        at(far)
                    } else if near > b[2] as i64 * ONE as i64 {
                        at(near)
                    } else {
                        ONE
                    }
                };
                let now: BTreeMap<Key, Fx> = d.drawn.iter().map(|dr| (dr.key, share(dr))).collect();
                // a terrain tile wholly past the haze's edge (sky.rs: the land
                // is the sky's own colour there) shows nothing of its own
                let edge = crate::sky::SkySpec::shipped().edge_m[1] as i64 * ONE as i64;
                let hidden = |k: &Key| {
                    k.kind == TERRAIN && {
                        let side = d.tile_side(k.level as u32);
                        to_square(
                            eye.x as i64,
                            eye.z as i64,
                            k.i as i64 * side,
                            k.j as i64 * side,
                            side,
                        ) >= edge
                    }
                };
                if t > 0 {
                    assert_eq!(
                        d.stats.sync_builds,
                        0,
                        "{:?} waited to be built at tick {t}, eye {:?}",
                        d.stats.last_sync,
                        (eye.x / ONE, eye.z / ONE)
                    );
                    for k in last.keys().chain(now.keys()).filter(|k| !hidden(k)) {
                        let (a, b) = (
                            last.get(k).copied().unwrap_or(0),
                            now.get(k).copied().unwrap_or(0),
                        );
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

    /// The person, 2026-10-20: "theres never a moment where all the grass
    /// within the immediate viewable area ... is rendered" (R9.44). At five
    /// places (the green, the meadow, the forest's edge, the river's bank by
    /// the bridge, the camp), a run of two seconds at 7 m/s and then a
    /// whole turn on the spot (the camera 2.6 m behind, so the eye circles),
    /// every frame: every grass tuft, tree and rock within 25 m of the
    /// player is in a part drawn this frame, and kept whole at its distance
    /// from the eye; the set of tufts drawn within 25 m is the full set the
    /// land's generator gives; and after the first frame nothing waits to be
    /// built.
    #[test]
    fn every_tuft_tree_and_rock_within_25_m_is_drawn_whole_every_frame() {
        use crate::scatter::{self, GRASS, ROCK, WHOLE};
        let w = crate::world::World::new(1);
        let look = crate::look::tests::look();
        let l = &w.land;
        let sp = &l.spec.scatter;
        let r = int(25) as i64;
        // (x, z, heading, metres a second): and at a sprint's speeds, 21
        // m/s (three times the run, half way up the build), 35 (its top,
        // five times) and 70 (ten times, the top with five speedster
        // pieces: T18, R9.423)
        let places = [
            (2000, 2790, 0, 7),
            (1500, 2650, 80, 7),
            (2860, 1900, 340, 7),
            (640, 2300, 270, 7),
            (3213, 1010, 200, 7),
            (1500, 2600, 100, 21),
            (1500, 2600, 100, 35),
            (1500, 2600, 100, 70),
        ];
        let mut tufts_seen = 0;
        let mut things_seen = 0;
        for (x, z, yaw, mps) in places {
            let mut d = Detail::new(&l.spec.lod, 100);
            let mut feet = v3(int(x), 0, int(z));
            let mut facing = fx::deg(yaw);
            for t in 0..(120 + 120) {
                if t < 120 {
                    feet = feet.add(crate::geom::facing(facing, 0).scale(ONE * mps / 60));
                } else {
                    facing = fx::wrap(facing + fx::TURN / 120);
                }
                feet.y = w.ground(feet.x, feet.z);
                let eye = feet
                    .sub(crate::geom::facing(facing, 0).scale(ONE * 26 / 10))
                    .add(v3(0, int(2), 0));
                let mut out = Shapes::default();
                d.frame(&w, &look, eye, &mut out);
                if t > 0 {
                    assert_eq!(
                        d.stats.sync_builds, 0,
                        "{:?} waited at ({x}, {z}) at {mps} m/s, frame {t}",
                        d.stats.last_sync
                    );
                }
                let drawn: std::collections::BTreeSet<Key> = d
                    .drawn
                    .iter()
                    .filter(|k| k.key.kind == SCATTER)
                    .map(|k| k.key)
                    .collect();
                let near = |p: V3| v3(p.x - feet.x, 0, p.z - feet.z).len_xz() as i64 <= r;
                let kept = |part: u8, p: V3| {
                    let e = v3(p.x - eye.x, 0, p.z - eye.z).len_xz() as i64;
                    scatter::kept(scatter::band_fade(scatter::band(sp, part), e))
                };
                for part in [GRASS, WHOLE, ROCK] {
                    let side = scatter::side_of(sp, part) as i64;
                    let (i0, i1) = (
                        (feet.x as i64 - r).div_euclid(side),
                        (feet.x as i64 + r).div_euclid(side),
                    );
                    let (j0, j1) = (
                        (feet.z as i64 - r).div_euclid(side),
                        (feet.z as i64 + r).div_euclid(side),
                    );
                    let mut full = Vec::new();
                    let mut shown = Vec::new();
                    for j in j0..=j1 {
                        for i in i0..=i1 {
                            let at: Vec<V3> = if part == GRASS {
                                scatter::tufts(l, GRASS, i as i32, j as i32)
                                    .into_iter()
                                    .map(|t| t.0)
                                    .collect()
                            } else {
                                scatter::items(l, i as i32, j as i32)
                                    .into_iter()
                                    .filter(|it| (it.thing == scatter::Thing::Rock) == (part == ROCK))
                                    .map(|it| it.at)
                                    .collect()
                            };
                            let key = Key {
                                kind: SCATTER,
                                level: part,
                                i: i as i32,
                                j: j as i32,
                                part: 0,
                            };
                            for p in at.into_iter().filter(|&p| near(p)) {
                                assert_eq!(
                                    kept(part, p),
                                    ONE,
                                    "part {part} at {:?} not whole",
                                    (p.x / ONE, p.z / ONE)
                                );
                                full.push(p);
                                if drawn.contains(&key) {
                                    shown.push(p);
                                }
                            }
                        }
                    }
                    if part == GRASS {
                        tufts_seen += full.len();
                    } else {
                        things_seen += full.len();
                    }
                    assert_eq!(
                        shown, full,
                        "part {part} at ({x}, {z}) frame {t}: a chunk within 25 m not drawn"
                    );
                }
                // and the probe the browser check reads says so
                let (missing, looked) = d.near_missing(l, feet, NEAR);
                assert_eq!(missing, 0);
                assert!(looked >= 9, "{looked} chunks looked at");
            }
        }
        // the walks pass through grass and among trees and rocks
        assert!(tufts_seen > 100_000, "{tufts_seen}");
        assert!(things_seen > 1_000, "{things_seen}");
    }

    /// R9.52: two facets sharing an edge, a-b-c red and c-d-a blue, become
    /// five vertices, not six: a, b, c (the first facet's own last, red),
    /// d, then a again as the second facet's own last (blue), its first two
    /// (c and d) shared by place; indices 0 1 2 and 2 3 4. Each triangle's
    /// last vertex carries its own colour; every normal is twice as long,
    /// so the shader knows to take them flat.
    /// T13, R9.53: grass in every tier's range wherever the meadow grows.
    /// Standing on the meadow west of the village, after the detail levels
    /// settle: the blades whole 10 m off, the tufts whole 50 m off, the
    /// meadow's clumps whole 150 m off, each in a part drawn this frame; and
    /// none of them on a road or in the water.
    #[test]
    fn grass_stands_in_every_tier_out_to_the_meadow_s_clumps() {
        use crate::scatter::{self, GRASS, MEADOW, TUFT};
        let w = crate::world::World::new(1);
        let look = crate::look::tests::look();
        let l = &w.land;
        let sp = &l.spec.scatter;
        let mut d = Detail::new(&l.spec.lod, 100);
        let mut eye = v3(int(1500), 0, int(2650));
        eye.y = w.ground(eye.x, eye.z) + int(2);
        for _ in 0..400 {
            let mut out = Shapes::default();
            d.frame(&w, &look, eye, &mut out);
        }
        let drawn: std::collections::BTreeSet<Key> = d.drawn.iter().map(|k| k.key).collect();
        for (part, at) in [(GRASS, 10), (TUFT, 50), (MEADOW, 150)] {
            let side = scatter::side_of(sp, part);
            let mut found = 0;
            for a in 0..16 {
                // a ring round the eye at that distance
                let ang = fx::TURN * a / 16;
                let p = eye.add(v3(
                    fx::mul(fx::cos(ang), int(at)),
                    0,
                    fx::mul(fx::sin(ang), int(at)),
                ));
                let (i, j) = (p.x.div_euclid(side), p.z.div_euclid(side));
                let key = Key {
                    kind: SCATTER,
                    level: part,
                    i,
                    j,
                    part: 0,
                };
                let tufts = scatter::tufts(l, part, i, j);
                if tufts.is_empty() {
                    continue;
                }
                assert!(drawn.contains(&key), "part {part}'s chunk {at} m off not drawn");
                for (t, _) in &tufts {
                    let e = v3(t.x - eye.x, 0, t.z - eye.z).len_xz() as i64;
                    let k = scatter::kept(scatter::band_fade(scatter::band(sp, part), e));
                    let (dist, _) = (e, k);
                    if (dist - int(at) as i64).abs() < int(3) as i64 {
                        assert_eq!(k, ONE, "part {part} at {} m not whole", dist / ONE as i64);
                    }
                    let sc = l.scope_all();
                    let (rd, rh) = l.road_distance(t.x, t.z, &sc);
                    assert!(rh == 0 || rd > rh, "a tuft on a road");
                    assert!(!l.is_water(t.x, t.z), "a tuft in the water");
                }
                found += 1;
            }
            assert!(
                found >= 12,
                "part {part}: grass in {found} of 16 places {at} m off"
            );
        }
    }

    #[test]
    fn two_facets_sharing_an_edge_take_five_vertices_each_its_own_last() {
        let up = [0, ONE, 0];
        let v = |p: [i32; 3], c: [i32; 3]| [p[0], p[1], p[2], up[0], up[1], up[2], c[0], c[1], c[2]];
        let (red, blue) = ([ONE, 0, 0], [0, 0, ONE]);
        let (a, b, c, d) = ([0, 0, 0], [ONE, 0, 0], [ONE, 0, ONE], [0, 0, ONE]);
        let soup: Vec<i32> = [
            v(a, red),
            v(b, red),
            v(c, red),
            v(c, blue),
            v(d, blue),
            v(a, blue),
        ]
        .concat();
        let (verts, idx) = index_flat(&soup, 2);
        assert_eq!(idx, vec![0, 1, 2, 2, 3, 4]);
        assert_eq!(verts.len(), 5 * crate::prim::VERTEX_VALUES);
        let at = |k: usize| &verts[k * crate::prim::VERTEX_VALUES..(k + 1) * crate::prim::VERTEX_VALUES];
        assert_eq!(&at(2)[6..9], &red);
        assert_eq!(&at(4)[6..9], &blue);
        assert_eq!(at(4)[4], 2 * ONE, "the normal twice as long");
    }

    #[test]
    fn the_page_is_handed_each_new_mesh_once() {
        let l = land();
        let mut d = detail();
        let mut out = Shapes::default();
        d.terrain(&l, v3(int(2000), 0, int(2800)), &mut out);
        let ups = d.uploads();
        assert!(ups.len() > 30);
        let (id, count, indices) = (ups[0], ups[1], ups[2]);
        assert!(id >= 100, "after the static table");
        assert_eq!(
            d.lent(id).unwrap().len(),
            count as usize * crate::prim::VERTEX_VALUES
        );
        assert_eq!(d.lent_indices(id).unwrap().len(), indices as usize);
        // the frames after build ahead, a few at a time, until all is built
        let mut frames = 0;
        loop {
            d.terrain(&l, v3(int(2000), 0, int(2800)), &mut out);
            frames += 1;
            if d.uploads().is_empty() {
                break;
            }
            assert!(d.stats.builds <= d.spec.builds_per_frame);
            assert!(frames < 2000, "the page is handed meshes without end");
        }
        assert!(frames > 1);
        // nothing new: nothing handed over, and the last lot is let go
        assert!(d.lent(id).is_none());
        d.terrain(&l, v3(int(2000), 0, int(2800)), &mut out);
        assert!(d.uploads().is_empty());
    }
}
