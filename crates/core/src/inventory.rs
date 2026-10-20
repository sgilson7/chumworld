//! `inventory`: what the player carries and wears (M4, task U).
//!
//! Counts of each item, the sword in the sword hand, the five armour slots,
//! the quick items on 1, 2 and 3 (`input::ITEM_1` …) as Ocarina of Time's C
//! buttons held them, the item the free hand holds ready, and coins. Plain
//! arrays indexed by `items::Item` and `items::Slot`: no float, no hash map.
//!
//! Pressing 1, 2 or 3 in play readies that key's item in the free hand
//! (`ready`): M1 throws it on F, M5 throws the boomerang, and a bandage is
//! M1's to apply. Pressing the same key again puts it away.
//!
//! Acts (`inventory.<name>`), for M5 and for checks:
//! - `inventory.give` `<id>` or `<id>:<count>`: grants the item, with the
//!   toast "Received: …";
//! - `inventory.take` `<id>` or `<id>:<count>`;
//! - `inventory.coins` `<n>`: grants coins.
//!
//! Test-only, so screenshots and checks can show a full inventory:
//! `test.give_all` (every item to its stack, every speedster piece worn).

use crate::input::{Input, ITEM_1, ITEM_2, ITEM_3};
use crate::items::{Hand, Item, Kind, Slot};
use crate::world::World;

pub const QUICK_KEYS: [u32; 3] = [ITEM_1, ITEM_2, ITEM_3];
const N: usize = Item::ALL.len();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// how many of each item, by `Item as usize`
    pub count: [u32; N],
    /// the sword in the sword hand
    pub weapon: Option<Item>,
    /// what each armour slot holds, by `Slot as usize`
    pub worn: [Option<Item>; 5],
    /// the item on each of 1, 2 and 3
    pub quick: [Option<Item>; 3],
    /// the quick key whose item the free hand holds ready
    pub ready: Option<usize>,
    pub coins: u32,
    /// whether the trainer has taught the imbue (T2; M5 sets it)
    pub knows_imbue: bool,
    /// the quick keys held last tick, so a press counts once
    last_keys: u32,
}

impl Default for State {
    /// The start: the short sword in hand and nothing else (CW-7).
    fn default() -> State {
        let mut count = [0; N];
        for i in Item::ALL {
            if i.def().start {
                count[i as usize] = 1;
            }
        }
        State {
            count,
            weapon: Some(Item::ShortSword),
            worn: [None; 5],
            quick: [None; 3],
            ready: None,
            coins: 0,
            knows_imbue: false,
            last_keys: 0,
        }
    }
}

impl State {
    pub fn has(&self, i: Item) -> bool {
        self.count[i as usize] > 0
    }

    pub fn count(&self, i: Item) -> u32 {
        self.count[i as usize]
    }

    /// Whether the item is worn or in the sword hand.
    pub fn equipped(&self, i: Item) -> bool {
        self.weapon == Some(i) || self.worn.contains(&Some(i))
    }

    /// The quick key (0, 1 or 2) an item is on.
    pub fn key_of(&self, i: Item) -> Option<usize> {
        self.quick.iter().position(|q| *q == Some(i))
    }

    /// Adds up to `n`, held to the item's stack; returns how many fitted.
    pub fn add(&mut self, i: Item, n: u32) -> u32 {
        let c = &mut self.count[i as usize];
        let room = i.def().stack.saturating_sub(*c);
        let got = n.min(room);
        *c += got;
        got
    }

    /// Removes up to `n`; returns how many there were to remove. An item
    /// that runs out leaves its slot, its hand and its key.
    pub fn remove(&mut self, i: Item, n: u32) -> u32 {
        let c = &mut self.count[i as usize];
        let gone = n.min(*c);
        *c -= gone;
        if *c == 0 {
            if self.weapon == Some(i) {
                self.weapon = None;
            }
            for w in self.worn.iter_mut().filter(|w| **w == Some(i)) {
                *w = None;
            }
            for q in self.quick.iter_mut().filter(|q| **q == Some(i)) {
                *q = None;
            }
        }
        gone
    }

    /// Puts a carried item on quick key `key` (0, 1 or 2). An item already
    /// on another key moves, and the item there before goes to the key it
    /// left (Ocarina of Time swaps them).
    pub fn assign(&mut self, i: Item, key: usize) -> Result<(), Refusal> {
        if key >= 3 {
            return Err(Refusal::NoSuchKey);
        }
        if !self.has(i) {
            return Err(Refusal::NotCarried(i));
        }
        if !i.def().quick {
            return Err(Refusal::NotQuick(i));
        }
        let before = self.quick[key];
        if let Some(from) = self.key_of(i) {
            self.quick[from] = before;
        }
        self.quick[key] = Some(i);
        Ok(())
    }

