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
