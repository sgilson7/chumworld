//! `animals` (M7 forest, P13, T19): deer, rabbits and boar in the forest
//! and along its edges.
//!
//! The person: "spruce up the rest of the woods with some wandering
//! animals". A new creature kind, not a foe: none can be fought, so none
//! is in `World::foes`, takes a blade or has a health bar (SECOND-ORDER
//! R9.446). Each moves by the rules here, the same for every run: no
//! pilot moves a body (CW-3 is about foes), and nothing here moves one.
//!
//! # Where they are
//!
//! `data/animals.json`. The herds are placed once from the land's seed
//! (`place`, CW-4): each a kind drawn by its share, a home where that kind
//! lives (the forest's thinning edge and a little out onto the open, or
//! deep in it), clear of water, the roads and the camp, `apart_m` from the
//! others; then one in every clearing without foes, resting
//! (`terrain::Land::glades`).
//!
//! # When they stand (CW-11)
//!
//! As the wanderers do: a herd stands once the player is within `wake_m`
//! of its home, if no more than `most_near` animals then stand; past
//! `park_m` it is parked (its animals taken out), so a parked herd costs a
//! distance a tick. Not in a bare world (`Phase::Off`), and held still
//! while the story is (`story.still`).
//!
//! # What they do
//!
//! Each grazes (its head lowered), walks to a new spot within `roam_m` of
//! home, or lies down to rest; and runs from the player who comes within
//! `flee_m`, straight away from them, until `calm_m` off, then walks back
//! toward home. A trunk, a stump, a log, a big rock (`obstacles::
//! trunks_near`) or water ahead turns it aside.

use serde::Deserialize;

use crate::fx::{self, Fx, ONE};
use crate::geom::{facing, v3, yaw_of, Affine, V3};
use crate::look::{rgb, Look};
use crate::models::{self, Beast};
use crate::render::Shapes;
use crate::rng::Rng;
use crate::world::World;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KindSpec {
    pub name: String,
    pub beast: String,
    /// "edge" or "forest"
    #[serde(rename = "where")]
    pub lives: String,
    pub share: i32,
    pub count: [i32; 2],
    pub walk_cms: i32,
    pub run_cms: i32,
    pub stride_cm: i32,
    pub flee_m: i32,
    pub calm_m: i32,
    pub roam_m: i32,
    pub graze_s: [i32; 2],
    pub walk_s: [i32; 2],
    pub rest_pct: i32,
    pub radius_cm: i32,
}

/// `data/animals.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub wake_m: i32,
    pub park_m: i32,
    pub most_near: usize,
    pub herds: i32,
    pub tries: i32,
    pub apart_m: i32,
    pub camp_m: i32,
    pub glade_kinds: Vec<String>,
    pub kinds: Vec<KindSpec>,
}

/// An animal's kind of body, by its name in the file.
pub fn beast_named(name: &str) -> Option<Beast> {
    Some(match name {
        "deer" => Beast::Deer,
        "rabbit" => Beast::Rabbit,
        "boar" => Beast::Boar,
        _ => return None,
    })
}

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let s: Spec = serde_json::from_str(json).map_err(|e| format!("data/animals.json: {e}"))?;
        if s.kinds.is_empty() {
            return Err("data/animals.json: there are no kinds of animal.".into());
        }
        for k in &s.kinds {
            if beast_named(&k.beast).is_none() {
                return Err(format!(
                    "data/animals.json: \"{}\" has no body \"{}\".",
                    k.name, k.beast
                ));
            }
            if !matches!(k.lives.as_str(), "edge" | "forest") {
                return Err(format!(
                    "data/animals.json: \"{}\" lives at the edge or in the forest.",
                    k.name
                ));
            }
            if k.flee_m >= k.calm_m || k.count[0] < 1 || k.count[1] < k.count[0] {
                return Err(format!(
                    "data/animals.json: \"{}\" must calm further off than it flees, and come one or more at a time.",
                    k.name
                ));
            }
        }
        if let Some(g) = s
            .glade_kinds
            .iter()
            .find(|g| !s.kinds.iter().any(|k| &k.name == *g))
        {
            return Err(format!(
                "data/animals.json: a clearing names an animal \"{g}\" there is no kind of."
            ));
        }
        if s.park_m <= s.wake_m {
            return Err("data/animals.json: herds must park further off than they wake.".into());
        }
        Ok(s)
    }
}

pub fn spec() -> &'static Spec {
    static SPEC: std::sync::OnceLock<Spec> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| Spec::parse(include_str!("../../../data/animals.json")).expect("data/animals.json"))
}

