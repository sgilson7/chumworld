//! `sound` (M7 Polish, task P1 "feel"): what the game sounds like, decided
//! here and played by the page.
//!
//! Every sound is a row of `data/sound.json` by name (a *voice*). The core
//! makes each voice's samples from the row's layers (`synth.rs`), once, when
//! the page first asks after the reader's first click; it says, each tick,
//! which voices start (`cue`, the tick's *cues*), and each frame how loud,
//! how fast and from which side each of them is heard from the camera, and
//! how loud each looping voice is (`hear`, `loops`). The page builds an
//! audio buffer from each voice's samples and plays what it is told
//! (`web/sound.js`); it decides nothing (R6).
//!
//! A module adds a sound with one row in the data and one
//! `sound::cue(w, "name", at, gain, rate)` where the thing happens. The
//! events this module listens for itself, at the end of each tick
//! (`step`): feet coming down (by what they land on), the sword drawn and
//! put away, a blade cutting the air, blades meeting, a blow landing, a
//! body falling, the imbue starting, pulsing and ending, a volley leaving,
//! an orb turned, a throw, a thrown thing landing, the merchant's donkey
//! stopped, an orc giving chase and the raid's shouts, a pickup, the dream's
//! fade and the opening's end. A sound repeated sooner than its row's
//! `gap_ticks` is dropped, so a stream of hits does not pile up.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

pub mod synth;

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, V3};
use crate::player::{self, Sheath};
use crate::render::Camera;
use crate::world::World;

// ---- data/sound.json --------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub wave: String,
    #[serde(default)]
    pub hz: [i32; 2],
    #[serde(default)]
    pub at_ms: i32,
    #[serde(default)]
    pub ms: i32,
    #[serde(default)]
    pub attack_ms: i32,
    #[serde(default)]
    pub decay_ms: i32,
    pub gain_pct: i32,
    #[serde(default)]
    pub lp_hz: [i32; 2],
    #[serde(default)]
    pub hp_hz: [i32; 2],
    #[serde(default)]
    pub bp_hz: [i32; 2],
    #[serde(default)]
    pub bp_q_pct: i32,
    #[serde(default)]
    pub am_mhz: i32,
    #[serde(default)]
    pub am_pct: i32,
    #[serde(default)]
    pub vib_mhz: i32,
    #[serde(default)]
    pub vib_pct: i32,
    #[serde(default)]
    pub density: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceSpec {
    #[serde(rename = "loop", default)]
    pub looped: bool,
    pub ms: i32,
    pub peak_pct: i32,
    pub level_pct: i32,
    pub bus: String,
    pub gap_ticks: u32,
    pub near_m: i32,
    pub reach_m: i32,
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limiter {
    pub threshold_db: i32,
    pub knee_db: i32,
    pub ratio: i32,
    pub attack_ms: i32,
    pub release_ms: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub rate: i32,
    pub smooth_ms: i32,
    pub loop_fade_ms: i32,
    pub pan_pct: i32,
    pub buses: BTreeMap<String, i32>,
    pub limiter: Limiter,
    /// by name, in name order: a voice's number is its place here
    pub voices: BTreeMap<String, VoiceSpec>,
}

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let s: Spec = serde_json::from_str(json).map_err(|e| format!("data/sound.json: {e}"))?;
        for (name, v) in &s.voices {
            if !s.buses.contains_key(&v.bus) {
                return Err(format!(
                    "data/sound.json: {name} plays on a bus that is not listed, \"{}\".",
                    v.bus
                ));
            }
            if v.layers.is_empty() || v.ms <= 0 {
                return Err(format!("data/sound.json: {name} has no layers or no length."));
            }
        }
        Ok(s)
    }
}

pub fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| Spec::parse(include_str!("../../../data/sound.json")).expect("data/sound.json"))
}

/// A voice's number, by name.
pub fn id(name: &str) -> Option<usize> {
    spec().voices.keys().position(|k| k == name)
}

