//! The input record: what one tick of one body is given to act on (CW-3).
//!
//! The player's keys and mouse become one `Input` per tick, and a pilot (an
//! enemy's behaviour tree, M1's `pilot.rs`) returns the same record. Nothing
//! else moves a body, so a duel replays from its start state and its list of
//! records alone (CW-4).
//!
//! The page sends one bit set per display frame: `web/input.js` turns each
//! key code into an action name through `data/controls.json`, and
//! `numbers()` gives each name its bit from `ACTIONS` below. Keys use the
//! low 24 bits and mouse buttons the bits from `BUTTON_SHIFT` up; the core
//! splits them into `keys` and `buttons`. Mouse movement and the wheel arrive
//! in hundredths of a CSS pixel and add up between ticks (`Pending`), and the
//! first tick a frame runs takes them all.

use serde::{Deserialize, Serialize};

/// Key actions, one bit each.
pub const FORWARD: u32 = 1 << 0;
pub const BACK: u32 = 1 << 1;
pub const LEFT: u32 = 1 << 2;
pub const RIGHT: u32 = 1 << 3;
/// Shift: run when the sword is sheathed (M1: step in when it is drawn).
pub const RUN: u32 = 1 << 4;
/// Space: a short sidestep (M1).
pub const DODGE: u32 = 1 << 5;
/// Q: the imbue (M6).
pub const IMBUE: u32 = 1 << 6;
/// F: wind and throw the chosen throwable (M1).
pub const THROW: u32 = 1 << 7;
/// X: draw or sheathe the sword (M1).
pub const DRAW: u32 = 1 << 8;
/// E: talk, open, pick up (M5).
pub const USE: u32 = 1 << 9;
/// 1, 2, 3: quick items (M4).
pub const ITEM_1: u32 = 1 << 10;
pub const ITEM_2: u32 = 1 << 11;
pub const ITEM_3: u32 = 1 << 12;
/// Tab and M: the menu ring and the map (M4).
pub const MENU: u32 = 1 << 13;
pub const MAP: u32 = 1 << 14;

/// Buttons sit above the keys in the page's bit set.
pub const BUTTON_SHIFT: u32 = 24;
const KEY_MASK: u32 = (1 << BUTTON_SHIFT) - 1;

/// Mouse buttons, one bit each, as `Input::buttons` holds them.
/// Left: hold for two hands on the hilt (M1).
pub const TWO_HANDS: u32 = 1 << 0;
/// Right: hold to turn the wrist (M1).
pub const WRIST: u32 = 1 << 1;
/// Middle: lock on to the nearest foe (M1).
pub const LOCK_ON: u32 = 1 << 2;

/// Every action's name and its bit in the page's bit set, in the order
/// `numbers()` reports them. `data/controls.json` names these.
pub const ACTIONS: &[(&str, u32)] = &[
    ("forward", FORWARD),
    ("back", BACK),
    ("left", LEFT),
    ("right", RIGHT),
    ("run", RUN),
    ("dodge", DODGE),
    ("imbue", IMBUE),
    ("throw", THROW),
    ("draw", DRAW),
    ("use", USE),
    ("item_1", ITEM_1),
    ("item_2", ITEM_2),
    ("item_3", ITEM_3),
    ("menu", MENU),
    ("map", MAP),
    ("two_hands", TWO_HANDS << BUTTON_SHIFT),
    ("wrist", WRIST << BUTTON_SHIFT),
    ("lock_on", LOCK_ON << BUTTON_SHIFT),
];

/// One tick of input for one body: the player's, or a pilot's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    /// Key actions held this tick (`FORWARD`, `RUN`, ...).
    pub keys: u32,
    /// Mouse buttons held this tick (`TWO_HANDS`, `WRIST`, `LOCK_ON`).
    pub buttons: u32,
    /// Mouse movement this tick, hundredths of a CSS pixel; right and down
    /// are positive.
    pub dx: i32,
    pub dy: i32,
    /// Wheel movement this tick, hundredths of the browser's wheel unit;
    /// toward the reader is positive.
    pub wheel: i32,
}

