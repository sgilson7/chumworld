//! M7's sound, heard offline (the agent cannot listen): the game is played
//! through `Game` as the page plays it, a frame a tick, and every cue and
//! every loop the core hands the page each frame is mixed here the way
//! `web/sound.js` and Web Audio mix them (a voice's samples stepped at its
//! rate, its gain, an equal-power pan, a loop's level smoothed toward the
//! core's with the same time constant). The mix is written to
//! `shots/m7feel/sound-mix.wav` (16-bit stereo, not committed) for the
//! person to hear, and its levels are printed: the loudest sample before the
//! page's limiter, how much of the mix would clip without it, the loudness
//! second by second, and how many times each voice started. A probe, not a
//! check: it asserts only that the mix is not silent and that nothing is
//! louder than the limiter can hold (+6 dB over full scale).
//!
//!     cargo test --release -p tool_core --test sound_probe -- --ignored --nocapture

use std::collections::BTreeMap;
use std::time::Instant;

use tool_core::fx::ONE;
use tool_core::game::Game;
use tool_core::input::{DRAW, FORWARD, LEFT, RUN, SWING};
use tool_core::sound;
use tool_core::ticks::TICK_US;

fn game() -> Game {
    let mut g = Game::new(
        include_str!("../../../data/strings.json"),
        include_str!("../../../GLOSSARY.md"),
        include_str!("../../../data/look.json"),
        42,
    )
    .unwrap();
    g.resize(1920, 1080, 100);
    g
}

struct Voice {
    data: Vec<f32>,
    looped: bool,
}

struct Playing {
    voice: usize,
    at: f64,
    rate: f64,
    gain: f64,
    pan: f64,
}

struct Mixer {
    rate: f64,
    voices: Vec<Voice>,
    playing: Vec<Playing>,
    loops: BTreeMap<usize, (f64, f64, f64, f64, f64)>, // at, rate, gain now, gain wanted, pan
    smooth: f64,
    out: Vec<(f32, f32)>,
    started: BTreeMap<String, usize>,
    loop_means: BTreeMap<usize, f64>,
    frames: u32,
}

impl Mixer {
    fn new() -> Mixer {
        let s = sound::settings();
        let f = |v: i32| v as f64 / ONE as f64;
        let voices = (0..sound::count())
            .map(|v| Voice {
                data: sound::samples(v)
                    .into_iter()
                    .map(|x| x as f32 / ONE as f32)
                    .collect(),
                looped: sound::voice(v).unwrap().1.looped,
            })
            .collect();
        Mixer {
            rate: f(s[0]),
            voices,
            playing: Vec::new(),
            loops: BTreeMap::new(),
            smooth: f(s[1]),
            out: Vec::new(),
            started: BTreeMap::new(),
            loop_means: BTreeMap::new(),
            frames: 0,
        }
    }

    /// One frame of the page: this frame's cues started, the loops set,
    /// then a tick's worth of samples mixed.
    fn frame(&mut self, g: &Game) {
        let f = |v: i32| v as f64 / ONE as f64;
        for c in g.sounds.chunks(sound::CUE_VALUES) {
            let v = (c[0] / ONE) as usize;
            *self
                .started
                .entry(sound::voice(v).unwrap().0.clone())
                .or_default() += 1;
            self.playing.push(Playing {
                voice: v,
                at: 0.0,
                rate: f(c[2]),
                gain: f(c[1]),
                pan: f(c[3]),
            });
        }
        for c in g.loops.chunks(sound::CUE_VALUES) {
            let v = (c[0] / ONE) as usize;
            let e = self.loops.entry(v).or_insert((0.0, f(c[2]), 0.0, 0.0, 0.0));
            e.1 = f(c[2]);
            e.3 = f(c[1]);
            e.4 = f(c[3]);
            *self.loop_means.entry(v).or_default() += f(c[1]);
        }
        self.frames += 1;
        let n = (self.rate * TICK_US as f64 / 1e6).round() as usize;
        let k = 1.0 - (-1.0 / (self.smooth * self.rate)).exp();
        for _ in 0..n {
            let (mut l, mut r) = (0.0f64, 0.0f64);
            for p in &mut self.playing {
                let d = &self.voices[p.voice].data;
                let s = d.get(p.at as usize).copied().unwrap_or(0.0) as f64 * p.gain;
                let a = (p.pan + 1.0) * std::f64::consts::FRAC_PI_4;
                l += s * a.cos();
                r += s * a.sin();
                p.at += p.rate;
            }
            for (v, e) in self.loops.iter_mut() {
                let d = &self.voices[*v].data;
                e.2 += (e.3 - e.2) * k;
                let s = d[e.0 as usize % d.len()] as f64 * e.2;
                let a = (e.4 + 1.0) * std::f64::consts::FRAC_PI_4;
                l += s * a.cos();
                r += s * a.sin();
                e.0 += e.1;
            }
            self.out.push((l as f32, r as f32));
        }
        let voices = &self.voices;
        self.playing
            .retain(|p| (p.at as usize) < voices[p.voice].data.len() || voices[p.voice].looped);
    }