/// A voice's row, by number.
pub fn voice(v: usize) -> Option<(&'static String, &'static VoiceSpec)> {
    spec().voices.iter().nth(v)
}

pub fn count() -> usize {
    spec().voices.len()
}

/// A voice's samples, Q16, made from its row (the page asks once a voice).
pub fn samples(v: usize) -> Vec<i32> {
    match voice(v) {
        Some((name, row)) => synth::render(spec(), name, row),
        None => Vec::new(),
    }
}

// ---- the tick's cues ----------------------------------------------------------

/// A voice to start: where (none: heard everywhere alike, as the menus
/// are), how loud (`ONE` its row's level) and how fast (`ONE` as made).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cue {
    pub voice: u16,
    pub at: Option<V3>,
    pub gain: Fx,
    pub rate: Fx,
}

/// What this module remembers from tick to tick, to hear what changed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    /// this tick's cues, cleared as the next tick begins
    pub cues: Vec<Cue>,
    /// the tick each voice last started, by voice
    pub last: Vec<u64>,
    /// each fighter's stride, and each villager's, last tick
    pub strides: Vec<i32>,
    pub walkers: Vec<i32>,
    /// each fighter's blade is cutting the air (above `SWISH_ON`)
    pub cutting: Vec<bool>,
    /// what the raid's orcs were doing, by raider
    pub modes: Vec<u8>,
    /// the imbue was on; the fade was down
    pub imbued: bool,
    pub veiled: bool,
    /// the opening was under way
    pub opening: bool,
    /// the next orb's number, the orbs turned, the things in the air, the
    /// boomerang's throws, the merchant's hits, the things carried, coins
    pub orbs_next: u32,
    pub turned: Vec<u32>,
    pub flying: usize,
    pub boomerangs: u32,
    pub cart_hits: u32,
    pub carried: u32,
    pub coins: u32,
    /// ticks to the raid's next shout
    pub shout_in: u32,
    /// the first tick has been heard: before it, nothing has changed
    pub started: bool,
}

/// A blade's point faster than this cuts the air audibly; it is heard
/// again once it has slowed below `SWISH_OFF` (metres a second).
pub const SWISH_ON: Fx = fx::int(7);
pub const SWISH_OFF: Fx = fx::int(7) / 2;
/// Blades meeting slower than this make no sound.
pub const CLASH_MIN: Fx = ONE;
/// A stride under this share of a walk's is a shuffle: no step heard.
pub const STRIDE_MIN: Fx = ONE / 3;
/// The raid's shouts, one every 1.5 to 4 s.
pub const SHOUT_EVERY: (u32, u32) = (90, 240);

/// Starts voice `name` this tick, unless it started less than its row's
/// `gap_ticks` ago. An unknown name is a mistake in the code: a test
/// catches it (`every_sound_the_core_names_has_a_row`).
pub fn cue(w: &mut World, name: &str, at: Option<V3>, gain: Fx, rate: Fx) {
    let Some(v) = id(name) else {
        debug_assert!(false, "no sound named {name} in data/sound.json");
        return;
    };
    let gap = spec().voices[name].gap_ticks as u64;
    let s = &mut w.sound;
    if s.last.len() < count() {
        s.last.resize(count(), u64::MAX);
    }
    let last = s.last[v];
    if gap > 0 && last != u64::MAX && w.tick < last + gap {
        return;
    }
    s.last[v] = w.tick;
    s.cues.push(Cue {
        voice: v as u16,
        at,
        gain: gain.clamp(0, ONE),
        rate: rate.max(ONE / 4),
    });
}

/// A share from `lo` to `hi` (0 to `ONE`), held there.
fn share(x: Fx, lo: Fx, hi: Fx) -> Fx {
    if hi <= lo {
        return ONE;
    }
    fx::div((x - lo).clamp(0, hi - lo), hi - lo)
}

