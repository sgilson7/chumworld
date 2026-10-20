//! `water`: the river and the village pond, solved with the water code of
//! Extremely Lifelike Fishing (CW-19, T4), owned by M2 World (task W).
//!
//! What is ported, and from where (`LICENSES.md`):
//!
//! - **The steady flow** (`solve`, the fishing game's `river/solve.rs`,
//!   verbatim): the standard step method from the energy equation in both
//!   regimes, calm water carried up from the exit and rushing water down
//!   from each lip, meeting in hydraulic jumps at Bélanger's depths.
//! - **The surface over the ground** (`field`, after its `river/bake.rs`):
//!   from the solved numbers, the speed across the channel, the bend's tilt,
//!   the pile-up at the walls, standing waves after a jump, pillows, V-waves
//!   and eddies round the bridge's piers and the shrine's islet, foam and
//!   how rough the chop is.
//! - **The waves** (`State`, after its `sea.rs`): a Gerstner sum whose
//!   phases are wrapping binary angles stepped once a tick, so neither the
//!   core nor the shader keeps a growing time.
//! - **The flow map** (after its `flowmap.rs`): the current, the foam and
//!   the roughness for the water shader. The fishing game packs them into a
//!   texture; here each vertex of the water's mesh carries its own (in the
//!   slots of the normal and the colour), so the renderer needs no texture.
//!
//! The river runs from the north (upstream) to the south: a centreline cut
//! into stations every `station_m` metres, through the journal map's curve
//! and on beyond the walkable square as scenery. Its bed falls gently, with
//! two riffles; the flow is solved between `solve_from_z` and `solve_to_z`
//! and carried on beyond as it stands at either end. The terrain carves the
//! channel to the solved surface (`terrain.rs`), so the banks meet the water
//! where the water is.
//!
//! Steps at 8 in `World::step`: the waves' phases.

pub mod solve;

use serde::Deserialize;

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, V3};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WidthSpec {
    pub z: i32,
    pub half_cm: i32,
    pub reach_m: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LevelSpec {
    pub z: i32,
    pub cm: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RiffleSpec {
    pub from_z: i32,
    pub to_z: i32,
    pub slope_per_1000: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PierSpec {
    pub z: i32,
    pub across_cm: Vec<i32>,
    pub radius_cm: i32,
    pub rise_cm: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IsletSpec {
    pub at: [i32; 2],
    pub radius_cm: i32,
    pub top_cm: i32,
}

/// `data/places.json`'s river.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RiverSpec {
    pub line: Vec<[i32; 2]>,
    pub solve_from_z: i32,
    pub solve_to_z: i32,
    pub station_m: i32,
    pub half_cm: i32,
    pub widths: Vec<WidthSpec>,
    pub level_at: LevelSpec,
    pub slope_per_1000: i32,
    pub riffles: Vec<RiffleSpec>,
    pub discharge_m3_s: i32,
    pub n_x1000: i32,
    pub n_riffle_x1000: i32,
    pub bank_cm: i32,
    pub valley_m: i32,
    pub piers: Vec<PierSpec>,
    pub islets: Vec<IsletSpec>,
}

/// The river at one station.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Station {
    /// on the centreline; y is the solved still surface
    pub at: V3,
    /// the unit direction downstream
    pub dir: V3,
    /// metres down the river from where it enters the square (negative
    /// upstream of it)
    pub s: Fx,
    pub half: Fx,
    /// the bed's mean height across the channel
    pub bed: Fx,
    pub depth: Fx,
    pub speed: Fx,
    pub froude: Fx,
    pub regime: solve::Regime,
    pub riffle: bool,
    /// how fast the river turns, radians a metre, right positive
    pub curve: Fx,
    /// how fast the half width grows, metres a metre
    pub widen: Fx,
}

/// A hydraulic jump at `s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Jump {
    pub s: Fx,
    pub h1: Fx,
    pub h2: Fx,
    pub v1: Fx,
    pub fr1: Fx,
    pub roller: Fx,
}

/// Something standing in the water the flow goes round: a pier or an islet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rock {
    pub at: V3,
    pub r: Fx,
    pub top: Fx,
    /// the flow's direction past it, and its speed there without it
    pub flow: V3,
    pub speed: Fx,
    pub s: Fx,
    pub across: Fx,
}