fn m(v: i32) -> Fx {
    fx::int(v)
}

const SECOND: i32 = 60;

// ---- state ------------------------------------------------------------------------

/// What an animal is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Graze,
    Walk,
    Rest,
    Flee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Animal {
    pub pos: V3,
    pub prev: V3,
    pub yaw: i32,
    pub prev_yaw: i32,
    pub mode: Mode,
    /// ticks left of a pause
    pub wait: i32,
    pub target: V3,
    /// its stride (an angle, a turn a stride), and how far its head is
    /// down and its body lying (0 to ONE), with their last tick's
    pub phase: i32,
    pub graze: Fx,
    pub lie: Fx,
    pub prev_phase: i32,
    pub prev_graze: Fx,
    pub prev_lie: Fx,
}

/// A herd as placed, and its animals while it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Herd {
    /// its kind, by its place in the file's `kinds`
    pub kind: usize,
    pub home: V3,
    pub count: i32,
    /// resting in a clearing
    pub glade: bool,
    pub animals: Vec<Animal>,
    pub rng: Rng,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    pub placed: bool,
    pub herds: Vec<Herd>,
    /// the cap on animals standing near the player (`most_near`; a test
    /// may lower it)
    pub most: usize,
}

// ---- placing ----------------------------------------------------------------------

/// Every herd, from the land's seed (module doc).
pub fn place(land: &crate::terrain::Land) -> Vec<Herd> {
    let sp = spec();
    let f = &land.layout.forest;
    let all = land.scope_all();
    let seed = land.spec.seed as u64;
    let mut rng = Rng::new((seed << 24) ^ 0xa41_3a15);
    let camp = land
        .layout
        .places
        .iter()
        .find(|p| p.kind == "camp")
        .map(|p| crate::places::metres(p.centre));
    let mut out: Vec<Herd> = Vec::new();
    // the forest's box and a band past its edge
    let (rx, rz) = (f.radii_m[0] + 160, f.radii_m[1] + 160);
    for _ in 0..sp.tries.max(0) {
        if out.len() as i32 >= sp.herds {
            break;
        }
        let x = m(f.centre[0] - rx) + rng.below((2 * m(rx)) as u32) as Fx;
        let z = m(f.centre[1] - rz) + rng.below((2 * m(rz)) as u32) as Fx;
        // which kinds live here: the forest's thinning edge (and just out
        // onto the open) or deep in it; one of them drawn by its share
        let w = land.forest(x, z, &all);
        let here = if w > ONE as i64 * 6 / 10 {
            "forest"
        } else if w < ONE as i64 * 55 / 100 && (w > 0 || near_forest(land, &all, x, z)) {
            "edge"
        } else {
            ""
        };
        let fits: Vec<usize> = (0..sp.kinds.len())
            .filter(|&k| sp.kinds[k].lives == here)
            .collect();
        let total: i32 = fits.iter().map(|&k| sp.kinds[k].share.max(0)).sum();
        let mut pick = rng.below(total.max(1) as u32) as i32;
        let kind = fits
            .iter()
            .copied()
            .find(|&k| {
                pick -= sp.kinds[k].share.max(0);
                pick < 0
            })
            .unwrap_or(0);
        let k = &sp.kinds[kind];
        let lives = !fits.is_empty() && total > 0;
        let c = v3(x, 0, z);
        let (rd, rh) = land.road_distance(x, z, &all);
        let ok = lives
            && rd - rh > m(12)
            && !land.is_water(x, z)
            && land.near(x, z, &all).is_none()
            && camp.is_none_or(|p| p.sub(c).len_xz() >= m(sp.camp_m))
            && land
                .glades
                .iter()
                .all(|g| g.centre.sub(c).len_xz() >= g.r + m(60))
            && out.iter().all(|h| h.home.sub(c).len_xz() >= m(sp.apart_m));
        if !ok {
            continue;
        }
        let count = rng.between(k.count[0], k.count[1]);
        let r = Rng::new(rng.below(u32::MAX) as u64 ^ (out.len() as u64) << 40);
        out.push(Herd {
            kind,
            home: v3(x, land.ground(x, z), z),
            count,
            glade: false,
            animals: Vec::new(),
            rng: r,
        });
    }
    // one resting in each clearing without foes
    for g in land.glades.iter().filter(|g| !g.foes) {
        let names = &sp.glade_kinds;
        if names.is_empty() {
            break;
        }
        let name = &names[g.pick as usize % names.len()];
        let Some(kind) = sp.kinds.iter().position(|k| &k.name == name) else {
            continue;
        };
        let k = &sp.kinds[kind];
        let c = g.centre;
        out.push(Herd {
            kind,
            home: v3(c.x, land.ground(c.x, c.z), c.z),
            count: k.count[1],
            glade: true,
            animals: Vec::new(),
            rng: Rng::new((seed << 16) ^ g.pick as u64 ^ 0x91ade),
        });
    }
    out
}