/// A small spread round `ONE` (± `pct`%) for tick `t` and thing `k`, so a
/// voice repeated does not sound stamped (not the world's chance: hearing
/// changes nothing in the world).
fn spread(t: u64, k: usize, pct: i32) -> Fx {
    let h = crate::terrain::hash(t as i32, k as i32, 0x50_d0);
    let u = (h % 2001) as i32 - 1000; // −1000..1000
    ONE + ONE * pct / 100 * u / 1000
}

/// Whether an angle moving from `a` to `b` (the short way forward) passes `q`.
fn passes(a: i32, b: i32, q: i32) -> bool {
    let d = fx::wrap(b.wrapping_sub(a));
    let to = fx::wrap(q.wrapping_sub(a));
    d > 0 && to > 0 && to <= d
}

/// A foot comes down when its stride passes the point it touches the
/// ground (`motion::feet`: a foot is lifted while the cosine of its phase is
/// above 0, so it lands as the phase passes a quarter turn; the other
/// foot, half a turn on, at three quarters).
pub fn lands(before: i32, now: i32) -> bool {
    passes(before, now, fx::QUARTER_TURN) || passes(before, now, -fx::QUARTER_TURN)
}

/// The step's voice for what is underfoot at (x, z): the floor of the
/// player's house, the bridge's planks, a road, or grass.
pub fn underfoot(w: &World, x: Fx, z: Fx) -> &'static str {
    if let Some(h) = w.solid.home {
        if h.wall.holds(x, z, 0) {
            return "step_floor";
        }
    }
    if w.solid.decks.iter().any(|(span, _)| span.holds(x, z, 0)) {
        return "step_wood";
    }
    let r = ONE * 4;
    let sc = w.land.scope(x - r, z - r, x + r, z + r);
    let (d, half) = w.land.road_distance(x, z, &sc);
    if d <= half {
        return "step_road";
    }
    "step_grass"
}

/// Step 9, at the end of a tick that ran (not a hit-stop's): what was
/// heard this tick.
pub fn step(w: &mut World) {
    let started = w.sound.started;
    feet(w, started);
    blades(w, started);
    magic(w, started);
    things(w, started);
    story(w, started);
    w.sound.started = true;
}

/// Begins a tick: last tick's cues are gone.
pub fn begin(w: &mut World) {
    w.sound.cues.clear();
}

fn feet(w: &mut World, started: bool) {
    let n = player::count(w);
    let mut steps = Vec::new();
    let mut strides = Vec::with_capacity(n);
    for id in 0..n {
        let (b, f) = (player::body(w, id), player::fighter(w, id));
        let ph = f.gait.phase;
        strides.push(ph);
        let before = w.sound.strides.get(id).copied();
        let walking = f.gait.swing >= STRIDE_MIN && !f.health.down();
        let lying = id == 0 && crate::story::in_bed(w);
        if let (true, Some(before), true, false) = (started, before, walking, lying) {
            if lands(before, ph) {
                let loud = if id == 0 { ONE } else { ONE * 3 / 4 };
                let fast = share(f.gait.swing, ONE, ONE * 2);
                steps.push((b.pos, loud * (3 + fast / (ONE / 2)) / 5, id));
            }
        }
    }
    w.sound.strides = strides;
    let mut walkers = Vec::new();
    for (k, v) in w.story.villagers.iter().enumerate() {
        let ph = v.gait.phase;
        walkers.push(ph);
        if v.gone || !started || v.gait.swing < STRIDE_MIN {
            continue;
        }
        if let Some(before) = w.sound.walkers.get(k) {
            if lands(*before, ph) {
                steps.push((v.body.pos, ONE * 2 / 3, 100 + k));
            }
        }
    }
    w.sound.walkers = walkers;
    for (at, gain, k) in steps {
        let name = underfoot(w, at.x, at.z);
        let rate = spread(w.tick, k, 8);
        // feet are not held back by each other's gap: each body's own
        push_free(w, name, Some(at), gain, rate);
    }
}

