//! The lineup: every model stood in rows in front of the player, for review
//! (`act("models.lineup", …)`, `tools/shoot.py`'s model views). It is the
//! module's state, act and draw under the extension contract (`world.rs`):
//! it changes nothing in play, and is off until asked for.
//!
//! The rows are laid out from where the player stood and looked when the
//! lineup was asked for, so the over-the-shoulder camera looks at them:
//! figures 7 m ahead, the small props 14 m, the buildings 26 m. A close
//! view puts one model 1.5 m ahead and 0.75 m right of the feet, just
//! right of the camera's line, about 3.9 m from the lens.

use super::*;
use crate::geom::{facing, right_of};
use crate::world::World;

/// What the lineup shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Show {
    /// every figure in a row, every prop in rows behind
    All,
    /// every prop in rows, no figures
    Props,
    /// one figure, close
    One(Kind),
    /// every held item, stacked in the air
    Weapons,
    /// the speedster pieces worn and lying
    Speedster,
    /// the cart and its donkey, side on
    Cart,
}

/// The lineup's state: what it shows (none: off), where it stands from, and
/// how far its figures turn from facing the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Lineup {
    pub show: Option<Show>,
    pub feet: V3,
    pub yaw: i32,
    pub turn: i32,
}

/// The names `models.lineup` takes, and what each shows.
const NAMES: &[(&str, Show)] = &[
    ("all", Show::All),
    ("props", Show::Props),
    ("player", Show::One(Kind::Player)),
    ("orc", Show::One(Kind::Orc)),
    ("bandit", Show::One(Kind::Bandit)),
    ("lookout", Show::One(Kind::Lookout)),
    ("leader", Show::One(Kind::Leader)),
    ("merchant", Show::One(Kind::Merchant)),
    ("trainer", Show::One(Kind::Trainer)),
    ("villager", Show::One(Kind::Villager)),
    ("weapons", Show::Weapons),
    ("speedster", Show::Speedster),
    ("cart", Show::Cart),
];

/// `models.<name>`: `lineup` takes `off`, or a name from `NAMES` with an
/// optional turn in degrees after a colon (`all:40`).
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    if name != "lineup" {
        return Err(format!("Unknown action \"models.{name}\"."));
    }
    let (what, turn) = match value.split_once(':') {
        Some((a, t)) => {
            let deg: i32 = t
                .trim()
                .parse()
                .map_err(|_| format!("The lineup's turn is whole degrees, such as 40, not \"{t}\"."))?;
            (a.trim(), fx::deg(deg.clamp(-180, 180)))
        }
        None => (value.trim(), 0),
    };
    if what == "off" {
        w.lineup = Lineup::default();
        return Ok(());
    }
    let show = NAMES
        .iter()
        .find(|(n, _)| *n == what)
        .map(|(_, s)| *s)
        .ok_or_else(|| format!("The lineup has no view \"{what}\"."))?;
    w.lineup = Lineup {
        show: Some(show),
        feet: w.player.pos,
        yaw: w.aim.yaw,
        turn,
    };
    Ok(())
}

