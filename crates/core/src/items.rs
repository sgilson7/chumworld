//! `items`: every item in the MVP, from `data/items.json` (M4, task U).
//!
//! Each item has its one route (how it is acquired) and its one mechanic
//! (CW-6), the starting gear changes nothing (CW-7), and the speedster's
//! pieces add speed and take grip by the numbers pinned for 0 to 5 pieces
//! (CW-8). The file is compiled into the core: its routes, mechanics, prices
//! and stacks are rules, not presentation. Names and descriptions are
//! reader text and live in `data/strings.json` (`item_<id>`,
//! `item_<id>_about`).
//!
//! # The doors other milestones read
//!
//! - M1's `motion.rs`: `speedster_worn(w)`, then `speedster_speed(n)` and
//!   `speedster_grip(n)` (shares of `fx::ONE`); M1 decides how footing uses
//!   them. `held(w)` is what the sword hand holds, `blade_cm` its length.
//! - M1's throwing and M5's boomerang: `inventory::ready(w)` is the item the
//!   free hand holds (chosen with 1, 2 and 3), `inventory::take` uses one.
//! - M5: `inventory::give(w, item, n)` grants an item and shows the toast.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::fx::{Fx, ONE};
use crate::pose::Held;
use crate::world::World;

/// Every item, in the order `data/items.json` lists them. `Item as usize`
/// indexes the inventory's counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Item {
    ShortSword,
    Sword,
    Boomerang,
    Rock,
    Knife,
    Bandage,
    ChestKey,
    Hood,
    Jacket,
    Gloves,
    Trousers,
    Boots,
}

impl Item {
    pub const ALL: [Item; 12] = [
        Item::ShortSword,
        Item::Sword,
        Item::Boomerang,
        Item::Rock,
        Item::Knife,
        Item::Bandage,
        Item::ChestKey,
        Item::Hood,
        Item::Jacket,
        Item::Gloves,
        Item::Trousers,
        Item::Boots,
    ];

    /// The id in `data/items.json`, and in string keys (`item_<id>`).
    pub fn id(self) -> &'static str {
        match self {
            Item::ShortSword => "short_sword",
            Item::Sword => "sword",
            Item::Boomerang => "boomerang",
            Item::Rock => "rock",
            Item::Knife => "knife",
            Item::Bandage => "bandage",
            Item::ChestKey => "chest_key",
            Item::Hood => "hood",
            Item::Jacket => "jacket",
            Item::Gloves => "gloves",
            Item::Trousers => "trousers",
            Item::Boots => "boots",
        }
    }

    pub fn from_id(id: &str) -> Option<Item> {
        Item::ALL.iter().copied().find(|i| i.id() == id)
    }

    /// This item's entry in the catalogue.
    pub fn def(self) -> &'static Def {
        &catalogue().items[self as usize]
    }
}

/// What kind of thing an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Weapon,
    Throwable,
    Supply,
    Key,
    Armour,
}

/// Which hand a weapon goes in: the sword hand (Equipment) or the free hand
/// (Select Item, thrown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Hand {
    Sword,
    Free,
}

/// The five armour slots, head to feet (Fable's pieces, one per slot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Slot {
    Head,
    Body,
    Hands,
    Legs,
    Feet,
}

impl Slot {
    pub const ALL: [Slot; 5] = [Slot::Head, Slot::Body, Slot::Hands, Slot::Legs, Slot::Feet];

    pub fn id(self) -> &'static str {
        match self {
            Slot::Head => "head",
            Slot::Body => "body",
            Slot::Hands => "hands",
            Slot::Legs => "legs",
            Slot::Feet => "feet",
        }
    }
}