/// A cue without the gap (steps, which many bodies make at once).
fn push_free(w: &mut World, name: &str, at: Option<V3>, gain: Fx, rate: Fx) {
    let Some(v) = id(name) else {
        debug_assert!(false, "no sound named {name}");
        return;
    };
    w.sound.cues.push(Cue {
        voice: v as u16,
        at,
        gain: gain.clamp(0, ONE),
        rate,
    });
}

fn blades(w: &mut World, started: bool) {
    let n = player::count(w);
    let mut cutting = Vec::with_capacity(n);
    let mut out: Vec<(&str, V3, Fx, Fx)> = Vec::new();
    for id in 0..n {
        let f = player::fighter(w, id);
        let tip = f.arm.pts[crate::arm::TIP];
        let speed = tip.p.sub(tip.q).len() * crate::ticks::TICKS_PER_SECOND as i32;
        let was = w.sound.cutting.get(id).copied().unwrap_or(false);
        let drawn = f.sheath == Sheath::Drawn;
        let now = drawn && if was { speed > SWISH_OFF } else { speed > SWISH_ON };
        if now && !was && started {
            let g = ONE / 3 + share(speed, SWISH_ON, fx::int(16)) * 2 / 3;
            let rate = ONE * 85 / 100 + share(speed, SWISH_ON, fx::int(18)) * 35 / 100;
            out.push(("swish", tip.p, g, rate));
        }
        cutting.push(now);
        match f.sheath {
            Sheath::Drawing(t) if t == player::REACH_TICKS => {
                out.push(("draw", f.arm.pts[crate::arm::TIP].p, ONE, spread(w.tick, id, 4)))
            }
            Sheath::Sheathing(t) if t == player::ALIGN_TICKS => {
                out.push(("sheathe", player::body(w, id).pos.add(v3(0, ONE, 0)), ONE, ONE))
            }
            _ => {}
        }
    }
    w.sound.cutting = cutting;
    for c in w.blades.clashes.clone() {
        if c.speed >= CLASH_MIN {
            let g = ONE * 2 / 5 + share(c.speed, CLASH_MIN, fx::int(12)) * 3 / 5;
            out.push(("clash", c.at, g, spread(w.tick, c.a as usize, 5)));
        }
    }
    for h in w.blades.hits.clone() {
        if crate::health::damage(h.speed, h.part) > 0 {
            let g = ONE / 2 + share(h.speed, 0, fx::int(12)) / 2;
            out.push(("hit", h.at, g, spread(w.tick, h.on as usize, 6)));
        }
    }
    for k in w.feel.downed.clone() {
        let at = player::body(w, k as usize).pos;
        out.push(("fall", at, ONE, ONE));
    }
    for (name, at, g, r) in out {
        cue(w, name, Some(at), g, r);
    }
}

fn magic(w: &mut World, started: bool) {
    let on = crate::imbue::on(w);
    let was = w.sound.imbued;
    w.sound.imbued = on;
    if started && on && !was {
        cue(w, "imbue_on", None, ONE, ONE);
    } else if started && was && !on {
        cue(w, "imbue_off", None, ONE, ONE);
    } else if on {
        let left = crate::imbue::left(w);
        if left > 0 && left <= crate::imbue::WARN && left % crate::imbue::BLINK == 0 {
            cue(w, "imbue_pulse", None, ONE, ONE);
        }
    }
    // a volley leaving: orbs numbered from last tick's next
    let first = w.sound.orbs_next;
    let new: Vec<V3> = w
        .orbs
        .flying
        .iter()
        .filter(|o| o.id >= first)
        .map(|o| o.pos)
        .collect();
    w.sound.orbs_next = w.orbs.next;
    if started && !new.is_empty() {
        let mut sum = V3::ZERO;
        for p in &new {
            sum = sum.add(*p);
        }
        let at = v3(
            sum.x / new.len() as i32,
            sum.y / new.len() as i32,
            sum.z / new.len() as i32,
        );
        cue(w, "whoosh", Some(at), ONE, spread(w.tick, 7, 6));
    }
    // an orb turned this tick
    let turned: Vec<(u32, V3)> = w
        .orbs
        .flying
        .iter()
        .filter(|o| o.turned.is_some())
        .map(|o| (o.id, o.pos))
        .collect();
    let fresh: Vec<V3> = turned
        .iter()
        .filter(|(k, _)| !w.sound.turned.contains(k))
        .map(|(_, p)| *p)
        .collect();
    w.sound.turned = turned.iter().map(|(k, _)| *k).collect();
    if started {
        for (k, p) in fresh.into_iter().enumerate() {
            cue(w, "ping", Some(p), ONE, spread(w.tick, k, 10));
        }
    }
}

