//! Making a voice's samples from its layers (`data/sound.json`), in whole
//! numbers: Q16 samples (`ONE` is full scale) at `Spec::rate` a second.
//! Each layer is a wave or a noise, its pitch swept straight from one
//! frequency to another, through an attack, a hold and a fall, then through
//! its filters; the layers are summed, the voice's ends eased so nothing
//! clicks, a loop's end crossfaded into its start, and the whole scaled so
//! its loudest sample is `peak_pct` of full scale. The same row makes the
//! same samples on every machine (D2): the noise comes from a generator
//! seeded by the voice's name and the layer's place.

use super::{Layer, Spec, VoiceSpec};
use crate::fx::{ONE, TURN};
use std::sync::OnceLock;

/// Q30, the envelope's and the filters' precision.
const Q30: i64 = 1 << 30;
const SINE_BITS: u32 = 12;

/// A sine table, a turn in 4 096 steps, from `fx::sin`.
fn sine_table() -> &'static [i32] {
    static T: OnceLock<Vec<i32>> = OnceLock::new();
    T.get_or_init(|| {
        let n = 1 << SINE_BITS;
        (0..=n).map(|k| crate::fx::sin(k * (TURN / n))).collect()
    })
}

/// The sine of a phase (a full turn is 2³²), Q16, between table steps by a
/// straight line.
fn sine(phase: u32) -> i64 {
    let t = sine_table();
    let k = (phase >> (32 - SINE_BITS)) as usize;
    let frac = ((phase << SINE_BITS) >> 16) as i64; // 0..65 535
    let (a, b) = (t[k] as i64, t[k + 1] as i64);
    a + (((b - a) * frac) >> 16)
}

/// A generator of whole-number noise (splitmix64), seeded by the voice and
/// the layer, kept apart from the world's chance (`rng.rs`).
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// From −ONE to ONE.
    fn sample(&mut self) -> i64 {
        ((self.next() >> 47) as i64) - ONE as i64
    }
}

fn seed_of(name: &str, layer: usize) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h ^ (layer as u64).wrapping_mul(0x9e37_79b9)
}

/// A one-pole filter's share a sample for a corner at `hz`: 2πf / (r + 2πf),
/// Q30.
fn pole(hz: i64, rate: i64) -> i64 {
    let w = hz * 6_283_185 / 1_000_000; // 2π f, whole radians a second
    let w = w.max(1);
    w * Q30 / (rate + w)
}

/// The resonant band's tuning for a centre at `hz`: 2 sin(π f / r), Q30.
fn band_tune(hz: i64, rate: i64) -> i64 {
    // π f / r as a share of a turn is f / (2 r)
    let hz = hz.min(rate / 5);
    let phase = (hz << 31) / rate; // f / 2r of 2³²
    2 * (sine(phase as u32) << 14)
}

/// Samples in `ms` milliseconds.
pub fn samples(ms: i32, rate: i32) -> usize {
    (ms as i64 * rate as i64 / 1000).max(0) as usize
}

