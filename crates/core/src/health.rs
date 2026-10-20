//! `health` (M1 Duel, task D): a hit takes health by the blade's speed at
//! contact, and a hit on an arm weakens that arm's servo for a while
//! (CW-16; no limb is cut in the MVP). A fighter at no health is down: it
//! falls over `FALL_TICKS`, its arms go limp and its pilot stops. Steps
//! after `blade` at 4 in `World::step`; its numbers reach the screen through
//! `player::hud`.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use crate::blade::Part;
use crate::fx::{Fx, ONE};
use crate::player;
use crate::world::World;

/// The player's health.
pub const PLAYER: i32 = 100;
/// A blow slower than this at contact takes nothing (1.5 m/s: a blade laid
/// on a body); above it, `PER_MPS_PCT` hundredths for each metre a second
/// (2.5: a cut through the front lands at 10 to 13 m/s, R3.44).
pub const MIN_HIT: Fx = ONE * 15 / 10;
pub const PER_MPS_PCT: i32 = 250;
/// What a blow takes by where it lands, percent.
pub const HEAD_PCT: i32 = 150;
pub const TORSO_PCT: i32 = 100;
pub const ARM_PCT: i32 = 60;
pub const LEG_PCT: i32 = 70;
/// A hit arm's servo drops to this share of its strength, and recovers to
/// whole over `WEAK_TICKS` (2.5 s).
pub const WEAK_PCT: i32 = 35;
pub const WEAK_TICKS: u16 = 150;
/// Down, a body takes this long to fall.
pub const FALL_TICKS: u16 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Health {
    pub hp: i32,
    pub most: i32,
    /// ticks of weakness left in each arm
    pub weak: [u16; 2],
    /// ticks since it went down (0 standing)
    pub down_ticks: u16,
}

impl Health {
    pub fn new(most: i32) -> Health {
        Health {
            hp: most,
            most,
            weak: [0, 0],
            down_ticks: 0,
        }
    }

    pub fn down(&self) -> bool {
        self.hp <= 0
    }

    /// The share left, percent, rounded down.
    pub fn pct(&self) -> i32 {
        (self.hp.max(0) * 100 / self.most.max(1)).min(100)
    }

    /// How far fallen, `ONE` lying down, eased in.
    pub fn fall(&self) -> Fx {
        if self.down_ticks == 0 {
            return 0;
        }
        let t = (self.down_ticks.min(FALL_TICKS) as i64 * ONE as i64 / FALL_TICKS as i64) as Fx;
        crate::fx::mul(t, t)
    }

    /// An arm's servo strength now, percent.
    pub fn strength(&self, arm: usize) -> i32 {
        100 - (100 - WEAK_PCT) * self.weak[arm] as i32 / WEAK_TICKS as i32
    }

    /// A blow: health taken by speed and where it landed; an arm hit is
    /// weakened. Returns what was taken.
    pub fn take(&mut self, speed: Fx, part: Part) -> i32 {
        let dmg = damage(speed, part);
        if dmg > 0 && !self.down() {
            self.hp -= dmg;
            if let Part::Arm(s) = part {
                self.weak[s] = WEAK_TICKS;
            }
        }
        dmg
    }
}

/// Health a blow at `speed` (metres a second) takes on `part`: (speed −
/// 1.5) × 2.5, times the part's share, rounded down; nothing below 1.5 m/s.
pub fn damage(speed: Fx, part: Part) -> i32 {
    let over = (speed - MIN_HIT).max(0) as i64;
    let pct = match part {
        Part::Head => HEAD_PCT,
        Part::Torso => TORSO_PCT,
        Part::Arm(_) => ARM_PCT,
        Part::Leg(_) => LEG_PCT,
    } as i64;
    (over * PER_MPS_PCT as i64 * pct / 10_000 / ONE as i64) as i32
}

/// Step 4, after the blades: this tick's hits and throwables' blows taken,
/// weakness wearing off, the fallen falling.
pub fn step(w: &mut World) {
    let hits = w.blades.hits.clone();
    let up: Vec<bool> = (0..player::count(w))
        .map(|id| !player::fighter(w, id).health.down())
        .collect();
    for h in &hits {
        player::parts_mut(w, h.on as usize).1.health.take(h.speed, h.part);
    }
    // who went down this tick (the story's hook, `feel::State::downed`)
    w.feel.downed = (0..player::count(w))
        .filter(|&id| up[id] && player::fighter(w, id).health.down())
        .map(|id| id as u8)
        .collect();
    for id in 0..player::count(w) {
        let h = &mut player::parts_mut(w, id).1.health;
        for s in 0..2 {
            h.weak[s] = h.weak[s].saturating_sub(1);
        }
        if h.down() {
            h.down_ticks = h.down_ticks.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blow_takes_health_by_its_speed_and_where_it_lands() {
        // 10 m/s on the torso: (10 − 1.5) × 2.5 = 21.25, rounded down to
        // 21; on the head × 1.5 = 31.9: 31; on an arm × 0.6 = 12.75: 12
        let ten = crate::fx::int(10);
        assert_eq!(damage(ten, Part::Torso), 21);
        assert_eq!(damage(ten, Part::Head), 31);
        assert_eq!(damage(ten, Part::Arm(1)), 12);
        assert_eq!(damage(ten, Part::Leg(0)), 14, "8.5 × 2.5 × 0.7 = 14.9");
        assert_eq!(damage(ONE, Part::Head), 0, "a blade laid on takes nothing");
    }

    #[test]
    fn a_hit_arm_is_weakened_then_recovers() {
        let mut h = Health::new(100);
        h.take(crate::fx::int(6), Part::Arm(1));
        assert_eq!(h.hp, 100 - 6, "4.5 × 2.5 × 0.6 = 6.75");
        assert_eq!(h.strength(1), WEAK_PCT);
        assert_eq!(h.strength(0), 100, "the other arm is whole");
        h.weak[1] = WEAK_TICKS / 2;
        // halfway: 100 − 65 × 75 / 150 = 67.5, rounded toward zero
        assert_eq!(h.strength(1), 68);
    }

    #[test]
    fn at_no_health_a_fighter_is_down_and_falls() {
        // 10 m/s on the torso takes 21: exactly all of 21
        let mut h = Health::new(21);
        h.take(crate::fx::int(10), Part::Torso);
        assert_eq!(h.hp, 0);
        assert!(h.down());
        assert_eq!(h.pct(), 0);
        h.down_ticks = FALL_TICKS / 2;
        assert_eq!(h.fall(), ONE / 4, "halfway in time, a quarter fallen");
        h.down_ticks = FALL_TICKS * 3;
        assert_eq!(h.fall(), ONE);
    }
}
