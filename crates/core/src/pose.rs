//! The shared figure: where each joint of a person (or an orc) is this tick.
//!
//! Set by the coordinator before M1 to M4 ran in parallel, so that the duel
//! (M1) and the models (M3) meet in one type. M1 fills a `Pose` from its
//! bodies and arms each tick; M3's `models::figure` draws a character from
//! one. Neither edits the other's module: a new joint is an added field here,
//! agreed through the coordinator.
//!
//! Joints are in world coordinates (`geom` conventions: x east, y up, z
//! south). Left and right are the figure's own.

use crate::geom::V3;

/// Who a figure is: picks the model, its colours and its proportions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Player,
    Orc,
    Bandit,
    Leader,
    Lookout,
    Merchant,
    Trainer,
    Villager,
}

/// What a hand holds: picks the weapon's model and, in M1, its blade length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Held {
    Nothing,
    ShortSword,
    Sword,
    /// an orc's heavy blade
    Cleaver,
    Boomerang,
    Rock,
    Knife,
}

/// Every joint of one figure this tick. `prev`/`now` pairs are interpolated
/// by the caller (`Pose::lerp`) before drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pose {
    pub pelvis: V3,
    pub chest: V3,
    /// the centre of the head
    pub head: V3,
    /// which way the figure faces, an angle as in `camera::Aim::yaw`
    pub facing: i32,
    pub shoulder: [V3; 2],
    pub elbow: [V3; 2],
    pub hand: [V3; 2],
    pub hip: [V3; 2],
    pub knee: [V3; 2],
    pub foot: [V3; 2],
    /// the held item's grip (the hilt, where the hands close) and its tip
    pub grip: V3,
    pub tip: V3,
}

/// Index into the `[V3; 2]` joints.
pub const LEFT: usize = 0;
pub const RIGHT: usize = 1;

impl Pose {
    /// `alpha` of the way from `self` to `to` (`fx` fraction), joint by joint.
    pub fn lerp(&self, to: &Pose, alpha: i32) -> Pose {
        let two = |a: [V3; 2], b: [V3; 2]| [a[0].lerp(b[0], alpha), a[1].lerp(b[1], alpha)];
        Pose {
            pelvis: self.pelvis.lerp(to.pelvis, alpha),
            chest: self.chest.lerp(to.chest, alpha),
            head: self.head.lerp(to.head, alpha),
            facing: crate::fx::lerp_angle(self.facing, to.facing, alpha),
            shoulder: two(self.shoulder, to.shoulder),
            elbow: two(self.elbow, to.elbow),
            hand: two(self.hand, to.hand),
            hip: two(self.hip, to.hip),
            knee: two(self.knee, to.knee),
            foot: two(self.foot, to.foot),
            grip: self.grip.lerp(to.grip, alpha),
            tip: self.tip.lerp(to.tip, alpha),
        }
    }
}

/// A figure's proportions in metres (`fx`), one set per `Kind`. The duel
/// (M1) builds its arms and legs to these lengths and the models (M3) dress
/// them; both read them here so the sword arm and the drawn arm agree. Heads
/// are about a quarter of the height (CW-17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Build {
    /// feet to the crown
    pub height: i32,
    /// the head's height, crown to chin
    pub head: i32,
    /// feet to the shoulders' line
    pub shoulder_y: i32,
    /// centre to each shoulder
    pub shoulder_x: i32,
    pub upper_arm: i32,
    /// elbow to the middle of the grip
    pub forearm: i32,
    /// feet to the hips' line
    pub hip_y: i32,
    /// centre to each hip
    pub hip_x: i32,
    pub thigh: i32,
    pub shin: i32,
    /// how broad the torso is, in percent of the player's
    pub girth_pct: i32,
}

/// Centimetres to `fx` metres.
const fn cm(c: i32) -> i32 {
    c * crate::fx::ONE / 100
}

/// The player's proportions, from which the others are scaled: 1.70 m, a
/// 0.42 m head (25%), long arms so the sword can pass over the head.
const PLAYER: Build = Build {
    height: cm(170),
    head: cm(42),
    shoulder_y: cm(124),
    shoulder_x: cm(20),
    upper_arm: cm(31),
    forearm: cm(29),
    hip_y: cm(72),
    hip_x: cm(10),
    thigh: cm(37),
    shin: cm(35),
    girth_pct: 100,
};

/// A kind's height in centimetres and girth in percent of the player's.
fn size(kind: Kind) -> (i32, i32) {
    match kind {
        Kind::Player => (170, 100),
        Kind::Orc => (200, 135),
        Kind::Bandit => (175, 105),
        Kind::Leader => (185, 115),
        Kind::Lookout => (165, 90),
        Kind::Merchant => (160, 125),
        Kind::Trainer => (172, 100),
        Kind::Villager => (165, 100),
    }
}

/// The proportions of a kind: the player's scaled to its height, then
/// widened by its girth. The head keeps the player's quarter of the height.
pub fn build(kind: Kind) -> Build {
    let (h, girth) = size(kind);
    let s = |v: i32| (v as i64 * h as i64 / 170) as i32;
    let w = |v: i32| (s(v) as i64 * girth as i64 / 100) as i32;
    Build {
        height: s(PLAYER.height),
        head: s(PLAYER.head),
        shoulder_y: s(PLAYER.shoulder_y),
        shoulder_x: w(PLAYER.shoulder_x),
        upper_arm: s(PLAYER.upper_arm),
        forearm: s(PLAYER.forearm),
        hip_y: s(PLAYER.hip_y),
        hip_x: w(PLAYER.hip_x),
        thigh: s(PLAYER.thigh),
        shin: s(PLAYER.shin),
        girth_pct: girth,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heads_are_a_quarter_of_the_height_give_or_take_two_points() {
        // CW-17: 42 cm of 170 cm is 24.7%; every kind scales both together.
        for k in [Kind::Player, Kind::Orc, Kind::Bandit, Kind::Leader, Kind::Lookout, Kind::Merchant, Kind::Trainer, Kind::Villager] {
            let b = build(k);
            let pct = b.head as i64 * 100 / b.height as i64;
            assert!((23..=27).contains(&pct), "{k:?}: {pct}%");
        }
    }

    #[test]
    fn the_player_is_one_metre_seventy_and_an_orc_two_metres() {
        assert_eq!(build(Kind::Player).height, 170 * crate::fx::ONE / 100);
        // 2 m, less the one unit (1/65536 m) that 170 cm loses to rounding
        assert!((build(Kind::Orc).height - 2 * crate::fx::ONE).abs() <= 1);
        // an orc's shoulders: 20 cm × 200/170 × 135% = 31.76 cm
        assert_eq!(build(Kind::Orc).shoulder_x, ((20 * crate::fx::ONE / 100) as i64 * 200 / 170 * 135 / 100) as i32);
    }
}