/// Where a point lies against the river: how far from the centreline
/// (`d`, never negative), how far across toward the right bank looking
/// downstream (`across`), how far down (`s`), and the station's numbers
/// there, interpolated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Near {
    pub d: Fx,
    pub across: Fx,
    pub s: Fx,
    pub surface: Fx,
    pub half: Fx,
    pub depth: Fx,
    /// the station before the point
    pub k: usize,
}

#[derive(Clone, PartialEq, Eq)]
pub struct River {
    pub spec: RiverSpec,
    pub stations: Vec<Station>,
    /// the station each point of `spec.line` falls on
    pub knots: Vec<usize>,
    pub jumps: Vec<Jump>,
    pub rocks: Vec<Rock>,
    /// the solved stations' first and one past their last
    pub solved: (usize, usize),
    pub step: Fx,
}

impl std::fmt::Debug for River {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "River({} stations)", self.stations.len())
    }
}

fn cm(v: i32) -> Fx {
    v * ONE / 100
}

/// 0 to ONE, easing in and out (smoothstep).
pub fn smooth(t: Fx) -> Fx {
    let t = t.clamp(0, ONE);
    fx::mul(fx::mul(t, t), 3 * ONE - 2 * t)
}

/// A point on the centreline's curve between knots `p0` and `p1`, with
/// tangents `t0` and `t1` (Hermite), at `t` (Q16), all in i64 Q16 metres.
fn hermite(p0: [i64; 2], p1: [i64; 2], t0: [i64; 2], t1: [i64; 2], t: i64) -> [i64; 2] {
    let o = ONE as i64;
    let t2 = t * t >> 16;
    let t3 = t2 * t >> 16;
    let h00 = 2 * t3 - 3 * t2 + o;
    let h10 = t3 - 2 * t2 + t;
    let h01 = -2 * t3 + 3 * t2;
    let h11 = t3 - t2;
    let c = |k: usize| (h00 * p0[k] + h10 * t0[k] + h01 * p1[k] + h11 * t1[k]) >> 16;
    [c(0), c(1)]
}

fn len2(dx: i64, dz: i64) -> i64 {
    crate::terrain::root((dx * dx + dz * dz) as u64) as i64
}

impl River {
    pub fn new(spec: &RiverSpec) -> River {
        let mut r = River {
            spec: spec.clone(),
            stations: Vec::new(),
            knots: Vec::new(),
            jumps: Vec::new(),
            rocks: Vec::new(),
            solved: (0, 0),
            step: spec.station_m.max(1) * ONE,
        };
        r.lay_out();
        r.solve();
        r.place_rocks();
        r
    }

