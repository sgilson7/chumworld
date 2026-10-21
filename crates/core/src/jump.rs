//! `jump`: off a ledge, through the air, and down again (T21, phase A;
//! CW-Q18's default: Zelda's hop, no jump key).
//!
//! Every body stands on the surface under it (`solid::surface`), as before,
//! until the surface drops away faster than a slope can: a **ledge**, more
//! than `STEP_DOWN` plus `SLOPE` of the ground the body crossed this tick.
//! There the body leaves its feet. Going at least `HOP_SPEED` with its weapon
//! put away it **hops**, rising at `HOP_VY`; slower, or with a weapon drawn
//! (CW-Q18: no jumping in a fight), it simply falls. In the air it falls
//! under `GRAVITY`, keeps only a share of its feet's grip (`motion`'s
//! `AIR_GRIP_DIV`) and cannot dodge, and it lands on the first surface its
//! feet come down to. Climbs (`roadside::Climb`) keep out a body whose feet
//! are below their top, so a hop that comes up short meets a pillar's side.
//!
//! The same rule for every body (CW-3 is about decisions, not about walls).
//! Hand-worked: rising at 3 m/s under 9.8 m/s², the feet top out 0.49 m up
//! after 19 ticks and come back to the height they left after 37 (0.61 s);
//! at the run speed, 7 m/s, that is 4.3 m along.

use crate::fx::{Fx, ONE};
use crate::motion::Gait;

/// A drop the feet step down without leaving the ground.
pub const STEP_DOWN: Fx = ONE * 35 / 100;
/// The steepest ground walked down without leaving it, metres down a metre
/// along: a sprint at ten times the run crosses 1.2 m a tick, and the
/// meadow's steepest rolls never part a body from its feet (R9.422's test).
pub const SLOPE: Fx = ONE * 6 / 10;
/// Fall speed gained a tick: 9.8 m/s² is 9.8 / 3600 m a tick a tick.
pub const GRAVITY: Fx = ONE * 98 / 36_000;
/// A hop's rise: 3 m/s, a twentieth of a metre a tick.
pub const HOP_VY: Fx = ONE / 20;
/// The share of the run speed a body must be going to hop off a ledge.
pub const HOP_SHARE_PCT: i32 = 90;
/// The fastest a body falls: 30 m/s.
pub const TERMINAL: Fx = ONE / 2;

/// The height of a body's feet this tick, from where they were last tick
/// (`prev_y`), the surface now under them (`floor`), how far the body went
/// along the ground (`gone`) and how fast, a tick (`speed`), and whether it
/// may hop (`may_hop`: its weapon away, and up). Moves the gait's `air` and
/// `vy`.
pub fn settle(g: &mut Gait, prev_y: Fx, floor: Fx, gone: Fx, speed: Fx, may_hop: bool) -> Fx {
    if !g.air {
        let drop = prev_y - floor;
        if drop <= STEP_DOWN + crate::fx::mul(gone, SLOPE) {
            return floor;
        }
        // a ledge: off the feet, hopping if going fast enough
        g.air = true;
        g.vy = if may_hop && speed >= hop_speed() {
            HOP_VY
        } else {
            0
        };
    }
    let y = prev_y + g.vy;
    g.vy = (g.vy - GRAVITY).max(-TERMINAL);
    if y <= floor {
        g.air = false;
        g.vy = 0;
        return floor;
    }
    y
}

/// The speed a tick to hop at: `HOP_SHARE_PCT` of the bare run.
pub fn hop_speed() -> Fx {
    crate::motion::per_tick(crate::motion::RUN, 0) * HOP_SHARE_PCT / 100
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fly(g: &mut Gait, from: Fx, floor_at: impl Fn(i32) -> Fx, speed: Fx, may_hop: bool) -> Vec<Fx> {
        let mut y = from;
        let mut ys = Vec::new();
        for k in 0..200 {
            y = settle(g, y, floor_at(k), speed, speed, may_hop);
            ys.push(y);
            if !g.air && k > 0 {
                break;
            }
        }
        ys
    }

    #[test]
    fn running_off_a_ledge_hops_up_and_comes_down_on_the_floor_below() {
        let mut g = Gait::default();
        let run = hop_speed() * 100 / HOP_SHARE_PCT;
        let top = ONE * 3;
        // off the ledge at tick 0: the floor is the ground, 3 m below
        let ys = fly(&mut g, top, |_| 0, run, true);
        // the first tick rises by 3 m/s = 3 276 units
        assert_eq!(ys[0], top + HOP_VY);
        // the highest, tick by tick: the rise moves first, then gravity takes
        // its 178, so the feet climb 3 276, 3 098, … while the rise is
        // positive: 19 ticks (k = 0..18), 19 × 3 276 − 178 × (0 + … + 18)
        // = 62 244 − 30 438 = 31 806 units, 0.485 m (the smooth v²/2g is
        // 0.459 m; whole ticks add half a tick's rise)
        let apex = *ys.iter().max().unwrap() - top;
        assert_eq!(apex, 19 * 3276 - 178 * (18 * 19 / 2));
        // down on the floor, and on its feet
        assert_eq!(*ys.last().unwrap(), 0);
        assert!(!g.air && g.vy == 0);
        // down 3 m from a rise of 0.05 a tick: ½ g t² − 0.05 t = 3, so
        // t = (0.05 + √(0.0025 + 6 g)) / g = 68.9 ticks with g = 178 / 65 536
        let ticks = ys.len();
        assert!((67..=72).contains(&ticks), "{ticks} ticks in the air");
    }

    #[test]
    fn walking_off_a_ledge_falls_without_rising_and_a_drawn_sword_does_not_hop() {
        let walk = crate::motion::per_tick(crate::motion::WALK, 0);
        let mut g = Gait::default();
        let ys = fly(&mut g, ONE * 2, |_| 0, walk, true);
        assert!(ys.iter().all(|&y| y <= ONE * 2), "a walk never rises");
        assert_eq!(*ys.last().unwrap(), 0);
        let run = hop_speed() * 100 / HOP_SHARE_PCT;
        let mut g = Gait::default();
        let ys = fly(&mut g, ONE * 2, |_| 0, run, false);
        assert!(ys.iter().all(|&y| y <= ONE * 2), "drawn, no hop");
    }

    #[test]
    fn a_step_down_or_a_slope_keeps_the_feet_on_the_ground() {
        let mut g = Gait::default();
        // 0.35 m straight down is a step
        assert_eq!(settle(&mut g, STEP_DOWN, 0, 0, 0, true), 0);
        assert!(!g.air);
        // a sprint crossing 1.17 m on a 50% slope drops 0.585 m: under
        // 0.35 + 0.6 × 1.17 = 1.05 m, so still on its feet
        let gone = ONE * 117 / 100;
        assert_eq!(settle(&mut g, gone / 2, 0, gone, gone, true), 0);
        assert!(!g.air);
        // but 1.1 m down in that tick is a ledge
        assert!(settle(&mut g, ONE * 11 / 10, 0, gone, gone, true) > 0);
        assert!(g.air);
    }

    #[test]
    fn a_fall_never_passes_thirty_metres_a_second() {
        let mut g = Gait::default();
        let ys = fly(&mut g, ONE * 400, |_| 0, 0, false);
        for w in ys.windows(2) {
            assert!(w[0] - w[1] <= TERMINAL, "fell {} in a tick", w[0] - w[1]);
        }
    }
}