/// Within 120 m of the forest: a point just out on the open by its edge.
fn near_forest(land: &crate::terrain::Land, sc: &crate::terrain::Scope, x: Fx, z: Fx) -> bool {
    (0..8).any(|k| {
        let d = facing(fx::deg(45 * k), 0).scale(m(120));
        land.forest(x + d.x, z + d.z, sc) > ONE as i64 / 3
    })
}

/// The herds are placed the first time they are needed.
pub fn ensure(w: &mut World) {
    if !w.animals.placed {
        w.animals.herds = place(&w.land);
        w.animals.placed = true;
        w.animals.most = spec().most_near;
    }
}

/// How many animals stand.
pub fn standing(w: &World) -> usize {
    w.animals.herds.iter().map(|h| h.animals.len()).sum()
}

// ---- the tick ---------------------------------------------------------------------

/// Herds stand and park by the player's distance; each animal standing
/// grazes, walks, rests or runs. After the wanderers (`World::step`).
pub fn step(w: &mut World) {
    if w.story.phase == crate::story::Phase::Off {
        return;
    }
    ensure(w);
    let sp = spec();
    let me = w.player.pos;
    // park first, so the room they leave is free to stand in this tick
    for h in &mut w.animals.herds {
        if h.animals.is_empty() {
            continue;
        }
        let far = m(sp.park_m);
        if h.home.sub(me).len_xz() > far && h.animals.iter().all(|a| a.pos.sub(me).len_xz() > far) {
            h.animals.clear();
        }
    }
    let mut count = standing(w);
    // the nearest first, so the cap keeps those the player is beside
    let mut order: Vec<(Fx, usize)> = w
        .animals
        .herds
        .iter()
        .enumerate()
        .map(|(k, h)| (h.home.sub(me).len_xz(), k))
        .collect();
    order.sort();
    for (_, hi) in order {
        let h = &w.animals.herds[hi];
        if !h.animals.is_empty() || h.home.sub(me).len_xz() > m(sp.wake_m) {
            continue;
        }
        if count + h.count as usize > w.animals.most {
            continue;
        }
        count += h.count as usize;
        stand(w, hi);
    }
    // held still for a view (`story.still`), they stand but do not move
    if w.story.still {
        return;
    }
    for hi in 0..w.animals.herds.len() {
        for ai in 0..w.animals.herds[hi].animals.len() {
            tend(w, hi, ai, me);
        }
    }
}

fn stand(w: &mut World, hi: usize) {
    let land = w.land.clone();
    let h = &mut w.animals.herds[hi];
    let n = h.count;
    for k in 0..n {
        let a = fx::TURN / n.max(1) * k + h.rng.below(fx::TURN as u32 / 8) as i32;
        let d = m(2) + h.rng.below(m(4) as u32) as Fx;
        let mut p = h.home.add(facing(a, 0).scale(d));
        p.y = land.ground(p.x, p.z);
        let yaw = h.rng.below(fx::TURN as u32) as i32;
        let (mode, wait) = if h.glade {
            (Mode::Rest, (40 + h.rng.below(40) as i32) * SECOND)
        } else {
            (Mode::Graze, (1 + h.rng.below(6) as i32) * SECOND)
        };
        let lie = if mode == Mode::Rest { ONE } else { 0 };
        h.animals.push(Animal {
            pos: p,
            prev: p,
            yaw,
            prev_yaw: yaw,
            mode,
            wait,
            target: p,
            phase: 0,
            graze: 0,
            lie,
            prev_phase: 0,
            prev_graze: 0,
            prev_lie: lie,
        });
    }
}

/// Toward `to` by at most `by`.
fn toward(v: Fx, to: Fx, by: Fx) -> Fx {
    if v < to {
        (v + by).min(to)
    } else {
        (v - by).max(to)
    }
}