fn things(w: &mut World, started: bool) {
    let flying = w.throwables.flying.len();
    let thrown_now = flying > w.sound.flying;
    w.sound.flying = flying;
    let boomerangs = w.boomerang.thrown;
    let boomerang_now = boomerangs > w.sound.boomerangs;
    w.sound.boomerangs = boomerangs;
    let hits = w.merchant.hits;
    let stopped = hits > w.sound.cart_hits;
    w.sound.cart_hits = hits;
    let carried: u32 = w.inventory.count.iter().sum();
    let coins = w.inventory.coins;
    let gained = carried > w.sound.carried || coins > w.sound.coins;
    w.sound.carried = carried;
    w.sound.coins = coins;
    if !started {
        return;
    }
    if thrown_now {
        let at = w.throwables.flying.last().map(|t| t.p);
        cue(w, "throw", at, ONE, spread(w.tick, 3, 6));
    }
    if boomerang_now {
        cue(w, "throw", Some(crate::boomerang::hand(w)), ONE, ONE * 9 / 10);
    }
    for i in w.throwables.landed.clone() {
        let g = ONE / 3 + share(i.speed, 0, fx::int(15)) * 2 / 3;
        cue(w, "thunk", Some(i.at), g, spread(w.tick, 4, 8));
    }
    if stopped {
        let at = w.merchant.at;
        cue(w, "bray", Some(at), ONE, ONE);
    }
    if gained {
        cue(w, "chime", None, ONE, ONE);
    }
}

fn story(w: &mut World, started: bool) {
    let veil = w.story.veil;
    let was = w.sound.veiled;
    w.sound.veiled = veil;
    let opening = w.story.opening();
    let ended = w.sound.opening && w.story.phase == crate::story::Phase::Free;
    w.sound.opening = opening;
    let modes: Vec<u8> = w.story.raid.iter().map(|r| r.mode as u8).collect();
    let before = std::mem::replace(&mut w.sound.modes, modes.clone());
    if !started {
        w.sound.shout_in = SHOUT_EVERY.0;
        return;
    }
    if veil && !was && w.story.phase != crate::story::Phase::Free {
        cue(w, "dream", None, ONE, ONE);
    }
    if ended {
        cue(w, "free", None, ONE, ONE);
    }
    if !opening {
        return;
    }
    // an orc giving chase shouts
    let mut shouts = Vec::new();
    for (k, m) in modes.iter().enumerate() {
        if *m == crate::story::Mode::Chase as u8 && before.get(k).is_some_and(|b| b != m) {
            shouts.push(k);
        }
    }
    // and the raid shouts on its own, now and then
    w.sound.shout_in = w.sound.shout_in.saturating_sub(1);
    if w.sound.shout_in == 0 {
        let h = crate::terrain::hash(w.tick as i32, 11, 0x5407);
        let span = SHOUT_EVERY.1 - SHOUT_EVERY.0 + 1;
        w.sound.shout_in = SHOUT_EVERY.0 + h % span;
        if !w.story.raid.is_empty() {
            shouts.push((h / span) as usize % w.story.raid.len());
        }
    }
    for k in shouts {
        let r = &w.story.raid[k];
        let Some(foe) = w.foes.get(r.foe) else {
            continue;
        };
        if foe.fighter.health.down() {
            continue;
        }
        let at = foe.body.pos.add(v3(0, ONE * 17 / 10, 0));
        let rate = spread(w.tick, k, 14);
        cue(w, "shout", Some(at), ONE, rate);
    }
}

