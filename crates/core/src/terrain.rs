//! `terrain`: the land's heights and colours, from a fixed seed and the
//! layout (M2 World, task W). Heights are whole Q16 metres and a pure
//! function of the point (CW-1, CW-4): `World::ground` answers from here,
//! and the tiles `lod.rs` draws are built from the same function.
//!
//! # The shape
//!
//! The walkable square (4 × 4 km, its origin at the south-west corner of
//! the journal's map, x east, z south) is gentle plains: value noise in four
//! octaves, a slight fall toward the south, and the hills `data/places.json`
//! names (the trainer's hill west of the river among them). The north-east
//! forest stands a little higher. Beyond the square the land rises into
//! ridges, the scenery to the 16 km horizon (CW-Q14), with the river's
//! valley cut through them.
//!
//! Then the layout is pressed into it, in order: the river carves its
//! channel to the water `water.rs` solves (a parabola under the water, a
//! bank that meets it at the same slope, then the valley back up to the
//! land); the places' plateaus (the village at height 0, so the game opens
//! with its feet on the ground at `world::START`) and the village pond; the
//! roads, lanes and trails, flat across their width at the height of their
//! middle, except over the river, where the bridge spans it; and every
//! building's footprint, flat to its middle's height.
//!
//! # The look
//!
//! Protection of the Spheres' faceted ground: each triangle's colour is the
//! mean of its corners', each corner shaded by a seeded unevenness, each
//! facet a little more, and a steep facet turns to rock. Grass in a
//! patchwork of tones with sun-dried patches, a darker floor in the forest,
//! strips of crop round the scarecrow, packed earth round the houses, dirt
//! roads, sand at the water's edge, rock and snow on the far ridges. A thin
//! feature (a road, a bank) is mixed in by how much of a facet it covers,
//! and the patchwork fades level by level, so a far facet's colour does not
//! change from one to the next and shimmer as the view moves (CW-18).
//!
//! # Doors
//!
//! `is_water(w, x, z)` for the map menu (M4) and the footing (M1).

use std::rc::Rc;

