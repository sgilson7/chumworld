//! `wind` (M7 Polish, task P2 "land", R9.54): one wind over the meadow,
//! after the waves of Extremely Lifelike Fishing's water (`water.rs`,
//! R4.9). A few travelling waves summed, their directions, lengths, speeds
//! and strengths in `data/wind.json`; their phases are wrapping binary
//! angles stepped once a tick, so neither the core nor the shader keeps a
//! growing time (CW-4, as the water's).
//!
//! The same sum (`gust`, the shaders' `wind`) bends the near blades, the
//! tufts and the meadow's far clumps (their tips swing, their feet stay),
//! and, on the ground itself where grass grows, rolls bands of light sheen
//! and shadow across the field, so the gusts line up where one tier of
//! grass hands over to the next. Held for a picture (`story.still`) or
//! paused, the wind holds still with everything else, so a still view
//! draws the same pixels frame after frame (CW-18).
//!
//! Steps at 8 in `World::step`, beside the water.

use serde::Deserialize;

use crate::fx::{self, Fx, ONE};
use crate::geom::V3;

/// How many waves the wind sums: four, one `mat4`'s columns.
pub const WAVES: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WaveSpec {
    /// the way it blows toward, degrees (0 north, 90 west, as headings)
    pub toward_deg: i32,
    pub length_m: i32,
    /// how fast its gusts travel, centimetres a second
    pub speed_cms: i32,
    /// its share of the whole, percent; the four sum to 100 at most
    pub amp_pct: i32,
}

/// `data/wind.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub waves: Vec<WaveSpec>,
    /// how far a blade's tip swings at a full gust, centimetres
    pub bend_cm: i32,
    /// how much a gust lightens the grass under it, and its lull darkens it
    pub sheen_pct: i32,
    pub shadow_pct: i32,
    /// the darker troughs between the grass's clumps: how dark, and the two
    /// wavelengths crossing (cm)
    pub trough_pct: i32,
    pub trough_cm: [i32; 2],
    /// how much greener than red and blue a ground's colour must be to be
    /// grass (0 to 255), and how much more it takes to be wholly grass
    pub grass_key: [i32; 2],
}

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let s: Spec =
            serde_json::from_str(json).map_err(|e| format!("The wind file could not be read: {e}"))?;
        if s.waves.len() != WAVES {
            return Err(format!("The wind file needs {WAVES} waves."));
        }
        if s.waves.iter().map(|w| w.amp_pct).sum::<i32>() > 100 {
            return Err("The wind file's waves must sum to 100% at most.".into());
        }
        Ok(s)
    }

    pub fn shipped() -> Spec {
        Spec::parse(include_str!("../../../data/wind.json")).expect("data/wind.json")
    }
}

/// The shipped wind, read once.
pub fn spec() -> &'static Spec {
    // (read once; the tests read the file again as they need)
    static SPEC: std::sync::OnceLock<Spec> = std::sync::OnceLock::new();
    SPEC.get_or_init(Spec::shipped)
}

/// The phases, binary angles (a full turn is 2³²).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    pub phase: [u32; WAVES],
}

/// One wave's numbers in Q16: its unit direction (x, z), radians a metre,
/// its share; and its phase's step a tick (binary angle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wave {
    pub dx: Fx,
    pub dz: Fx,
    pub k: Fx,
    pub amp: Fx,
    pub step: u32,
}

impl Wave {
    pub fn new(w: &WaveSpec) -> Wave {
        let a = fx::deg(w.toward_deg);
        // a heading's way: facing(a) is (−sin a, 0, −cos a), north at 0
        let (dx, dz) = (-fx::sin(a), -fx::cos(a));
        let len = w.length_m.max(1) as i64;
        // a turn every length: 2π / λ radians a metre
        let k = (crate::water::TAU as i64 / len) as Fx;
        // speed / λ turns a second, / 60 a tick, as a binary angle
        let step = ((w.speed_cms as i64) * (1i64 << 32) / 100 / len / crate::ticks::TICKS_PER_SECOND) as u32;
        Wave {
            dx,
            dz,
            k,
            amp: w.amp_pct * ONE / 100,
            step,
        }
    }
}

pub fn waves(s: &Spec) -> Vec<Wave> {
    s.waves.iter().map(Wave::new).collect()
}

/// Step 8: the gusts move on, unless the world is held still for a view.
pub fn step(w: &mut crate::world::World) {
    if w.story.still {
        return;
    }
    let ws = waves(spec());
    for (p, wave) in w.wind.phase.iter_mut().zip(&ws) {
        *p = p.wrapping_add(wave.step);
    }
}

/// The wind at (x, z) in the world: −ONE to ONE, the sum of each wave's
/// share times sin(k (d · p) − phase) (what the shaders' `wind` sums).
pub fn gust(s: &Spec, st: &State, x: Fx, z: Fx) -> Fx {
    let mut sum = 0i64;
    for (wave, &phase) in waves(s).iter().zip(&st.phase) {
        // the along-wave distance in turns: (d · p) / λ
        let along = (wave.dx as i64 * x as i64 + wave.dz as i64 * z as i64) >> 16;
        let turns = along * wave.k as i64 / crate::water::TAU as i64;
        let angle = ((turns << 16) as u32).wrapping_sub(phase);
        sum += fx::mul(wave.amp, fx::sin((angle >> 16) as i32 * (fx::TURN / 65_536))) as i64;
    }
    sum as Fx
}