    /// Equips a carried item: a sword goes in the sword hand, an armour
    /// piece in its slot. Choosing what is already worn takes it off; the
    /// sword hand is never left empty by choosing.
    pub fn equip(&mut self, i: Item) -> Result<(), Refusal> {
        if !self.has(i) {
            return Err(Refusal::NotCarried(i));
        }
        let d = i.def();
        match (d.kind, d.hand, d.slot) {
            (Kind::Weapon, Some(Hand::Sword), _) => self.weapon = Some(i),
            (Kind::Armour, _, Some(s)) => {
                let w = &mut self.worn[s as usize];
                *w = if *w == Some(i) { None } else { Some(i) };
            }
            _ => return Err(Refusal::NotWorn(i)),
        }
        Ok(())
    }

    /// The item the free hand holds ready.
    pub fn ready_item(&self) -> Option<Item> {
        self.ready.and_then(|k| self.quick[k])
    }
}

/// Why the inventory refused; `ui` turns it into the message the reader sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NoSuchKey,
    NotCarried(Item),
    NotQuick(Item),
    NotWorn(Item),
}

/// The item the free hand holds ready, for M1's throw and M5's boomerang.
pub fn ready(w: &World) -> Option<Item> {
    w.inventory.ready_item()
}

/// Gives `n` of an item and shows "Received: …"; returns how many fitted.
pub fn give(w: &mut World, i: Item, n: u32) -> u32 {
    let got = w.inventory.add(i, n);
    if got > 0 {
        crate::ui::received(w, i, got);
    }
    got
}

/// Takes `n` of an item (a throw, a sale, the key into the chest); returns
/// how many there were.
pub fn take(w: &mut World, i: Item, n: u32) -> u32 {
    w.inventory.remove(i, n)
}

/// One tick: a quick key pressed readies its item, or puts it away. M7
/// travel: the flute is not held ready; its key plays it (`travel::request`).
pub fn step(w: &mut World, input: &Input) {
    let mut play = false;
    let inv = &mut w.inventory;
    for (k, bit) in QUICK_KEYS.iter().enumerate() {
        let pressed = input.keys & bit != 0 && inv.last_keys & bit == 0;
        if pressed && inv.quick[k] == Some(Item::Flute) {
            play = true;
        } else if pressed && inv.quick[k].is_some() {
            inv.ready = if inv.ready == Some(k) { None } else { Some(k) };
        }
    }
    if inv.ready_item().is_none() {
        inv.ready = None;
    }
    inv.last_keys = input.keys & (ITEM_1 | ITEM_2 | ITEM_3);
    if play {
        crate::travel::request(w);
    }
}

/// `<id>` or `<id>:<count>`.
fn item_and_count(value: &str) -> Result<(Item, u32), String> {
    let (id, n) = match value.split_once(':') {
        Some((id, n)) => (id, n.trim().parse::<u32>().map_err(|_| bad_count(value))?),
        None => (value, 1),
    };
    let item = Item::from_id(id.trim()).ok_or_else(|| format!("There is no item \"{}\".", id.trim()))?;
    Ok((item, n))
}

fn bad_count(value: &str) -> String {
    format!("A count is a whole number, as in \"rock:3\", not \"{value}\".")
}

/// `inventory.<name>` acts.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    match name {
        "give" => {
            let (i, n) = item_and_count(value)?;
            give(w, i, n);
        }
        "take" => {
            let (i, n) = item_and_count(value)?;
            take(w, i, n);
        }
        "coins" => {
            let n: u32 = value.trim().parse().map_err(|_| bad_count(value))?;
            w.inventory.coins = w.inventory.coins.saturating_add(n);
        }
        _ => return Err(format!("Unknown action \"inventory.{name}\".")),
    }
    Ok(())
}