/// The acts that tick: a press on the menus, settings, dialogue or the
/// shop, and Play and Resume.
pub fn ticks_on(name: &str) -> bool {
    ["menu.", "settings.", "dialogue.", "shop.", "screen."]
        .iter()
        .any(|p| name.starts_with(p))
}

/// A dialogue blips every `BLIP_EVERY` letters as they appear.
pub const BLIP_EVERY: usize = 3;

/// Whether the letters shown going from `before` to `after` pass a blip.
pub fn blips(before: usize, after: usize) -> bool {
    after > before && (before / BLIP_EVERY != after / BLIP_EVERY || before == 0)
}

/// Each speaker's voice, a little higher or lower.
pub fn voice_rate(who: &str) -> Fx {
    match who {
        "trainer" => ONE * 85 / 100,
        "merchant" => ONE * 110 / 100,
        "you" => ONE,
        _ => ONE * 95 / 100,
    }
}

// ---- what the camera hears ------------------------------------------------------

/// The values the page plays a cue by, and a loop's: voice, gain, rate
/// and pan (−`ONE` left to `ONE` right), each Q16 (the voice times `ONE`).
pub const CUE_VALUES: usize = 4;

/// How loud and from which side a voice at `at` is heard from `cam`, its
/// row's level, its bus's and the reader's volume taken in.
pub fn heard(v: usize, at: Option<V3>, cam: &Camera, volume_pct: i32) -> (Fx, Fx) {
    let sp = spec();
    let Some((_, row)) = voice(v) else {
        return (0, 0);
    };
    let bus = sp.buses.get(&row.bus).copied().unwrap_or(100);
    let base = ONE * row.level_pct / 100 * bus / 100 * volume_pct.clamp(0, 100) / 100;
    let Some(p) = at else {
        return (base, 0);
    };
    if row.reach_m <= 0 {
        return (base, 0);
    }
    let to = p.sub(cam.eye);
    let d = to.len();
    let near = fx::int(row.near_m.max(1));
    let reach = fx::int(row.reach_m);
    if d >= reach {
        return (0, 0);
    }
    // near_m / d past near_m, easing to nothing over the last quarter of reach
    let fall = if d <= near { ONE } else { fx::div(near, d) };
    let edge = share(reach - d, 0, reach / 4);
    let gain = fx::mul(fx::mul(base, fall), edge);
    let right = v3(-cam.fwd.z, 0, cam.fwd.x).norm();
    let flat = v3(to.x, 0, to.z);
    let pan = if flat.len() < ONE / 10 {
        0
    } else {
        fx::mul(flat.norm().dot(right), ONE * sp.pan_pct / 100)
    };
    // close by, a sound is in both ears
    let pan = fx::mul(pan, share(d, 0, near * 2));
    (gain, pan)
}

/// The cues as the page plays them, heard from `cam`.
pub fn play_list(cues: &[Cue], cam: &Camera, volume_pct: i32) -> Vec<i32> {
    let mut out = Vec::with_capacity(cues.len() * CUE_VALUES);
    for c in cues {
        let (g, pan) = heard(c.voice as usize, c.at, cam, volume_pct);
        if g <= 0 {
            continue;
        }
        out.extend([c.voice as i32 * ONE, fx::mul(g, c.gain), c.rate, pan]);
    }
    out
}