    /// The centreline through the knots, a station every `step` metres of
    /// its length. Each span is a Hermite curve whose tangent at a knot runs
    /// from the knot before to the knot after, scaled to the span's own
    /// length, so a short span next to a long one does not loop.
    fn lay_out(&mut self) {
        let o = ONE as i64;
        let pts: Vec<[i64; 2]> = self
            .spec
            .line
            .iter()
            .map(|p| [p[0] as i64 * o, p[1] as i64 * o])
            .collect();
        let n = pts.len();
        if n < 2 {
            return;
        }
        // the unit direction through each knot
        let dir: Vec<[i64; 2]> = (0..n)
            .map(|k| {
                let (a, b) = (pts[k.saturating_sub(1)], pts[(k + 1).min(n - 1)]);
                let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
                let l = len2(dx, dz).max(1);
                [dx * o / l, dz * o / l]
            })
            .collect();
        let step = self.step as i64;
        let mut stations: Vec<[i64; 2]> = vec![pts[0]];
        let mut knots = vec![0usize];
        // the length run since the last station, and the last sample
        let mut since: i64 = 0;
        let mut prev = pts[0];
        for k in 0..n - 1 {
            let (p0, p1) = (pts[k], pts[k + 1]);
            let l = len2(p1[0] - p0[0], p1[1] - p0[1]);
            let t0 = [dir[k][0] * l / o, dir[k][1] * l / o];
            let t1 = [dir[k + 1][0] * l / o, dir[k + 1][1] * l / o];
            // samples every half metre or so along the span
            let samples = (l / (o / 2)).max(1);
            for q in 1..=samples {
                let p = hermite(p0, p1, t0, t1, q * o / samples);
                let mut from = prev;
                let mut seg = len2(p[0] - from[0], p[1] - from[1]);
                // a station wherever a whole step of length is passed
                while seg > 0 && since + seg >= step {
                    let need = step - since;
                    let at = [
                        from[0] + (p[0] - from[0]) * need / seg,
                        from[1] + (p[1] - from[1]) * need / seg,
                    ];
                    stations.push(at);
                    since = 0;
                    seg -= need;
                    from = at;
                }
                since += seg;
                prev = p;
            }
            knots.push(stations.len() - 1);
        }
        // s = 0 at the station nearest where the line enters the square (its
        // first knot at or past z = 0)
        let entry = self.spec.line.iter().position(|p| p[1] >= 0).unwrap_or(0);
        let zero = knots[entry] as i64;
        let count = stations.len();
        let s_of = |i: usize| ((i as i64 - zero) * step) as Fx;
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let (a, b) = (stations[i.saturating_sub(1)], stations[(i + 1).min(count - 1)]);
            let d = v3((b[0] - a[0]) as Fx, 0, (b[1] - a[1]) as Fx).norm();
            out.push(Station {
                at: v3(stations[i][0] as Fx, 0, stations[i][1] as Fx),
                dir: d,
                s: s_of(i),
                half: 0,
                bed: 0,
                depth: 0,
                speed: 0,
                froude: 0,
                regime: solve::Regime::Calm,
                riffle: false,
                curve: 0,
                widen: 0,
            });
        }
        // the half width, widening and narrowing smoothly round the given points
        let base = cm(self.spec.half_cm);
        for st in &mut out {
            let mut half = base;
            for w in &self.spec.widths {
                let off = (st.at.z - w.z * ONE).abs();
                let reach = w.reach_m.max(1) * ONE;
                if off < reach {
                    let k = smooth(ONE - fx::div(off, reach));
                    half += fx::mul(cm(w.half_cm) - base, k);
                }
            }
            st.half = half;
            st.riffle = self
                .spec
                .riffles
                .iter()
                .any(|r| st.at.z >= r.from_z * ONE && st.at.z < r.to_z * ONE);
        }
        for i in 0..count {
            let (lo, hi) = (i.saturating_sub(1), (i + 1).min(count - 1));
            let span = (hi - lo).max(1) as i32;
            out[i].widen = (out[hi].half - out[lo].half) / span;
            // the turn a metre: the change of heading over the span, in
            // radians (sin of a small angle), right positive
            let (a, b) = (out[lo].dir, out[hi].dir);
            let cross = fx::mul(a.x, b.z) - fx::mul(a.z, b.x);
            out[i].curve = fx::div(cross, self.step * span);
        }
        self.stations = out;
        self.knots = knots;
    }

    /// The steady flow, between the stations whose z lies in the solved
    /// range, with the bed set so the water stands at `level_at`.
    fn solve(&mut self) {
        let (from, to) = (self.spec.solve_from_z * ONE, self.spec.solve_to_z * ONE);
        let n = self.stations.len();
        let first = self.stations.iter().position(|s| s.at.z >= from).unwrap_or(0);
        let last = self
            .stations
            .iter()
            .rposition(|s| s.at.z <= to)
            .map(|k| k + 1)
            .unwrap_or(n);
        self.solved = (first, last.max(first + 1).min(n));
        // the bed falls station by station: slope_per_1000 a metre, or the riffle's
        let mut bed = vec![0 as Fx; n];
        for i in 1..n {
            let slope = if self.stations[i].riffle {
                self.spec
                    .riffles
                    .iter()
                    .find(|r| {
                        self.stations[i].at.z >= r.from_z * ONE && self.stations[i].at.z < r.to_z * ONE
                    })
                    .map(|r| r.slope_per_1000)
                    .unwrap_or(self.spec.slope_per_1000)
            } else {
                self.spec.slope_per_1000
            };
            bed[i] = bed[i - 1] - self.step * slope / 1000;
        }
        let (a, b) = self.solved;
        let steps: Vec<solve::Cross> = (a..b)
            .map(|i| solve::Cross {
                bed: solve::from16(bed[i]),
                width: solve::from16(self.stations[i].half * 2),
                n: (if self.stations[i].riffle {
                    self.spec.n_riffle_x1000
                } else {
                    self.spec.n_x1000
                }) as i64
                    * solve::ONE
                    / 1000,
            })
            .collect();
        let q = solve::from16(self.spec.discharge_m3_s.max(1) * ONE);
        let (flow, jumps) = solve::solve(&steps, q, solve::from16(self.step));
        for (k, f) in flow.iter().enumerate() {
            let st = &mut self.stations[a + k];
            st.depth = solve::to16(f.depth);
            st.speed = solve::to16(f.speed);
            st.froude = solve::to16(f.froude);
            st.regime = f.regime;
        }
        // beyond the solved stretch the flow carries on as it stands at its ends
        let (top, bottom) = (self.stations[a], self.stations[b - 1]);
        for i in 0..n {
            if i < a || i >= b {
                let from = if i < a { top } else { bottom };
                let st = &mut self.stations[i];
                st.depth = from.depth;
                st.speed = from.speed;
                st.froude = from.froude;
                st.regime = solve::Regime::Calm;
            }
        }
        // the level: the water stands at level_at where the river crosses its z
        let at = self
            .stations
            .iter()
            .position(|s| s.at.z >= self.spec.level_at.z * ONE)
            .unwrap_or(0);
        let lift = cm(self.spec.level_at.cm) - (bed[at] + self.stations[at].depth);
        for (i, st) in self.stations.iter_mut().enumerate() {
            st.bed = bed[i] + lift;
            st.at.y = st.bed + st.depth;
        }
        self.jumps = jumps
            .iter()
            .map(|j| {
                let (h1, h2) = (solve::to16(j.h1), solve::to16(j.h2));
                Jump {
                    s: self.stations[a + j.at].s,
                    h1,
                    h2,
                    v1: solve::to16(j.v1),
                    fr1: solve::to16(j.fr1),
                    roller: (4 * (h2 - h1)).clamp(ONE * 3 / 2, 6 * ONE),
                }
            })
            .collect();
    }

    fn place_rocks(&mut self) {
        let mut rocks = Vec::new();
        for p in &self.spec.piers {
            let k = self
                .stations
                .iter()
                .position(|s| s.at.z >= p.z * ONE)
                .unwrap_or(0);
            let st = self.stations[k];
            for &a in &p.across_cm {
                let across = cm(a);
                let right = v3(-st.dir.z, 0, st.dir.x);
                rocks.push(Rock {
                    at: v3(st.at.x, 0, st.at.z).add(right.scale(across)),
                    r: cm(p.radius_cm),
                    top: st.at.y + cm(p.rise_cm),
                    flow: st.dir,
                    speed: fx::mul(st.speed, profile(fx::div(across, st.half.max(1)))),
                    s: st.s,
                    across,
                });
            }
        }
        for isl in &self.spec.islets {
            let at = v3(isl.at[0] * ONE, 0, isl.at[1] * ONE);
            if let Some(n) = self.near(at.x, at.z, 50 * ONE) {
                let st = self.station(n.s);
                rocks.push(Rock {
                    at,
                    r: self.islet_waterline(isl),
                    top: st.at.y + cm(isl.top_cm),
                    flow: st.dir,
                    speed: st.speed,
                    s: n.s,
                    across: n.across,
                });
            }
        }
        rocks.sort_by_key(|k| k.s);
        self.rocks = rocks;
    }

    /// How far out from an islet's middle its shore lies: where its dome,
    /// `top_cm` over the water at the middle and down to the bed at
    /// `radius_cm`, meets the water.
    pub fn islet_waterline(&self, isl: &IsletSpec) -> Fx {
        let at = v3(isl.at[0] * ONE, 0, isl.at[1] * ONE);
        let depth = self
            .near(at.x, at.z, 50 * ONE)
            .map(|n| n.depth * 3 / 2)
            .unwrap_or(ONE);
        let top = cm(isl.top_cm);
        // (r/R)² = top / (top + depth)
        let share = fx::div(top, (top + depth).max(1));
        fx::mul(cm(isl.radius_cm), fx::sqrt(share))
    }

    /// The station at `s` metres down, its numbers interpolated.
    pub fn station(&self, s: Fx) -> Station {
        let n = self.stations.len();
        if n == 0 {
            return Station {
                at: V3::ZERO,
                dir: v3(0, 0, ONE),
                s,
                half: ONE,
                bed: 0,
                depth: 0,
                speed: 0,
                froude: 0,
                regime: solve::Regime::Calm,
                riffle: false,
                curve: 0,
                widen: 0,
            };
        }
        let s0 = self.stations[0].s;
        let rel = (s - s0).max(0) as i64;
        let i = ((rel / self.step as i64) as usize).min(n - 1);
        let j = (i + 1).min(n - 1);
        let t = ((rel - i as i64 * self.step as i64) * ONE as i64 / self.step as i64).clamp(0, ONE as i64) as Fx;
        let (a, b) = (&self.stations[i], &self.stations[j]);
        let l = |p: Fx, q: Fx| p + fx::mul(q - p, t);
        Station {
            at: a.at.lerp(b.at, t),
            dir: a.dir.lerp(b.dir, t).norm(),
            s,
            half: l(a.half, b.half),
            bed: l(a.bed, b.bed),
            depth: l(a.depth, b.depth),
            speed: l(a.speed, b.speed),
            froude: l(a.froude, b.froude),
            regime: if t < ONE / 2 { a.regime } else { b.regime },
            riffle: if t < ONE / 2 { a.riffle } else { b.riffle },
            curve: l(a.curve, b.curve),
            widen: l(a.widen, b.widen),
        }
    }

    /// The point `s` down the river and `across` from its centreline.
    pub fn point(&self, s: Fx, across: Fx) -> V3 {
        let st = self.station(s);
        let right = v3(-st.dir.z, 0, st.dir.x);
        v3(st.at.x, 0, st.at.z).add(right.scale(across))
    }

    /// The nearest point of the centreline's knots' chords to (x, z): the
    /// chord's index, its parameter (Q16) and the squared distance, in
    /// 256ths of a metre, squared (i64).
    fn nearest_chord(&self, x: Fx, z: Fx) -> Option<(usize, i64, i64)> {
        let line = &self.spec.line;
        let o = ONE as i64;
        let (px, pz) = (x as i64 >> 8, z as i64 >> 8);
        let mut best: Option<(usize, i64, i64)> = None;
        for k in 0..line.len().saturating_sub(1) {
            let (ax, az) = (line[k][0] as i64 * 256, line[k][1] as i64 * 256);
            let (dx, dz) = (line[k + 1][0] as i64 * 256 - ax, line[k + 1][1] as i64 * 256 - az);
            let l2 = (dx * dx + dz * dz).max(1);
            let dot = (px - ax) * dx + (pz - az) * dz;
            let t = (dot * o / l2).clamp(0, o);
            let (ex, ez) = (px - ax - dx * t / o, pz - az - dz * t / o);
            let d2 = ex * ex + ez * ez;
            if best.map(|b| d2 < b.2).unwrap_or(true) {
                best = Some((k, t, d2));
            }
        }
        best
    }

    /// How far (x, z) is from the knots' chords, a coarse distance to the
    /// river for the valley it cuts through the mountains (`terrain.rs`),
    /// Q16 metres in i64.
    pub fn coarse_distance(&self, x: Fx, z: Fx) -> i64 {
        self.nearest_chord(x, z)
            .map(|b| (crate::terrain::root(b.2 as u64) as i64) << 8)
            .unwrap_or(i64::MAX)
    }

    /// Where (x, z) lies against the river, if it is within `reach` of the
    /// centreline (plus the curve's sway from its chords).
    pub fn near(&self, x: Fx, z: Fx, reach: Fx) -> Option<Near> {
        self.near_from(x, z, reach, &mut None)
    }

    /// The same, the search starting from the station `hint` holds (the
    /// last point's, for a run of nearby points), and left there.
    pub fn near_from(&self, x: Fx, z: Fx, reach: Fx, hint: &mut Option<usize>) -> Option<Near> {
        let n = self.stations.len();
        if n < 2 {
            return None;
        }
        let guess = match *hint {
            Some(k) => k.min(n - 1),
            None => {
                let (k, t, d2) = self.nearest_chord(x, z)?;
                // the curve strays from its chords by tens of metres at the most
                let far = (reach as i64 >> 8) + 400 * 256;
                if d2 > far * far {
                    return None;
                }
                let (a, b) = (self.knots[k], self.knots[(k + 1).min(self.knots.len() - 1)]);
                a + ((b - a) as i64 * t / ONE as i64) as usize
            }
        };
        let d2 = |i: usize| {
            let p = self.stations[i].at;
            let (dx, dz) = ((p.x - x) as i64, (p.z - z) as i64);
            dx * dx + dz * dz
        };
        let mut i = guess.min(n - 1);
        while i > 0 && d2(i - 1) < d2(i) {
            i -= 1;
        }
        while i + 1 < n && d2(i + 1) < d2(i) {
            i += 1;
        }
        *hint = Some(i);
        // project onto the span before or after the nearest station
        let mut found: Option<Near> = None;
        for lo in [i.saturating_sub(1), i] {
            let hi = (lo + 1).min(n - 1);
            if hi == lo {
                continue;
            }
            let (p, q) = (self.stations[lo].at, self.stations[hi].at);
            let (dx, dz) = ((q.x - p.x) as i64, (q.z - p.z) as i64);
            let l2 = (dx * dx + dz * dz).max(1);
            let dot = (x - p.x) as i64 * dx + (z - p.z) as i64 * dz;
            // past the river's ends the line runs straight on
            let t = if lo == 0 || hi == n - 1 {
                dot * ONE as i64 / l2
            } else {
                (dot * ONE as i64 / l2).clamp(0, ONE as i64)
            };
            let (cx, cz) = (p.x as i64 + dx * t / ONE as i64, p.z as i64 + dz * t / ONE as i64);
            let (ex, ez) = (x as i64 - cx, z as i64 - cz);
            let dist = len2(ex, ez);
            if found.map(|f| dist < f.d as i64).unwrap_or(true) {
                // right of the way downstream is positive across
                let side = dx * ez - dz * ex;
                let tt = t.clamp(0, ONE as i64) as Fx;
                let (sa, sb) = (&self.stations[lo], &self.stations[hi]);
                let l = |u: Fx, w: Fx| u + fx::mul(w - u, tt);
                found = Some(Near {
                    d: dist.min(i32::MAX as i64) as Fx,
                    across: if side >= 0 { dist as Fx } else { -(dist as Fx) },
                    s: sa.s + ((t * self.step as i64) / ONE as i64) as Fx,
                    surface: l(sa.at.y, sb.at.y),
                    half: l(sa.half, sb.half),
                    depth: l(sa.depth, sb.depth),
                    k: lo,
                });
            }
        }
        found.filter(|f| f.d <= reach)
    }
}

