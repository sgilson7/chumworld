//! `forest` (M7 polish, P13, T19): what helps you find the bandit camp and
//! what fills the forest round it.
//!
//! The person: "the bandit camp is really hard to find in the woods right
//! now without the waypoint, and the rest of the woods are completely
//! barren".
//!
//! - **The flag.** A tall pole stands in the camp (`Prop::Flagpole`,
//!   `data/places.json`), its top above the canopy round it, and a faceted
//!   cloth hangs from it in panels that ripple with the land's wind
//!   (`wind::gust`), holding still under `story.still` (`cloth`).
//! - **The smoke.** The camp's fire sends up a soft column (`smoke.rs`):
//!   a few faceted puffs that rise, widen a little, and thin and fade with
//!   height, added to what is behind them with no depth written, so the
//!   order they are drawn in changes nothing (CW-18).
//! - **The clearings.** The land places four to six from its seed
//!   (`terrain::Land::glades`); half hold one or two foes (`wanderers`,
//!   kinds on "glade"), the rest animals resting (`animals`). Each has a
//!   stump and a fallen log (`scatter::items`).
//! - **The trees and the floor** are `scatter`'s (`forest_kind`, `floor`).
//!
//! Numbers in `data/forest.json`.

use serde::Deserialize;

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, Affine, V3};
use crate::look::Look;
use crate::models::{self, Prop};
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::world::World;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FlagSpec {
    /// the cloth: how long out from the pole, how tall at the pole and at
    /// its end (mm), in how many panels
    pub length_mm: i32,
    pub height_mm: [i32; 2],
    pub panels: i32,
    /// how far each panel turns at a full gust and how far the ripple
    /// runs along it (degrees), and the way it blows when there is none
    pub swing_deg: i32,
    pub ripple_deg: i32,
    pub rest_deg: i32,
    pub cloth: [i32; 3],
    pub stripe: [i32; 3],
}

/// `data/forest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub flag: FlagSpec,
    pub smoke: crate::smoke::Spec,
}

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let s: Spec = serde_json::from_str(json).map_err(|e| format!("data/forest.json: {e}"))?;
        if s.flag.panels < 2 || s.flag.panels > 8 {
            return Err("data/forest.json: the flag needs 2 to 8 panels.".into());
        }
        s.smoke.check().map_err(|e| format!("data/forest.json: {e}"))?;
        Ok(s)
    }
}

pub fn spec() -> &'static Spec {
    static SPEC: std::sync::OnceLock<Spec> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| Spec::parse(include_str!("../../../data/forest.json")).expect("data/forest.json"))
}

/// The camp's props as placed (`data/places.json`).
fn camp_spots(w: &World) -> Vec<crate::places::Spot> {
    w.land
        .layout
        .places
        .iter()
        .find(|p| p.kind == "camp")
        .map(|p| crate::places::spots(w, p))
        .unwrap_or_default()
}

/// The camp's flagpole.
pub fn flagpole(w: &World) -> Option<crate::places::Spot> {
    camp_spots(w).into_iter().find(|s| s.prop == Prop::Flagpole)
}

/// The top of the camp's flagpole, in the world.
pub fn flag_top(w: &World) -> Option<V3> {
    flagpole(w).map(|s| s.at.add(v3(0, models::FLAGPOLE_TOP, 0)))
}

/// The camp's campfire, where its smoke rises.
pub fn campfire(w: &World) -> Option<V3> {
    let c = crate::camp::centre(w);
    camp_spots(w)
        .into_iter()
        .filter(|s| s.prop == Prop::Campfire)
        .min_by_key(|s| s.at.sub(c).len_xz())
        .map(|s| s.at)
}

/// The cloth's panels as a model about the pole's foot: each a thin slab
/// hung from the pole's top down its fly, turning from the one before by
/// the wind there (`wind::gust` at the pole, a ripple travelling out along
/// the cloth), so it waves; each a little shorter than the one before and
/// a different thickness, so no two share a plane (CW-18). `gust` is the
/// wind at the pole (−ONE to ONE) and `phase` how far the ripple has run.
pub fn cloth(gust: Fx, phase: i32) -> models::Model {
    let f = &spec().flag;
    let t = models::table();
    let mut m = models::Model::default();
    let n = f.panels;
    let seg = f.length_mm * ONE / 1000 / n;
    let top = models::FLAGPOLE_TOP - ONE * 25 / 100;
    // the way the cloth streams: its rest, swung by the gust
    let mut yaw = fx::deg(f.rest_deg) + fx::mul(gust, fx::deg(f.swing_deg));
    let mut at = v3(0, top, 0);
    for k in 0..n {
        let h = (f.height_mm[0] + (f.height_mm[1] - f.height_mm[0]) * k / (n - 1).max(1)) * ONE / 1000;
        let ripple = fx::mul(
            fx::sin(phase - k * fx::deg(70)),
            fx::deg(f.ripple_deg) * (k + 1) / n,
        );
        let a = yaw + ripple;
        let dir = v3(fx::cos(a), 0, -fx::sin(a));
        let mid = at.add(dir.scale(seg / 2)).add(v3(0, -h / 2, 0));
        let thick = ONE * (30 + 6 * (k % 2)) / 1000;
        let place = Affine::translate(mid)
            .then(&Affine::rot_y(a))
            .then(&Affine::scale(v3(seg + ONE / 50, h, thick)));
        let rgb = if k % 2 == 0 {
            crate::look::rgb(f.cloth)
        } else {
            crate::look::rgb(f.stripe)
        };
        m.add(Mesh::Cube, place, rgb, t.ink_prop);
        at = at.add(dir.scale(seg));
        yaw = a;
    }
    m
}

