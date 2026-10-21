//! `smoke` (M7 forest, P13, T19): a soft column of smoke over a fire, seen
//! from far off (the bandit camp's, `forest::draw`).
//!
//! Not a stack of balls: a few faceted puffs, each taller than it is wide,
//! rising along one column so each overlaps the next, widening a little
//! as they climb (a taper), fading in at the fire and thinning to nothing
//! at the top (their strength, not their size, goes, so none pops), and
//! leaning with the wind more the higher they are. Each is a glow added to
//! what is behind it with no depth written, so the order they are drawn in
//! changes no sum, and every frame draws as many (CW-18). Held still for a
//! view (`story.still`), the smoke holds still.

use serde::Deserialize;

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, Affine, V3};
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::world::World;

/// A column's numbers (`data/forest.json`'s `smoke`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Spec {
    /// how many puffs, and how long each rises (ticks)
    pub puffs: i32,
    pub life_ticks: i32,
    /// how high a puff rises over the fire, and how far the wind leans it
    pub rise_m: i32,
    pub drift_m: i32,
    /// a puff's width at the fire and at the top (cm), and how much taller
    /// than wide it is (percent)
    pub width_cm: [i32; 2],
    pub tall_pct: i32,
    /// its strength at its thickest (percent of a full glow), the share of
    /// its life it takes to come in, and the share after which it thins
    pub strength_pct: i32,
    pub fade_in_pct: i32,
    pub thin_from_pct: i32,
    /// its colour low and high
    pub dark: [i32; 3],
    pub light: [i32; 3],
}

impl Spec {
    pub fn check(&self) -> Result<(), String> {
        if self.puffs < 2 || self.puffs > 16 {
            return Err("the smoke needs 2 to 16 puffs.".into());
        }
        if self.life_ticks < 60 || self.rise_m <= 0 {
            return Err("the smoke needs a life of a second and a rise.".into());
        }
        if !(0..self.thin_from_pct).contains(&self.fade_in_pct) || self.thin_from_pct >= 100 {
            return Err("the smoke must come in before it thins, and thin before its end.".into());
        }
        Ok(())
    }
}

/// One puff `k` at `tick` (and `alpha` of the next): its middle over the
/// fire's foot, its width, and its strength (0 to ONE of a full glow).
pub fn puff(sp: &Spec, foot: V3, k: i32, tick: u64, alpha: Fx) -> (V3, Fx, Fx) {
    let life = sp.life_ticks.max(1) as i64 * ONE as i64;
    let n = sp.puffs.max(1) as i64;
    let t = (tick as i64 * ONE as i64 + alpha as i64 + k as i64 * life / n).rem_euclid(life);
    // its share of its life
    let s = (t / sp.life_ticks.max(1) as i64) as Fx;
    // it rises quickly at first and slows: 1 − (1 − s)²
    let rise = fx::mul(fx::int(sp.rise_m), ONE - fx::mul(ONE - s, ONE - s));
    let drift = fx::mul(fx::int(sp.drift_m), fx::mul(s, s));
    let wide = (sp.width_cm[0] * ONE / 100) + fx::mul(s, (sp.width_cm[1] - sp.width_cm[0]) * ONE / 100);
    let (a, b) = (sp.fade_in_pct * ONE / 100, sp.thin_from_pct * ONE / 100);
    let share = if s < a {
        fx::div(s, a.max(1))
    } else if s > b {
        fx::div(ONE - s, (ONE - b).max(1))
    } else {
        ONE
    };
    // the column thins as it climbs too
    let share = fx::mul(share, ONE - s / 2);
    let strength = fx::mul(sp.strength_pct * ONE / 100, share);
    // each puff a little to its own side, so the column is not a rod
    let wander = fx::mul(fx::sin(fx::deg(137 * k)), fx::mul(ONE * 4 / 10, s));
    let at = foot.add(v3(
        fx::mul(drift, ONE * 8 / 10) + wander,
        ONE * 12 / 10 + rise,
        -fx::mul(drift, ONE * 6 / 10) + wander / 2,
    ));
    (at, wide, strength)
}

/// The column over `foot`, drawn into `out`: every puff, in a fixed order.
pub fn column(sp: &Spec, w: &World, foot: V3, alpha: Fx, out: &mut Shapes) {
    let (tick, alpha) = if w.story.still { (0, 0) } else { (w.tick, alpha) };
    let (dark, light) = (crate::look::rgb(sp.dark), crate::look::rgb(sp.light));
    for k in 0..sp.puffs {
        let (at, wide, strength) = puff(sp, foot, k, tick, alpha);
        let up = fx::div(at.y - foot.y, fx::int(sp.rise_m) + ONE).clamp(0, ONE);
        let rgb = [0, 1, 2].map(|i| dark[i] + fx::mul(light[i] - dark[i], up));
        let place = Affine::translate(at)
            .then(&Affine::rot_y(fx::deg(67 * k)))
            .then(&Affine::scale(v3(wide, wide * sp.tall_pct / 100, wide)));
        out.glow(Mesh::Blob, place, rgb, strength);
    }
}