impl Input {
    /// A tick's record from the page's bit set and the movement it takes.
    pub fn from_page(bits: u32, m: Mouse) -> Input {
        Input {
            keys: bits & KEY_MASK,
            buttons: bits >> BUTTON_SHIFT,
            dx: m.dx,
            dy: m.dy,
            wheel: m.wheel,
        }
    }

    pub fn key(&self, bit: u32) -> bool {
        self.keys & bit != 0
    }

    pub fn button(&self, bit: u32) -> bool {
        self.buttons & bit != 0
    }

    /// The movement keys as a direction in the body's frame: x to the
    /// right, z backward, each −1, 0 or 1. Opposite keys cancel.
    pub fn walk_axes(&self) -> (i32, i32) {
        let x = self.key(RIGHT) as i32 - self.key(LEFT) as i32;
        let z = self.key(BACK) as i32 - self.key(FORWARD) as i32;
        (x, z)
    }
}

/// Mouse movement and wheel, in hundredths of a CSS pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mouse {
    pub dx: i32,
    pub dy: i32,
    pub wheel: i32,
}

/// Movement the page has reported that no tick has taken yet. Display
/// frames come faster or slower than ticks; whatever arrives between two
/// ticks goes to the next one, so nothing is lost or counted twice. The
/// camera is drawn with it already applied (`camera::view`), so turning
/// responds on the frame the mouse moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pending {
    m: Mouse,
}

impl Pending {
    pub fn add(&mut self, dx: i32, dy: i32, wheel: i32) {
        self.m.dx = self.m.dx.saturating_add(dx);
        self.m.dy = self.m.dy.saturating_add(dy);
        self.m.wheel = self.m.wheel.saturating_add(wheel);
    }

    /// What has arrived since the last take, and nothing left behind.
    pub fn take(&mut self) -> Mouse {
        std::mem::take(&mut self.m)
    }

    pub fn peek(&self) -> Mouse {
        self.m
    }

    pub fn clear(&mut self) {
        self.m = Mouse::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_bits_split_into_keys_and_buttons() {
        // forward (bit 0) + run (bit 4) + the right button (bit 25)
        let bits = 1 | (1 << 4) | (1 << 25);
        let i = Input::from_page(bits, Mouse::default());
        assert_eq!(i.keys, 0b1_0001);
        assert_eq!(i.buttons, 0b10);
        assert!(i.key(FORWARD) && i.key(RUN) && i.button(WRIST));
        assert!(!i.button(TWO_HANDS) && !i.key(BACK));
    }

    #[test]
    fn every_action_has_its_own_bit_and_name() {
        for (k, (name, bit)) in ACTIONS.iter().enumerate() {
            assert_eq!(bit.count_ones(), 1, "{name}");
            for (other, obit) in &ACTIONS[k + 1..] {
                assert_ne!(name, other);
                assert_ne!(bit, obit, "{name} and {other}");
            }
        }
    }

    #[test]
    fn opposite_movement_keys_cancel() {
        let i = Input {
            keys: FORWARD | BACK | RIGHT,
            ..Input::default()
        };
        assert_eq!(i.walk_axes(), (1, 0));
        let j = Input {
            keys: FORWARD | LEFT,
            ..Input::default()
        };
        assert_eq!(j.walk_axes(), (-1, -1));
    }

    #[test]
    fn pending_movement_goes_to_one_tick_once() {
        let mut p = Pending::default();
        p.add(150, -20, 0);
        p.add(50, 0, 300);
        assert_eq!(
            p.take(),
            Mouse {
                dx: 200,
                dy: -20,
                wheel: 300
            }
        );
        assert_eq!(p.take(), Mouse::default(), "taken once");
    }

    #[test]
    fn an_input_record_survives_a_trip_through_json() {
        let i = Input {
            keys: FORWARD | RUN,
            buttons: LOCK_ON,
            dx: -340,
            dy: 25,
            wheel: 100,
        };
        let text = serde_json::to_string(&i).unwrap();
        assert_eq!(serde_json::from_str::<Input>(&text).unwrap(), i);
    }
}