/// Whether a ground's colour (Q16 0 to ONE a channel) is grass, as the
/// shader shades it: greener than its red and blue by the first key, and
/// how much (0 to ONE, wholly at the second key; `scene.frag`'s smoothstep,
/// here a straight ramp, the same at its ends).
pub fn grass_share(s: &Spec, c: crate::look::Rgb) -> Fx {
    let green = c[1] - c[0].max(c[2]);
    let (a, b) = (s.grass_key[0] * ONE / 255, s.grass_key[1] * ONE / 255);
    ((green - a) as i64 * ONE as i64 / (b - a).max(1) as i64).clamp(0, ONE as i64) as Fx
}

/// The wind's uniforms, once: the waves' directions, radians a metre and
/// shares (`uWindA`, a column a field); the bend (m), sheen and shadow, the trough's
/// darkness (`uWindK`); the troughs' radians a metre and the grass's keys
/// (`uTrough`).
pub const UNIFORMS: &[(&str, &str, usize)] = &[
    ("uWindA", "uniformMatrix4fv", 16),
    ("uWindK", "uniform4fv", 4),
    ("uTrough", "uniform4fv", 4),
    ("uTroughDir", "uniform4fv", 4),
    ("uWindAA", "uniform4fv", 4),
];

/// The troughs' two directions (`uTroughDir`): along (1, 0.6) and (−0.4, 1).
pub const TROUGH_DIRS: [(Fx, Fx); 2] = [(ONE, ONE * 6 / 10), (-ONE * 4 / 10, ONE)];

pub fn uniform_values(s: &Spec) -> Vec<i32> {
    // a column a field, a row a wave (the shaders multiply to pick them)
    let ws = waves(s);
    let mut out: Vec<i32> = ws.iter().map(|w| w.dx).collect();
    out.extend(ws.iter().map(|w| w.dz));
    out.extend(ws.iter().map(|w| w.k));
    out.extend(ws.iter().map(|w| w.amp));
    let pct = |v: i32| v * ONE / 100;
    out.extend([
        s.bend_cm * ONE / 100,
        pct(s.sheen_pct),
        pct(s.shadow_pct),
        pct(s.trough_pct),
    ]);
    let k = |cm: i32| (crate::water::TAU as i64 * 100 / cm.max(1) as i64) as Fx;
    out.extend([
        k(s.trough_cm[0]),
        k(s.trough_cm[1]),
        s.grass_key[0] * ONE / 255,
        s.grass_key[1] * ONE / 255,
    ]);
    out.extend([
        TROUGH_DIRS[0].0,
        TROUGH_DIRS[0].1,
        TROUGH_DIRS[1].0,
        TROUGH_DIRS[1].1,
    ]);
    // a pattern fades as its phase changes across a pixel, gone at a radian
    // a pixel (troughs, then gusts): finer, it would crawl as the view pans
    out.extend([ONE, ONE, 0, 0]);
    out
}

/// The wind's values a frame (`uWindPhase`, `uTroughPhase`): each wave's
/// phase as the shader adds it, in radians wrapped to a turn, with the
/// render origin's share taken in (the shaders' positions are from the
/// origin, `render::origin_for`, so the field does not jump when it moves);
/// and the troughs' two phases at the origin.
pub const FRAME: &[(&str, &str, usize)] =
    &[("uWindPhase", "uniform4fv", 4), ("uTroughPhase", "uniform2fv", 2)];

/// A binary angle in radians, −π to π, Q16.
fn radians(a: u32) -> Fx {
    ((a as i32 as i64 * crate::water::TAU as i64) >> 32) as Fx
}