/// One item's entry in `data/items.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Def {
    pub id: String,
    pub kind: Kind,
    #[serde(default)]
    pub hand: Option<Hand>,
    #[serde(default)]
    pub slot: Option<Slot>,
    #[serde(default)]
    pub set: Option<String>,
    pub routes: Vec<String>,
    pub mechanic: String,
    #[serde(default)]
    pub start: bool,
    #[serde(default)]
    pub blade_cm: i32,
    #[serde(default)]
    pub reach_m: i32,
    #[serde(default)]
    pub flight_ticks: i32,
    #[serde(default)]
    pub heals: i32,
    /// where it sits in Select Item's grid
    #[serde(default)]
    pub grid: Option<usize>,
    /// whether it can go on 1, 2 or 3
    #[serde(default)]
    pub quick: bool,
    /// whether the merchant sells it
    #[serde(default)]
    pub sold: bool,
    /// whether the merchant refuses to buy it
    #[serde(default)]
    pub keep: bool,
    /// coins
    pub value: u32,
    /// the most one can carry
    pub stack: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Set {
    pub speed_pct_per_piece: i32,
    pub grip_pct_per_piece: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Imbue {
    pub routes: Vec<String>,
    pub mechanic: String,
    pub lasts_ticks: u32,
    pub cooldown_ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Merchant {
    pub buys_at_pct: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Grid {
    pub across: usize,
    pub down: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Sets {
    pub speedster: Set,
}

/// The whole file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Catalogue {
    pub items: Vec<Def>,
    pub slots: Vec<Slot>,
    pub sets: Sets,
    pub imbue: Imbue,
    pub merchant: Merchant,
    pub grid: Grid,
}

/// Reads a catalogue and checks that it lists exactly the items `Item`
/// names, in its order.
pub fn parse(json: &str) -> Result<Catalogue, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("data/items.json could not be read: {e}"))?;
    strip_notes(&mut value);
    let c: Catalogue =
        serde_json::from_value(value).map_err(|e| format!("data/items.json could not be read: {e}"))?;
    let ids: Vec<&str> = c.items.iter().map(|d| d.id.as_str()).collect();
    let want: Vec<&str> = Item::ALL.iter().map(|i| i.id()).collect();
    if ids != want {
        return Err(format!("data/items.json lists {ids:?}; the core knows {want:?}."));
    }
    Ok(c)
}

/// Keys starting with `_` hold notes for people.
fn strip_notes(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            map.retain(|k, _| !k.starts_with('_'));
            for x in map.values_mut() {
                strip_notes(x);
            }
        }
        serde_json::Value::Array(list) => list.iter_mut().for_each(strip_notes),
        _ => {}
    }
}

/// The catalogue the game runs on, read once.
pub fn catalogue() -> &'static Catalogue {
    static CATALOGUE: OnceLock<Catalogue> = OnceLock::new();
    CATALOGUE.get_or_init(|| {
        parse(include_str!("../../../data/items.json")).expect("data/items.json matches items::Item")
    })
}

/// How many pieces of the speedster's armour can be worn at once.
pub const SPEEDSTER_PIECES: u8 = 5;

/// The speed a body has with `n` speedster pieces worn, as a share of its
/// speed without them (`ONE` = 100%): 1 + 0.2 n (CW-8), rounded to the
/// nearest unit.
pub fn speedster_speed(n: u8) -> Fx {
    let n = n.min(SPEEDSTER_PIECES) as i64;
    let pct = 100 + catalogue().sets.speedster.speed_pct_per_piece as i64 * n;
    ((ONE as i64 * pct + 50) / 100) as Fx
}

/// The grip a body has with `n` speedster pieces worn, as a share of full
/// grip (`ONE`): 0.55ⁿ (CW-8), worked in whole numbers and rounded to the
/// nearest unit once, so the five values are exact.
pub fn speedster_grip(n: u8) -> Fx {
    let n = n.min(SPEEDSTER_PIECES) as u32;
    let p = catalogue().sets.speedster.grip_pct_per_piece as i64;
    let num = ONE as i64 * p.pow(n);
    let den = 100i64.pow(n);
    ((num + den / 2) / den) as Fx
}

/// How many speedster pieces the player wears, 0 to 5.
pub fn speedster_worn(w: &World) -> u8 {
    w.inventory
        .worn
        .iter()
        .flatten()
        .filter(|i| i.def().set.as_deref() == Some("speedster"))
        .count() as u8
}

/// The speedster pieces worn, one bit each, so M3 can draw the figure
/// dressed: `HOOD`, `JACKET`, `GLOVES`, `TROUSERS`, `BOOTS`.
pub const HOOD: u8 = 1;
pub const JACKET: u8 = 2;
pub const GLOVES: u8 = 4;
pub const TROUSERS: u8 = 8;
pub const BOOTS: u8 = 16;