fn tend(w: &mut World, hi: usize, ai: usize, me: V3) {
    let sp = spec();
    let k = &sp.kinds[w.animals.herds[hi].kind];
    let home = w.animals.herds[hi].home;
    let mut a = w.animals.herds[hi].animals[ai];
    a.prev = a.pos;
    a.prev_yaw = a.yaw;
    a.prev_phase = a.phase;
    a.prev_graze = a.graze;
    a.prev_lie = a.lie;
    let d = a.pos.sub(me).len_xz();
    if d < m(k.flee_m) {
        a.mode = Mode::Flee;
    }
    let mut speed = 0;
    let mut want = a.yaw;
    match a.mode {
        Mode::Flee => {
            if d > m(k.calm_m) {
                // calm again: back toward home
                a.mode = Mode::Walk;
                a.target = home;
                a.wait = 0;
            } else {
                want = yaw_of(a.pos.sub(me));
                speed = k.run_cms;
            }
        }
        Mode::Walk => {
            if a.target.sub(a.pos).len_xz() < ONE {
                let r = &mut w.animals.herds[hi].rng;
                let rest = r.chance(k.rest_pct.max(0) as u32, 100);
                a.mode = if rest { Mode::Rest } else { Mode::Graze };
                let [lo, hi_s] = k.graze_s;
                a.wait = r.between(lo, hi_s) * SECOND * if rest { 3 } else { 1 };
            } else {
                want = yaw_of(a.target.sub(a.pos));
                speed = k.walk_cms;
            }
        }
        Mode::Graze | Mode::Rest => {
            a.wait -= 1;
            if a.wait <= 0 {
                let r = &mut w.animals.herds[hi].rng;
                let turn = r.below(fx::TURN as u32) as i32;
                let out = m(k.roam_m) * r.between(30, 100) / 100;
                a.target = home.add(facing(turn, 0).scale(out));
                a.mode = Mode::Walk;
            }
        }
    }
    a.graze = toward(a.graze, if a.mode == Mode::Graze { ONE } else { 0 }, ONE / 20);
    a.lie = toward(a.lie, if a.mode == Mode::Rest { ONE } else { 0 }, ONE / 40);
    // up before it moves
    if speed > 0 && a.lie > 0 {
        speed = 0;
    }
    // turn toward the way it wants, 9° a tick at most
    let dy = fx::wrap(want - a.yaw);
    a.yaw = fx::wrap(a.yaw + dy.clamp(-fx::deg(9), fx::deg(9)));
    if speed > 0 {
        let step = speed * ONE / 100 / SECOND;
        let next = a.pos.add(facing(a.yaw, 0).scale(step));
        let r = k.radius_cm * ONE / 100;
        let blocked = w.land.is_water(next.x, next.z)
            || w.obstacles
                .trunks_near(w, next, r)
                .iter()
                .any(|(c, cr)| c.sub(next).len_xz() < cr + r);
        if blocked {
            // turn aside, the way that is not toward the player
            let side = if fx::wrap(yaw_of(me.sub(a.pos)) - a.yaw) > 0 {
                -1
            } else {
                1
            };
            a.yaw = fx::wrap(a.yaw + side * fx::deg(35));
        } else {
            a.pos = v3(next.x, w.land.ground(next.x, next.z), next.z);
            a.phase = a.phase.wrapping_add(
                (step as i64 * fx::TURN as i64 / (k.stride_cm.max(1) as i64 * ONE as i64 / 100)) as i32,
            );
        }
    }
    w.animals.herds[hi].animals[ai] = a;
}

// ---- drawing ----------------------------------------------------------------------

/// Every animal standing, `alpha` of the way from its last tick, in the
/// herds' order (CW-18).
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    let sp = spec();
    for h in &w.animals.herds {
        let beast = beast_named(&sp.kinds[h.kind].beast).unwrap_or(Beast::Deer);
        for a in &h.animals {
            let at = a.prev.lerp(a.pos, alpha);
            let yaw = a.prev_yaw + fx::mul(fx::wrap(a.yaw - a.prev_yaw), alpha);
            let phase = a
                .prev_phase
                .wrapping_add(fx::mul(a.phase.wrapping_sub(a.prev_phase), alpha));
            let graze = fx::lerp(a.prev_graze, a.graze, alpha);
            let lie = fx::lerp(a.prev_lie, a.lie, alpha);
            let place = Affine::translate(at).then(&Affine::rot_y(yaw));
            models::beast_model(beast, phase, graze, lie)
                .placed(&place)
                .emit(look.ink(), out);
            let wide = match beast {
                Beast::Deer => ONE * 12 / 10,
                Beast::Rabbit => ONE * 35 / 100,
                Beast::Boar => ONE,
            };
            out.shadow(
                at.add(v3(0, crate::player::SHADOW_LIFT, 0)),
                wide,
                rgb(look.palette.shadow),
                ONE,
            );
        }
    }
}

#[cfg(test)]
mod tests;