pub fn frame_values(s: &Spec, st: &State, origin: V3) -> Vec<i32> {
    let mut out = Vec::new();
    for (wave, &phase) in waves(s).iter().zip(&st.phase) {
        // phase − k (d · origin), as a binary angle: the origin's along-wave
        // distance in turns, (d · o) / λ
        let along = (wave.dx as i64 * origin.x as i64 + wave.dz as i64 * origin.z as i64) >> 16;
        let turns = along * wave.k as i64 / crate::water::TAU as i64;
        out.push(radians(phase.wrapping_sub((turns << 16) as u32)));
    }
    // the troughs run along (1, 0.6) and (−0.4, 1) (`scene.frag`): their
    // phase at the origin, in turns of their wavelengths
    for (cm, (a, b)) in s.trough_cm.iter().zip(TROUGH_DIRS) {
        let along = (a as i64 * origin.x as i64 + b as i64 * origin.z as i64) >> 16;
        let turns = (along * 100 / (*cm).max(1) as i64) as u32;
        out.push(radians(turns << 16));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::int;

    /// A wave toward 90° (west) 40 m long at 4 m/s: a tenth of a turn a
    /// second, 2³² / 10 / 60 = 7 158 278 a tick; its direction (−1, 0); 2π /
    /// 40 = 0.157 radians a metre, 10 294 in Q16.
    #[test]
    fn a_gust_wave_steps_a_tenth_of_a_turn_a_second() {
        let w = Wave::new(&WaveSpec {
            toward_deg: 90,
            length_m: 40,
            speed_cms: 400,
            amp_pct: 50,
        });
        assert!((w.step as i64 - 7_158_278).abs() <= 1, "{}", w.step);
        assert!(w.dx < -ONE + 4 && w.dz.abs() <= 4, "{} {}", w.dx, w.dz);
        assert!((w.k - 10_294).abs() <= 1, "{}", w.k);
        assert_eq!(w.amp, ONE / 2);
    }

    /// CW-4 and CW-18: the same ticks give the same wind, whatever the seed;
    /// held still, it holds; and the shaders take the origin's share from
    /// the core, so a point's wind is the same whichever origin draws it.
    #[test]
    fn the_wind_is_the_same_for_the_same_ticks_and_holds_when_still() {
        let run = |seed: u64| {
            let mut w = crate::world::World::new(seed);
            for _ in 0..90 {
                w.step(&crate::input::Input::default());
            }
            w.wind
        };
        assert_eq!(run(1), run(9));
        assert_ne!(run(1), State::default());
        let mut w = crate::world::World::new(1);
        w.story.still = true;
        let before = w.wind;
        for _ in 0..30 {
            w.step(&crate::input::Input::default());
        }
        assert_eq!(w.wind, before, "held still");
        // the wind at a point: sum a share sin(k d·p − φ); at the origin
        // with no phase, sin 0 = 0
        let s = Spec::shipped();
        assert!(gust(&s, &State::default(), 0, 0).abs() <= 8);
        let st = run(1);
        let g = gust(&s, &st, int(1234), int(2345));
        assert!(g.abs() <= ONE, "{g}");
    }

    /// The frame's phases carry the origin: for a point p, the shader's
    /// k d · (p − o) − φ' equals the core's k d · p − φ, so the blades and
    /// the ground at the hand-over, both drawn from the same origin by the
    /// same sum, meet the same gust; tested at the origin itself, where the
    /// shader's term is −φ': sin(−φ') = sin(k d·o − φ) = the core's gust
    /// there, a wave at a time.
    /// R9.54: the blades and the ground meet the same wind, the same sum in
    /// both shaders, word for word; and the ground is grass where the
    /// meadow is and not where the road, the river's bed or rock are.
    #[test]
    fn the_blades_and_the_ground_sum_the_same_wind_and_agree_where_grass_is() {
        let vert = include_str!("../../../data/shaders/scene.vert");
        let frag = include_str!("../../../data/shaders/scene.frag");
        let body = |src: &str| {
            let at = src.find("float wind(vec2 p) {").expect("a wind function");
            src[at..at + src[at..].find("\n}").unwrap()].to_string()
        };
        assert_eq!(body(vert), body(frag));
        assert!(vert.contains("w.xz += vec2(uWindA[0].x, uWindA[1].x) * (wind(w.xz) * bend * uWindK.x);"));
        assert!(frag.contains("float gust = wind(p) * coarse;"));
        let s = Spec::shipped();
        let rgb = |c: [i32; 3]| crate::look::rgb(c);
        // the meadow's grasses (data/land.json): 186 − 150 = 36 over the key
        // of 34: wholly grass; the sun-dried patch 192, 190: none
        assert_eq!(grass_share(&s, rgb([150, 186, 104])), ONE);
        assert_eq!(grass_share(&s, rgb([198, 168, 122])), 0, "the road");
        assert_eq!(grass_share(&s, rgb([126, 120, 94])), 0, "the river's bed");
        assert_eq!(grass_share(&s, rgb([152, 142, 130])), 0, "rock");
        // the forest floor, 138 − 102 = 36: grass-shaded too, as the
        // meadow under the trees is
        assert_eq!(grass_share(&s, rgb([102, 138, 78])), ONE);
    }

    #[test]
    fn the_frame_s_phases_carry_the_origin() {
        let s = Spec::shipped();
        let st = State {
            phase: [12_345_678, 2_000_000_000, 3_000_000_000, 777],
        };
        let o = crate::geom::v3(int(1792), 0, int(2560));
        let f = frame_values(&s, &st, o);
        let core = gust(&s, &st, o.x, o.z) as i64;
        let ws = waves(&s);
        let mut shader = 0i64;
        for (w, &phi) in ws.iter().zip(&f) {
            // sin(−φ') with φ' in radians: −φ' / 2π turns
            let a = (-(phi as i64) * fx::TURN as i64 / crate::water::TAU as i64) as i32;
            shader += fx::mul(w.amp, fx::sin(a)) as i64;
        }
        assert!((shader - core).abs() < ONE as i64 / 100, "{shader} {core}");
    }
}