/// The lineup's shapes, in a fixed order (CW-18). Nothing in it moves.
pub fn draw(w: &World, _alpha: Fx, look: &Look, out: &mut Shapes) {
    let l = &w.lineup;
    let Some(show) = l.show else {
        return;
    };
    let fw = facing(l.yaw, 0);
    let r = right_of(l.yaw);
    let spot = |ahead: i32, across: i32| l.feet.add(fw.scale(mm(ahead))).add(r.scale(mm(across)));
    // facing the player, turned by the lineup's turn
    let back = fx::wrap(l.yaw + fx::HALF_TURN + l.turn);
    match show {
        Show::All => {
            let row = cast();
            let n = row.len() as i32;
            for (i, (kind, dress)) in row.iter().enumerate() {
                let across = (2 * i as i32 - (n - 1)) * 650;
                stand(*kind, *dress, spot(7000, across), back, look, out);
            }
            props_rows(14_000, 27_000, l, look, out);
        }
        Show::Props => props_rows(10_000, 22_000, l, look, out),
        Show::One(kind) => {
            let dress = Dress::default();
            stand(kind, dress, spot(1000, 750), back, look, out);
        }
        Show::Weapons => {
            // each item level across the view, the stack rising from the waist
            let items = [
                Held::Knife,
                Held::ShortSword,
                Held::Sword,
                Held::Cleaver,
                Held::Boomerang,
                Held::Rock,
            ];
            let toward = fw.neg();
            for (i, item) in items.iter().enumerate() {
                let y = mm(500 + 300 * i as i32);
                let len = grip_to_tip(*item);
                let mid = spot(1300, 1000).add(v3(0, y, 0));
                let grip = mid.sub(r.scale(len / 2)).add(toward.scale(mm(60)));
                let tip = mid.add(r.scale(len / 2)).sub(toward.scale(mm(60)));
                held(*item, grip, tip, look, out);
            }
        }
        Show::Speedster => {
            let all = Dress {
                worn: SPEEDSTER_ALL,
                variant: 0,
            };
            stand(Kind::Player, all, spot(2600, 500), back, look, out);
            stand(Kind::Player, Dress::default(), spot(2600, 1700), back, look, out);
            for (i, item) in Pickup::ALL.iter().enumerate() {
                let across = 150 + 560 * i as i32;
                pickup(
                    *item,
                    spot(1300, across),
                    back + fx::deg(10 * i as i32),
                    look,
                    out,
                );
            }
        }
        Show::Cart => {
            // the cart broadside, its donkey in front
            let side = fx::wrap(l.yaw + fx::QUARTER_TURN);
            let base = spot(6500, 1400);
            let ahead = facing(side, 0);
            prop_shapes(Prop::Cart, base, side, look, out);
            donkey(
                base.add(ahead.scale(DONKEY_TO_CART)),
                side,
                fx::QUARTER_TURN / 2,
                look,
                out,
            );
        }
    }
}

/// The small props in a row `near` mm ahead, the buildings `far`.
fn props_rows(near: i32, far: i32, l: &Lineup, look: &Look, out: &mut Shapes) {
    let small = [
        Prop::Fence,
        Prop::Well,
        Prop::Chest,
        Prop::Campfire,
        Prop::Bed,
        Prop::Scarecrow,
        Prop::Signpost,
        Prop::Tent,
        Prop::Shrine,
        Prop::Cart,
    ];
    lay_out(&small, near, 900, l, look, out);
    let big = [Prop::Bridge, Prop::Hut, Prop::House, Prop::Barn, Prop::HollowTree];
    lay_out(&big, far, 2400, l, look, out);
}

/// The lineup's figures: every kind, each villager look, in order.
pub fn cast() -> Vec<(Kind, Dress)> {
    let mut out: Vec<(Kind, Dress)> = KINDS
        .iter()
        .filter(|k| **k != Kind::Villager)
        .map(|k| (*k, Dress::default()))
        .collect();
    for v in 0..VILLAGER_VARIANTS {
        out.push((Kind::Villager, Dress { worn: 0, variant: v }));
    }
    out
}

/// A figure at its rest pose, holding its own item.
fn stand(kind: Kind, dress: Dress, feet: V3, yaw: i32, look: &Look, out: &mut Shapes) {
    let pose = rest_pose(kind, feet, yaw);
    figure_dressed(kind, &pose, dress, look, out);
    held(kind_held(kind), pose.grip, pose.tip, look, out);
}

/// Props in a row `ahead` mm in front of the lineup's feet, each its own
/// radius apart plus `gap`, facing the player.
fn lay_out(row: &[Prop], ahead: i32, gap: i32, l: &Lineup, look: &Look, out: &mut Shapes) {
    let fw = facing(l.yaw, 0);
    let r = right_of(l.yaw);
    let widths: Vec<Fx> = row.iter().map(|p| footprint(*p)).collect();
    let total: Fx = widths.iter().sum::<Fx>() + mm(gap) * (row.len() as i32 - 1);
    let mut x = -total / 2;
    for (p, wd) in row.iter().zip(&widths) {
        let at = l.feet.add(fw.scale(mm(ahead))).add(r.scale(x + wd / 2));
        prop_shapes(*p, at, fx::wrap(l.yaw + fx::HALF_TURN), look, out);
        x += wd + mm(gap);
    }
}

/// How wide a prop stands across the row, from its parts.
fn footprint(p: Prop) -> Fx {
    let m = props::model(p);
    let half = m
        .parts
        .iter()
        .map(|q| q.at.t.x.abs() + (q.at.c0.x.abs() + q.at.c1.x.abs() + q.at.c2.x.abs()) / 2)
        .max()
        .unwrap_or(0);
    2 * half
}