/// The speed across the channel as a share of the mean: 1.25 (1 − u⁴), whose
/// mean across is one, so the discharge holds (the fishing game's `bake.rs`).
pub fn profile(u: Fx) -> Fx {
    let u2 = fx::mul(u, u);
    (fx::mul(ONE - fx::mul(u2, u2), ONE * 5 / 4)).max(0)
}

// ---- the waves (after the fishing game's sea.rs) ----

/// The waves the water shader sums: four, two `mat4`s of them (a shader's
/// array size would be a number of its own, D3).
pub const WAVES: usize = 4;

/// Gravity, 9.81 m/s², Q16.
pub const G: Fx = 642_908;
/// A full turn in radians, 6.283185, Q16.
pub const TAU: Fx = 411_775;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WaveSpec {
    pub toward_deg: i32,
    pub length_cm: i32,
    pub amp_mm: i32,
    pub steep_pct: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MeshSpec {
    pub along_cm: i32,
    pub fine_along_cm: i32,
    pub fine_reach_m: i32,
    pub across_cm: i32,
    pub under_m: i32,
    pub chunk_m: i32,
    pub near_m: i32,
    pub band_pct: i32,
    pub far_along_m: i32,
    pub far_half_m: i32,
    pub pond_cell_cm: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LookSpec {
    pub deep: [i32; 3],
    pub shallow: [i32; 3],
    pub crest: [i32; 3],
    pub foam: [i32; 3],
    pub fresnel_base: i32,
    pub fresnel_power: i32,
    pub crest_power: i32,
    pub glint: [i32; 4],
    pub body: [i32; 4],
    pub crest_light: [i32; 4],
    pub foam_fold: [i32; 4],
    pub foam_edge: [i32; 4],
    pub detail: [i32; 4],
    pub detail_fade_m: i32,
    pub flow: [i32; 4],
    pub streak: [i32; 4],
    pub hash: [i32; 4],
    pub noise_period: i32,
    pub see_through: [i32; 3],
    pub shore: [i32; 3],
    pub lod_waves: [i32; 2],
}

/// `data/water.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WaterSpec {
    pub waves: Vec<WaveSpec>,
    pub mesh: MeshSpec,
    pub look: LookSpec,
}

impl WaterSpec {
    pub fn parse(json: &str) -> Result<WaterSpec, String> {
        let s: WaterSpec =
            serde_json::from_str(json).map_err(|e| format!("The water file could not be read: {e}"))?;
        if s.waves.len() != WAVES {
            return Err(format!(
                "The water file gives {} waves; the water is drawn with {WAVES}.",
                s.waves.len()
            ));
        }
        Ok(s)
    }

    pub fn shipped() -> WaterSpec {
        WaterSpec::parse(include_str!("../../../data/water.json")).expect("data/water.json")
    }
}

/// One wave, ready to sum (the fishing game's `sea::Wave`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wave {
    /// unit direction of travel (x, z)
    pub dx: Fx,
    pub dz: Fx,
    pub length: Fx,
    pub amp: Fx,
    /// how far a crest sways sideways
    pub sway: Fx,
    /// k × A
    pub slope: Fx,
    /// the binary angle added each tick
    pub step: u32,
}