/// Test-only: every item to its stack, every speedster piece worn, the
/// sword in hand, the throwables on 1, 2 and 3, and a purse of coins.
pub fn give_all(w: &mut World) {
    let inv = &mut w.inventory;
    for i in Item::ALL {
        inv.add(i, i.def().stack);
    }
    for s in Slot::ALL {
        inv.worn[s as usize] = Item::ALL.iter().copied().find(|i| i.def().slot == Some(s));
    }
    inv.weapon = Some(Item::Sword);
    inv.quick = [Some(Item::Boomerang), Some(Item::Rock), Some(Item::Knife)];
    inv.coins = inv.coins.max(148);
    inv.knows_imbue = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;

    fn press(w: &mut World, keys: u32) {
        step(
            w,
            &Input {
                keys,
                ..Input::default()
            },
        );
    }

    #[test]
    fn the_player_starts_with_the_short_sword_in_hand_and_nothing_else() {
        let s = State::default();
        assert_eq!(s.weapon, Some(Item::ShortSword));
        assert_eq!(s.count.iter().sum::<u32>(), 1);
        assert_eq!((s.coins, s.quick, s.worn), (0, [None; 3], [None; 5]));
    }

    #[test]
    fn stacks_hold_their_limit_and_running_out_empties_hand_and_key() {
        let mut s = State::default();
        // rocks stack to 20: 15 + 15 fits 20 and refuses 10
        assert_eq!(s.add(Item::Rock, 15), 15);
        assert_eq!(s.add(Item::Rock, 15), 5);
        assert_eq!(s.count(Item::Rock), 20);
        s.assign(Item::Rock, 0).unwrap();
        assert_eq!(s.remove(Item::Rock, 19), 19);
        assert_eq!(s.quick[0], Some(Item::Rock), "one rock left stays on its key");
        assert_eq!(s.remove(Item::Rock, 5), 1);
        assert_eq!(s.quick[0], None);
        assert_eq!(s.remove(Item::ShortSword, 1), 1);
        assert_eq!(s.weapon, None, "a sold sword leaves the hand");
    }

    #[test]
    fn assigning_a_quick_item_swaps_it_with_the_key_it_leaves() {
        let mut s = State::default();
        s.add(Item::Rock, 3);
        s.add(Item::Knife, 2);
        s.assign(Item::Rock, 0).unwrap();
        s.assign(Item::Knife, 2).unwrap();
        // the knife onto 1, where the rock is: the rock moves to 3
        s.assign(Item::Knife, 0).unwrap();
        assert_eq!(s.quick, [Some(Item::Knife), None, Some(Item::Rock)]);
        assert_eq!(
            s.assign(Item::Bandage, 1),
            Err(Refusal::NotCarried(Item::Bandage))
        );
        s.add(Item::ChestKey, 1);
        assert_eq!(
            s.assign(Item::ChestKey, 1),
            Err(Refusal::NotQuick(Item::ChestKey))
        );
        assert_eq!(s.assign(Item::Rock, 3), Err(Refusal::NoSuchKey));
    }

    #[test]
    fn equipping_puts_a_sword_in_hand_and_a_piece_in_its_slot_or_takes_it_off() {
        let mut s = State::default();
        assert_eq!(s.equip(Item::Sword), Err(Refusal::NotCarried(Item::Sword)));
        s.add(Item::Sword, 1);
        s.equip(Item::Sword).unwrap();
        assert_eq!(s.weapon, Some(Item::Sword));
        s.equip(Item::Sword).unwrap();
        assert_eq!(
            s.weapon,
            Some(Item::Sword),
            "choosing the sword in hand keeps it there"
        );
        s.add(Item::Boots, 1);
        s.equip(Item::Boots).unwrap();
        assert_eq!(s.worn, [None, None, None, None, Some(Item::Boots)]);
        s.equip(Item::Boots).unwrap();
        assert_eq!(s.worn, [None; 5], "choosing a worn piece takes it off");
        s.add(Item::Rock, 1);
        assert_eq!(s.equip(Item::Rock), Err(Refusal::NotWorn(Item::Rock)));
    }

    #[test]
    fn a_quick_key_pressed_readies_its_item_once_and_again_puts_it_away() {
        let mut w = World::new(1);
        w.inventory.add(Item::Rock, 4);
        w.inventory.assign(Item::Rock, 1).unwrap();
        press(&mut w, ITEM_1);
        assert_eq!(ready(&w), None, "nothing on 1");
        press(&mut w, ITEM_2);
        assert_eq!(ready(&w), Some(Item::Rock));
        // held for a second tick: still one press
        press(&mut w, ITEM_2);
        assert_eq!(ready(&w), Some(Item::Rock));
        press(&mut w, 0);
        press(&mut w, ITEM_2);
        assert_eq!(ready(&w), None, "pressed again: put away");
        press(&mut w, 0);
        press(&mut w, ITEM_2);
        take(&mut w, Item::Rock, 4);
        press(&mut w, 0);
        assert_eq!(
            (ready(&w), w.inventory.ready),
            (None, None),
            "the last rock thrown"
        );
    }

    #[test]
    fn give_and_take_go_through_the_act_door_by_id_and_count() {
        let mut w = World::new(1);
        act(&mut w, "give", "knife:3").unwrap();
        act(&mut w, "give", "bandage").unwrap();
        act(&mut w, "take", "knife:1").unwrap();
        act(&mut w, "coins", "25").unwrap();
        assert_eq!(w.inventory.count(Item::Knife), 2);
        assert_eq!(w.inventory.count(Item::Bandage), 1);
        assert_eq!(w.inventory.coins, 25);
        assert_eq!(
            act(&mut w, "give", "lute"),
            Err("There is no item \"lute\".".into())
        );
        assert_eq!(
            act(&mut w, "give", "rock:many"),
            Err("A count is a whole number, as in \"rock:3\", not \"rock:many\".".into())
        );
    }

    #[test]
    fn the_full_test_inventory_wears_all_five_speedster_pieces() {
        let mut w = World::new(1);
        give_all(&mut w);
        assert_eq!(crate::items::speedster_worn(&w), 5);
        assert_eq!(crate::items::held(&w), crate::pose::Held::Sword);
        assert_eq!(w.inventory.count(Item::Rock), 20);
        assert_eq!(w.inventory.count(Item::Knife), 10);
    }
}