use serde::Deserialize;

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, V3};
use crate::look::{mix, rgb, Rgb};
use crate::places::{metres, Layout};
use crate::prim::VERTEX_VALUES;
use crate::water::{Near, River, WaterSpec, Wave};
use crate::world::World;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Octave {
    pub wave_m: i32,
    pub amp_cm: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Sides {
    pub north: i32,
    pub west: i32,
    pub east: i32,
    pub south: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MountainSpec {
    pub rise_m: [i32; 2],
    pub base_m: i32,
    pub ridge_m: Sides,
    pub waves_m: [i32; 3],
    pub gap_m: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ColourSpec {
    pub grass: Vec<[i32; 3]>,
    pub dry: [i32; 3],
    pub floor: [i32; 3],
    pub canopy: [i32; 3],
    pub road: [i32; 3],
    pub road_dark: [i32; 3],
    pub earth: [i32; 3],
    pub sand: [i32; 3],
    pub bed: [i32; 3],
    pub crop: [i32; 3],
    pub furrow: [i32; 3],
    pub rock: [i32; 3],
    pub mountain: [i32; 3],
    pub upland: [i32; 3],
    pub snow: [i32; 3],
    /// M7: the gravel of the river's bars and fords
    pub gravel: [i32; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShadingSpec {
    pub patch_m: i32,
    pub meadow_m: i32,
    pub dry_pct: i32,
    pub corner_pct: i32,
    pub facet_pct: i32,
    pub steep_pct: i32,
    pub patchwork_pct: Vec<i32>,
    pub village_earth_pct: i32,
    pub upland_from_m: i32,
    pub rock_from_m: [i32; 2],
    pub snow_from_m: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LodSpec {
    pub tile_cells: i32,
    pub levels: u32,
    pub band_pct: i32,
    pub prefetch_pct: i32,
    pub evict_pct: i32,
    pub builds_per_frame: usize,
    pub skirt_cm: i32,
    pub river_dip_from_level: u32,
    pub river_dip_cm: i32,
    pub extent_m: [i32; 2],
}

/// `data/land.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LandSpec {
    pub seed: u32,
    pub noise: Vec<Octave>,
    pub tilt_cm_per_km: [i32; 2],
    pub forest_rise_cm: i32,
    pub mountains: MountainSpec,
    pub colour: ColourSpec,
    pub shading: ShadingSpec,
    pub lod: LodSpec,
    pub scatter: crate::scatter::ScatterSpec,
}

impl LandSpec {
    pub fn parse(json: &str) -> Result<LandSpec, String> {
        let s: LandSpec =
            serde_json::from_str(json).map_err(|e| format!("The land file could not be read: {e}"))?;
        if s.colour.grass.is_empty() {
            return Err("The land file has no grass colours.".into());
        }
        if s.shading.patchwork_pct.len() < s.lod.levels as usize {
            return Err("The land file's patchwork needs a share for every level.".into());
        }
        if s.lod.tile_cells < 2 || s.lod.levels == 0 {
            return Err("The land file's tiles need two cells and a level at least.".into());
        }
        Ok(s)
    }
}

// ---- integer helpers, Q16 in i64 ----

const O: i64 = ONE as i64;

fn qmul(a: i64, b: i64) -> i64 {
    (a * b) >> 16
}

/// 0 to ONE between `e0` and `e1`, easing in and out.
fn sstep(e0: i64, e1: i64, x: i64) -> i64 {
    if e1 <= e0 {
        return if x >= e1 { O } else { 0 };
    }
    let t = ((x - e0) * O / (e1 - e0)).clamp(0, O);
    qmul(qmul(t, t), 3 * O - 2 * t)
}

fn lerp(a: i64, b: i64, t: i64) -> i64 {
    a + qmul(b - a, t)
}

/// The integer square root, rounded down: Newton's method from a power of
/// two just above the root, a few steps (`fx::isqrt_u64` starts from `n`
/// itself, dozens of steps for a large one).
pub fn root(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let bits = 64 - n.leading_zeros();
    let mut x = 1u64 << bits.div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

fn hypot(dx: i64, dz: i64) -> i64 {
    root((dx * dx + dz * dz) as u64) as i64
}

/// A whole-number hash of a lattice point and a seed.
pub fn hash(x: i32, z: i32, seed: u32) -> u32 {
    let mut h = seed ^ (x as u32).wrapping_mul(0x9E37_79B1) ^ (z as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    h
}

/// −ONE to ONE at a lattice point.
fn lattice(x: i64, z: i64, seed: u32) -> i64 {
    (hash(x as i32, z as i32, seed) >> 15) as i64 - O
}

/// The quintic ease t³(t(6t − 15) + 10), Q16.
fn fade(t: i64) -> i64 {
    let t3 = qmul(qmul(t, t), t);
    qmul(t3, qmul(t, 6 * t - 15 * O) + 10 * O)
}

/// Value noise at (x, z), Q16 metres: a lattice every `wave_m` metres,
/// eased between, −ONE to ONE.
pub fn noise(x: Fx, z: Fx, wave_m: i32, seed: u32) -> i64 {
    let w = wave_m.max(1) as i64;
    let (cx, cz) = (x as i64 / w, z as i64 / w);
    let (ix, iz) = (cx.div_euclid(O), cz.div_euclid(O));
    let (u, v) = (fade(cx.rem_euclid(O)), fade(cz.rem_euclid(O)));
    let a = lattice(ix, iz, seed);
    let b = lattice(ix + 1, iz, seed);
    let c = lattice(ix, iz + 1, seed);
    let d = lattice(ix + 1, iz + 1, seed);
    lerp(lerp(a, b, u), lerp(c, d, u), v)
}

/// The same, 0 to ONE.
fn noise01(x: Fx, z: Fx, wave_m: i32, seed: u32) -> i64 {
    (noise(x, z, wave_m, seed) + O) / 2
}

// ---- the layout's features, ready to measure against ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Seg {
    a: (i64, i64),
    b: (i64, i64),
    half: i64,
    /// it passes over the river (the bridge's span): measured against it
    wet: bool,
    /// the bounding box, grown by the half width and the shoulder
    lo: (i64, i64),
    hi: (i64, i64),
    /// M7: which road it is part of, and how far along that road it starts
    /// and how long it is (Q16 metres): where on the road's smoothed
    /// profile it lies (`Land::profiles`)
    road: usize,
    s0: i64,
    len: i64,
}

impl Seg {
    /// The distance to the segment and its nearest point.
    fn nearest(&self, x: i64, z: i64) -> (i64, (i64, i64)) {
        let (d, p, _) = self.nearest_t(x, z);
        (d, p)
    }

    /// The same, and how far along it (Q16 share).
    fn nearest_t(&self, x: i64, z: i64) -> (i64, (i64, i64), i64) {
        let (dx, dz) = (self.b.0 - self.a.0, self.b.1 - self.a.1);
        let l2 = ((dx >> 8) * (dx >> 8) + (dz >> 8) * (dz >> 8)).max(1);
        let dot = ((x - self.a.0) >> 8) * (dx >> 8) + ((z - self.a.1) >> 8) * (dz >> 8);
        let t = (dot * O / l2).clamp(0, O);
        let p = (self.a.0 + dx * t / O, self.a.1 + dz * t / O);
        (hypot(x - p.0, z - p.1), p, t)
    }
}

/// A rectangle flattened to `base`: a building's footprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Flat {
    c: (i64, i64),
    hx: i64,
    hz: i64,
    cos: i64,
    sin: i64,
    base: i64,
    reach: i64,
}

impl Flat {
    /// How far outside the rectangle (0 inside).
    fn outside(&self, x: i64, z: i64) -> i64 {
        let (dx, dz) = (x - self.c.0, z - self.c.1);
        // into the prop's own axes: undo its turn (rot_y)
        let lx = qmul(dx, self.cos) - qmul(dz, self.sin);
        let lz = qmul(dx, self.sin) + qmul(dz, self.cos);
        let qx = (lx.abs() - self.hx).max(0);
        let qz = (lz.abs() - self.hz).max(0);
        hypot(qx, qz)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Plateau {
    c: (i64, i64),
    r0: i64,
    r1: i64,
    level: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pond {
    c: (i64, i64),
    r: i64,
    depth: i64,
    level: i64,
    bank: i64,
    rise: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Field {
    c: (i64, i64),
    hx: i64,
    hz: i64,
    cos: i64,
    sin: i64,
    strip: i64,
}

/// Which features can reach a box of the ground: what a tile measures
/// against, so a plains tile far from everything costs only its noise.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    roads: Vec<usize>,
    flats: Vec<usize>,
    plateaus: Vec<usize>,
    ponds: Vec<usize>,
    fields: Vec<usize>,
    hills: Vec<usize>,
    clearings: Vec<usize>,
    river: bool,
    mountains: bool,
    forest: bool,
}

/// Colours as Q16.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Paint {
    grass: Vec<Rgb>,
    grass_mean: Rgb,
    dry: Rgb,
    floor: Rgb,
    canopy: Rgb,
    road: Rgb,
    road_dark: Rgb,
    earth: Rgb,
    sand: Rgb,
    bed: Rgb,
    crop: Rgb,
    furrow: Rgb,
    rock: Rgb,
    mountain: Rgb,
    upland: Rgb,
    snow: Rgb,
    gravel: Rgb,
}

/// The land: its numbers, its layout, its river, and the layout's features
/// measured once.
#[derive(Clone)]
pub struct Land {
    pub spec: LandSpec,
    pub layout: Layout,
    pub river: River,
    /// the water's waves and look (`data/water.json`), and the waves ready to sum
    pub waters: WaterSpec,
    pub waves: Vec<Wave>,
    paint: Paint,
    roads: Vec<Seg>,
    flats: Vec<Flat>,
    plateaus: Vec<Plateau>,
    ponds: Vec<Pond>,
    fields: Vec<Field>,
    /// M7: each road's height along it, a station every `PROFILE_STEP`,
    /// smoothed (R9.46): what it is flattened to across
    profiles: Vec<Vec<i64>>,
    /// a centre and a radius with no trees
    pub clearings: Vec<(V3, Fx)>,
    /// how far from the river's centreline its valley reaches, at the most
    pub reach: Fx,
    /// the raw land's height at the village centre, taken off everywhere
    zero: i64,
}

impl std::fmt::Debug for Land {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Land(seed {}, {:?})", self.spec.seed, self.river)
    }
}

/// Two lands are the same when they are built from the same numbers.
impl PartialEq for Land {
    fn eq(&self, o: &Land) -> bool {
        self.spec == o.spec && self.layout == o.layout && self.waters == o.waters
    }
}
impl Eq for Land {}

/// The road's shoulder: how far past its edge it blends back to the land.
const SHOULDER: i64 = 3 * O;
/// A road's profile: a station this often along it, and the running mean's
/// reach either way, in stations (M7, R9.46).
const PROFILE_STEP: i64 = 2 * O;
const PROFILE_REACH: usize = 8;
/// A road's crown: its middle stands its half width over this over its edges.
const CROWN: i64 = 40;
/// How far the bank and its blend can reach past the water's edge, beyond
/// which the river cannot change the land.
const BANK_MAX: i64 = 30 * O;

impl Land {
    /// The land the game ships with.
    pub fn shipped() -> Land {
        Land::new(
            LandSpec::parse(include_str!("../../../data/land.json")).expect("data/land.json"),
            Layout::shipped(),
            WaterSpec::shipped(),
        )
    }

    pub fn new(spec: LandSpec, layout: Layout, waters: WaterSpec) -> Land {
        let c = &spec.colour;
        let grass: Vec<Rgb> = c.grass.iter().map(|&g| rgb(g)).collect();
        let n = grass.len() as i32;
        let grass_mean = [0, 1, 2].map(|k| grass.iter().map(|g| g[k]).sum::<Fx>() / n);
        let paint = Paint {
            grass,
            grass_mean,
            dry: rgb(c.dry),
            floor: rgb(c.floor),
            canopy: rgb(c.canopy),
            road: rgb(c.road),
            road_dark: rgb(c.road_dark),
            earth: rgb(c.earth),
            sand: rgb(c.sand),
            bed: rgb(c.bed),
            crop: rgb(c.crop),
            furrow: rgb(c.furrow),
            rock: rgb(c.rock),
            mountain: rgb(c.mountain),
            upland: rgb(c.upland),
            snow: rgb(c.snow),
            gravel: rgb(c.gravel),
        };
        let river = River::new(&layout.river);
        let reach = river.stations.iter().map(|s| s.half).max().unwrap_or(0)
            + BANK_MAX as Fx
            + layout.river.valley_m * ONE;
        let waves = waters
            .waves
            .iter()
            .map(|w| Wave::new(w, waters.waves.len()))
            .collect();
        let mut land = Land {
            spec,
            layout,
            river,
            waters,
            waves,
            paint,
            roads: Vec::new(),
            flats: Vec::new(),
            plateaus: Vec::new(),
            ponds: Vec::new(),
            fields: Vec::new(),
            profiles: Vec::new(),
            clearings: Vec::new(),
            reach,
            zero: 0,
        };
        let village = land
            .layout
            .places
            .first()
            .map(|p| metres(p.centre))
            .unwrap_or(V3::ZERO);
        land.zero = land.raw(village.x, village.z, &land.scope_all());
        land.lay_out();
        land
    }

    /// The layout's features, measured in order: each plateau's level from
    /// the land and river under its centre, the ponds from their plateau,
    /// each footprint's base from everything before it.
    fn lay_out(&mut self) {
        let layout = self.layout.clone();
        let m = |v: i32| v as i64 * O;
        for (k, p) in layout.places.iter().enumerate() {
            let c = (m(p.centre[0]), m(p.centre[1]));
            if let Some([r0, r1]) = p.flat_m {
                // the village (the first place) is the plateau at 0
                let level = if k == 0 {
                    0
                } else {
                    self.pre(c.0 as Fx, c.1 as Fx, &self.scope_all())
                };
                self.plateaus.push(Plateau {
                    c,
                    r0: m(r0),
                    r1: m(r1),
                    level,
                });
            }
            if p.clearing_m > 0 {
                self.clearings.push((metres(p.centre), p.clearing_m * ONE));
            }
        }
        for p in &layout.places {
            if let Some(pond) = &p.pond {
                let c = (m(p.centre[0] + pond.at[0]), m(p.centre[1] + pond.at[1]));
                let ground = self.pre(c.0 as Fx, c.1 as Fx, &self.scope_all());
                let level = ground + pond.level_cm as i64 * O / 100;
                let depth = pond.depth_cm as i64 * O / 100;
                let r = m(pond.radius_m);
                let rise = (ground - level).max(O / 10);
                self.ponds.push(Pond {
                    c,
                    r,
                    depth,
                    level,
                    // the bank meets the water at the bowl's own slope
                    bank: rise * r / depth.max(1),
                    rise,
                });
            }
        }
        for (ri, r) in layout.roads.iter().enumerate() {
            let half = r.half_cm as i64 * O / 100;
            let mut s0 = 0;
            for k in 0..r.line.len().saturating_sub(1) {
                let (a, b) = (
                    (m(r.line[k][0]), m(r.line[k][1])),
                    (m(r.line[k + 1][0]), m(r.line[k + 1][1])),
                );
                let g = half + SHOULDER;
                // sampled every 2 m: does it come within the river's banks?
                let len = hypot(b.0 - a.0, b.1 - a.1);
                let wet = (0..=len / (2 * O)).any(|q| {
                    let t = q * 2 * O * O / len.max(1);
                    let (x, z) = (a.0 + qmul(b.0 - a.0, t), a.1 + qmul(b.1 - a.1, t));
                    self.river.near(x as Fx, z as Fx, (BANK_MAX * 2) as Fx).is_some()
                });
                self.roads.push(Seg {
                    a,
                    b,
                    half,
                    wet,
                    lo: (a.0.min(b.0) - g, a.1.min(b.1) - g),
                    hi: (a.0.max(b.0) + g, a.1.max(b.1) + g),
                    road: ri,
                    s0,
                    len,
                });
                s0 += len;
            }
        }
        // each in turn, so a road that joins an earlier one meets it level
        for ri in 0..layout.roads.len() {
            let p = self.profile(ri);
            self.profiles.push(p);
        }
        for f in &layout.fields {
            let yaw = fx::deg(f.yaw_deg);
            self.fields.push(Field {
                c: (m(f.centre[0]), m(f.centre[1])),
                hx: m(f.half_m[0]),
                hz: m(f.half_m[1]),
                cos: fx::cos(yaw) as i64,
                sin: fx::sin(yaw) as i64,
                strip: m(f.strip_m.max(1)),
            });
        }
        let fl = &layout.flatten;
        let reach = m(fl.margin_m + fl.falloff_m);
        for p in &layout.places {
            for q in p.props.iter().filter(|q| !q.inside) {
                let Some(size) = layout.footprints_m.get(&q.prop) else {
                    continue;
                };
                let c = (m(p.centre[0] + q.at[0]), m(p.centre[1] + q.at[1]));
                let yaw = fx::deg(q.yaw_deg);
                // a footprint over the river's water would fill it: the
                // bridge's ends rest on the banks instead
                let all = self.scope_all();
                let base =
                    self.before_flats(c.0 as Fx, c.1 as Fx, &all, self.near(c.0 as Fx, c.1 as Fx, &all));
                self.flats.push(Flat {
                    c,
                    hx: m(size[0]) / 2,
                    hz: m(size[1]) / 2,
                    cos: fx::cos(yaw) as i64,
                    sin: fx::sin(yaw) as i64,
                    base,
                    reach: reach + m(size[0].max(size[1])),
                });
            }
        }
    }

    /// Every feature: for a single point.
    pub fn scope_all(&self) -> Scope {
        Scope {
            roads: (0..self.roads.len()).collect(),
            flats: (0..self.flats.len()).collect(),
            plateaus: (0..self.plateaus.len()).collect(),
            ponds: (0..self.ponds.len()).collect(),
            fields: (0..self.fields.len()).collect(),
            hills: (0..self.layout.hills.len()).collect(),
            clearings: (0..self.clearings.len()).collect(),
            river: true,
            mountains: true,
            forest: true,
        }
    }

    /// The features that can reach the box from (x0, z0) to (x1, z1).
    pub fn scope(&self, x0: Fx, z0: Fx, x1: Fx, z1: Fx) -> Scope {
        let (x0, z0, x1, z1) = (x0 as i64, z0 as i64, x1 as i64, z1 as i64);
        let touches = |lo: (i64, i64), hi: (i64, i64)| lo.0 <= x1 && hi.0 >= x0 && lo.1 <= z1 && hi.1 >= z0;
        let around = |c: (i64, i64), r: i64| touches((c.0 - r, c.1 - r), (c.0 + r, c.1 + r));
        let m = |v: i32| v as i64 * O;
        let forest = &self.layout.forest;
        let fr = m(forest.radii_m[0].max(forest.radii_m[1]) + forest.edge_m * 2);
        let (sq0, sq1) = (0, 4000 * O);
        let rise0 = m(self.spec.mountains.rise_m[0]);
        // the river: the box's middle within its reach of a chord, give or take the box
        let (mx, mz) = ((x0 + x1) / 2, (z0 + z1) / 2);
        let half_diag = hypot(x1 - x0, z1 - z0) / 2;
        let river = self.river.coarse_distance(mx as Fx, mz as Fx) < self.reach as i64 + half_diag + 400 * O;
        Scope {
            roads: (0..self.roads.len())
                .filter(|&k| touches(self.roads[k].lo, self.roads[k].hi))
                .collect(),
            flats: (0..self.flats.len())
                .filter(|&k| around(self.flats[k].c, self.flats[k].reach))
                .collect(),
            plateaus: (0..self.plateaus.len())
                .filter(|&k| around(self.plateaus[k].c, self.plateaus[k].r1))
                .collect(),
            ponds: (0..self.ponds.len())
                .filter(|&k| around(self.ponds[k].c, self.ponds[k].r + self.ponds[k].bank + O))
                .collect(),
            fields: (0..self.fields.len())
                .filter(|&k| {
                    let f = &self.fields[k];
                    around(f.c, f.hx.max(f.hz) * 3 / 2)
                })
                .collect(),
            hills: (0..self.layout.hills.len())
                .filter(|&k| {
                    let h = &self.layout.hills[k];
                    around((m(h.at[0]), m(h.at[1])), m(h.radius_m))
                })
                .collect(),
            clearings: (0..self.clearings.len())
                .filter(|&k| {
                    let (c, r) = self.clearings[k];
                    around((c.x as i64, c.z as i64), r as i64 + 20 * O)
                })
                .collect(),
            river,
            mountains: x0 < sq0 - rise0 || z0 < sq0 - rise0 || x1 > sq1 + rise0 || z1 > sq1 + rise0,
            forest: around((m(forest.centre[0]), m(forest.centre[1])), fr),
        }
    }

    /// How deep in the forest (x, z) is, 0 to ONE: its ellipse with a ragged
    /// edge, and none in a clearing.
    pub fn forest(&self, x: Fx, z: Fx, sc: &Scope) -> i64 {
        if !sc.forest {
            return 0;
        }
        let f = &self.layout.forest;
        let (dx, dz) = (
            x as i64 - f.centre[0] as i64 * O,
            z as i64 - f.centre[1] as i64 * O,
        );
        let (rx, rz) = (f.radii_m[0].max(1) as i64, f.radii_m[1].max(1) as i64);
        // ((dx/rx)² + (dz/rz)²), Q16
        let (u, v) = (dx / rx, dz / rz);
        let e = qmul(u, u) + qmul(v, v);
        let edge = (f.edge_m as i64 * O) / rx.max(rz);
        let ragged = noise(x, z, 170, self.spec.seed ^ 0x0f0e) * edge * 2 / O;
        let mut w = O - sstep(O - edge, O + edge, e + ragged);
        for &k in &sc.clearings {
            let (c, r) = self.clearings[k];
            let d = hypot(x as i64 - c.x as i64, z as i64 - c.z as i64);
            w = qmul(w, sstep(r as i64, r as i64 + 12 * O, d));
        }
        w
    }

    /// How far outside the walkable square (x, z) is.
    fn outside(x: i64, z: i64) -> i64 {
        let e = 4000 * O;
        let dx = (-x).max(x - e).max(0);
        let dz = (-z).max(z - e).max(0);
        hypot(dx, dz)
    }

    /// The ridges beyond the square, 0 to ONE times their height: three
    /// octaves of ridged noise, each finer one riding on the coarser.
    fn ridges(&self, x: Fx, z: Fx) -> i64 {
        let w = &self.spec.mountains.waves_m;
        let s = self.spec.seed;
        let r = |k: usize| {
            let n = O - noise(x, z, w[k], s ^ (0x5151 + k as u32)).abs();
            qmul(n, n)
        };
        let (a, b, c) = (r(0), r(1), r(2));
        (a + qmul(b, a) / 2 + qmul(c, a) / 4) * 4 / 7
    }

    /// The land before the layout is pressed into it: noise, the tilt, the
    /// hills, the forest's rise and the ridges beyond the square.
    fn raw(&self, x: Fx, z: Fx, sc: &Scope) -> i64 {
        let s = &self.spec;
        let mut h: i64 = 0;
        for (k, o) in s.noise.iter().enumerate() {
            let amp = o.amp_cm as i64 * O / 100;
            h += qmul(noise(x, z, o.wave_m, s.seed.wrapping_add(101 * k as u32)), amp);
        }
        // the tilt, measured from the village centre, cm a km
        let (vx, vz) = (2000 * O, 2800 * O);
        h += (x as i64 - vx) * s.tilt_cm_per_km[0] as i64 / 100_000;
        h += (z as i64 - vz) * s.tilt_cm_per_km[1] as i64 / 100_000;
        for &k in &sc.hills {
            let hill = &self.layout.hills[k];
            let (dx, dz) = (x as i64 - hill.at[0] as i64 * O, z as i64 - hill.at[1] as i64 * O);
            let r = hill.radius_m.max(1) as i64;
            let (u, v) = (dx / r, dz / r);
            let q = qmul(u, u) + qmul(v, v);
            if q < O {
                let b = O - q;
                h += qmul(qmul(b, b), hill.height_m as i64 * O);
            }
        }
        h += qmul(self.forest(x, z, sc), s.forest_rise_cm as i64 * O / 100);
        if sc.mountains {
            let m = &s.mountains;
            let out = Land::outside(x as i64, z as i64);
            let rise = sstep(m.rise_m[0] as i64 * O, m.rise_m[1] as i64 * O, out);
            if rise > 0 {
                // each side its own height, by the way out from the square's middle
                let (dx, dz) = (x as i64 - 2000 * O, z as i64 - 2000 * O);
                let sum = (dx.abs() + dz.abs()).max(1);
                let side = |v: i64, a: i32| v.max(0) * a as i64 / sum;
                let r = &m.ridge_m;
                let height = side(-dz, r.north) + side(dz, r.south) + side(dx, r.east) + side(-dx, r.west);
                let gap = sstep(
                    m.gap_m[0] as i64 * O,
                    m.gap_m[1] as i64 * O,
                    self.river.coarse_distance(x, z),
                );
                let ridge = qmul(self.ridges(x, z), height * O);
                h += qmul(qmul(rise, gap), m.base_m as i64 * O + ridge);
            }
        }
        h - self.zero
    }

    /// The river's channel and valley pressed into the land `h0`.
    fn valley(&self, x: Fx, z: Fx, h0: i64, near: Option<Near>) -> i64 {
        let Some(n) = near else {
            return h0;
        };
        let w = n.surface as i64;
        let half = n.half.max(ONE) as i64;
        // a parabola as deep in the middle as gives the solved mean depth
        let deep = (n.depth as i64 * 3 / 2).max(O / 5);
        let (bank, bank_w, _) = self.bank_shape(&n);
        let valley = self.layout.river.valley_m as i64 * O;
        let d = n.d as i64;
        let mut y = if d <= half {
            let u = d * O / half;
            w - qmul(deep, O - qmul(u, u))
        } else if d <= half + bank_w {
            let t = (d - half) * O / bank_w;
            w + qmul(bank, qmul(t, 2 * O - t))
        } else {
            lerp(w + bank, h0, sstep(half + bank_w, half + bank_w + valley, d))
        };
        // the islets: domes up out of the water
        for isl in &self.layout.river.islets {
            let (dx, dz) = (x as i64 - isl.at[0] as i64 * O, z as i64 - isl.at[1] as i64 * O);
            let r = isl.radius_cm as i64 * O / 100;
            let dd = hypot(dx, dz);
            if dd < r {
                let u = dd * O / r;
                let top = isl.top_cm as i64 * O / 100;
                y = y.max(w + top - qmul(top + deep, qmul(u, u)));
            }
        }
        y
    }

    /// The river's bank at `n` (R9.47): how high it rises over the water,
    /// how wide it is, and how much of a gravel bar it is (0 to ONE). Each
    /// bank's height wanders along the river, 60% to 130% of `bank_cm`; on
    /// the inside of a bend it is a low, wide gravel bar, on the outside a
    /// cut bank. Its width is its height over the bowl's own slope at the
    /// edge (`half / deep`), so the bank always meets the water at the
    /// slope the water's bowl does (the depth test, CW-18).
    pub fn bank_shape(&self, n: &Near) -> (i64, i64, i64) {
        let half = n.half.max(ONE) as i64;
        let deep = (n.depth as i64 * 3 / 2).max(O / 5);
        let base = self.layout.river.bank_cm as i64 * O / 100;
        let side: u32 = if n.across >= 0 { 0x6b6b } else { 0x7c7c };
        let wander = noise01(n.s, side as Fx * 64, 160, self.spec.seed ^ side);
        let mut k = O * 60 / 100 + qmul(wander, O * 70 / 100);
        // the inside of a bend: the turn a metre against the side
        let curve = self.river.station(n.s).curve as i64;
        let inner = if n.across >= 0 { curve } else { -curve };
        let bar = sstep(O / 600, O / 150, inner);
        k = qmul(k, O - qmul(bar, O * 6 / 10));
        let bank = qmul(base, k).max(O / 5);
        let bank_w = (bank * half / deep).clamp(2 * O, BANK_MAX / 2);
        (bank, bank_w, bar)
    }

    /// The land with the river and the plateaus and ponds in it: what a road
    /// is flattened to and a footprint is measured on.
    fn pre(&self, x: Fx, z: Fx, sc: &Scope) -> i64 {
        self.pre_near(x, z, sc, self.near(x, z, sc))
    }

    /// The river near (x, z), if the scope reaches it.
    pub fn near(&self, x: Fx, z: Fx, sc: &Scope) -> Option<Near> {
        if sc.river {
            self.river.near(x, z, self.reach)
        } else {
            None
        }
    }

    fn pre_near(&self, x: Fx, z: Fx, sc: &Scope, near: Option<Near>) -> i64 {
        let mut h = self.valley(x, z, self.raw(x, z, sc), near);
        for &k in &sc.plateaus {
            let p = &self.plateaus[k];
            let d = hypot(x as i64 - p.c.0, z as i64 - p.c.1);
            if d < p.r1 {
                h = lerp(h, p.level, O - sstep(p.r0, p.r1, d));
            }
        }
        for &k in &sc.ponds {
            let p = &self.ponds[k];
            let d = hypot(x as i64 - p.c.0, z as i64 - p.c.1);
            if d < p.r {
                let u = d * O / p.r;
                h = h.min(p.level - qmul(p.depth, O - qmul(u, u)));
            } else if d < p.r + p.bank {
                let t = (d - p.r) * O / p.bank.max(1);
                h = h.min(p.level + qmul(p.rise, qmul(t, 2 * O - t)));
            }
        }
        h
    }

    /// Road `ri`'s height along it (R9.46): the land under its line (the
    /// river's valley and the plateaus in it), a station every
    /// `PROFILE_STEP`, smoothed by three passes of a running mean
    /// `PROFILE_REACH` stations either way, so it rises and falls with the
    /// land's swells but carries none of its small bumps and hollows; a
    /// station over the river's banks (where the bridge spans) keeps the
    /// land's own height and takes no part in its neighbours' means.
    fn profile(&self, ri: usize) -> Vec<i64> {
        let segs: Vec<&Seg> = self.roads.iter().filter(|s| s.road == ri).collect();
        let total: i64 = segs.iter().map(|s| s.len).sum();
        let n = (total / PROFILE_STEP) as usize + 2;
        let all = self.scope_all();
        let mut raw = Vec::with_capacity(n);
        let mut wet = Vec::with_capacity(n);
        for k in 0..n {
            let at = (k as i64 * PROFILE_STEP).min(total);
            let s = segs.iter().find(|s| at <= s.s0 + s.len).or(segs.last()).copied();
            let Some(s) = s else {
                raw.push(0);
                wet.push(false);
                continue;
            };
            let t = ((at - s.s0) * O / s.len.max(1)).clamp(0, O);
            let (x, z) = (s.a.0 + (s.b.0 - s.a.0) * t / O, s.a.1 + (s.b.1 - s.a.1) * t / O);
            raw.push(self.pre(x as Fx, z as Fx, &all));
            wet.push(s.wet && self.over_river(x, z));
        }
        // an end that meets an earlier road is pinned to that road's level
        let mut pinned = vec![false; n];
        for k in [0, n - 1] {
            let at = (k as i64 * PROFILE_STEP).min(total);
            let Some(s) = segs.iter().find(|s| at <= s.s0 + s.len).or(segs.last()) else {
                continue;
            };
            let t = ((at - s.s0) * O / s.len.max(1)).clamp(0, O);
            let (x, z) = (s.a.0 + (s.b.0 - s.a.0) * t / O, s.a.1 + (s.b.1 - s.a.1) * t / O);
            let earlier = self
                .roads
                .iter()
                .filter(|o| o.road < ri)
                .map(|o| (o.nearest_t(x, z), o))
                .filter(|((d, _, _), o)| *d <= o.half + O)
                .min_by_key(|((d, _, _), _)| *d);
            if let Some(((_, _, t), o)) = earlier {
                if !wet[k] {
                    raw[k] = self.road_level(o, t);
                    pinned[k] = true;
                }
            }
        }
        let mut h = raw.clone();
        for _ in 0..3 {
            let prev = h.clone();
            for k in 0..n {
                if wet[k] || pinned[k] {
                    continue;
                }
                let (mut sum, mut count) = (0i64, 0i64);
                for q in k.saturating_sub(PROFILE_REACH)..(k + PROFILE_REACH + 1).min(n) {
                    if !wet[q] {
                        sum += prev[q];
                        count += 1;
                    }
                }
                h[k] = sum / count.max(1);
            }
        }
        // and eased onto the pin over the first `PROFILE_REACH` stations,
        // so the two roads meet level and with no step
        for (end, pin) in [(0usize, pinned[0]), (n - 1, pinned[n - 1])] {
            if !pin {
                continue;
            }
            for q in 1..PROFILE_REACH.min(n) {
                let k = if end == 0 { q } else { n - 1 - q };
                if wet[k] {
                    break;
                }
                let t = sstep(0, PROFILE_REACH as i64 * O, q as i64 * O);
                h[k] = lerp(h[end], h[k], t);
            }
        }
        h
    }

    /// The meadow's sun-dried colour.
    pub fn dry_colour(&self) -> Rgb {
        self.paint.dry
    }

    /// The roads' dirt and its darker patches and ruts.
    pub fn road_paint(&self) -> (Rgb, Rgb) {
        (self.paint.road, self.paint.road_dark)
    }

    /// Whether the road's line at (x, z) is over the river's water and
    /// banks, where the bridge spans it and the road is not flattened.
    pub fn over_river(&self, x: i64, z: i64) -> bool {
        self.river
            .near(x as Fx, z as Fx, self.reach)
            .map(|n| (n.d as i64) < n.half as i64 + BANK_MAX / 2 + 3 * O)
            .unwrap_or(false)
    }

    /// The smoothed height of segment `s`'s road `t` of the way along it.
    fn road_level(&self, s: &Seg, t: i64) -> i64 {
        let prof = &self.profiles[s.road];
        let at = s.s0 + qmul(s.len, t);
        let k = (at / PROFILE_STEP) as usize;
        let f = at % PROFILE_STEP * O / PROFILE_STEP;
        let a = prof.get(k).or(prof.last()).copied().unwrap_or(0);
        let b = prof.get(k + 1).or(prof.last()).copied().unwrap_or(a);
        lerp(a, b, f)
    }

    /// Everything but the footprints.
    fn before_flats(&self, x: Fx, z: Fx, sc: &Scope, near: Option<Near>) -> i64 {
        let mut h = self.pre_near(x, z, sc, near);
        for &k in &sc.roads {
            let s = &self.roads[k];
            let (xi, zi) = (x as i64, z as i64);
            if xi < s.lo.0 || xi > s.hi.0 || zi < s.lo.1 || zi > s.hi.1 {
                continue;
            }
            let (d, p, t) = s.nearest_t(xi, zi);
            if d >= s.half + SHOULDER {
                continue;
            }
            // no flattening over the river's water and banks: the bridge spans them
            let over_river = s.wet
                && self
                    .river
                    .near(p.0 as Fx, p.1 as Fx, self.reach)
                    .map(|n| (n.d as i64) < n.half as i64 + BANK_MAX / 2 + 3 * O)
                    .unwrap_or(false);
            if over_river {
                continue;
            }
            // a gentle crown: the middle a fortieth of the half width over
            // the edges, so rain would run off (R9.46)
            let u = (d * O / s.half.max(1)).min(O);
            let level = self.road_level(s, t) + qmul(s.half / CROWN, O - qmul(u, u));
            h = lerp(h, level, O - sstep(s.half, s.half + SHOULDER, d));
        }
        h
    }

    /// The land's height at (x, z): the smooth surface every tile samples.
    pub fn height_in(&self, x: Fx, z: Fx, sc: &Scope) -> Fx {
        self.height_near(x, z, sc, self.near(x, z, sc))
    }

    /// The same, with the river near the point already found.
    pub fn height_near(&self, x: Fx, z: Fx, sc: &Scope, near: Option<Near>) -> Fx {
        let mut h = self.before_flats(x, z, sc, near);
        let fl = &self.layout.flatten;
        let (margin, falloff) = (fl.margin_m as i64 * O, fl.falloff_m as i64 * O);
        for &k in &sc.flats {
            let f = &self.flats[k];
            let e = f.outside(x as i64, z as i64);
            if e < margin + falloff {
                h = lerp(h, f.base, O - sstep(margin, margin + falloff, e));
            }
        }
        h.clamp(i32::MIN as i64 / 2, i32::MAX as i64 / 2) as Fx
    }

    pub fn height(&self, x: Fx, z: Fx) -> Fx {
        self.height_in(x, z, &self.scope_all())
    }

    /// The finest drawn surface at (x, z): the triangles of 2 m cells the
    /// nearest tiles draw, so feet stand on the ground as drawn.
    pub fn ground(&self, x: Fx, z: Fx) -> Fx {
        let c = 2 * ONE;
        let (i, j) = (x.div_euclid(c), z.div_euclid(c));
        let u = (x - i * c) as i64 * O / c as i64;
        let v = (z - j * c) as i64 * O / c as i64;
        let sc = self.scope(i * c, j * c, (i + 1) * c, (j + 1) * c);
        let h = |a: i32, b: i32| self.height_in((i + a) * c, (j + b) * c, &sc) as i64;
        let (ha, hb, hc, hd) = (h(0, 0), h(1, 0), h(1, 1), h(0, 1));
        let y = if (i + j) & 1 == 0 {
            // the diagonal from the north-west corner to the south-east
            if u >= v {
                ha + qmul(hb - ha, u) + qmul(hc - hb, v)
            } else {
                ha + qmul(hc - hd, u) + qmul(hd - ha, v)
            }
        } else if u + v <= O {
            ha + qmul(hb - ha, u) + qmul(hd - ha, v)
        } else {
            hc + qmul(hd - hc, O - u) + qmul(hb - hc, O - v)
        };
        y as Fx
    }

    /// The finest drawn surface over a square of 2 m cells from cell (i0,
    /// j0), `n` a side, its corners' heights worked out once: what grass is
    /// stood on, many to a cell (`scatter::tufts`). The same heights and the
    /// same triangles as `ground`.
    pub fn patch(&self, i0: i32, j0: i32, n: i32) -> GroundPatch {
        let c = 2 * ONE;
        let sc = self.scope(i0 * c - c, j0 * c - c, (i0 + n + 1) * c, (j0 + n + 1) * c);
        let w = n + 1;
        let mut hint = None;
        let mut hs = Vec::with_capacity((w * w) as usize);
        for b in 0..w {
            for a in 0..w {
                let (x, z) = ((i0 + a) * c, (j0 + b) * c);
                let near = if sc.river {
                    self.river.near_from(x, z, self.reach, &mut hint)
                } else {
                    None
                };
                hs.push(self.height_near(x, z, &sc, near));
            }
        }
        GroundPatch { i0, j0, n, hs }
    }

    /// Whether (x, z) is under the river's or the pond's still water.
    pub fn is_water(&self, x: Fx, z: Fx) -> bool {
        self.water_depth(x, z) > 0
    }

    /// How deep the still water is over the ground at (x, z); none (zero or
    /// less) on dry land.
    pub fn water_depth(&self, x: Fx, z: Fx) -> Fx {
        let g = self.height(x, z);
        let mut level = i32::MIN;
        if let Some(n) = self.river.near(x, z, self.reach) {
            if n.d < n.half + ONE * 4 {
                level = n.surface;
            }
        }
        for p in &self.ponds {
            if hypot(x as i64 - p.c.0, z as i64 - p.c.1) < p.r + p.bank {
                level = level.max(p.level as Fx);
            }
        }
        if level == i32::MIN {
            return 0;
        }
        level - g
    }

    /// The ponds: centre, radius and still level.
    pub fn ponds(&self) -> Vec<(V3, Fx, Fx)> {
        self.ponds
            .iter()
            .map(|p| {
                (
                    v3(p.c.0 as Fx, p.level as Fx, p.c.1 as Fx),
                    p.r as Fx,
                    p.bank as Fx,
                )
            })
            .collect()
    }

    /// How near the nearest road, lane or trail is, and its half width.
    pub fn road_distance(&self, x: Fx, z: Fx, sc: &Scope) -> (Fx, Fx) {
        let mut best = (i32::MAX, 0);
        for &k in &sc.roads {
            let s = &self.roads[k];
            let (d, _) = s.nearest(x as i64, z as i64);
            let edge = d - s.half;
            if edge < (best.0 - best.1) as i64 {
                best = (d.min(i32::MAX as i64) as Fx, s.half as Fx);
            }
        }
        best
    }

    /// How far outside the nearest footprint (x, z) is.
    pub fn footprint_distance(&self, x: Fx, z: Fx, sc: &Scope) -> Fx {
        sc.flats
            .iter()
            .map(|&k| self.flats[k].outside(x as i64, z as i64))
            .min()
            .unwrap_or(i64::MAX)
            .min(i32::MAX as i64) as Fx
    }

    /// Whether (x, z) is in a field of crops (the scarecrow's).
    pub fn in_field(&self, x: Fx, z: Fx, sc: &Scope) -> bool {
        sc.fields.iter().any(|&k| {
            let fl = &self.fields[k];
            let (dx, dz) = (x as i64 - fl.c.0, z as i64 - fl.c.1);
            let lx = qmul(dx, fl.cos) - qmul(dz, fl.sin);
            let lz = qmul(dx, fl.sin) + qmul(dz, fl.cos);
            lx.abs() < fl.hx && lz.abs() < fl.hz
        })
    }

    /// Whether (x, z) is within `clear` of a pond's bank.
    pub fn in_pond(&self, x: Fx, z: Fx, clear: Fx) -> bool {
        self.ponds
            .iter()
            .any(|p| hypot(x as i64 - p.c.0, z as i64 - p.c.1) < p.r + p.bank + clear as i64)
    }

    /// Whether (x, z) is in a cleared place (the camp, the hollow tree).
    pub fn cleared(&self, x: Fx, z: Fx, sc: &Scope) -> bool {
        sc.clearings.iter().any(|&k| {
            let (c, r) = self.clearings[k];
            hypot(x as i64 - c.x as i64, z as i64 - c.z as i64) < r as i64
        })
    }

    /// The meadow's own colour at (x, z) on a tile of `level`: the patchwork
    /// of grass tones (fading to their mean level by level), the sun-dried
    /// patches and the forest floor, before any road, building, bank or
    /// ridge. Grass is tinted by it (`scatter.rs`), so far grass is the
    /// meadow's texture (R9.44).
    pub fn meadow_in(&self, x: Fx, z: Fx, level: u32, sc: &Scope) -> Rgb {
        let p = &self.paint;
        let sh = &self.spec.shading;
        let seed = self.spec.seed;
        let lvl = (level as usize).min(sh.patchwork_pct.len() - 1);
        let patchwork = sh.patchwork_pct[lvl] as i64 * O / 100;
        // the patchwork of tones, fading to their mean with distance
        let n = noise01(x, z, sh.patch_m, seed ^ 0x7a7a);
        let tones = p.grass.len() as i64;
        let pos = n * (tones - 1);
        let k = ((pos / O) as usize).min(p.grass.len() - 1);
        let t = (pos % O) as Fx;
        let tone = mix(p.grass[k], p.grass[(k + 1).min(p.grass.len() - 1)], t);
        let mut c = mix(p.grass_mean, tone, patchwork as Fx);
        // sun-dried patches across the meadows, which carry to every level
        let m = noise01(x, z, sh.meadow_m, seed ^ 0x3b3b);
        let dry = qmul(sstep(O * 55 / 100, O * 85 / 100, m), sh.dry_pct as i64 * O / 100);
        c = mix(c, p.dry, dry as Fx);
        // the forest floor, and far off the canopy's colour where no tree is drawn
        let f = self.forest(x, z, sc);
        if f > 0 {
            let floor = if level >= 4 { p.canopy } else { p.floor };
            c = mix(c, floor, f as Fx);
        }
        c
    }

    /// The colour of the ground at a corner of a tile of `level`, whose cells
    /// are `cell` wide: what a facet there is mixed from.
    #[allow(clippy::too_many_arguments)]
    pub fn colour_in(
        &self,
        x: Fx,
        z: Fx,
        h: Fx,
        level: u32,
        cell: Fx,
        sc: &Scope,
        near: Option<Near>,
    ) -> Rgb {
        let p = &self.paint;
        let sh = &self.spec.shading;
        let seed = self.spec.seed;
        let lvl = (level as usize).min(sh.patchwork_pct.len() - 1);
        let patchwork = sh.patchwork_pct[lvl] as i64 * O / 100;
        let mut c = self.meadow_in(x, z, level, sc);
        let cell = cell as i64;
        // a thin feature covers this share of a facet `cell` wide
        let cover = |d: i64, half: i64| ((half + cell / 2 - d) * O / cell.max(1)).clamp(0, O);
        for &k in &sc.fields {
            let fl = &self.fields[k];
            let (dx, dz) = (x as i64 - fl.c.0, z as i64 - fl.c.1);
            let lx = qmul(dx, fl.cos) - qmul(dz, fl.sin);
            let lz = qmul(dx, fl.sin) + qmul(dz, fl.cos);
            let edge = (lx.abs() - fl.hx).max(lz.abs() - fl.hz);
            let inside = cover(edge.max(0) + fl.hx.min(fl.hz), fl.hx.min(fl.hz));
            if inside > 0 && edge < cell {
                // strips of crop and earth; their mean once a facet is wider than a strip
                let strip = (lx.div_euclid(fl.strip) & 1) == 0;
                let fine = if strip { p.crop } else { p.furrow };
                let mean = mix(p.crop, p.furrow, ONE / 2);
                let sharp = (O - (cell - fl.strip / 2).max(0) * O / fl.strip.max(1)).clamp(0, O);
                let ground = mix(mean, fine, sharp as Fx);
                c = mix(c, ground, inside as Fx);
            }
        }
        // packed earth round the houses
        let foot = self.footprint_distance(x, z, sc) as i64;
        if foot < 8 * O {
            let worn = qmul(O - sstep(O, 8 * O, foot), sh.village_earth_pct as i64 * O / 100);
            let worn = qmul(worn, noise01(x, z, 7, seed ^ 0x1e1e) / 2 + O / 2);
            c = mix(c, p.earth, worn as Fx);
        }
        // the roads, lanes and trails: dirt, patchy
        let (rd, rh) = self.road_distance(x, z, sc);
        if rh > 0 {
            // on the finest level the road's own surface is drawn over it
            // near (`scatter::ROAD`): the ground's dirt then stays inside
            // the road's edges, so none shows past them in jagged facets
            let rh = if level == 0 { rh - cell as Fx / 2 } else { rh };
            let cov = cover(rd as i64, rh as i64);
            if cov > 0 {
                let patch = noise01(x, z, 11, seed ^ 0x5d5d);
                let dirt = mix(p.road, p.road_dark, (patch * 7 / 10) as Fx);
                c = mix(c, dirt, cov as Fx);
            }
        }
        // the river's banks and bed
        if let Some(n) = near {
            let (d, half) = (n.d as i64, n.half as i64);
            // the bed's colour in the channel only: land lower than the water
            // beyond the banks is still meadow
            let wet = (h as i64) < n.surface as i64 && d < half + BANK_MAX / 2;
            if wet {
                // a stony bed, gravel where it is shallow (the fords)
                c = mix(
                    p.gravel,
                    p.bed,
                    (sstep(0, O / 2, n.surface as i64 - h as i64)) as Fx,
                );
            } else {
                let band = cover((d - half).abs(), 2 * O + cell / 4);
                if band > 0 {
                    c = mix(c, p.sand, band as Fx);
                }
                // a gravel bar on the inside of a bend, across its low bank
                let (_, bank_w, bar) = self.bank_shape(&n);
                if bar > 0 {
                    let on = cover(d - half, bank_w);
                    c = mix(c, p.gravel, qmul(on, bar) as Fx);
                }
            }
        }
        for &k in &sc.ponds {
            let pd = &self.ponds[k];
            let d = hypot(x as i64 - pd.c.0, z as i64 - pd.c.1);
            if d < pd.r + pd.bank + O {
                let wet = (h as i64) < pd.level;
                c = if wet {
                    mix(p.sand, p.bed, sstep(0, O / 3, pd.level - h as i64) as Fx)
                } else {
                    mix(c, p.sand, (O - sstep(pd.r, pd.r + pd.bank + O, d)) as Fx)
                };
            }
        }
        // the far ridges: upland grass, rock, snow, the bands ragged
        let hm = h as i64;
        if hm > sh.upland_from_m as i64 * O {
            let wobble = noise(x, z, 300, seed ^ 0x2c2c) * 70;
            let up = sstep(
                sh.upland_from_m as i64 * O,
                sh.upland_from_m as i64 * O + 120 * O,
                hm + wobble,
            );
            c = mix(c, p.upland, up as Fx);
            let rock = sstep(
                sh.rock_from_m[0] as i64 * O,
                sh.rock_from_m[1] as i64 * O,
                hm + wobble,
            );
            c = mix(c, p.mountain, rock as Fx);
            let snow = sstep(
                sh.snow_from_m[0] as i64 * O,
                sh.snow_from_m[1] as i64 * O,
                hm + wobble * 2,
            );
            c = mix(c, p.snow, snow as Fx);
        }
        // each corner's seeded unevenness
        let jitter = (hash(fx::floor(x), fx::floor(z), seed ^ 0xc0c0) >> 16) as i64 - O / 2;
        let k = O + qmul(jitter * 2, sh.corner_pct as i64 * O / 100) * patchwork / O;
        c.map(|v| (qmul(v as i64, k)).clamp(0, O) as Fx)
    }

    /// One tile of the terrain: level `level` (cells 2^(level+1) m), the
    /// `level`'s tile (i, j). Its triangles, their colours baked in, local to
    /// its north-west corner, and a skirt hanging from each edge so a coarser
    /// or finer neighbour leaves no crack.
    pub fn tile(&self, level: u32, i: i32, j: i32) -> Tile {
        let lod = &self.spec.lod;
        let n = lod.tile_cells;
        let cell = ONE << (level + 1);
        let side = cell * n;
        let (x0, z0) = (i * side, j * side);
        let sc = self.scope(x0 - cell, z0 - cell, x0 + side + cell, z0 + side + cell);
        let w = (n + 1) as usize;
        let dip = level >= lod.river_dip_from_level;
        let dip_by = lod.river_dip_cm * ONE / 100;
        let mut hs = vec![0 as Fx; w * w];
        let mut cs = vec![[0 as Fx; 3]; w * w];
        let mut hint = None;
        for b in 0..=n {
            for a in 0..=n {
                let (x, z) = (x0 + a * cell, z0 + b * cell);
                let near = if sc.river {
                    self.river.near_from(x, z, self.reach, &mut hint)
                } else {
                    None
                };
                let mut h = self.height_near(x, z, &sc, near);
                if dip {
                    if let Some(nr) = near {
                        // a coarse facet cannot hold the channel: it dips under
                        // the water where the channel runs, so the water shows
                        if (nr.d as i64) < nr.half as i64 + cell as i64 * 3 / 4 {
                            h = h.min(nr.surface - dip_by);
                        }
                    }
                }
                let k = b as usize * w + a as usize;
                hs[k] = h;
                cs[k] = self.colour_in(x, z, h, level, cell, &sc, near);
            }
        }
        let sh = &self.spec.shading;
        let lvl = (level as usize).min(sh.patchwork_pct.len() - 1);
        let patchwork = sh.patchwork_pct[lvl] as i64;
        let rock = self.paint.rock;
        // the box the tile's mesh is built about, and the power of two its x
        // and z are divided by (lod.rs, Culling)
        let lo = hs.iter().copied().min().unwrap_or(0);
        let hi = hs.iter().copied().max().unwrap_or(0);
        let (half, mid) = (side / 2, ((lo as i64 + hi as i64) / 2) as Fx);
        let tall = (hi as i64 - lo as i64) / 2 + O * 40;
        let radius = root((2 * half as i64 * half as i64 + tall * tall) as u64) as i64;
        let kk = crate::lod::unit_for(radius);
        let local = |q: V3| {
            v3(
                ((q.x - half) as i64 * O / kk as i64) as Fx,
                q.y - mid,
                ((q.z - half) as i64 * O / kk as i64) as Fx,
            )
        };
        let mut out = Vec::with_capacity(((2 * n * n + 8 * n) * 3) as usize * VERTEX_VALUES);
        let p = |a: i32, b: i32| v3(a * cell, hs[b as usize * w + a as usize], b * cell);
        let col = |a: i32, b: i32| cs[b as usize * w + a as usize];
        let seed = self.spec.seed;
        let tri = |out: &mut Vec<i32>, q: [(i32, i32); 3], salt: u32| {
            let (pa, pb, pc) = (p(q[0].0, q[0].1), p(q[1].0, q[1].1), p(q[2].0, q[2].1));
            let mean = [0, 1, 2]
                .map(|k| (col(q[0].0, q[0].1)[k] + col(q[1].0, q[1].1)[k] + col(q[2].0, q[2].1)[k]) / 3);
            let nrm = facet_normal(pa, pb, pc);
            let steep = ((ONE * 4 / 5 - nrm.y).clamp(0, ONE / 5) * 5) as i64;
            let mut c = mix(mean, rock, (steep * sh.steep_pct as i64 / 100) as Fx);
            let gi = i * n + q[0].0;
            let gj = j * n + q[0].1;
            let jitter = (hash(gi, gj, seed ^ salt ^ level) >> 16) as i64 - O / 2;
            let k = O + jitter * 2 * sh.facet_pct as i64 / 100 * patchwork / 100;
            c = c.map(|v| qmul(v as i64, k).clamp(0, O) as Fx);
            facet(out, [local(pa), local(pb), local(pc)], local_normal(nrm, kk), c);
        };
        for b in 0..n {
            for a in 0..n {
                let (na, ne, se, sw) = ((a, b), (a + 1, b), (a + 1, b + 1), (a, b + 1));
                if ((i * n + a) + (j * n + b)) & 1 == 0 {
                    tri(&mut out, [na, sw, se], 0x11);
                    tri(&mut out, [na, se, ne], 0x22);
                } else {
                    tri(&mut out, [na, sw, ne], 0x33);
                    tri(&mut out, [ne, sw, se], 0x44);
                }
            }
        }
        // the skirts: down from each edge, facing out, as deep as a finer
        // neighbour's edge strays from this one's, and some
        let skirt = lod.skirt_cm * ONE / 100;
        let edges: [[(i32, i32); 2]; 4] = [
            // north, east, south, west: each walked so its skirt faces out
            [(0, 0), (1, 0)],
            [(n, 0), (0, 1)],
            [(n, n), (-1, 0)],
            [(0, n), (0, -1)],
        ];
        for [(sa, sb), (da, db)] in edges {
            for e in 0..n {
                let (a0, b0) = (sa + da * e, sb + db * e);
                let (a1, b1) = (a0 + da, b0 + db);
                let (top0, top1) = (p(a0, b0), p(a1, b1));
                let mut stray: i64 = 0;
                for q in 1..4 {
                    let (x, z) = (
                        x0 + (a0 * cell) + (a1 - a0) * cell * q / 4,
                        z0 + (b0 * cell) + (b1 - b0) * cell * q / 4,
                    );
                    let line = top0.y as i64 + (top1.y as i64 - top0.y as i64) * q as i64 / 4;
                    stray = stray.max((self.height_in(x, z, &sc) as i64 - line).abs());
                    // the midpoint is a finer neighbour's corner; a quarter
                    // point, the next's: the midpoint's stray, doubled, serves
                    if q == 2 {
                        stray *= 2;
                        break;
                    }
                }
                let depth = (stray * 3 / 2) as Fx + skirt + if dip { dip_by } else { 0 };
                let (bot0, bot1) = (top0.sub(v3(0, depth, 0)), top1.sub(v3(0, depth, 0)));
                let c = [0, 1, 2].map(|q| (col(a0, b0)[q] + col(a1, b1)[q]) / 2 * 85 / 100);
                // facing out, flat: the edge's direction turned a quarter
                let along = v3(da * ONE, 0, db * ONE);
                let outward = v3(along.z, 0, -along.x);
                let nl = local_normal(outward, kk);
                facet(&mut out, [local(top0), local(top1), local(bot0)], nl, c);
                facet(&mut out, [local(top1), local(bot1), local(bot0)], nl, c);
            }
        }
        let centre = v3(x0 + half, mid, z0 + half);
        Tile {
            vertices: out,
            corner: v3(x0, 0, z0),
            centre,
            radius: radius.min(i32::MAX as i64) as Fx,
            at: crate::lod::placed(centre, kk),
        }
    }
}

/// The unit normal of a triangle, counter-clockwise from the side it
/// faces, in i64 so long edges cannot overflow.
pub fn facet_normal(a: V3, b: V3, c: V3) -> V3 {
    let (u, w) = (b.sub(a), c.sub(a));
    let (ux, uy, uz) = (u.x as i64, u.y as i64, u.z as i64);
    let (wx, wy, wz) = (w.x as i64, w.y as i64, w.z as i64);
    unit([uy * wz - uz * wy, uz * wx - ux * wz, ux * wy - uy * wx])
}

/// A vector of any length made unit, in Q16.
pub fn unit(v: [i64; 3]) -> V3 {
    let mut v = v;
    // shrink until its square fits
    while v.iter().any(|c| c.abs() > 1 << 30) {
        v = v.map(|c| c >> 4);
    }
    let l = root((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) as u64) as i64;
    if l == 0 {
        return v3(0, ONE, 0);
    }
    v3((v[0] * O / l) as Fx, (v[1] * O / l) as Fx, (v[2] * O / l) as Fx)
}

/// A world normal as the mesh built with x and z divided by `k` carries
/// it: the shader turns it back with the inverse scale (`scene.vert`).
pub fn local_normal(n: V3, k: Fx) -> V3 {
    unit([n.x as i64 * k as i64, n.y as i64 * O, n.z as i64 * k as i64])
}

/// One flat triangle of a world mesh with its normal and colour, as
/// `prim::push_coloured` lays a vertex out.
pub fn facet(out: &mut Vec<i32>, p: [V3; 3], n: V3, rgb: Rgb) {
    for q in p {
        out.extend_from_slice(&[q.x, q.y, q.z, n.x, n.y, n.z, rgb[0], rgb[1], rgb[2]]);
    }
}

/// The drawn surface over a square of 2 m cells (`Land::patch`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundPatch {
    i0: i32,
    j0: i32,
    n: i32,
    hs: Vec<Fx>,
}

impl GroundPatch {
    /// The height at (x, z), inside the patch's square, as `Land::ground`
    /// gives it; `None` outside.
    pub fn at(&self, x: Fx, z: Fx) -> Option<Fx> {
        let c = 2 * ONE;
        let (i, j) = (x.div_euclid(c), z.div_euclid(c));
        let (a, b) = (i - self.i0, j - self.j0);
        if a < 0 || b < 0 || a >= self.n || b >= self.n {
            return None;
        }
        let u = (x - i * c) as i64 * O / c as i64;
        let v = (z - j * c) as i64 * O / c as i64;
        let w = self.n + 1;
        let h = |p: i32, q: i32| self.hs[((b + q) * w + a + p) as usize] as i64;
        let (ha, hb, hc, hd) = (h(0, 0), h(1, 0), h(1, 1), h(0, 1));
        let y = if (i + j) & 1 == 0 {
            if u >= v {
                ha + qmul(hb - ha, u) + qmul(hc - hb, v)
            } else {
                ha + qmul(hc - hd, u) + qmul(hd - ha, v)
            }
        } else if u + v <= O {
            ha + qmul(hb - ha, u) + qmul(hd - ha, v)
        } else {
            hc + qmul(hd - hc, O - u) + qmul(hb - hc, O - v)
        };
        Some(y as Fx)
    }
}

/// One tile's mesh and where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    /// triangles of `prim::VERTEX_VALUES` a vertex, local to `corner`
    pub vertices: Vec<i32>,
    /// its north-west corner, at height 0
    pub corner: V3,
    /// the middle of its box, and a radius holding it
    pub centre: V3,
    pub radius: Fx,
    /// the affine it is drawn with: its mesh is built about `centre`, x and
    /// z divided by a power of two at least `radius` (`lod.rs`, Culling)
    pub at: crate::geom::Affine,
}

/// The land the world is built on, shared by its copies.
pub fn shipped() -> Rc<Land> {
    Rc::new(Land::shipped())
}

/// Whether (x, z) is under water: the map menu (M4) colours it, and M1's
/// footing may slow in it. The signature stays.
pub fn is_water(w: &World, x: Fx, z: Fx) -> bool {
    w.land.is_water(x, z)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::fx::int;

    thread_local! {
        static LAND: Rc<Land> = shipped();
    }

    /// The shipped land, built once a test thread.
    pub fn land() -> Rc<Land> {
        LAND.with(|l| l.clone())
    }

    #[test]
    fn the_village_plateau_stands_at_height_zero_where_the_game_opens() {
        let l = land();
        let s = crate::world::START;
        assert_eq!(l.ground(s.x, s.z), 0);
        // flat within its first radius, 90 m (beside the lane north, whose
        // crown stands 4 cm over it since M7, R9.46)
        for (dx, dz) in [(80, 0), (12, -80), (-60, 50)] {
            let (x, z) = (s.x + int(dx), s.z + int(dz));
            assert!(
                l.height(x, z).abs() < ONE / 100,
                "({dx}, {dz}): {}",
                l.height(x, z)
            );
        }
    }

    #[test]
    fn the_quick_square_root_is_the_floor_of_the_root() {
        for n in [
            0u64,
            1,
            2,
            3,
            4,
            15,
            16,
            17,
            99,
            100,
            1 << 40,
            (1 << 40) + 1,
            u64::MAX / 4,
            999_999_999_999,
        ] {
            let r = root(n);
            assert!(r * r <= n && (r + 1) * (r + 1) > n, "{n}: {r}");
        }
        // 2^62 − 1 is just under (2^31)², so its root is 2^31 − 1
        assert_eq!(root((1u64 << 62) - 1), (1 << 31) - 1);
    }

    #[test]
    fn noise_is_whole_and_eased_between_its_lattice_points() {
        // at a lattice point the noise is the lattice's value
        let seed = 9;
        for (i, j) in [(0, 0), (3, -2), (-7, 5)] {
            assert_eq!(
                noise(int(40 * i), int(40 * j), 40, seed),
                lattice(i as i64, j as i64, seed)
            );
        }
        // halfway, with the quintic ease at ½ (½³ (½ (3 − 15) + 10) = ½), the
        // mean of the four corners
        let (a, b, c, d) = (
            lattice(0, 0, seed),
            lattice(1, 0, seed),
            lattice(0, 1, seed),
            lattice(1, 1, seed),
        );
        let mid = noise(int(20), int(20), 40, seed);
        assert!((mid - (a + b + c + d) / 4).abs() <= 2, "{mid}");
        assert_eq!(fade(O / 2), O / 2);
        // a quarter of the way: ¼³ (¼ (1.5 − 15) + 10) = 6.625 / 64 = 0.1035,
        // eased, where a straight line would give 0.25
        assert!((fade(O / 4) - O * 1035 / 10_000).abs() < 8, "{}", fade(O / 4));
        assert_eq!((fade(0), fade(O)), (0, O));
    }

    #[test]
    fn the_ground_is_the_drawn_triangles_of_two_metre_cells() {
        let l = land();
        // on a corner, the height itself
        let (x, z) = (int(1500), int(2500));
        assert_eq!(l.ground(x, z), l.height(x, z));
        // half way along a cell's north edge, the mean of its two corners
        let (a, b) = (l.height(x, z) as i64, l.height(x + int(2), z) as i64);
        let g = l.ground(x + ONE, z) as i64;
        assert!((g - (a + b) / 2).abs() <= 2, "{g} vs {}", (a + b) / 2);
        // cell (750, 1250) is even: its diagonal runs north-west to south-east,
        // so its middle is the mean of those two corners
        let (c, d) = (l.height(x, z) as i64, l.height(x + int(2), z + int(2)) as i64);
        let m = l.ground(x + ONE, z + ONE) as i64;
        assert!((m - (c + d) / 2).abs() <= 2, "{m} vs {}", (c + d) / 2);
    }

    #[test]
    fn a_patch_of_the_ground_stands_where_the_ground_does() {
        let l = land();
        // 12 cells of 2 m from (1500, 2600), by the lane and over a rise,
        // and one over the river's bank by the bridge
        for (i0, j0) in [(750, 1300), (310, 1120)] {
            let p = l.patch(i0, j0, 12);
            for (dx, dz) in [(0, 0), (1, 1), (7, 3), (13, 22), (23, 23), (11, 0)] {
                let (x, z) = (int(2 * i0 + dx) + ONE / 3, int(2 * j0 + dz) + ONE / 7);
                assert_eq!(p.at(x, z), Some(l.ground(x, z)), "({dx}, {dz})");
            }
            assert_eq!(p.at(int(2 * i0 + 24), int(2 * j0)), None, "outside");
        }
    }

    #[test]
    fn the_river_s_banks_meet_its_water_and_its_bed_lies_under_it() {
        let l = land();
        let r = &l.river;
        // under the bridge the water stands at −4 m (data/places.json), at the
        // station it is set on; the narrows under the bridge run faster and
        // fall a few centimetres a station
        let n = r.near(int(627), int(2248), int(50)).unwrap();
        assert!((n.surface - int(-4)).abs() < ONE / 20, "{}", n.surface);
        // across the river at z = 1700, from the centreline out
        let k = r.stations.iter().position(|s| s.at.z >= int(1700)).unwrap();
        let st = r.stations[k];
        let right = v3(-st.dir.z, 0, st.dir.x);
        let at = |m: Fx| v3(st.at.x, 0, st.at.z).add(right.scale(m));
        let h = |m: Fx| {
            let p = at(m);
            l.height(p.x, p.z)
        };
        let deep = h(0);
        assert!(
            deep < st.at.y - st.depth,
            "the middle is deeper than the mean depth"
        );
        // at the water's edge, the bank is at the water
        let edge = h(st.half);
        assert!((edge - st.at.y).abs() < ONE / 20, "{} vs {}", edge, st.at.y);
        // ten centimetres out the bank has risen only by its slope, as the
        // water's bowl falls by its own: a few centimetres
        let out = h(st.half + ONE / 10);
        assert!(
            out > st.at.y && out - st.at.y < ONE / 20,
            "{} over the water",
            out - st.at.y
        );
        // and two metres out, above it on both sides
        assert!(h(st.half + int(2)) > st.at.y + ONE / 4);
        assert!(h(-st.half - int(2)) > st.at.y + ONE / 4);
    }

    #[test]
    fn roads_are_flat_across_and_buildings_stand_on_flat_ground() {
        let l = land();
        // across the road at x = 1500 (between (1200, 2200) and (1600, 2214))
        let z = int(2200) + int(14) * 300 / 400;
        let a = l.height(int(1500), z - int(2));
        let b = l.height(int(1500), z + int(2));
        assert!((a - b).abs() < ONE / 50, "{a} {b}");
        // a house's footprint: the village's at (−46, −10), all at one height
        let (cx, cz) = (int(2000 - 46), int(2800 - 10));
        let base = l.height(cx, cz);
        for (dx, dz) in [(-3, -3), (3, 3), (-3, 3), (3, -3)] {
            assert_eq!(l.height(cx + int(dx), cz + int(dz)), base);
        }
        // the trainer's hut, on its hill: flat, and well above the plains
        let (hx, hz) = (int(330), int(1750));
        let hut = l.height(hx, hz);
        assert!(hut > int(30), "the hill is {} m", hut / ONE);
        assert_eq!(l.height(hx + int(2), hz - int(2)), hut);
        // the shrine, on its islet's dome, stands on a level footprint too
        let (sx, sz) = (int(738), int(1000));
        let base = l.height(sx, sz);
        for (dx, dz) in [(-1, -1), (1, 1), (-1, 1), (1, -1)] {
            assert_eq!(
                l.height(sx + int(dx) * 3 / 2, sz + int(dz) * 3 / 2),
                base,
                "({dx}, {dz})"
            );
        }
    }

    /// R9.46: along the middle of every road and lane, a metre at a time,
    /// the ground never rises or falls by more than 8 cm (an 8% grade), nor
    /// along a trail by more than 30 cm (the trail up the trainer's hill
    /// climbs about 25 m in 190 m, 13% on the mean); the bridge's span
    /// over the river's banks is the bridge's, not the road's. Where the
    /// lane meets the road at the crossroads the two meet level: no step of
    /// a centimetre between the lane's last metre and the road.
    #[test]
    fn a_road_s_middle_never_rises_or_falls_more_than_its_grade() {
        let l = land();
        let mut checked = 0;
        for r in &l.layout.roads {
            let most = if r.kind == "trail" { 30 } else { 8 };
            for k in 0..r.line.len() - 1 {
                let (a, b) = (r.line[k], r.line[k + 1]);
                let (dx, dz) = ((b[0] - a[0]) as i64, (b[1] - a[1]) as i64);
                let len = root((dx * dx + dz * dz) as u64) as i64;
                let mut prev: Option<Fx> = None;
                for m in 0..=len {
                    let x = (a[0] as i64 * O + dx * O * m / len) as Fx;
                    let z = (a[1] as i64 * O + dz * O * m / len) as Fx;
                    // the walkable square's roads; beyond it the road runs on
                    // into the hills as scenery
                    let inside = (0..=int(4000)).contains(&x) && (0..=int(4000)).contains(&z);
                    if !inside || l.over_river(x as i64, z as i64) {
                        prev = None;
                        continue;
                    }
                    let h = l.ground(x, z);
                    if let Some(p) = prev {
                        assert!(
                            ((h - p).abs() as i64) * 100 <= most * O,
                            "the {} at ({}, {}): {} cm in a metre",
                            r.kind,
                            x / ONE,
                            z / ONE,
                            (h - p) * 100 / ONE
                        );
                        checked += 1;
                    }
                    prev = Some(h);
                }
            }
        }
        assert!(checked > 8_000, "{checked} metres");
        // the lane meets the road at (2000, 2250): a metre down the lane
        // and on the road there, within a centimetre
        let (lane, road) = (l.ground(int(2000), int(2251)), l.ground(int(2000), int(2250)));
        assert!((lane - road).abs() <= ONE / 100, "{lane} {road}");
    }

    /// R9.47: the banks vary. Down the river through the square, both
    /// sides: each bank stands 60% to 130% of 2 m over the water off a bend
    /// (1.2 to 2.6 m), lower on a bar; it is as wide as its height over the
    /// bowl's slope at the edge (half / deep), so where that width is not
    /// held to its 2 m to 15 m it meets the water at the bowl's own slope;
    /// and somewhere it is a gravel bar.
    #[test]
    fn the_river_s_banks_vary_and_meet_the_water_at_its_slope() {
        let l = land();
        let (mut bars, mut lo, mut hi) = (0, i64::MAX, 0);
        for st in l
            .river
            .stations
            .iter()
            .filter(|s| s.at.z > 0 && s.at.z < int(4000))
            .step_by(5)
        {
            for side in [-1, 1] {
                let n = crate::water::Near {
                    d: st.half,
                    across: side * st.half,
                    s: st.s,
                    surface: st.at.y,
                    half: st.half,
                    depth: st.depth,
                    k: 0,
                };
                let (bank, w, bar) = l.bank_shape(&n);
                assert!(bank <= ONE as i64 * 26 / 10 + 8, "{bank}");
                if bar == 0 {
                    assert!(bank >= ONE as i64 * 12 / 10 - 8, "{bank}");
                    lo = lo.min(bank);
                    hi = hi.max(bank);
                }
                let deep = (st.depth as i64 * 3 / 2).max(O / 5);
                let exact = bank * st.half as i64 / deep;
                if exact > 2 * O && exact < BANK_MAX / 2 {
                    assert!((w - exact).abs() <= 2);
                }
                bars += (bar > O / 2) as i32;
            }
        }
        assert!(bars > 10, "{bars} stations on a bar");
        // off the bars, the banks themselves wander by more than half a metre
        assert!(
            hi - lo > O / 2,
            "banks from {} to {} cm",
            lo * 100 / O,
            hi * 100 / O
        );
    }

    #[test]
    fn the_land_rises_into_mountains_beyond_the_square() {
        let l = land();
        // 8 km north of the square, away from the river's valley
        let far = (-1000..=5000)
            .step_by(500)
            .map(|x| l.height(int(x), int(-8000)))
            .max()
            .unwrap();
        assert!(far > int(500), "the north's ridges reach {} m", far / ONE);
        let plains = l.height(int(2500), int(1800));
        assert!(plains.abs() < int(25), "the plains are {} m", plains / ONE);
    }

    #[test]
    fn the_pond_and_the_river_are_water_and_the_village_is_not() {
        let l = land();
        assert!(l.is_water(int(2030), int(2826)), "the pond");
        assert!(l.is_water(int(627), int(2248)), "under the bridge");
        assert!(!l.is_water(int(2000), int(2800)), "the green");
        assert!(!l.is_water(int(700), int(2248)), "the east bank");
        // the shrine's islet stands out of the ford
        assert!(!l.is_water(int(738), int(1000)));
        assert!(l.is_water(int(738) + int(9), int(1000)));
    }

    #[test]
    fn a_tile_tiles_its_square_and_its_skirts_face_out() {
        let l = land();
        let t = l.tile(1, 7, 10);
        let n = l.spec.lod.tile_cells;
        let tris = t.vertices.len() / (3 * VERTEX_VALUES);
        assert_eq!(tris as i32, 2 * n * n + 8 * n);
        // the ground's triangles cover the tile's square once: 4 m cells, 32
        // a side, 128 m by 128 m
        let mut area: i128 = 0;
        for tr in t.vertices.chunks(3 * VERTEX_VALUES).take((2 * n * n) as usize) {
            let p = |k: usize| (tr[k * VERTEX_VALUES] as i128, tr[k * VERTEX_VALUES + 2] as i128);
            let (a, b, c) = (p(0), p(1), p(2));
            area += ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).abs() / 2;
            // every ground facet faces up
            assert!(tr[4] > 0, "a facet faces down");
        }
        // built about its middle with x and z divided by k (a power of two
        // over its radius, 128 m here): the area times k² is the square's
        let k = t.at.c0.x as i128;
        assert_eq!(k, int(128) as i128);
        assert_eq!(
            area * k * k / (ONE as i128 * ONE as i128),
            (128 * ONE as i128) * (128 * ONE as i128)
        );
        // and each facet once: no two share their middle
        let mut middles: Vec<(i64, i64)> = t
            .vertices
            .chunks(3 * VERTEX_VALUES)
            .take((2 * n * n) as usize)
            .map(|tr| {
                let c = |k: usize| (0..3).map(|q| tr[q * VERTEX_VALUES + k] as i64).sum::<i64>();
                (c(0), c(2))
            })
            .collect();
        middles.sort();
        middles.dedup();
        assert_eq!(middles.len() as i32, 2 * n * n);
        assert_eq!(t.at.t, t.centre);
        assert_eq!(t.centre.x, int(7 * 128 + 64));
        // the north skirt faces north (−z), the east's east
        let skirt = |k: usize| &t.vertices[((2 * n * n) as usize + k) * 3 * VERTEX_VALUES..];
        assert!(skirt(0)[5] < -ONE * 9 / 10, "north: {}", skirt(0)[5]);
        assert!(skirt(2 * n as usize)[3] > ONE * 9 / 10, "east");
        assert_eq!(t.corner, v3(int(7 * 128), 0, int(10 * 128)));
    }
}