/// The looping voices, in name order, each with where it is heard from
/// and how strongly (`ONE` its row's level) now; none heard while the game
/// is not running (`live` false: paused, a menu, the title).
pub fn loop_sources(w: &World, eye: V3) -> Vec<(usize, Option<V3>, Fx, Fx)> {
    let mut out = Vec::new();
    for (v, (name, row)) in spec().voices.iter().enumerate() {
        if !row.looped {
            continue;
        }
        let (at, gain, rate) = match name.as_str() {
            "wind" => (None, wind(w, eye), ONE),
            "river" => river(w, eye),
            "fire" => fire(w, eye),
            "cart" => {
                let m = &w.merchant;
                let moving = m.active() && m.at != m.prev_at;
                (Some(m.at), if moving { ONE } else { 0 }, ONE)
            }
            "whirr" => match &w.boomerang.flight {
                Some(f) => (Some(f.p), ONE, ONE),
                None => (None, 0, ONE),
            },
            _ => (None, 0, ONE),
        };
        out.push((v, at, gain, rate));
    }
    out
}

/// The loops as the page plays them: every looping voice, always in the
/// same order, at nothing when not heard.
pub fn loop_list(w: &World, cam: &Camera, live: bool, volume_pct: i32) -> Vec<i32> {
    let mut out = Vec::new();
    for (v, at, strength, rate) in loop_sources(w, cam.eye) {
        let (g, pan) = heard(v, at, cam, volume_pct);
        let g = if live { fx::mul(g, strength) } else { 0 };
        out.extend([v as i32 * ONE, g, rate, pan]);
    }
    out
}

/// The wind: steady on the open plains, quieter among trees and indoors.
fn wind(w: &World, eye: V3) -> Fx {
    if crate::story::cut_away(w) {
        return ONE * 2 / 5;
    }
    let up = eye.y - w.ground(eye.x, eye.z);
    ONE * 3 / 4 + share(up, ONE * 2, fx::int(8)) / 4
}

/// The river, from the nearest point of its banks.
fn river(w: &World, eye: V3) -> (Option<V3>, Fx, Fx) {
    let reach = fx::int(spec().voices["river"].reach_m);
    let r = &w.land.river;
    match r.near(eye.x, eye.z, reach) {
        Some(n) => {
            let mut p = r.point(n.s, 0);
            p.y = n.surface;
            // heard from the bank's edge rather than the middle
            let off = (n.d - n.half).max(0);
            let to = v3(p.x - eye.x, 0, p.z - eye.z);
            let at = if to.len() > 0 {
                eye.add(to.norm().scale(off)).add(v3(0, p.y - eye.y, 0))
            } else {
                p
            };
            (Some(at), ONE, ONE)
        }
        None => (None, 0, ONE),
    }
}

/// The nearest burning house during the raid (`story::fires`).
fn fire(w: &World, eye: V3) -> (Option<V3>, Fx, Fx) {
    let near = crate::story::fires(w)
        .into_iter()
        .min_by_key(|p| p.sub(eye).len());
    match near {
        Some(p) => (Some(p), ONE, ONE),
        None => (None, 0, ONE),
    }
}

/// The values the page sets its sound up with, each Q16: the sample rate,
/// the smoothing of a loop's level (seconds), the limiter's threshold,
/// knee (dB), ratio, attack and release (seconds), the values a cue takes,
/// the values a loop takes, and how many voices there are.
pub fn settings() -> Vec<i32> {
    let s = spec();
    let secs = |ms: i32| (ms as i64 * ONE as i64 / 1000) as i32;
    vec![
        s.rate * ONE,
        secs(s.smooth_ms),
        s.limiter.threshold_db * ONE,
        s.limiter.knee_db * ONE,
        s.limiter.ratio * ONE,
        secs(s.limiter.attack_ms),
        secs(s.limiter.release_ms),
        CUE_VALUES as i32 * ONE,
        CUE_VALUES as i32 * ONE,
        count() as i32 * ONE,
    ]
}

#[cfg(test)]
mod tests;