/// A layer's samples, `n` long, placed from its start.
fn layer(spec: &Spec, name: &str, k: usize, l: &Layer, n: usize) -> Vec<i64> {
    let rate = spec.rate as i64;
    let mut out = vec![0i64; n];
    let start = samples(l.at_ms, spec.rate).min(n);
    let len = if l.ms > 0 {
        samples(l.ms, spec.rate).min(n - start)
    } else {
        n - start
    };
    if len == 0 {
        return out;
    }
    let mut noise = Noise(seed_of(name, k));
    let attack = samples(l.attack_ms, spec.rate) as i64;
    // the fall: each sample keeps 1 − 1 / (τ r) of the level
    let tau = samples(l.decay_ms, spec.rate) as i64;
    let keep = if tau > 0 { Q30 - Q30 / tau } else { Q30 };
    let mut level = Q30;
    let mut phase: u32 = 0;
    let mut am_phase: u32 = 0;
    let mut vib_phase: u32 = 0;
    let (mut lp, mut lp2, mut hp_lp) = (0i64, 0i64, 0i64);
    let (mut b_low, mut b_band) = (0i64, 0i64);
    let mut crack = 0i64;
    let crack_keep = Q30 - Q30 / (rate / 1400).max(1);
    let sweep = |r: [i32; 2], i: i64| r[0] as i64 + (r[1] as i64 - r[0] as i64) * i / len as i64;
    let turn32 = 1i64 << 32;
    for i in 0..len as i64 {
        // the source
        let mut hz = sweep(l.hz, i);
        if l.vib_pct > 0 {
            let v = sine(vib_phase);
            hz += hz * l.vib_pct as i64 * v / 100 / ONE as i64;
            vib_phase = vib_phase.wrapping_add((l.vib_mhz as i64 * turn32 / (rate * 1000)) as u32);
        }
        let inc = (hz.max(0) * turn32 / rate) as u32;
        let s = match l.wave.as_str() {
            "sine" => sine(phase),
            "tri" => {
                let p = (phase >> 16) as i64; // 0..65 535
                if p < 32_768 {
                    p * 4 - ONE as i64
                } else {
                    3 * ONE as i64 - p * 4
                }
            }
            "square" => {
                if phase < 1 << 31 {
                    ONE as i64
                } else {
                    -(ONE as i64)
                }
            }
            "saw" => ((phase >> 15) as i64) - ONE as i64,
            "crackle" => {
                // a click now and then, each falling away in about 0.7 ms
                let roll = (noise.next() >> 32) as i64;
                if roll * rate < l.density as i64 * (1i64 << 32) {
                    crack = noise.sample();
                }
                let out = crack;
                crack = crack * crack_keep / Q30;
                out
            }
            _ => noise.sample(),
        };
        phase = phase.wrapping_add(inc);
        let mut x = s;
        // the filters, in order: low, high, band
        if l.lp_hz[0] > 0 {
            let a = pole(sweep(l.lp_hz, i), rate);
            lp += (x - lp) * a / Q30;
            lp2 += (lp - lp2) * a / Q30;
            x = lp2;
        }
        if l.hp_hz[0] > 0 {
            let a = pole(sweep(l.hp_hz, i), rate);
            hp_lp += (x - hp_lp) * a / Q30;
            x -= hp_lp;
        }
        if l.bp_hz[0] > 0 {
            let f = band_tune(sweep(l.bp_hz, i), rate);
            let damp = Q30 * 100 / l.bp_q_pct.max(20) as i64;
            b_low += f * b_band / Q30;
            let high = x - b_low - damp * b_band / Q30;
            b_band += f * high / Q30;
            x = b_band;
        }
        // the swell
        if l.am_pct > 0 {
            let v = sine(am_phase); // −ONE..ONE
            let dip = l.am_pct as i64 * (ONE as i64 - v) / 200;
            x = x * (ONE as i64 - dip) / ONE as i64;
            am_phase = am_phase.wrapping_add((l.am_mhz as i64 * turn32 / (rate * 1000)) as u32);
        }
        // the envelope: up over the attack, then the fall
        let shape = if i < attack {
            Q30 * i / attack
        } else {
            let e = level;
            level = level * keep / Q30;
            e
        };
        out[start + i as usize] = x * shape / Q30 * l.gain_pct as i64 / 100;
    }
    out
}

/// A voice's samples, Q16, as the page plays them.
pub fn render(spec: &Spec, name: &str, v: &VoiceSpec) -> Vec<i32> {
    let n = samples(v.ms, spec.rate);
    let fade = samples(spec.loop_fade_ms, spec.rate).min(n / 2);
    // a loop is made a crossfade longer, and its end laid over its start
    let made = if v.looped { n + fade } else { n };
    let mut mix = vec![0i64; made];
    for (k, l) in v.layers.iter().enumerate() {
        for (m, s) in mix.iter_mut().zip(layer(spec, name, k, l, made)) {
            *m += s;
        }
    }
    let mut out: Vec<i64> = if v.looped {
        let mut o = mix[..n].to_vec();
        for i in 0..fade {
            let t = i as i64 * Q30 / fade.max(1) as i64;
            o[i] = (mix[n + i] * (Q30 - t) + mix[i] * t) / Q30;
        }
        o
    } else {
        // eased in over half a millisecond and out over the last five
        let ins = samples(1, spec.rate).max(1) / 2 + 1;
        let outs = samples(5, spec.rate).max(1);
        for (i, m) in mix.iter_mut().enumerate() {
            if i < ins {
                *m = *m * i as i64 / ins as i64;
            }
            let left = n - i;
            if left <= outs {
                *m = *m * (left as i64 - 1).max(0) / outs as i64;
            }
        }
        mix
    };
    let peak = out.iter().map(|s| s.abs()).max().unwrap_or(0);
    if peak > 0 {
        let want = ONE as i64 * v.peak_pct as i64 / 100;
        for s in out.iter_mut() {
            *s = *s * want / peak;
        }
    }
    out.into_iter().map(|s| s as i32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sine_table_reads_the_quarter_turns_exactly() {
        assert_eq!(sine(0), 0);
        assert_eq!(sine(1 << 30), ONE as i64, "a quarter turn");
        assert_eq!(sine(1 << 31), 0, "a half");
        assert_eq!(sine(3 << 30), -(ONE as i64), "three quarters");
        // an eighth: sin 45° = 0.7071 × 65 536 = 46 341
        assert_eq!(sine(1 << 29), 46_341);
    }

    #[test]
    fn a_one_pole_corner_at_a_tenth_of_the_rate_keeps_its_share() {
        // 2π × 2 205 = 13 854 radians a second; 13 854 / (22 050 + 13 854)
        // = 0.38586 of Q30 = 414 309 948
        let a = pole(2205, 22_050);
        assert!((a - 414_309_948).abs() < 200_000, "{a}");
    }
}