pub fn speedster_mask(w: &World) -> u8 {
    let bit = |i: Item| match i {
        Item::Hood => HOOD,
        Item::Jacket => JACKET,
        Item::Gloves => GLOVES,
        Item::Trousers => TROUSERS,
        Item::Boots => BOOTS,
        _ => 0,
    };
    w.inventory.worn.iter().flatten().fold(0, |m, i| m | bit(*i))
}

/// What the player's sword hand holds.
pub fn held(w: &World) -> Held {
    match w.inventory.weapon {
        Some(Item::ShortSword) => Held::ShortSword,
        Some(Item::Sword) => Held::Sword,
        _ => Held::Nothing,
    }
}

/// The blade's length in centimetres for what the sword hand holds, 0 for
/// an empty hand.
pub fn blade_cm(w: &World) -> i32 {
    w.inventory.weapon.map(|i| i.def().blade_cm).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs() -> &'static [Def] {
        &catalogue().items
    }

    fn gear() -> Vec<&'static Def> {
        defs()
            .iter()
            .filter(|d| matches!(d.kind, Kind::Weapon | Kind::Armour))
            .collect()
    }

    /// CW-6: every weapon and armour piece has exactly one route, and no
    /// other weapon or armour piece is acquired that way.
    #[test]
    fn every_weapon_and_armour_piece_has_exactly_one_route_of_its_own() {
        let g = gear();
        assert_eq!(g.len(), 8, "three weapons and five speedster pieces");
        for d in &g {
            assert_eq!(d.routes.len(), 1, "{}: {:?}", d.id, d.routes);
            assert!(!d.routes[0].is_empty(), "{}", d.id);
        }
        for (k, a) in g.iter().enumerate() {
            for b in &g[k + 1..] {
                assert_ne!(a.routes, b.routes, "{} and {} share a route", a.id, b.id);
            }
        }
    }

    /// CW-6: every weapon and armour piece has a mechanic no other item
    /// has. The pieces of one set share the set's mechanic, and nothing
    /// outside the set has it (SECOND-ORDER.md R6.2); the short sword's is
    /// the baseline, the named exception.
    #[test]
    fn every_weapon_and_armour_piece_has_a_mechanic_no_other_item_has() {
        for a in gear() {
            assert!(!a.mechanic.is_empty(), "{}", a.id);
            for b in defs() {
                if a.id == b.id || a.mechanic != b.mechanic {
                    continue;
                }
                assert!(
                    a.set.is_some() && a.set == b.set,
                    "{} and {} share the mechanic {}",
                    a.id,
                    b.id,
                    a.mechanic
                );
            }
        }
        assert_eq!(Item::ShortSword.def().mechanic, "baseline");
        // the five pieces are one set, one per slot
        let set: Vec<Slot> = defs()
            .iter()
            .filter(|d| d.set.as_deref() == Some("speedster"))
            .filter_map(|d| d.slot)
            .collect();
        assert_eq!(set, Slot::ALL.to_vec());
    }

    /// The routes the journal's items entry gives (2026-10-20-items).
    #[test]
    fn each_item_is_acquired_the_way_the_journal_says() {
        let route = |i: Item| i.def().routes.join(" ");
        assert_eq!(route(Item::ShortSword), "start");
        assert_eq!(route(Item::Sword), "leader_duel");
        assert_eq!(route(Item::Boomerang), "camp_chest");
        assert_eq!(route(Item::ChestKey), "leader_drop");
        assert_eq!(route(Item::Hood), "merchant");
        assert_eq!(route(Item::Jacket), "scarecrow");
        assert_eq!(route(Item::Gloves), "lookout");
        assert_eq!(route(Item::Trousers), "hollow_tree");
        assert_eq!(route(Item::Boots), "river_shrine");
        assert_eq!(catalogue().imbue.routes, vec!["trainer".to_string()]);
        // the boomerang's flight (journal: 18 m reach, 150 ticks out and back)
        assert_eq!(Item::Boomerang.def().reach_m, 18);
        assert_eq!(Item::Boomerang.def().flight_ticks, 150);
        assert_eq!(Item::ShortSword.def().blade_cm, 70);
        assert_eq!(Item::Sword.def().blade_cm, 100);
    }

    /// CW-9's numbers, as the file holds them.
    #[test]
    fn the_imbue_lasts_ten_seconds_and_recovers_twenty_from_the_cast() {
        assert_eq!(catalogue().imbue.lasts_ticks, 600);
        assert_eq!(catalogue().imbue.cooldown_ticks, 1200);
    }

    /// CW-7: the starting gear is the short sword alone, and it changes
    /// nothing: full speed, full grip, no set, no healing, the baseline.
    #[test]
    fn the_starting_gear_changes_nothing() {
        let start: Vec<Item> = Item::ALL.iter().copied().filter(|i| i.def().start).collect();
        assert_eq!(start, vec![Item::ShortSword]);
        let d = Item::ShortSword.def();
        assert_eq!((d.set.clone(), d.slot, d.heals), (None, None, 0));
        let w = World::new(1);
        assert_eq!(speedster_worn(&w), 0);
        assert_eq!(speedster_speed(speedster_worn(&w)), ONE);
        assert_eq!(speedster_grip(speedster_worn(&w)), ONE);
        assert_eq!(held(&w), Held::ShortSword);
        assert_eq!(blade_cm(&w), 70);
    }

    /// CW-8, worked by hand: ONE is 65 536.
    /// speed: 65 536 × (100 + 20 n) / 100 = 65 536, 78 643.2, 91 750.4,
    /// 104 857.6, 117 964.8, 131 072, rounded to the nearest.
    /// grip: 65 536 × 0.55ⁿ = 65 536, 36 044.8, 19 824.64, 10 903.552,
    /// 5 996.9536, 3 298.32448, rounded to the nearest.
    #[test]
    fn speedster_speed_and_grip_are_pinned_for_zero_to_five_pieces() {
        let speed: Vec<Fx> = (0..=5).map(speedster_speed).collect();
        assert_eq!(speed, vec![65_536, 78_643, 91_750, 104_858, 117_965, 131_072]);
        let grip: Vec<Fx> = (0..=5).map(speedster_grip).collect();
        assert_eq!(grip, vec![65_536, 36_045, 19_825, 10_904, 5_997, 3_298]);
        // a sixth piece cannot be worn: the values stop at five
        assert_eq!(speedster_speed(6), 131_072);
        assert_eq!(speedster_grip(9), 3_298);
    }

    /// The hood (1) and the boots (16): 17. All five: 31. None at the start.
    #[test]
    fn the_speedster_mask_has_a_bit_for_each_piece_worn() {
        let mut w = World::new(1);
        assert_eq!(speedster_mask(&w), 0);
        for i in [Item::Hood, Item::Boots] {
            w.inventory.add(i, 1);
            w.inventory.equip(i).unwrap();
        }
        assert_eq!(speedster_mask(&w), 17);
        crate::inventory::give_all(&mut w);
        assert_eq!(speedster_mask(&w), 31);
    }

    #[test]
    fn the_catalogue_must_list_the_items_the_core_knows_in_order() {
        let mut v: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/items.json")).unwrap();
        v["items"].as_array_mut().unwrap().swap(0, 1);
        let e = parse(&v.to_string()).unwrap_err();
        assert!(
            e.starts_with("data/items.json lists [\"sword\", \"short_sword\""),
            "{e}"
        );
        let mut v: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/items.json")).unwrap();
        v["items"][0]["colour"] = "red".into();
        assert!(parse(&v.to_string()).is_err(), "an unknown field is refused");
    }

    #[test]
    fn select_item_places_each_free_hand_item_in_its_own_cell() {
        let g = &catalogue().grid;
        let mut cells: Vec<usize> = defs().iter().filter_map(|d| d.grid).collect();
        let n = cells.len();
        cells.sort();
        cells.dedup();
        assert_eq!(cells.len(), n, "two items in one cell");
        assert!(cells.iter().all(|&c| c < g.across * g.down));
        // what goes in the grid: everything not worn and not in the sword hand
        for d in defs() {
            let elsewhere = d.slot.is_some() || d.hand == Some(Hand::Sword);
            assert_eq!(d.grid.is_none(), elsewhere, "{}", d.id);
        }
    }
}