    fn wav(&self, path: &std::path::Path) {
        let mut b = Vec::new();
        let n = self.out.len() as u32;
        let rate = self.rate as u32;
        b.extend(b"RIFF");
        b.extend((36 + n * 4).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(rate.to_le_bytes());
        b.extend((rate * 4).to_le_bytes());
        b.extend(4u16.to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"data");
        b.extend((n * 4).to_le_bytes());
        // a soft limiter like the page's, so the file plays as the page does
        let lim = |x: f32| (x.tanh() * 32000.0) as i16;
        for (l, r) in &self.out {
            b.extend(lim(*l).to_le_bytes());
            b.extend(lim(*r).to_le_bytes());
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b).unwrap();
    }
}

fn run(g: &mut Game, m: &mut Mixer, frames: u32, bits: impl Fn(u32) -> u32, dx: impl Fn(u32) -> i32) {
    for k in 0..frames {
        let now = g.clock.last().unwrap_or(0) + TICK_US;
        g.advance(now, bits(k), dx(k), 0, 0);
        if g.screen == tool_core::game::Screen::Dialogue {
            let _ = g.act("dialogue.next", "");
        }
        m.frame(g);
    }
}

#[test]
#[ignore]
fn probe_the_mix() {
    let made = Instant::now();
    let mut m = Mixer::new();
    let made_ms = made.elapsed().as_secs_f64() * 1000.0;
    let mut g = game();
    // the opening: in bed, up, out of the door and away west, running
    g.act("screen.show", "play").unwrap();
    run(&mut g, &mut m, 240, |_| 0, |_| 0);
    run(
        &mut g,
        &mut m,
        600,
        |k| if k < 60 { FORWARD } else { FORWARD | RUN },
        |k| if k < 40 { 300 } else { 0 },
    );
    // a duel with an orc that fights back: drawn, swinging
    g.act("duel.spawn", "orc").unwrap();
    run(&mut g, &mut m, 60, |k| if k == 2 { DRAW } else { 0 }, |_| 0);
    run(
        &mut g,
        &mut m,
        900,
        |k| {
            let swing = if k % 50 < 12 { SWING } else { 0 };
            let step = if k % 200 < 40 { LEFT } else { 0 };
            swing | step
        },
        |k| {
            if k % 50 < 12 {
                -1500
            } else if k % 50 < 30 {
                800
            } else {
                0
            }
        },
    );
    // pause: everything should fall silent
    g.act("key.escape", "").unwrap();
    run(&mut g, &mut m, 60, |_| 0, |_| 0);
    let rate = m.rate as usize;
    let peak = m
        .out
        .iter()
        .map(|(l, r)| l.abs().max(r.abs()))
        .fold(0.0f32, f32::max);
    let over = m
        .out
        .iter()
        .filter(|(l, r)| l.abs() > 1.0 || r.abs() > 1.0)
        .count();
    let rms: Vec<String> = m
        .out
        .chunks(rate)
        .map(|c| {
            let e: f64 = c
                .iter()
                .map(|(l, r)| (*l as f64).powi(2) + (*r as f64).powi(2))
                .sum::<f64>()
                / (2 * c.len()) as f64;
            format!("{:.0}", 20.0 * e.sqrt().max(1e-9).log10())
        })
        .collect();
    let tail = &m.out[m.out.len() - rate / 2..];
    let tail_peak = tail
        .iter()
        .map(|(l, r)| l.abs().max(r.abs()))
        .fold(0.0f32, f32::max);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shots/m7feel/sound-mix.wav");
    m.wav(&path);
    println!(
        "voices made in {made_ms:.1} ms ({} voices, {} samples)",
        sound::count(),
        m.voices.iter().map(|v| v.data.len()).sum::<usize>()
    );
    println!("mix: {:.1} s, peak {peak:.3} ({:.1} dBFS) before the limiter, {over} of {} samples over full scale ({:.4}%)", m.out.len() as f64 / m.rate, 20.0 * (peak as f64).log10(), m.out.len(), 100.0 * over as f64 / m.out.len() as f64);
    println!("loudness by second (dBFS RMS): {}", rms.join(" "));
    println!("the last half second, paused: peak {tail_peak:.4}");
    println!("started: {:?}", m.started);
    println!(
        "loops, mean level a frame: {:?}",
        m.loop_means
            .iter()
            .map(|(k, v)| (
                sound::voice(*k).unwrap().0.clone(),
                format!("{:.3}", v / m.frames as f64)
            ))
            .collect::<Vec<_>>()
    );
    println!("written to {}", path.display());
    assert!(peak > 0.05, "the mix is silent");
    assert!(peak < 2.0, "louder than the limiter holds");
}

/// What M7's feel costs the core a tick, natively in release: the tick in
/// the raid, and within it the sound's listening and the boom's easing (each
/// timed again alone on a copy of the world), and the burning houses' shapes.
#[test]
#[ignore]
fn probe_feel_tick_cost() {
    let mut w = tool_core::story::opening(42);
    let look = tool_core::look::Look::parse(include_str!("../../../data/look.json")).unwrap();
    let (mut tick, mut snd, mut boom) = (0.0f64, 0.0f64, 0.0f64);
    let mut worst = 0.0f64;
    let (mut boom_worst, mut trees_worst) = (0.0f64, 0.0f64);
    let mut worst_at = 0;
    let n = 1200;
    for k in 0..n {
        let i = tool_core::input::Input {
            keys: if k > 100 {
                FORWARD | if k % 300 < 150 { RUN } else { 0 }
            } else {
                0
            },
            dx: if k % 120 < 30 { 400 } else { 0 },
            ..Default::default()
        };
        let t = Instant::now();
        w.step(&i);
        let e = t.elapsed().as_secs_f64() * 1000.0;
        tick += e;
        if e > worst {
            worst = e;
            worst_at = k;
        }
        let mut c = w.clone();
        let t = Instant::now();
        sound::begin(&mut c);
        sound::step(&mut c);
        snd += t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        tool_core::boom::step(&mut c);
        let b = t.elapsed().as_secs_f64() * 1000.0;
        boom += b;
        boom_worst = boom_worst.max(b);
        let t = Instant::now();
        let _ = tool_core::boom::trees_near(&c, c.player.pos);
        trees_worst = trees_worst.max(t.elapsed().as_secs_f64() * 1000.0);
    }
    let w = tool_core::story::opening(42);
    let mut s = tool_core::render::Shapes::default();
    w.draw(0, &look, &mut s);
    let all = s.len();
    let fires = tool_core::story::burning(&w).len();
    println!(
        "raid, {n} ticks: {:.4} ms a tick mean, {worst:.3} ms worst (tick {worst_at}); sound {:.4} ms, boom {:.4} ms a tick (alone), boom worst {boom_worst:.3} ms, a look-up of the trees near {trees_worst:.3} ms at worst; {all} moving shapes on the first frame with {fires} houses burning",
        tick / n as f64,
        snd / n as f64,
        boom / n as f64
    );
    assert!(tick / (n as f64) < 2.0);
}

#[test]
#[ignore]
fn probe_opening_build_cost() {
    let _ = tool_core::story::opening(1);
    let t = Instant::now();
    for k in 0..5 {
        let _ = tool_core::story::opening(k);
    }
    let all = t.elapsed().as_secs_f64() * 1000.0 / 5.0;
    let w = tool_core::world::World::new(1);
    let t = Instant::now();
    for _ in 0..5 {
        let _ = tool_core::boom::blocks(&w);
    }
    let b = t.elapsed().as_secs_f64() * 1000.0 / 5.0;
    println!("the opening built afresh (a dream's wake): {all:.2} ms, of which the boom's blocks {b:.2} ms");
}