/// The flag's cloth as it is now: the wind at the pole and the ripple's
/// phase from the tick, both still while the story is held still
/// (`story.still`), so a still view draws the same pixels (CW-18).
pub fn cloth_now(w: &World, alpha: Fx) -> Option<(V3, models::Model)> {
    let pole = flagpole(w)?;
    let (tick, alpha) = if w.story.still { (0, 0) } else { (w.tick, alpha) };
    let gust = crate::wind::gust(crate::wind::spec(), &w.wind, pole.at.x, pole.at.z);
    // the ripple runs a turn every 50 ticks
    let phase =
        ((tick % 50) as i64 * fx::TURN as i64 / 50 + alpha as i64 * fx::TURN as i64 / 50 / ONE as i64) as i32;
    Some((pole.at, cloth(gust, phase)))
}

/// The flag's cloth and the camp fire's smoke (`story::draw`): in the
/// story's world only, not a bare one (`Phase::Off`, the test duels).
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    if w.story.phase == crate::story::Phase::Off {
        return;
    }
    if let Some((foot, m)) = cloth_now(w, alpha) {
        m.placed(&Affine::translate(foot)).emit(look.ink(), out);
    }
    if let Some(fire) = campfire(w) {
        crate::smoke::column(&spec().smoke, w, fire, alpha, out);
    }
}

// ---- seeing the camp ------------------------------------------------------------

/// What stands in the way of a line from `eye` to `to`, if anything: the
/// ground (every 4 m), or a tree as it is drawn at its distance from the
/// eye (whole near, a cone further, the forest's far crowns beyond, the
/// levels of `scatter::band`), each part taken as an upright cylinder as
/// wide as its widest and as tall as it is. The first met, as a point on
/// the line and what it was.
pub fn sight(land: &crate::terrain::Land, eye: V3, to: V3) -> Option<(V3, &'static str)> {
    use crate::scatter::{self, Thing};
    let sp = &land.spec.scatter;
    let len = to.sub(eye).len_xz().max(1);
    let n = (len / fx::int(4)).max(1);
    for k in 1..n {
        let p = eye.lerp(to, k * ONE / n);
        if land.ground(p.x, p.z) >= p.y {
            return Some((p, "ground"));
        }
    }
    let mut hits: Vec<(Fx, V3, &'static str)> = Vec::new();
    // the line's nearest point to (x, z): its share along and its height there
    let along = |c: V3| -> (Fx, Fx, V3) {
        let d = to.sub(eye);
        let q = c.sub(eye);
        let dd = (d.x as i128 * d.x as i128 + d.z as i128 * d.z as i128).max(1);
        let qd = q.x as i128 * d.x as i128 + q.z as i128 * d.z as i128;
        let t = (qd * ONE as i128 / dd).clamp(0, ONE as i128) as Fx;
        let p = eye.lerp(to, t);
        (t, p.sub(c).len_xz(), p)
    };
    let meet = |at: &Affine, what: &'static str, hits: &mut Vec<(Fx, V3, &'static str)>| {
        let c = at.t;
        let (hx, hy, hz) = (at.c0.len() / 2, at.c1.len() / 2, at.c2.len() / 2);
        let r = hx.max(hz);
        let (t, off, p) = along(c);
        if t > 0 && t < ONE && off < r && p.y >= c.y - hy && p.y <= c.y + hy {
            hits.push((t, p, what));
        }
    };
    let side = sp.chunk_m * ONE;
    let (x0, x1) = (eye.x.min(to.x), eye.x.max(to.x));
    let (z0, z1) = (eye.z.min(to.z), eye.z.max(to.z));
    let full = fx::int(sp.full_m[1]);
    let cone = fx::int(sp.cone_m[1]);
    let far = fx::int(sp.cone_m[0]);
    for j in (z0 - 8 * ONE).div_euclid(side)..=(z1 + 8 * ONE).div_euclid(side) {
        for i in (x0 - 8 * ONE).div_euclid(side)..=(x1 + 8 * ONE).div_euclid(side) {
            for it in scatter::items(land, i, j) {
                if !it.thing.tree() || along(it.at).1 > fx::int(8) {
                    continue;
                }
                let d = it.at.sub(eye).len_xz();
                if d < full {
                    for (_, a, _) in scatter::tree_parts(sp, &it) {
                        meet(&a, "tree", &mut hits);
                    }
                } else if d < cone && it.thing != Thing::Snag {
                    // the cone level's one cone (`scatter::build`, CONE)
                    let (w, h) = match it.thing {
                        Thing::Birch => (ONE * 23 / 10, ONE * 52 / 10),
                        Thing::Broadleaf => (ONE * 42 / 10, ONE * 44 / 10),
                        Thing::Leafy => (ONE * 36 / 10, ONE * 42 / 10),
                        _ => (ONE * 36 / 10, ONE * 47 / 10),
                    };
                    let (w, h) = (w * it.size / 100, h * it.size / 100);
                    let a = Affine::translate(it.at.add(v3(0, ONE * 8 / 10 * it.size / 100 + h / 2, 0)))
                        .then(&Affine::scale(v3(w, h, w)));
                    meet(&a, "cone", &mut hits);
                }
            }
        }
    }
    let cs = scatter::side_of(sp, scatter::CLUSTER);
    for j in (z0 - 20 * ONE).div_euclid(cs)..=(z1 + 20 * ONE).div_euclid(cs) {
        for i in (x0 - 20 * ONE).div_euclid(cs)..=(x1 + 20 * ONE).div_euclid(cs) {
            for (a, _) in scatter::cluster_crowns(land, i, j) {
                if a.t.sub(eye).len_xz() >= far {
                    meet(&a, "crown", &mut hits);
                }
            }
        }
    }
    hits.into_iter().min_by_key(|h| h.0).map(|h| (h.1, h.2))
}

#[cfg(test)]
mod tests;