/// The period of a deep-water wave `length` long: √(2πL/g), Q16 seconds.
pub fn period(length: Fx) -> Fx {
    fx::sqrt(fx::div(fx::mul(TAU, length), G))
}

/// The binary angle a tick for a wave of `period` (Q16 seconds).
pub fn step_for_period(period: Fx) -> u32 {
    if period <= 0 {
        return 0;
    }
    ((1u64 << 48) / (period as u64 * crate::ticks::TICKS_PER_SECOND as u64)) as u32
}

impl Wave {
    pub fn new(s: &WaveSpec, count: usize) -> Wave {
        let a = s.toward_deg * fx::TURN / 360;
        let (dx, dz) = (fx::sin(a), fx::cos(a));
        let length = s.length_cm.max(1) * ONE / 100;
        let amp = s.amp_mm * ONE / 1000;
        let k = fx::div(TAU, length);
        let steep = s.steep_pct.clamp(0, 100) * ONE / 100;
        // N waves at their sharpest together still do not loop over
        let sway = fx::div(steep, k * count.max(1) as i32);
        Wave {
            dx,
            dz,
            length,
            amp,
            sway,
            slope: fx::mul(k, amp),
            step: step_for_period(period(length)),
        }
    }
}

/// The water as it changes: each wave's phase, and the ticks it has run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    pub phase: [u32; WAVES],
    pub ticks: u64,
}

impl State {
    pub fn new() -> State {
        State::default()
    }
}

/// One tick: every wave's phase moves on by its step.
pub fn step(w: &mut crate::world::World) {
    let waves = &w.land.waves;
    for (p, wave) in w.water.phase.iter_mut().zip(waves.iter()) {
        *p = p.wrapping_add(wave.step);
    }
    w.water.ticks += 1;
}

/// The water's static meshes, one group a stretch of river and one for the
/// pond (filled in below).
pub fn scene(
    w: &crate::world::World,
    look: &crate::look::Look,
    meshes: &mut crate::prim::MeshTable,
    out: &mut Vec<crate::render::Group>,
) {
    let _ = (w, look, meshes, out);
}
