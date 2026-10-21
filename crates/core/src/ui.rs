//! `ui`: the menus and the HUD as the core describes them (M4, task U).
//!
//! The page builds every menu from the view models here and sends every key
//! and click back through `act`; it decides nothing.
//!
//! - **The ring** (Ocarina of Time's four screens): Tab opens it and pauses
//!   the world, M opens it at the map, Q and E turn it, Tab closes it, Esc
//!   closes it to the pause screen. Select Item, Map, Quest and Equipment sit
//!   on the ring in that order, so E turns from Select Item to the map, as
//!   R did. `Menu` holds the ring's turn and a cursor for each screen.
//! - **The map** fills in where the player has walked: `Fog`, a bitset of
//!   32 m cells over the 4 km square, set round the player each tick.
//!   `map_pixels` draws it as parchment, shaded from `World::ground`, water
//!   from `terrain::is_water`, marks from `places::markers`.
//! - **The HUD**: health (`health`, `foe`: M1's doors), the imbue's ring
//!   (`imbue`: M6's door), the footing bar while speedster pieces are worn,
//!   the quick items on 1, 2 and 3, coins, and the "Received" toast.
//! - **Dialogue** (`say`, for M5) and **the shop** (`open_shop`, for M5)
//!   pause the world and take their own screen.
//! - **Settings**: look speed, mouse sensitivity, invert Y and volume, held
//!   for the session only (CW-12).
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::api::{fill, Action};
use crate::fx::{self, Fx, ONE};
use crate::game::{Game, Screen};
use crate::input::{MAP, MENU};
use crate::inventory::{self, Refusal};
use crate::items::{self, catalogue, Hand, Item, Slot};
use crate::page::Strings;
use crate::world::World;

/// The walkable square's side (PLAN.md: 4 km, origin at the south-west
/// corner; world z runs south, so the map's top row is z = 0).
pub const REGION_M: i32 = 4000;
/// The fog's cell, metres, and how many cells a side: 125 × 32 m = 4 km.
pub const CELL_M: i32 = 32;
pub const CELLS: i32 = REGION_M / CELL_M;
/// Cells round the player that walking reveals: a disc of this radius.
pub const REVEAL_CELLS: i32 = 4;
/// The map image's side in pixels: two to a cell, 16 m each.
pub const MAP_PX: i32 = 250;
const PX_M: i32 = REGION_M / MAP_PX;
/// How long "Received: …" shows, ticks (2.5 s).
pub const TOAST_TICKS: u32 = 150;
/// How fast dialogue text appears, characters a second.
pub const CHARS_PER_SECOND: u32 = 40;
const TICKS: u32 = crate::ticks::TICKS_PER_SECOND as u32;

/// The ring's four screens, in ring order: E turns to the next.
pub const FACES: [&str; 4] = ["select", "map", "quest", "equipment"];
pub const SELECT: usize = 0;
pub const MAP_FACE: usize = 1;
pub const QUEST: usize = 2;
pub const EQUIPMENT: usize = 3;

// ---------------------------------------------------------------- the fog

/// Where the player has walked: one bit a 32 m cell, row by row from the
/// north-west corner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fog {
    bits: Vec<u64>,
    /// counts up each time a cell is revealed, so the page redraws the map
    /// image only when it changed
    pub version: u32,
    last: Option<(i32, i32)>,
}

impl Default for Fog {
    fn default() -> Fog {
        Fog {
            bits: vec![0; ((CELLS * CELLS + 63) / 64) as usize],
            version: 0,
            last: None,
        }
    }
}

impl Fog {
    /// The cell holding world (x, z).
    pub fn cell_of(x: Fx, z: Fx) -> (i32, i32) {
        (fx::floor(x).div_euclid(CELL_M), fx::floor(z).div_euclid(CELL_M))
    }

    fn index(cx: i32, cz: i32) -> Option<usize> {
        ((0..CELLS).contains(&cx) && (0..CELLS).contains(&cz)).then(|| (cz * CELLS + cx) as usize)
    }

    pub fn seen(&self, cx: i32, cz: i32) -> bool {
        Fog::index(cx, cz).is_some_and(|k| self.bits[k / 64] >> (k % 64) & 1 == 1)
    }

    /// Marks a cell; whether it was new.
    fn mark(&mut self, cx: i32, cz: i32) -> bool {
        let Some(k) = Fog::index(cx, cz) else {
            return false;
        };
        let was = self.bits[k / 64];
        self.bits[k / 64] |= 1 << (k % 64);
        was != self.bits[k / 64]
    }

    /// Reveals the disc of cells round world (x, z). Nothing to do while the
    /// player stays in one cell.
    pub fn reveal(&mut self, x: Fx, z: Fx) {
        let c = Fog::cell_of(x, z);
        if self.last == Some(c) {
            return;
        }
        self.last = Some(c);
        let r = REVEAL_CELLS;
        let mut new = false;
        for dz in -r..=r {
            for dx in -r..=r {
                if dx * dx + dz * dz <= r * r {
                    new |= self.mark(c.0 + dx, c.1 + dz);
                }
            }
        }
        if new {
            self.version += 1;
        }
    }

    pub fn count(&self) -> u32 {
        self.bits.iter().map(|b| b.count_ones()).sum()
    }
}

// ---------------------------------------------------------- world state

/// "Received: …" over the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toast {
    pub item: Item,
    pub count: u32,
    pub ticks: u32,
}

/// Lines someone says, by string key, and how far the text has appeared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialogue {
    /// who speaks, a kind: `who_<kind>` names them, `portrait_<kind>.svg`
    /// draws them
    pub who: String,
    pub lines: Vec<String>,
    pub line: usize,
    /// ticks the current line has been appearing
    pub ticks: u32,
}

/// The reader's settings, for this session only (CW-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub invert_y: bool,
    /// the sword hand's speed under the mouse, percent (M1 reads it)
    pub hand_pct: i32,
    pub volume_pct: i32,
    /// M7 (T10): the swing cue, the cut the mouse would pick (`swing::cue`)
    pub cue: bool,
    /// M7 (P4): blood is shown (on by default; `wounds.rs`)
    pub blood: bool,
}

/// Test-only HUD values (`test.hud`), so a screenshot can show the bars
/// before M1 and M6 send theirs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Demo {
    pub health: Gauge,
    pub foe: Option<(String, Gauge)>,
    /// ticks since the imbue was cast
    pub since_cast: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub fog: Fog,
    pub toast: Option<Toast>,
    pub dialogue: Option<Dialogue>,
    pub shop_open: bool,
    /// the merchant's stock of one-of-a-kind items, by `Item as usize`
    /// (the hood; anything sold to the merchant can be bought back)
    pub stock: [u32; Item::ALL.len()],
    pub settings: Settings,
    pub demo: Option<Demo>,
}

impl Default for State {
    fn default() -> State {
        let mut stock = [0; Item::ALL.len()];
        for i in Item::ALL {
            if i.def().sold && i.def().stack == 1 {
                stock[i as usize] = 1;
            }
        }
        State {
            fog: Fog::default(),
            toast: None,
            dialogue: None,
            shop_open: false,
            stock,
            settings: Settings {
                invert_y: false,
                hand_pct: 100,
                volume_pct: 80,
                cue: true,
                blood: true,
            },
            demo: None,
        }
    }
}

/// One tick, last in `World::step`: the fog round the player, the toast.
pub fn step(w: &mut World) {
    let p = w.player.pos;
    w.ui.fog.reveal(p.x, p.z);
    if let Some(t) = &mut w.ui.toast {
        t.ticks = t.ticks.saturating_sub(1);
        if t.ticks == 0 {
            w.ui.toast = None;
        }
    }
}

/// Shows "Received: …" (`inventory::give` calls it).
pub fn received(w: &mut World, item: Item, count: u32) {
    w.ui.toast = Some(Toast {
        item,
        count,
        ticks: TOAST_TICKS,
    });
}

/// For M5: someone speaks these lines (string keys). The world pauses on
/// the dialogue screen until the reader has read them.
pub fn say(w: &mut World, who: &str, lines: &[&str]) {
    if lines.is_empty() {
        return;
    }
    w.ui.dialogue = Some(Dialogue {
        who: who.into(),
        lines: lines.iter().map(|l| l.to_string()).collect(),
        line: 0,
        ticks: 0,
    });
}

/// For M5: the merchant opens the shop. The world pauses on the shop screen.
pub fn open_shop(w: &mut World) {
    w.ui.shop_open = true;
}

// ------------------------------------------------- doors for M1 and M6

/// A bar's value and its most.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gauge {
    pub now: i32,
    pub most: i32,
}

/// The player's health. Placeholder, full; M1 replaces the body with its
/// `health.rs` value. The signature stays.
pub fn health(w: &World) -> Gauge {
    // M1: the player's own health (`health.rs`)
    match &w.ui.demo {
        Some(d) => d.health,
        None => Gauge {
            now: w.me.health.hp.max(0),
            most: w.me.health.most,
        },
    }
}

/// The engaged foe (a kind, `who_<kind>`) and its health. Placeholder,
/// none; M1 replaces the body. The signature stays.
pub fn foe(w: &World) -> Option<(String, Gauge)> {
    // M1: the foe the player holds on, while the sword is drawn
    if let Some(d) = &w.ui.demo {
        return d.foe.clone();
    }
    let k =
        w.me.engaged
            .map(|k| k as usize)
            .filter(|&k| k > 0 && k < crate::player::count(w))?;
    let f = crate::player::fighter(w, k);
    Some((
        crate::pilot::kind_name(f.kind).to_string(),
        Gauge {
            now: f.health.hp.max(0),
            most: f.health.most,
        },
    ))
}

/// Where the imbue stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Imbue {
    Ready,
    /// imbued, this many ticks left
    On(u32),
    /// recovering, this many ticks until ready (counted from the cast, CW-9)
    Cooling(u32),
}

/// The imbue's state, none until the trainer teaches it (T2).
/// M6: `imbue.rs`'s state (`imbue::status`); the test HUD's values
/// (`test.hud`) stand in for it while they are set. The signature stays.
pub fn imbue(w: &World) -> Option<Imbue> {
    let c = &catalogue().imbue;
    match &w.ui.demo {
        Some(d) if w.inventory.knows_imbue => Some(if d.since_cast < c.lasts_ticks {
            Imbue::On(c.lasts_ticks - d.since_cast)
        } else if d.since_cast < c.cooldown_ticks {
            Imbue::Cooling(c.cooldown_ticks - d.since_cast)
        } else {
            Imbue::Ready
        }),
        _ => crate::imbue::status(w),
    }
}

/// The quest screen's line. Placeholder: the opening's task; M5 replaces
/// the body as the story moves on. The signature stays.
pub fn task(w: &World) -> &'static str {
    // M5: the opening, the trainer, the camp, the speedster's pieces
    crate::story::task(w)
}

// ----------------------------------------------------------- game state

/// The ring and its cursors, kept while the game runs (`Game::menu`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    /// quarter turns of the ring since the start, E adding one: the page
    /// rotates the ring by it, so a turn eases the short way
    pub turn: i32,
    pub cursor: [usize; 4],
    /// a line under the chosen item, until the cursor moves
    pub note: Option<String>,
    /// where Back on the settings screen returns to
    pub settings_from: Screen,
    /// the keys held last tick of play, so Tab and M open once a press
    pub last_bits: u32,
    hud: Option<HudView>,
}

impl Default for Menu {
    fn default() -> Menu {
        Menu {
            turn: 0,
            cursor: [0; 4],
            note: None,
            settings_from: Screen::Title,
            last_bits: 0,
            hud: None,
        }
    }
}

impl Menu {
    pub fn face(&self) -> usize {
        self.turn.rem_euclid(4) as usize
    }
}

/// Tab or M pressed this tick of play: the screen the ring opens at.
pub fn menu_pressed(m: &mut Menu, bits: u32) -> Option<usize> {
    let pressed = bits & !m.last_bits;
    m.last_bits = bits;
    if pressed & MAP != 0 {
        Some(MAP_FACE)
    } else if pressed & MENU != 0 {
        Some(m.face())
    } else {
        None
    }
}

/// Opens the ring at `face`, turned the short way from where it was left.
pub fn open(g: &mut Game, face: usize) {
    let from = g.menu.face() as i32;
    let d = (face as i32 - from).rem_euclid(4);
    g.menu.turn += if d == 3 { -1 } else { d };
    g.menu.note = None;
    g.show(Screen::Menu);
}

/// A dialogue or the shop that the last tick opened.
pub fn wanted(w: &World) -> Option<Screen> {
    if w.ui.dialogue.is_some() {
        Some(Screen::Dialogue)
    } else if w.ui.shop_open {
        Some(Screen::Shop)
    } else if crate::travel::wanted(w) {
        // M7 travel: the flute's song screen, or the traveller's list
        Some(Screen::Song)
    } else {
        None
    }
}

/// Esc on one of this module's screens.
pub fn escape(g: &mut Game) {
    match g.screen {
        Screen::Menu => g.show(Screen::Paused),
        Screen::Shop => {
            g.world.ui.shop_open = false;
            g.show(Screen::Paused);
        }
        Screen::Settings => {
            let back = g.menu.settings_from;
            g.show(back);
        }
        // M7 travel: Esc puts the flute away, or leaves the traveller
        Screen::Song => crate::travel::cancel(g),
        _ => {}
    }
}

/// Dialogue text appears `ticks` further; whether the shown text changed.
pub fn reveal(w: &mut World, ticks: u32) -> bool {
    let Some(d) = &mut w.ui.dialogue else {
        return false;
    };
    let before = shown_chars(d.ticks);
    d.ticks = d.ticks.saturating_add(ticks);
    shown_chars(d.ticks) != before
}

fn shown_chars(ticks: u32) -> usize {
    (ticks as u64 * CHARS_PER_SECOND as u64 / TICKS as u64) as usize
}

/// Space on a dialogue: the rest of the line if it is still appearing, then
/// the next line, then back to play.
fn next_line(g: &mut Game) {
    let s = &g.strings;
    let Some(d) = &mut g.world.ui.dialogue else {
        return;
    };
    let len = s.get(&d.lines[d.line]).chars().count();
    if shown_chars(d.ticks) < len {
        d.ticks = (len as u64 * TICKS as u64).div_ceil(CHARS_PER_SECOND as u64) as u32;
    } else if d.line + 1 < d.lines.len() {
        d.line += 1;
        d.ticks = 0;
    } else {
        g.world.ui.dialogue = None;
        g.show(Screen::Play);
    }
    g.ui_version += 1;
}

// ------------------------------------------------------------- numbers

/// The numbers the glossary and the item texts name, as text.
pub fn values() -> Vec<(&'static str, String)> {
    let c = catalogue();
    let set = &c.sets.speedster;
    let secs = |t: u32| (t / TICKS).to_string();
    vec![
        ("short_sword.blade", Item::ShortSword.def().blade_cm.to_string()),
        ("sword.blade", Item::Sword.def().blade_cm.to_string()),
        ("boomerang.reach", Item::Boomerang.def().reach_m.to_string()),
        ("bandage.heals", Item::Bandage.def().heals.to_string()),
        ("speedster.speed_pct", set.speed_pct_per_piece.to_string()),
        ("speedster.grip_pct", set.grip_pct_per_piece.to_string()),
        ("speedster.pieces", items::SPEEDSTER_PIECES.to_string()),
        ("imbue.seconds", secs(c.imbue.lasts_ticks)),
        ("imbue.cooldown", secs(c.imbue.cooldown_ticks)),
        ("merchant.buys_pct", c.merchant.buys_at_pct.to_string()),
        (
            "toast.seconds",
            fx::decimal(fx::ratio(TOAST_TICKS as i32, TICKS as i32), 1),
        ),
    ]
}

/// A share of `ONE` as a whole percentage, rounded.
fn pct_of(x: Fx) -> i32 {
    ((x as i64 * 100 + ONE as i64 / 2) / ONE as i64) as i32
}

fn pct(now: i32, most: i32) -> i32 {
    if most <= 0 {
        return 0;
    }
    (now.clamp(0, most) as i64 * 100 / most as i64) as i32
}

/// Whole seconds left, rounded up, so "0 s" never shows before the end.
fn seconds_left(ticks: u32) -> String {
    ticks.div_ceil(TICKS).to_string()
}

// ---------------------------------------------------------- view models

/// An item as a slot, a button or a row shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemView {
    pub id: String,
    pub name: String,
    pub icon: String,
    /// how many, for items that stack; empty otherwise
    #[serde(skip_serializing_if = "String::is_empty")]
    pub count: String,
}

fn item_view(s: &Strings, inv: &inventory::State, i: Item) -> ItemView {
    ItemView {
        id: i.id().into(),
        name: s.get(&format!("item_{}", i.id())),
        icon: format!("data/icons/{}.svg", i.id()),
        count: if i.def().stack > 1 {
            inv.count(i).to_string()
        } else {
            String::new()
        },
    }
}

/// A bar with its words: never colour alone (BRIEF.md, Accessibility).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bar {
    pub id: String,
    pub label: String,
    pub text: String,
    /// filled share, 0 to 100
    pub pct: i32,
}

/// The imbue's cooldown ring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ring {
    pub label: String,
    pub text: String,
    /// "ready", "on" or "cooling"
    pub state: String,
    /// filled share, 0 to 100: what is left while on, what has recovered
    /// while cooling
    pub pct: i32,
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Quick {
    pub key: String,
    pub item: Option<ItemView>,
    /// the free hand holds this one
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Count {
    pub label: String,
    pub text: String,
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToastView {
    pub text: String,
    pub icon: String,
    pub shown: bool,
}

/// What shows over the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HudView {
    pub health: Bar,
    pub foe: Option<Bar>,
    pub imbue: Option<Ring>,
    /// speed and footing, while speedster pieces are worn
    pub footing: Vec<Bar>,
    pub quick: Vec<Quick>,
    pub coins: Count,
    pub toast: ToastView,
}

fn quick_view(s: &Strings, inv: &inventory::State) -> Vec<Quick> {
    (0..3)
        .map(|k| Quick {
            key: s.get(&format!("key_{}", k + 1)),
            item: inv.quick[k].map(|i| item_view(s, inv, i)),
            ready: inv.ready == Some(k),
        })
        .collect()
}

pub(crate) fn coins_view(s: &Strings, w: &World) -> Count {
    Count {
        label: s.get("coins"),
        text: w.inventory.coins.to_string(),
        icon: "data/icons/coin.svg".into(),
    }
}

/// Speed and footing for `n` speedster pieces (CW-8), as bars: speed fills
/// toward the set's whole (double at five pieces), footing is the grip left.
fn set_bars(s: &Strings, n: u8) -> Vec<Bar> {
    let speed = pct_of(items::speedster_speed(n));
    let most = pct_of(items::speedster_speed(items::SPEEDSTER_PIECES));
    let grip = pct_of(items::speedster_grip(n));
    let p = |v: i32| fill(&s.get("percent"), &[("n", v.to_string())]);
    vec![
        Bar {
            id: "speed".into(),
            label: s.get("speed"),
            text: p(speed),
            pct: pct(speed, most),
        },
        Bar {
            id: "footing".into(),
            label: s.get("footing"),
            text: p(grip),
            pct: grip,
        },
    ]
}

pub fn hud(g: &Game) -> HudView {
    let (s, w) = (&g.strings, &g.world);
    let inv = &w.inventory;
    let gauge = |id: &str, label: String, v: Gauge| Bar {
        id: id.into(),
        label,
        text: fill(
            &s.get("health_of"),
            &[("now", v.now.max(0).to_string()), ("most", v.most.to_string())],
        ),
        pct: pct(v.now, v.most),
    };
    let c = &catalogue().imbue;
    let imbue = imbue(w).map(|state| {
        let (state, text, p) = match state {
            Imbue::Ready => ("ready", s.get("imbue_ready"), 100),
            Imbue::On(t) => (
                "on",
                fill(&s.get("seconds"), &[("n", seconds_left(t))]),
                pct(t as i32, c.lasts_ticks as i32),
            ),
            Imbue::Cooling(t) => (
                "cooling",
                fill(&s.get("seconds"), &[("n", seconds_left(t))]),
                pct(c.cooldown_ticks as i32 - t as i32, c.cooldown_ticks as i32),
            ),
        };
        Ring {
            label: s.get("imbue"),
            text,
            state: state.into(),
            pct: p,
            icon: "data/icons/imbue.svg".into(),
        }
    });
    let n = items::speedster_worn(w);
    let toast = match w.ui.toast {
        Some(t) => {
            let mut name = s.get(&format!("item_{}", t.item.id()));
            if t.count > 1 {
                name = fill(
                    &s.get("item_times"),
                    &[("item", name), ("n", t.count.to_string())],
                );
            }
            ToastView {
                text: fill(&s.get("received"), &[("item", name)]),
                icon: format!("data/icons/{}.svg", t.item.id()),
                shown: true,
            }
        }
        None => ToastView {
            text: String::new(),
            icon: String::new(),
            shown: false,
        },
    };
    HudView {
        health: gauge("health", s.get("health"), health(w)),
        foe: foe(w).map(|(who, v)| gauge("foe", s.get(&format!("who_{who}")), v)),
        imbue,
        footing: if n > 0 { set_bars(s, n) } else { Vec::new() },
        quick: quick_view(s, inv),
        coins: coins_view(s, w),
        toast,
    }
}

/// Whether the HUD differs from the one the page last read, so the page is
/// told to read the view model again (`Game::ui_version`).
pub fn hud_changed(g: &mut Game) -> bool {
    let now = hud(g);
    if g.menu.hud.as_ref() == Some(&now) {
        return false;
    }
    g.menu.hud = Some(now);
    true
}

/// One slot on a ring screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SlotView {
    pub index: usize,
    /// where the page puts it: "grid", "hand", or an armour slot's id
    pub place: String,
    /// an empty slot's name ("Head")
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    pub item: Option<ItemView>,
    pub chosen: bool,
    /// worn, or in the sword hand
    pub worn: bool,
    /// the quick key it is on, or empty
    #[serde(skip_serializing_if = "String::is_empty")]
    pub quick: String,
}

/// The chosen slot, named and described under the screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Info {
    pub name: String,
    pub about: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetView {
    pub heading: String,
    pub icon: String,
    pub count: String,
    pub bars: Vec<Bar>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mark {
    pub kind: String,
    pub label: String,
    pub icon: String,
    /// from the map's left and top edges, as CSS percentages
    pub left: String,
    pub top: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapView {
    /// the image's version (`Fog::version`) and side; the page reads the
    /// pixels (`map_pixels`) when the version changes
    pub version: u32,
    pub size: i32,
    pub marks: Vec<Mark>,
    pub you: Mark,
    /// the player's heading, degrees clockwise from north
    pub heading: String,
    pub legend: String,
    pub walked: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuestView {
    pub imbue: Info,
    pub imbue_icon: String,
    pub knows_imbue: bool,
    pub coins: Count,
    pub task_heading: String,
    pub task: String,
    /// M7 travel: the songs learned, with their notes
    pub song: Option<crate::travel::SongCard>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub songs_heading: String,
}

/// One screen of the ring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Face {
    pub id: String,
    pub index: usize,
    pub heading: String,
    pub front: bool,
    pub slots: Vec<SlotView>,
    pub info: Option<Info>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hand_heading: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub figure: String,
    pub set: Option<SetView>,
    pub map: Option<MapView>,
    pub quest: Option<QuestView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyHint {
    pub key: String,
    pub text: String,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MenuView {
    pub turn: i32,
    pub face: usize,
    pub faces: Vec<Face>,
    pub left: KeyHint,
    pub right: KeyHint,
    pub close: KeyHint,
    pub quick: Vec<Quick>,
    pub assign: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueView {
    pub who: String,
    pub name: String,
    pub portrait: String,
    /// the text that has appeared, and the rest, which the page lays out
    /// unseen so the box does not reflow as words appear
    pub said: String,
    pub unsaid: String,
    pub done: bool,
    pub key: String,
    pub next: Action,
    /// M7 travel: the line that teaches the travel song shows its notes
    pub song: Option<crate::travel::SongCard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShopRow {
    pub item: ItemView,
    pub price: String,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShopView {
    pub heading: String,
    pub coins: Count,
    pub buy_heading: String,
    pub sell_heading: String,
    pub buy: Vec<ShopRow>,
    pub sell: Vec<ShopRow>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub empty: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub note: String,
    pub leave: Action,
}

/// A setting: a range the reader drags, or a switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Control {
    pub id: String,
    pub label: String,
    /// "range" or "switch"
    pub kind: String,
    pub value: i32,
    pub min: i32,
    pub max: i32,
    pub step: i32,
    /// the value in words ("100%", "On")
    pub text: String,
    /// the act it sends; a range sends its value, a switch sends `next`
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub next: String,
    pub on: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingsView {
    pub heading: String,
    pub controls: Vec<Control>,
    pub note: String,
    pub back: Action,
}

/// Everything this module adds to the view model, flattened into `api::Ui`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Views {
    /// which keys the page sends as `<keymap>.key` acts (`data/keys.json`),
    /// empty when keys go to play or to the page's own focus
    pub keymap: String,
    /// whether the HUD shows
    pub hud_shown: bool,
    pub play: HudView,
    pub menu: Option<MenuView>,
    pub dialogue: Option<DialogueView>,
    pub shop: Option<ShopView>,
    pub settings: Option<SettingsView>,
    /// M7 travel: the song screen, and the last sound travel asked for
    pub song: Option<crate::travel::SongView>,
    pub travel_sound: crate::travel::SoundView,
}

fn action(s: &Strings, name: &str, value: &str, label: &str, engage: bool) -> Action {
    Action {
        name: name.into(),
        value: value.into(),
        label: if label.is_empty() {
            String::new()
        } else {
            s.get(label)
        },
        enabled: true,
        engage,
    }
}

pub fn views(g: &Game) -> Views {
    let keymap = match g.screen {
        Screen::Menu => "menu",
        Screen::Dialogue => "dialogue",
        Screen::Song => "travel", // M7 travel: `travel.key`
        _ => "",
    };
    Views {
        keymap: keymap.into(),
        hud_shown: matches!(g.screen, Screen::Play | Screen::Dialogue),
        play: hud(g),
        menu: (g.screen == Screen::Menu).then(|| menu_view(g)),
        dialogue: if g.screen == Screen::Dialogue {
            dialogue_view(g)
        } else {
            None
        },
        shop: (g.screen == Screen::Shop).then(|| shop_view(g)),
        settings: (g.screen == Screen::Settings).then(|| settings_view(g)),
        song: crate::travel::view(g),
        travel_sound: crate::travel::sound_view(&g.world),
    }
}

/// The settings button and the menu's keys, added to the title and pause
/// screens as panels of their own, beside the ones `api::ui` builds.
pub fn panels(g: &Game, out: &mut Vec<crate::api::Panel>) {
    let id = match g.screen {
        Screen::Title => "title-more",
        Screen::Paused => "paused-more",
        _ => return,
    };
    let s = &g.strings;
    let row = |k: &str, d: &str| crate::api::Row {
        key: s.get(k),
        does: s.get(d),
    };
    out.push(crate::api::Panel {
        id: id.into(),
        heading: String::new(),
        text: String::new(),
        rows_heading: String::new(),
        rows: vec![
            row("key_menu", "control_menu"),
            row("key_map", "control_map"),
            row("key_quick", "control_quick"),
        ],
        actions: vec![action(s, "settings.open", "", "settings", false)],
    });
}

// ------------------------------------------------------------- the ring

/// Each slot's place on its screen, for moving the cursor with the arrows:
/// (column, row).
fn layout(face: usize) -> Vec<(i32, i32)> {
    let grid = &catalogue().grid;
    match face {
        SELECT => (0..grid.across * grid.down)
            .map(|k| ((k % grid.across) as i32, (k / grid.across) as i32))
            .collect(),
        // the two swords, then head, body, hands, legs, feet round the figure
        EQUIPMENT => vec![(0, 1), (0, 2), (4, 0), (2, 1), (4, 2), (2, 3), (4, 4)],
        _ => Vec::new(),
    }
}

/// The nearest slot in a direction: along it first, across it counting
/// double, the lower index on a tie.
fn moved(face: usize, from: usize, (dx, dy): (i32, i32)) -> usize {
    let at = layout(face);
    let Some(&(x, y)) = at.get(from) else {
        return from;
    };
    let mut best: Option<(i32, usize)> = None;
    for (k, &(cx, cy)) in at.iter().enumerate() {
        let (along, across) = if dx != 0 {
            ((cx - x) * dx, (cy - y).abs())
        } else {
            ((cy - y) * dy, (cx - x).abs())
        };
        if along <= 0 {
            continue;
        }
        let score = along + 2 * across;
        if best.is_none_or(|(b, _)| score < b) {
            best = Some((score, k));
        }
    }
    best.map(|(_, k)| k).unwrap_or(from)
}

/// The item in Select Item's cell `k`, if carried.
fn select_item(inv: &inventory::State, k: usize) -> Option<Item> {
    Item::ALL
        .iter()
        .copied()
        .find(|i| i.def().grid == Some(k) && inv.has(*i))
}

/// Equipment's slots: the two swords, then the five armour slots.
fn equipment_slot(k: usize) -> (Option<Item>, Option<Slot>) {
    match k {
        0 => (Some(Item::ShortSword), None),
        1 => (Some(Item::Sword), None),
        _ => (None, Slot::ALL.get(k - 2).copied()),
    }
}

/// The piece for an armour slot that the player carries (one per slot in
/// the MVP).
fn piece_for(inv: &inventory::State, slot: Slot) -> Option<Item> {
    Item::ALL
        .iter()
        .copied()
        .find(|i| i.def().slot == Some(slot) && inv.has(*i))
}

fn equipment_item(inv: &inventory::State, k: usize) -> Option<Item> {
    match equipment_slot(k) {
        (Some(i), _) => inv.has(i).then_some(i),
        (None, Some(slot)) => piece_for(inv, slot),
        _ => None,
    }
}

fn about(s: &Strings, i: Item) -> String {
    let mut v = values();
    v.extend(crate::game::base_values());
    fill(&s.get(&format!("item_{}_about", i.id())), &v)
}

fn select_face(g: &Game, front: bool) -> Face {
    let (s, inv) = (&g.strings, &g.world.inventory);
    let cursor = g.menu.cursor[SELECT];
    let grid = &catalogue().grid;
    let slots = (0..grid.across * grid.down)
        .map(|k| {
            let item = select_item(inv, k);
            SlotView {
                index: k,
                place: "grid".into(),
                label: String::new(),
                item: item.map(|i| item_view(s, inv, i)),
                chosen: k == cursor,
                worn: false,
                quick: item
                    .and_then(|i| inv.key_of(i))
                    .map(|q| s.get(&format!("key_{}", q + 1)))
                    .unwrap_or_default(),
            }
        })
        .collect();
    let info = select_item(inv, cursor).map(|i| {
        let mut notes = Vec::new();
        if i.def().stack > 1 {
            notes.push(fill(
                &s.get("carry_of"),
                &[
                    ("n", inv.count(i).to_string()),
                    ("most", i.def().stack.to_string()),
                ],
            ));
        }
        notes.push(match (i.def().quick, inv.key_of(i)) {
            (true, Some(q)) => fill(&s.get("on_key"), &[("key", s.get(&format!("key_{}", q + 1)))]),
            (true, None) => s.get("assign_hint"),
            (false, _) => s.get("no_key"),
        });
        if let Some(n) = &g.menu.note {
            notes.push(n.clone());
        }
        Info {
            name: s.get(&format!("item_{}", i.id())),
            about: about(s, i),
            notes,
        }
    });
    Face {
        id: "select".into(),
        index: SELECT,
        heading: s.get("select_item"),
        front,
        slots,
        info,
        hand_heading: String::new(),
        figure: String::new(),
        set: None,
        map: None,
        quest: None,
    }
}

fn equipment_face(g: &Game, front: bool) -> Face {
    let (s, inv) = (&g.strings, &g.world.inventory);
    let cursor = g.menu.cursor[EQUIPMENT];
    let slots = (0..7)
        .map(|k| {
            let item = equipment_item(inv, k);
            let (sword, slot) = equipment_slot(k);
            SlotView {
                index: k,
                place: slot.map(|s| s.id()).unwrap_or("hand").into(),
                label: match (sword, slot) {
                    (_, Some(sl)) => s.get(&format!("slot_{}", sl.id())),
                    _ => String::new(),
                },
                item: item.map(|i| item_view(s, inv, i)),
                chosen: k == cursor,
                worn: item.is_some_and(|i| inv.equipped(i)),
                quick: String::new(),
            }
        })
        .collect();
    let info = match (equipment_item(inv, cursor), equipment_slot(cursor)) {
        (Some(i), _) => {
            let mut notes = vec![match (i.def().hand, inv.equipped(i)) {
                (Some(Hand::Sword), true) => s.get("in_hand"),
                (Some(Hand::Sword), false) => s.get("hold_hint"),
                (_, true) => s.get("worn_hint"),
                (_, false) => s.get("wear_hint"),
            }];
            if let Some(n) = &g.menu.note {
                notes.push(n.clone());
            }
            Some(Info {
                name: s.get(&format!("item_{}", i.id())),
                about: about(s, i),
                notes,
            })
        }
        (None, (_, Some(slot))) => Some(Info {
            name: s.get(&format!("slot_{}", slot.id())),
            about: s.get("not_found"),
            notes: Vec::new(),
        }),
        (None, _) => None,
    };
    let n = items::speedster_worn(&g.world);
    Face {
        id: "equipment".into(),
        index: EQUIPMENT,
        heading: s.get("equipment"),
        front,
        slots,
        info,
        hand_heading: s.get("sword_hand"),
        figure: "data/icons/figure.svg".into(),
        set: Some(SetView {
            heading: s.get("speedster_armour"),
            icon: "data/icons/boots.svg".into(),
            count: fill(
                &s.get("pieces_worn"),
                &[("n", n.to_string()), ("of", items::SPEEDSTER_PIECES.to_string())],
            ),
            bars: set_bars(s, n),
        }),
        map: None,
        quest: None,
    }
}

/// A world position as a share of the map, "50.00%".
pub(crate) fn map_pct(v: Fx) -> String {
    let h = (v as i64 * 10_000 / (REGION_M as i64 * ONE as i64)).clamp(0, 10_000);
    format!("{}.{:02}%", h / 100, h % 100)
}

fn map_face(g: &Game, front: bool) -> Face {
    let (s, w) = (&g.strings, &g.world);
    let menus = menus();
    let marks = crate::places::markers(w)
        .into_iter()
        .filter(|(_, at)| {
            let (cx, cz) = Fog::cell_of(at.x, at.z);
            w.ui.fog.seen(cx, cz)
        })
        .map(|(kind, at)| Mark {
            kind: kind.into(),
            label: s.get(&format!("place_{kind}")),
            icon: if menus.place_icons.iter().any(|k| k == kind) {
                format!("data/icons/place_{kind}.svg")
            } else {
                "data/icons/place.svg".into()
            },
            left: map_pct(at.x),
            top: map_pct(at.z),
        })
        .collect();
    let p = w.player.pos;
    // a heading turns left as it grows (camera::Aim); the map turns clockwise
    let turn = fx::TURN as i64;
    let tenths = (-(w.player.facing as i64) * 3600 + turn / 2)
        .div_euclid(turn)
        .rem_euclid(3600);
    let walked = w.ui.fog.count() as i64 * 100 / (CELLS * CELLS) as i64;
    Face {
        id: "map".into(),
        index: MAP_FACE,
        heading: s.get("map"),
        front,
        slots: Vec::new(),
        info: None,
        hand_heading: String::new(),
        figure: String::new(),
        set: None,
        map: Some(MapView {
            version: w.ui.fog.version,
            size: MAP_PX,
            marks,
            you: Mark {
                kind: "you".into(),
                label: s.get("you"),
                icon: String::new(),
                left: map_pct(p.x),
                top: map_pct(p.z),
            },
            heading: format!("{}.{}", tenths / 10, tenths % 10),
            legend: s.get("map_legend"),
            walked: fill(&s.get("walked"), &[("n", walked.to_string())]),
        }),
        quest: None,
    }
}

fn quest_face(g: &Game, front: bool) -> Face {
    let (s, w) = (&g.strings, &g.world);
    let mut v = values();
    v.extend(crate::game::base_values());
    let knows = w.inventory.knows_imbue;
    Face {
        id: "quest".into(),
        index: QUEST,
        heading: s.get("quest"),
        front,
        slots: Vec::new(),
        info: None,
        hand_heading: String::new(),
        figure: String::new(),
        set: None,
        map: None,
        quest: Some(QuestView {
            imbue: Info {
                name: s.get("imbue"),
                about: fill(&s.get(if knows { "imbue_known" } else { "imbue_unknown" }), &v),
                notes: Vec::new(),
            },
            imbue_icon: "data/icons/imbue.svg".into(),
            knows_imbue: knows,
            coins: coins_view(s, w),
            task_heading: s.get("task_heading"),
            task: s.get(task(w)),
            song: crate::travel::known(w, s),
            songs_heading: s.get("songs_heading"),
        }),
    }
}

fn menu_view(g: &Game) -> MenuView {
    let s = &g.strings;
    let front = g.menu.face();
    let faces = (0..4)
        .map(|k| match k {
            SELECT => select_face(g, k == front),
            MAP_FACE => map_face(g, k == front),
            QUEST => quest_face(g, k == front),
            _ => equipment_face(g, k == front),
        })
        .collect();
    let name = |k: usize| {
        s.get(match k {
            SELECT => "select_item",
            MAP_FACE => "map",
            QUEST => "quest",
            _ => "equipment",
        })
    };
    MenuView {
        turn: g.menu.turn,
        face: front,
        faces,
        left: KeyHint {
            key: s.get("key_q"),
            text: name((front + 3) % 4),
            action: action(s, "menu.turn", "-1", "", false),
        },
        right: KeyHint {
            key: s.get("key_e"),
            text: name((front + 1) % 4),
            action: action(s, "menu.turn", "1", "", false),
        },
        close: KeyHint {
            key: s.get("key_tab"),
            text: s.get("close"),
            action: action(s, "menu.close", "", "close", true),
        },
        quick: quick_view(s, &g.world.inventory),
        assign: s.get("assign_keys"),
    }
}

fn turn(g: &mut Game, by: i32) {
    g.menu.turn += by;
    g.menu.note = None;
}

/// Points the cursor of the screen in front at slot `k`.
fn point(g: &mut Game, k: usize) {
    let face = g.menu.face();
    if k < layout(face).len() && g.menu.cursor[face] != k {
        g.menu.cursor[face] = k;
        g.menu.note = None;
    }
}

/// Enter on the slot under the cursor: Equipment wears or holds it.
fn choose(g: &mut Game) {
    let face = g.menu.face();
    // M7 travel: Enter on the flute plays it
    if face == SELECT && select_item(&g.world.inventory, g.menu.cursor[SELECT]) == Some(Item::Flute) {
        crate::travel::play(g);
        return;
    }
    if face != EQUIPMENT {
        return;
    }
    if let Some(i) = equipment_item(&g.world.inventory, g.menu.cursor[EQUIPMENT]) {
        g.menu.note = g.world.inventory.equip(i).err().map(|r| refusal(&g.strings, r));
    }
}

/// 1, 2 or 3 over a Select Item slot: that key holds the item.
fn assign(g: &mut Game, key: usize) {
    if g.menu.face() != SELECT {
        return;
    }
    if let Some(i) = select_item(&g.world.inventory, g.menu.cursor[SELECT]) {
        g.menu.note = g
            .world
            .inventory
            .assign(i, key)
            .err()
            .map(|r| refusal(&g.strings, r));
    }
}

fn refusal(s: &Strings, r: Refusal) -> String {
    let name = |i: Item| s.get(&format!("item_{}", i.id()));
    match r {
        Refusal::NotQuick(i) => fill(&s.get("refuse_key"), &[("item", name(i))]),
        Refusal::NotWorn(i) => fill(&s.get("refuse_wear"), &[("item", name(i))]),
        Refusal::NotCarried(i) => fill(&s.get("refuse_missing"), &[("item", name(i))]),
        Refusal::NoSuchKey => s.get("refuse_key_number"),
    }
}

fn menu_key(g: &mut Game, key: &str) -> Result<(), String> {
    let face = g.menu.face();
    let go = |g: &mut Game, d: (i32, i32)| {
        let k = moved(face, g.menu.cursor[face], d);
        point(g, k);
    };
    match key {
        "turn_left" => turn(g, -1),
        "turn_right" => turn(g, 1),
        "left" => go(g, (-1, 0)),
        "right" => go(g, (1, 0)),
        "up" => go(g, (0, -1)),
        "down" => go(g, (0, 1)),
        "choose" => choose(g),
        "assign_1" => assign(g, 0),
        "assign_2" => assign(g, 1),
        "assign_3" => assign(g, 2),
        "map" if face == MAP_FACE => g.show(Screen::Play),
        "map" => open(g, MAP_FACE),
        "close" => g.show(Screen::Play),
        other => return Err(format!("The menu has no key \"{other}\".")),
    }
    Ok(())
}

// --------------------------------------------------- dialogue and shop

fn dialogue_view(g: &Game) -> Option<DialogueView> {
    let s = &g.strings;
    let d = g.world.ui.dialogue.as_ref()?;
    let text = s.get(&d.lines[d.line]);
    let at = text
        .char_indices()
        .nth(shown_chars(d.ticks))
        .map(|(k, _)| k)
        .unwrap_or(text.len());
    let done = at == text.len();
    let last = d.line + 1 == d.lines.len();
    Some(DialogueView {
        who: d.who.clone(),
        name: s.get(&format!("who_{}", d.who)),
        portrait: format!("data/icons/portrait_{}.svg", d.who),
        said: text[..at].into(),
        unsaid: text[at..].into(),
        done,
        key: s.get("key_space"),
        next: action(
            s,
            "dialogue.next",
            "",
            if done && last { "close" } else { "next" },
            done && last,
        ),
        song: (d.lines[d.line] == crate::travel::TEACH_LINE).then(|| crate::travel::song_card(s)),
    })
}

/// What the merchant pays for an item: half its value (CW-Q9).
pub fn sale_price(i: Item) -> u32 {
    i.def().value * catalogue().merchant.buys_at_pct / 100
}

/// A string about `n` coins: `<key>_one` when there is one coin.
pub(crate) fn coins_text(s: &Strings, key: &str, n: u32, more: &[(&str, String)]) -> String {
    let key = if n == 1 {
        format!("{key}_one")
    } else {
        key.to_string()
    };
    let mut v = vec![("n", n.to_string())];
    v.extend(more.iter().cloned());
    fill(&s.get(&key), &v)
}

fn sells(w: &World, i: Item) -> bool {
    let d = i.def();
    (d.sold && d.stack > 1) || w.ui.stock[i as usize] > 0
}

fn shop_view(g: &Game) -> ShopView {
    let (s, w) = (&g.strings, &g.world);
    let inv = &w.inventory;
    let price = |n: u32| coins_text(s, "price", n, &[]);
    let buy = Item::ALL
        .iter()
        .copied()
        .filter(|i| sells(w, *i))
        .map(|i| ShopRow {
            // how many are carried, beside what is for sale, once there are any
            item: ItemView {
                count: if inv.has(i) {
                    item_view(s, inv, i).count
                } else {
                    String::new()
                },
                ..item_view(s, inv, i)
            },
            price: price(i.def().value),
            action: Action {
                enabled: inv.coins >= i.def().value && inv.count(i) < i.def().stack,
                ..action(s, "shop.buy", i.id(), "buy", false)
            },
        })
        .collect();
    let sell: Vec<ShopRow> = Item::ALL
        .iter()
        .copied()
        .filter(|i| inv.has(*i) && !i.def().keep)
        .map(|i| ShopRow {
            item: item_view(s, inv, i),
            price: price(sale_price(i)),
            action: action(s, "shop.sell", i.id(), "sell", false),
        })
        .collect();
    ShopView {
        heading: s.get("shop"),
        coins: coins_view(s, w),
        buy_heading: s.get("for_sale"),
        sell_heading: s.get("to_sell"),
        empty: if sell.is_empty() {
            s.get("nothing_to_sell")
        } else {
            String::new()
        },
        buy,
        sell,
        note: g.menu.note.clone().unwrap_or_default(),
        leave: action(s, "shop.close", "", "leave", true),
    }
}

fn buy(g: &mut Game, id: &str) -> Result<(), String> {
    let i = Item::from_id(id).ok_or_else(|| format!("There is no item \"{id}\"."))?;
    let s = &g.strings;
    let name = s.get(&format!("item_{}", i.id()));
    let (w, d) = (&mut g.world, i.def());
    g.menu.note = Some(if !sells(w, i) {
        fill(&s.get("refuse_not_sold"), &[("item", name)])
    } else if w.inventory.coins < d.value {
        coins_text(s, "refuse_coins", d.value - w.inventory.coins, &[("item", name)])
    } else if w.inventory.count(i) >= d.stack {
        fill(
            &s.get("refuse_full"),
            &[("n", d.stack.to_string()), ("item", name)],
        )
    } else {
        w.inventory.coins -= d.value;
        w.inventory.add(i, 1);
        if d.stack == 1 {
            w.ui.stock[i as usize] -= 1;
        }
        fill(&s.get("bought"), &[("item", name)])
    });
    Ok(())
}

fn sell(g: &mut Game, id: &str) -> Result<(), String> {
    let i = Item::from_id(id).ok_or_else(|| format!("There is no item \"{id}\"."))?;
    let s = &g.strings;
    let name = s.get(&format!("item_{}", i.id()));
    let w = &mut g.world;
    g.menu.note = Some(if i.def().keep {
        fill(&s.get("refuse_keep"), &[("item", name)])
    } else if !w.inventory.has(i) {
        fill(&s.get("refuse_missing"), &[("item", name)])
    } else {
        w.inventory.remove(i, 1);
        w.inventory.coins += sale_price(i);
        if i.def().stack == 1 {
            w.ui.stock[i as usize] += 1;
        }
        // a sword sold from the hand leaves the other in it, if carried
        if w.inventory.weapon.is_none() {
            w.inventory.weapon = [Item::Sword, Item::ShortSword]
                .into_iter()
                .find(|x| w.inventory.has(*x));
        }
        coins_text(s, "sold", sale_price(i), &[("item", name)])
    });
    Ok(())
}

// -------------------------------------------------------------- settings

fn settings_view(g: &Game) -> SettingsView {
    let s = &g.strings;
    let set = &g.world.ui.settings;
    let p = |v: i32| fill(&s.get("percent"), &[("n", v.to_string())]);
    let range = |id: &str, name: &str, value: i32, (min, max, step): (i32, i32, i32)| Control {
        id: id.into(),
        label: s.get(id),
        kind: "range".into(),
        value,
        min,
        max,
        step,
        text: p(value),
        name: name.into(),
        next: String::new(),
        on: false,
    };
    let look = (crate::camera::LOOK_PCT_MIN, crate::camera::LOOK_PCT_MAX, 5);
    SettingsView {
        heading: s.get("settings"),
        controls: vec![
            range("look_speed", "settings.look", g.world.aim.pct, look),
            range("mouse_sensitivity", "settings.hand", set.hand_pct, HAND_RANGE),
            Control {
                id: "invert_y".into(),
                label: s.get("invert_y"),
                kind: "switch".into(),
                value: set.invert_y as i32,
                min: 0,
                max: 1,
                step: 1,
                text: s.get(if set.invert_y { "on" } else { "off" }),
                name: "settings.invert".into(),
                next: (!set.invert_y).to_string(),
                on: set.invert_y,
            },
            range("volume", "settings.volume", set.volume_pct, VOLUME_RANGE),
            // M7 (T10): the swing cue
            Control {
                id: "swing_cue".into(),
                label: s.get("swing_cue"),
                kind: "switch".into(),
                value: set.cue as i32,
                min: 0,
                max: 1,
                step: 1,
                text: s.get(if set.cue { "on" } else { "off" }),
                name: "settings.cue".into(),
                next: (!set.cue).to_string(),
                on: set.cue,
            },
            // M7 (P4): blood on or off
            Control {
                id: "blood".into(),
                label: s.get("blood"),
                kind: "switch".into(),
                value: set.blood as i32,
                min: 0,
                max: 1,
                step: 1,
                text: s.get(if set.blood { "on" } else { "off" }),
                name: "settings.blood".into(),
                next: (!set.blood).to_string(),
                on: set.blood,
            },
        ],
        note: s.get("settings_note"),
        back: action(s, "settings.back", "", "back", false),
    }
}

/// The sword hand's speed and the volume, percent: least, most, step.
pub const HAND_RANGE: (i32, i32, i32) = (10, 400, 5);
pub const VOLUME_RANGE: (i32, i32, i32) = (0, 100, 5);

fn percent_value(value: &str, what: &str) -> Result<i32, String> {
    value
        .trim()
        .parse()
        .map_err(|_| format!("The {what} is a whole percentage, such as 100, not \"{value}\"."))
}

// ------------------------------------------------------------------ acts

/// Whether an act is this module's.
pub fn owns(name: &str) -> bool {
    ["menu.", "settings.", "dialogue.", "shop.", "test."]
        .iter()
        .any(|p| name.starts_with(p))
}

/// This module's acts. Keys and clicks that arrive on another screen than
/// theirs (a late key event) change nothing.
pub fn act(g: &mut Game, name: &str, value: &str) -> Result<(), String> {
    let screen = g.screen;
    let index = || {
        value
            .trim()
            .parse::<usize>()
            .map_err(|_| format!("A slot is a whole number, not \"{value}\"."))
    };
    match name {
        "menu.open" if screen == Screen::Play => {
            let face = if value == "map" { MAP_FACE } else { g.menu.face() };
            open(g, face);
        }
        "menu.key" if screen == Screen::Menu => menu_key(g, value)?,
        "menu.point" if screen == Screen::Menu => point(g, index()?),
        "menu.choose" if screen == Screen::Menu => {
            point(g, index()?);
            choose(g);
        }
        "menu.turn" if screen == Screen::Menu => turn(g, if value.trim() == "-1" { -1 } else { 1 }),
        "menu.close" if screen == Screen::Menu => g.show(Screen::Play),
        "menu.open" | "menu.key" | "menu.point" | "menu.choose" | "menu.turn" | "menu.close" => {}
        "settings.open" if matches!(screen, Screen::Title | Screen::Paused) => {
            g.menu.settings_from = screen;
            g.show(Screen::Settings);
        }
        "settings.back" if screen == Screen::Settings => {
            let back = g.menu.settings_from;
            g.show(back);
        }
        "settings.open" | "settings.back" => {}
        "settings.look" => g.act("look.speed", value)?,
        "settings.hand" => {
            let v = percent_value(value, "mouse sensitivity")?;
            g.world.ui.settings.hand_pct = v.clamp(HAND_RANGE.0, HAND_RANGE.1);
        }
        "settings.volume" => {
            let v = percent_value(value, "volume")?;
            g.world.ui.settings.volume_pct = v.clamp(VOLUME_RANGE.0, VOLUME_RANGE.1);
        }
        // M7 (T10): the swing cue on or off
        "settings.cue" => {
            g.world.ui.settings.cue = match value.trim() {
                "true" => true,
                "false" => false,
                other => return Err(format!("The swing cue is true or false, not \"{other}\".")),
            }
        }
        // M7 (P4)
        "settings.blood" => g.act("wounds.blood", value)?,
        "settings.invert" => {
            g.world.ui.settings.invert_y = match value.trim() {
                "true" => true,
                "false" => false,
                other => return Err(format!("Invert Y is true or false, not \"{other}\".")),
            }
        }
        "dialogue.next" | "dialogue.key" if screen == Screen::Dialogue => next_line(g),
        "dialogue.next" | "dialogue.key" => {}
        "shop.buy" if screen == Screen::Shop => buy(g, value)?,
        "shop.sell" if screen == Screen::Shop => sell(g, value)?,
        "shop.close" if screen == Screen::Shop => {
            g.world.ui.shop_open = false;
            g.menu.note = None;
            g.show(Screen::Play);
        }
        "shop.buy" | "shop.sell" | "shop.close" => {}
        // test-only: what screenshots and the browser check need before
        // M1, M5 and M6 send the real thing
        "test.give_all" => inventory::give_all(&mut g.world),
        "test.hud" => {
            g.world.inventory.knows_imbue = true;
            g.world.ui.demo = Some(Demo {
                health: Gauge { now: 64, most: 100 },
                foe: Some(("bandit".into(), Gauge { now: 35, most: 80 })),
                since_cast: 750,
            });
            inventory::give(&mut g.world, Item::Sword, 1);
            received(&mut g.world, Item::Sword, 1);
        }
        "test.explore" => explore(&mut g.world),
        // M6: the imbue taught, for tests and screenshots (`?act=test.imbue`)
        "test.imbue" => crate::imbue::learn(&mut g.world),
        // these two open on the next tick of play, as M5's would
        "test.dialogue" => say(&mut g.world, "trainer", &["sample_line_1", "sample_line_2"]),
        "test.shop" => {
            g.world.inventory.coins = g.world.inventory.coins.max(75);
            open_shop(&mut g.world);
        }
        _ => return Err(format!("Unknown action \"{name}\".")),
    }
    g.ui_version += 1;
    Ok(())
}

/// Test-only: reveals a walk from the village north-east toward the forest
/// and back along the road west, so the map screenshot shows a filled map.
fn explore(w: &mut World) {
    let m = |v: i32| fx::int(v);
    let path = [
        (2000, 2800),
        (2150, 2650),
        (2350, 2450),
        (2600, 2250),
        (2850, 2050),
        (3050, 1850),
        (2850, 1700),
        (2500, 1800),
        (2150, 1950),
        (1800, 2150),
        (1500, 2350),
        (1250, 2600),
        (1500, 2900),
        (1800, 3050),
        (2000, 2800),
    ];
    for pair in path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        for k in 0..=16 {
            let x = a.0 + (b.0 - a.0) * k / 16;
            let z = a.1 + (b.1 - a.1) * k / 16;
            w.ui.fog.reveal(m(x), m(z));
        }
    }
    w.ui.fog.last = None;
}

// -------------------------------------------------------------- the map

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Parchment {
    paper: [i32; 3],
    unseen: [i32; 3],
    hatch: [i32; 3],
    edge: [i32; 3],
    ink: [i32; 3],
    water: [i32; 3],
    land_tint_pct: i32,
    relief_pct: i32,
    grain: i32,
    edge_px: i32,
    edge_pct: i32,
    hatch_every: i32,
    // M7: the land's colours (R9.48)
    land_pct: i32,
    forest: [i32; 3],
    forest_dot: [i32; 3],
    forest_dot_every: i32,
    shade_pct: i32,
    shade_steps: i32,
    contour_m: i32,
    contour_pct: i32,
    shore: [i32; 3],
    road: [i32; 3],
    roof: [i32; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Menus {
    map: Parchment,
    place_icons: Vec<String>,
}

fn menus() -> &'static Menus {
    static MENUS: OnceLock<Menus> = OnceLock::new();
    MENUS.get_or_init(|| {
        let mut v: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/menus.json")).expect("data/menus.json");
        if let Some(m) = v.as_object_mut() {
            m.remove("_note");
        }
        serde_json::from_value(v).expect("data/menus.json")
    })
}

fn mix(a: [i32; 3], b: [i32; 3], pct: i32) -> [i32; 3] {
    let p = pct.clamp(0, 100);
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * p / 100)
}

/// The map as RGBA pixels, `MAP_PX` a side, row by row from the north-west
/// corner (M7, R9.48): where the player has walked, the land as Ocarina of
/// Time's and Majora's Mask's maps draw it, on M4's parchment: the ground's
/// own colours (`terrain::Land::map`) softened by the paper, the forest
/// dotted with trees, the relief lit from the north-west in a few flat
/// steps, a contour every 10 m, the water with a darker shore, the roads
/// and the buildings; where not, hatched paper; the edges darkened like
/// old paper. The page puts it on a canvas as it is.
pub fn map_pixels(g: &Game) -> Vec<u8> {
    let w = &g.world;
    let p = &menus().map;
    let fog = &w.ui.fog;
    let cells = w.land.map(MAP_PX);
    let at = |x: i32, y: i32| &cells[(y.clamp(0, MAP_PX - 1) * MAP_PX + x.clamp(0, MAP_PX - 1)) as usize];
    let mut out = Vec::with_capacity((MAP_PX * MAP_PX * 4) as usize);
    for py in 0..MAP_PX {
        for px in 0..MAP_PX {
            // how much of the pixel has been seen: the four nearest cells'
            // centres, weighted by nearness, in sixteenths
            let qx = 2 * px - 1;
            let qz = 2 * py - 1;
            let (cx, fxq) = (qx.div_euclid(4), qx.rem_euclid(4));
            let (cz, fzq) = (qz.div_euclid(4), qz.rem_euclid(4));
            let mut seen = 0;
            for (ox, wx) in [(0, 4 - fxq), (1, fxq)] {
                for (oz, wz) in [(0, 4 - fzq), (1, fzq)] {
                    if fog.seen(cx + ox, cz + oz) {
                        seen += wx * wz;
                    }
                }
            }
            let c0 = at(px, py);
            let mut lit = mix(p.paper, c0.colour, p.land_pct);
            if c0.forest > ONE / 4 {
                lit = mix(lit, p.forest, c0.forest * 80 / ONE);
                let every = p.forest_dot_every.max(2);
                if c0.forest > ONE / 2 && (px + 2 * (py / every)) % every == 0 && py % every == 0 {
                    lit = p.forest_dot;
                }
            }
            if c0.water {
                lit = mix(p.paper, p.water, 85);
                let shore = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|&(dx, dy)| !at(px + dx, py + dy).water);
                if shore {
                    lit = p.shore;
                }
            } else {
                // the relief, lit from the north-west, in flat steps: the
                // rise from the pixel up-left to the one down-right, over
                // their 22.6 m apart, per cent
                let rise = (at(px + 1, py + 1).h - at(px - 1, py - 1).h) as i64 * 100
                    / fx::int(PX_M * 2 * 1414 / 1000) as i64;
                let steps = p.shade_steps.max(1) as i64;
                let step = (rise.clamp(-steps * 3, steps * 3) / 3) as i32;
                if step != 0 {
                    let pct = (p.shade_pct * step.abs() / steps as i32).min(60);
                    lit = if step < 0 {
                        mix(lit, [255; 3], pct)
                    } else {
                        mix(lit, p.ink, pct)
                    };
                }
                let band = |c: &crate::terrain::MapCell| fx::floor(c.h).div_euclid(p.contour_m.max(1));
                if band(c0) != band(at(px + 1, py)) || band(c0) != band(at(px, py + 1)) {
                    lit = mix(lit, p.ink, p.contour_pct);
                }
                if c0.road {
                    lit = p.road;
                }
                if c0.building {
                    lit = p.roof;
                }
            }
            let hatched = (px + py).rem_euclid(p.hatch_every) == 0;
            let unseen = if hatched { p.hatch } else { p.unseen };
            let mut c = mix(unseen, lit, seen * 100 / 16);
            let hash = (px as u32).wrapping_mul(2_654_435_761) ^ (py as u32).wrapping_mul(40_503);
            let grain = (hash >> 13) as i32 % (2 * p.grain + 1) - p.grain;
            let edge = px.min(py).min(MAP_PX - 1 - px).min(MAP_PX - 1 - py);
            if edge < p.edge_px {
                c = mix(c, p.edge, p.edge_pct * (p.edge_px - edge) / p.edge_px);
            }
            for v in c {
                out.push((v + grain).clamp(0, 255) as u8);
            }
            out.push(255);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::{frames, game};
    use crate::input::{FORWARD, ITEM_1};

    fn json(g: &Game) -> serde_json::Value {
        serde_json::to_value(crate::api::ui(g)).unwrap()
    }

    fn press_tab(g: &mut Game) {
        frames(g, 1, MENU, 0);
        frames(g, 1, 0, 0);
    }

    fn playing() -> Game {
        let mut g = game();
        // M5: these start standing at the village centre, as before the
        // opening was the start (`story.skip`)
        g.act("story.skip", "").unwrap();
        g.act("screen.show", "play").unwrap();
        g
    }

    /// 2000 m / 32 = 62.5 and 2800 m / 32 = 87.5: the start is in cell
    /// (62, 87). A disc of radius 4 holds 9 + 2×7 + 2×7 + 2×5 + 2×1 = 49
    /// cells (dx = 0, ±1, ±2, ±3, ±4).
    #[test]
    fn walking_reveals_a_disc_of_the_map_round_the_player() {
        let mut g = playing();
        frames(&mut g, 1, 0, 0);
        let fog = &g.world.ui.fog;
        assert_eq!(fog.count(), 49);
        assert!(fog.seen(62, 87) && fog.seen(66, 87) && fog.seen(62, 83));
        assert!(!fog.seen(67, 87) && !fog.seen(65, 90) && !fog.seen(62, 82));
        let v = fog.version;
        // walk north-north-west (the start's heading) until the cell changes
        let start = Fog::cell_of(g.world.player.pos.x, g.world.player.pos.z);
        let mut n = 0;
        while Fog::cell_of(g.world.player.pos.x, g.world.player.pos.z) == start {
            frames(&mut g, 1, FORWARD, 0);
            n += 1;
            assert!(n < 2000, "the player did not leave the cell");
        }
        let fog = &g.world.ui.fog;
        assert!(fog.count() > 49 && fog.version > v);
        // and standing still reveals nothing more
        let (c, v) = (fog.count(), fog.version);
        frames(&mut g, 30, 0, 0);
        frames(&mut g, 30, 0, 0);
        let still = g.world.ui.fog.clone();
        assert!(still.count() >= c && still.version >= v);
    }

    #[test]
    fn the_same_walk_gives_the_same_map() {
        let walk = || {
            let mut g = playing();
            frames(&mut g, 600, FORWARD | crate::input::RUN, 40);
            (g.world.ui.fog.clone(), map_pixels(&g))
        };
        assert_eq!(walk(), walk());
    }

    #[test]
    fn tab_opens_the_ring_where_it_was_left_and_pauses_the_world() {
        let mut g = playing();
        frames(&mut g, 10, FORWARD, 0);
        press_tab(&mut g);
        assert_eq!(g.screen, Screen::Menu);
        let held = g.world.clone();
        frames(&mut g, 120, FORWARD, 300);
        assert_eq!(g.world, held, "the world holds still behind the menu");
        assert_eq!(json(&g)["capture"], false, "the menu lets go of the pointer");
        assert_eq!(json(&g)["keymap"], "menu");
        // E turns right, from Select Item to the map
        g.act("menu.key", "turn_right").unwrap();
        assert_eq!((g.menu.turn, g.menu.face()), (1, MAP_FACE));
        g.act("menu.key", "close").unwrap();
        assert_eq!(g.screen, Screen::Play);
        press_tab(&mut g);
        assert_eq!(g.menu.face(), MAP_FACE, "the ring opens where it was left");
    }

    #[test]
    fn q_and_e_turn_the_ring_through_its_four_screens_in_order() {
        let mut g = playing();
        press_tab(&mut g);
        let mut seen = Vec::new();
        for _ in 0..4 {
            g.act("menu.key", "turn_right").unwrap();
            seen.push(json(&g)["menu"]["faces"][g.menu.face()]["id"].clone());
        }
        assert_eq!(seen, ["map", "quest", "equipment", "select"]);
        assert_eq!(g.menu.turn, 4, "four turns: once round, still counting");
        g.act("menu.key", "turn_left").unwrap();
        assert_eq!((g.menu.turn, g.menu.face()), (3, EQUIPMENT));
        let u = json(&g);
        assert_eq!(u["menu"]["left"]["text"], "Quest");
        assert_eq!(u["menu"]["right"]["text"], "Select Item");
    }

    #[test]
    fn m_opens_the_ring_at_the_map_the_short_way_and_closes_it_again() {
        let mut g = playing();
        g.menu.turn = 3; // left at Equipment
        frames(&mut g, 1, MAP, 0);
        assert_eq!(g.screen, Screen::Menu);
        // Equipment (3) to Map (1) is two turns either way: +2
        assert_eq!((g.menu.turn, g.menu.face()), (5, MAP_FACE));
        g.act("menu.key", "map").unwrap();
        assert_eq!(g.screen, Screen::Play);
        g.menu.turn = 0;
        frames(&mut g, 1, 0, 0);
        frames(&mut g, 1, MAP, 0);
        assert_eq!(g.menu.turn, 1, "Select Item to Map: one turn right");
        g.act("menu.key", "turn_left").unwrap();
        g.act("menu.key", "map").unwrap();
        assert_eq!(g.menu.face(), MAP_FACE, "M on another screen turns to the map");
        // Quest (2) to Map (1): one turn left, not three right
        g.act("menu.key", "close").unwrap();
        g.menu.turn = 2;
        frames(&mut g, 1, 0, 0);
        frames(&mut g, 1, MAP, 0);
        assert_eq!(g.menu.turn, 1);
    }

    #[test]
    fn a_held_tab_opens_the_ring_once() {
        let mut g = playing();
        frames(&mut g, 1, MENU, 0);
        g.act("menu.key", "close").unwrap();
        frames(&mut g, 5, MENU, 0);
        assert_eq!(g.screen, Screen::Menu, "after closing, a fresh press opens again");
        g.act("menu.close", "").unwrap();
        g.menu.last_bits = MENU;
        frames(&mut g, 5, MENU, 0);
        assert_eq!(g.screen, Screen::Play, "Tab held from before opens nothing");
    }

    #[test]
    fn esc_closes_the_ring_to_the_pause_screen() {
        let mut g = playing();
        press_tab(&mut g);
        g.act("key.escape", "").unwrap();
        assert_eq!(g.screen, Screen::Paused);
        // a lost pointer or a hidden tab in the menu changes nothing
        g.act("screen.show", "play").unwrap();
        press_tab(&mut g);
        g.act("pointer.lost", "").unwrap();
        g.act("page.hidden", "").unwrap();
        assert_eq!(g.screen, Screen::Menu);
    }

    #[test]
    fn pressing_a_quick_key_over_a_slot_puts_the_item_on_it() {
        let mut g = playing();
        inventory::give(&mut g.world, Item::Rock, 5);
        inventory::give(&mut g.world, Item::ChestKey, 1);
        press_tab(&mut g);
        // rocks are in cell 1: right from cell 0
        g.act("menu.key", "right").unwrap();
        assert_eq!(g.menu.cursor[SELECT], 1);
        g.act("menu.key", "assign_2").unwrap();
        assert_eq!(g.world.inventory.quick, [None, Some(Item::Rock), None]);
        let u = json(&g);
        assert_eq!(u["menu"]["faces"][0]["slots"][1]["quick"], "2");
        assert_eq!(u["menu"]["faces"][0]["info"]["name"], "Rock");
        assert_eq!(u["menu"]["quick"][1]["item"]["id"], "rock");
        // the chest key, cell 5, goes on no key: the refusal shows under it
        g.act("menu.point", "5").unwrap();
        g.act("menu.key", "assign_1").unwrap();
        assert_eq!(g.world.inventory.quick[0], None);
        let notes = &json(&g)["menu"]["faces"][0]["info"]["notes"];
        assert_eq!(
            notes.as_array().unwrap().last().unwrap(),
            "The Chest key goes on no key."
        );
        // back in play, 2 readies the rock
        g.act("menu.key", "close").unwrap();
        frames(&mut g, 1, crate::input::ITEM_2, 0);
        assert_eq!(inventory::ready(&g.world), Some(Item::Rock));
        assert_eq!(json(&g)["play"]["quick"][1]["ready"], true);
        frames(&mut g, 1, 0, 0);
        frames(&mut g, 1, ITEM_1, 0);
        assert_eq!(inventory::ready(&g.world), Some(Item::Rock), "nothing on 1");
    }

    #[test]
    fn the_cursor_moves_to_the_nearest_slot_each_way_and_stops_at_the_edge() {
        // Select Item: 6 across, 3 down
        assert_eq!(moved(SELECT, 0, (1, 0)), 1);
        assert_eq!(moved(SELECT, 0, (0, 1)), 6);
        assert_eq!(moved(SELECT, 0, (-1, 0)), 0);
        assert_eq!(moved(SELECT, 17, (0, 1)), 17);
        assert_eq!(moved(SELECT, 7, (0, -1)), 1);
        // Equipment: the sword (0, 2) right goes to the body (2, 1), the
        // nearest by 2 + 2×1 = 4 against the legs' 2 + 2×1 = 4, the lower
        // index on a tie; the body down goes to the legs (2 + 0) before the
        // hands (1 + 2×2); the body right is a tie between the head (4, 0)
        // and the hands (4, 2), 2 + 2×1 each, so the head; the head down is
        // the hands, 2 + 0
        assert_eq!(moved(EQUIPMENT, 1, (1, 0)), 3);
        assert_eq!(moved(EQUIPMENT, 3, (0, 1)), 5);
        assert_eq!(moved(EQUIPMENT, 3, (1, 0)), 2);
        assert_eq!(moved(EQUIPMENT, 2, (0, 1)), 4);
    }

    #[test]
    fn equipment_wears_a_piece_and_shows_the_speedster_numbers_in_words() {
        let mut g = playing();
        for i in [Item::Hood, Item::Jacket, Item::Boots, Item::Sword] {
            inventory::give(&mut g.world, i, 1);
        }
        press_tab(&mut g);
        g.act("menu.key", "turn_left").unwrap();
        assert_eq!(g.menu.face(), EQUIPMENT);
        for k in ["2", "3", "6"] {
            g.act("menu.choose", k).unwrap();
        }
        g.act("menu.choose", "1").unwrap();
        assert_eq!(items::speedster_worn(&g.world), 3);
        assert_eq!(items::held(&g.world), crate::pose::Held::Sword);
        let u = json(&g);
        let set = &u["menu"]["faces"][3]["set"];
        assert_eq!(set["count"], "3 of 5 pieces worn");
        // speed 1.6 of a most of 2.0: 80% of the bar; footing 0.55³ =
        // 0.166 375, 17%
        assert_eq!(set["bars"][0]["text"], "160%");
        assert_eq!(set["bars"][0]["pct"], 80);
        assert_eq!(set["bars"][1]["text"], "17%");
        assert_eq!(set["bars"][1]["pct"], 17);
        assert_eq!(u["menu"]["faces"][3]["slots"][2]["worn"], true);
        assert_eq!(u["menu"]["faces"][3]["slots"][5]["item"], serde_json::Value::Null);
        assert_eq!(u["menu"]["faces"][3]["info"]["name"], "Sword");
        // the HUD's footing bar shows while pieces are worn
        g.act("menu.close", "").unwrap();
        let hud = &json(&g)["play"];
        assert_eq!(hud["footing"][1]["label"], "Footing");
        assert_eq!(hud["footing"][1]["text"], "17%");
        // taking the hood off: two pieces, 0.3025, 30%
        press_tab(&mut g);
        g.act("menu.choose", "2").unwrap();
        assert_eq!(items::speedster_worn(&g.world), 2);
        assert_eq!(json(&g)["menu"]["faces"][3]["set"]["bars"][1]["text"], "30%");
    }

    #[test]
    fn the_map_places_the_player_and_marks_the_places_walked_to() {
        let mut g = playing();
        frames(&mut g, 1, 0, 0);
        frames(&mut g, 1, MAP, 0);
        let u = json(&g);
        let map = &u["menu"]["faces"][1]["map"];
        // 2000 / 4000 and 2800 / 4000
        assert_eq!(map["you"]["left"], "50.00%");
        assert_eq!(map["you"]["top"], "70.00%");
        // facing 20° west of north: 340° clockwise
        assert_eq!(map["heading"], "340.0");
        assert_eq!(map["marks"][0]["label"], "The village");
        assert_eq!(map["size"], 250);
        assert_eq!(map_pixels(&g).len(), 250 * 250 * 4);
    }

    /// R9.48: walked, the map shows the land's own colours, not plain
    /// parchment. The start is pixel (125, 175), the village green: its
    /// grass (150 to 175 red, 176 to 201 green, data/land.json) at 72% over
    /// the paper (248, 238, 212) is about (177, 201, 134) give or take the
    /// patchwork: greener than it is red by 15 or more, and more than 40
    /// off the paper's red. Far off, unwalked, it is hatched paper still.
    #[test]
    fn the_map_image_shows_the_land_where_walked_and_hatched_paper_where_not() {
        let mut g = playing();
        frames(&mut g, 1, 0, 0);
        let px = map_pixels(&g);
        let at = |x: usize, y: usize| {
            let k = (y * 250 + x) * 4;
            [px[k] as i32, px[k + 1] as i32, px[k + 2] as i32]
        };
        let p = &menus().map;
        let green = at(125, 175);
        assert!(green[1] - green[0] >= 15, "{green:?} is not the green's grass");
        assert!((green[0] - p.paper[0]).abs() > 40, "{green:?} is parchment");
        let near = |a: [i32; 3], b: [i32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() <= p.grain);
        assert!(
            near(at(126, 40), p.unseen) || near(at(126, 40), p.hatch),
            "{:?}",
            at(126, 40)
        );
        assert_eq!(px[3], 255);
    }

    /// The map's pixels know the river, the roads and the forest: the south
    /// ford, 36 m across at (642, 3300), takes pixel (40, 206), whose middle
    /// (648, 3304) is 6 m from the river's, water; the lane
    /// north of the village at (1994, 2550), pixel (124, 159), is road, the
    /// camp's forest at (3100, 1200), pixel (193, 75), is deep forest.
    #[test]
    fn the_map_knows_the_water_the_roads_and_the_forest() {
        let g = playing();
        let cells = g.world.land.map(MAP_PX);
        let cell = |x: i32, y: i32| cells[(y * MAP_PX + x) as usize];
        assert!(cell(40, 206).water);
        assert!(cell(124, 159).road);
        assert!(cell(193, 75).forest > ONE * 9 / 10);
        assert!(!cell(125, 175).water && !cell(125, 175).road);
    }

    #[test]
    fn the_hud_shows_health_full_until_the_duel_sends_its_own() {
        let g = playing();
        let hud = &json(&g)["play"];
        assert_eq!(hud["health"]["text"], "100 / 100");
        assert_eq!(hud["health"]["pct"], 100);
        assert!(hud["foe"].is_null() && hud["imbue"].is_null());
        assert!(hud["footing"].as_array().unwrap().is_empty());
        assert_eq!(hud["coins"]["text"], "0");
        assert_eq!(json(&g)["hud_shown"], true);
    }

    /// The test values: cast 750 ticks ago, so on for 600 and cooling until
    /// 1 200: 450 ticks left, 7.5 s, shown as 8 s; recovered 750 / 1 200,
    /// 62%.
    #[test]
    fn the_imbue_ring_counts_down_the_cooldown_in_whole_seconds() {
        let mut g = playing();
        g.act("test.hud", "").unwrap();
        let hud = &json(&g)["play"];
        assert_eq!(hud["imbue"]["state"], "cooling");
        assert_eq!(hud["imbue"]["text"], "8 s");
        assert_eq!(hud["imbue"]["pct"], 62);
        assert_eq!(hud["health"]["text"], "64 / 100");
        assert_eq!(hud["foe"]["label"], "Bandit");
        assert_eq!(hud["foe"]["pct"], 43);
        g.world.ui.demo.as_mut().unwrap().since_cast = 120;
        let hud = &json(&g)["play"];
        // on: 480 ticks left of 600, 8 s, 80%
        assert_eq!(
            (hud["imbue"]["state"].clone(), hud["imbue"]["text"].clone()),
            ("on".into(), "8 s".into())
        );
        assert_eq!(hud["imbue"]["pct"], 80);
        g.world.ui.demo.as_mut().unwrap().since_cast = 1200;
        assert_eq!(json(&g)["play"]["imbue"]["text"], "Ready");
    }

    #[test]
    fn an_item_received_shows_a_toast_for_two_and_a_half_seconds() {
        let mut g = playing();
        inventory::give(&mut g.world, Item::Knife, 3);
        assert_eq!(json(&g)["play"]["toast"]["text"], "Received: Throwing knife × 3");
        let v = g.ui_version;
        frames(&mut g, 149, 0, 0);
        assert_eq!(json(&g)["play"]["toast"]["shown"], true);
        frames(&mut g, 1, 0, 0);
        assert_eq!(json(&g)["play"]["toast"]["shown"], false);
        assert!(g.ui_version > v, "the page is told the HUD changed");
        // a full stack gives nothing, and no toast
        g.world.inventory.add(Item::Rock, 20);
        assert_eq!(inventory::give(&mut g.world, Item::Rock, 1), 0);
        assert!(g.world.ui.toast.is_none());
    }

    /// 40 characters a second at 60 ticks: 30 ticks show 20 characters.
    #[test]
    fn dialogue_text_appears_at_a_steady_rate_and_space_reads_on() {
        let mut g = playing();
        say(&mut g.world, "trainer", &["sample_line_1", "sample_line_2"]);
        frames(&mut g, 1, 0, 0);
        assert_eq!(g.screen, Screen::Dialogue);
        let held = g.world.player;
        frames(&mut g, 30, FORWARD, 0);
        assert_eq!(g.world.player, held, "the world holds still while someone speaks");
        let d = json(&g)["dialogue"].clone();
        assert_eq!(d["said"].as_str().unwrap().chars().count(), 20);
        assert_eq!(d["name"], "The trainer");
        assert_eq!(d["done"], false);
        let line = g.strings.get("sample_line_1");
        assert_eq!(
            format!("{}{}", d["said"].as_str().unwrap(), d["unsaid"].as_str().unwrap()),
            line
        );
        // Space: the rest of the line, then the next line, then play
        g.act("dialogue.key", "next").unwrap();
        assert_eq!(json(&g)["dialogue"]["said"].as_str().unwrap(), line);
        g.act("dialogue.next", "").unwrap();
        assert_eq!(json(&g)["dialogue"]["said"], "");
        g.act("dialogue.next", "").unwrap();
        assert_eq!(json(&g)["dialogue"]["next"]["label"], "Close");
        g.act("dialogue.next", "").unwrap();
        assert_eq!(g.screen, Screen::Play);
        assert!(g.world.ui.dialogue.is_none());
    }

    #[test]
    fn the_shop_sells_at_value_and_buys_at_half() {
        let mut g = playing();
        inventory::give(&mut g.world, Item::Sword, 1);
        g.world.inventory.coins = 10;
        open_shop(&mut g.world);
        frames(&mut g, 1, 0, 0);
        assert_eq!(g.screen, Screen::Shop);
        let u = json(&g);
        let ids: Vec<&str> = u["shop"]["buy"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["item"]["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["rock", "knife", "bandage", "hood"]);
        assert_eq!(u["shop"]["buy"][3]["price"], "60 coins");
        assert_eq!(u["shop"]["buy"][3]["action"]["enabled"], false);
        // a rock: 10 − 2 = 8
        g.act("shop.buy", "rock").unwrap();
        assert_eq!(
            (g.world.inventory.coins, g.world.inventory.count(Item::Rock)),
            (8, 1)
        );
        // the hood at 60: 52 short
        g.act("shop.buy", "hood").unwrap();
        assert_eq!(
            json(&g)["shop"]["note"],
            "You need 52 more coins for the Speedster's hood."
        );
        // the sword sells for half of 80: 8 + 40 = 48, and the short sword
        // goes back in hand
        g.act("shop.sell", "sword").unwrap();
        assert_eq!(g.world.inventory.coins, 48);
        g.world.inventory.weapon = None;
        g.act("shop.sell", "rock").unwrap();
        assert_eq!(g.world.inventory.coins, 49, "a rock sells for half of 2");
        assert_eq!(json(&g)["shop"]["note"], "Sold: Rock, for 1 coin.");
        assert_eq!(g.world.inventory.weapon, Some(Item::ShortSword));
        // the chest key is kept
        inventory::give(&mut g.world, Item::ChestKey, 1);
        g.act("shop.sell", "chest_key").unwrap();
        assert!(g.world.inventory.has(Item::ChestKey));
        assert_eq!(
            json(&g)["shop"]["note"],
            "The merchant does not buy the Chest key."
        );
        // the hood is one of a kind: bought once, it leaves the stock
        g.world.inventory.coins = 60;
        g.act("shop.buy", "hood").unwrap();
        assert!(g.world.inventory.has(Item::Hood));
        assert_eq!(
            json(&g)["shop"]["buy"].as_array().unwrap().len(),
            4,
            "the sword sold is for sale now"
        );
        assert!(!sells(&g.world, Item::Hood));
        g.act("shop.close", "").unwrap();
        assert_eq!(g.screen, Screen::Play);
    }

    #[test]
    fn settings_open_from_the_title_and_the_pause_and_go_back_there() {
        let mut g = game();
        g.act("settings.open", "").unwrap();
        assert_eq!(g.screen, Screen::Settings);
        let u = json(&g);
        let labels: Vec<&str> = u["settings"]["controls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["label"].as_str().unwrap())
            .collect();
        assert_eq!(
            labels,
            [
                "Look speed",
                "Mouse sensitivity",
                "Invert Y",
                "Volume",
                "Swing cue",
                "Blood"
            ]
        );
        g.act("settings.volume", "35").unwrap();
        g.act("settings.hand", "1000").unwrap();
        assert_eq!(
            (g.world.ui.settings.volume_pct, g.world.ui.settings.hand_pct),
            (35, 400)
        );
        g.act("key.escape", "").unwrap();
        assert_eq!(g.screen, Screen::Title);
        g.act("screen.show", "play").unwrap();
        g.act("key.escape", "").unwrap();
        g.act("settings.open", "").unwrap();
        g.act("settings.look", "50").unwrap();
        assert_eq!(g.world.aim.pct, 50);
        g.act("settings.back", "").unwrap();
        assert_eq!(g.screen, Screen::Paused);
        assert_eq!(
            g.act("settings.volume", "loud"),
            Err("The volume is a whole percentage, such as 100, not \"loud\".".into())
        );
    }

    /// Invert Y: the same downward movement looks up instead of down.
    #[test]
    fn invert_y_turns_the_view_up_for_a_downward_mouse() {
        let pitch = |invert: &str| {
            let mut g = playing();
            g.act("settings.invert", invert).unwrap();
            let before = g.world.aim.pitch;
            g.advance(
                g.clock.last().unwrap_or(0) + crate::ticks::TICK_US,
                0,
                0,
                2_000,
                0,
            );
            g.world.aim.pitch - before
        };
        let (plain, inverted) = (pitch("false"), pitch("true"));
        assert!(plain < -400, "down looks down: {plain}");
        // the aim keeps its remainder rounded down (camera::Aim::rest), so
        // the two ways differ by at most one unit
        assert!((inverted + plain).abs() <= 1, "{plain} {inverted}");
    }

    #[test]
    fn the_title_and_pause_screens_offer_settings_and_the_menu_keys() {
        let g = game();
        let u = json(&g);
        let more = &u["panels"][1];
        assert_eq!(more["id"], "title-more");
        assert_eq!(more["actions"][0]["label"], "Settings");
        assert_eq!(more["rows"][0]["key"], "Tab");
        assert_eq!(more["rows"][0]["does"], "Menu");
        assert_eq!(u["panels"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_menu_key_on_another_screen_changes_nothing_and_an_unknown_one_is_refused() {
        let mut g = playing();
        g.act("menu.key", "turn_right").unwrap();
        assert_eq!((g.screen, g.menu.turn), (Screen::Play, 0));
        press_tab(&mut g);
        assert_eq!(
            g.act("menu.key", "jump"),
            Err("The menu has no key \"jump\".".into())
        );
    }

    /// Walks every string value of a view model, skipping the fields that
    /// hold ids, paths and act names rather than reader text.
    fn reader_text(v: &serde_json::Value, out: &mut Vec<String>) {
        const NOT_TEXT: &[&str] = &[
            "id", "kind", "icon", "portrait", "who", "place", "state", "keymap", "value", "next", "figure",
            "left", "top",
        ];
        match v {
            serde_json::Value::String(s) => out.push(s.clone()),
            serde_json::Value::Array(a) => a.iter().for_each(|x| reader_text(x, out)),
            serde_json::Value::Object(m) => {
                for (k, x) in m {
                    let act_name = k == "name" && x.as_str().is_some_and(|s| s.contains('.'));
                    if !NOT_TEXT.contains(&k.as_str()) && !act_name {
                        reader_text(x, out);
                    }
                }
            }
            _ => {}
        }
    }

    /// A key missing from data/strings.json reads as itself ("item_rock"),
    /// and a placeholder the core does not fill keeps its braces: neither may
    /// reach the page from any screen this module draws.
    #[test]
    fn every_string_the_menus_show_is_written_and_filled() {
        let mut g = playing();
        g.act("test.give_all", "").unwrap();
        g.act("test.hud", "").unwrap();
        g.act("test.explore", "").unwrap();
        let mut views = vec![json(&g)];
        for face in 0..4 {
            g.menu.turn = face;
            g.screen = Screen::Menu;
            for k in 0..18 {
                g.menu.cursor[face as usize] = k;
                views.push(json(&g));
            }
        }
        g.screen = Screen::Play;
        g.act("test.dialogue", "").unwrap();
        frames(&mut g, 1, 0, 0);
        views.push(json(&g));
        g.world.ui.dialogue = None;
        g.screen = Screen::Play;
        g.act("test.shop", "").unwrap();
        frames(&mut g, 1, 0, 0);
        g.act("shop.buy", "hood").unwrap();
        views.push(json(&g));
        g.screen = Screen::Title;
        g.act("settings.open", "").unwrap();
        views.push(json(&g));
        let mut texts = Vec::new();
        for v in &views {
            reader_text(v, &mut texts);
        }
        assert!(texts.len() > 500, "{}", texts.len());
        for t in &texts {
            assert!(!t.contains('{'), "a placeholder left unfilled: {t}");
            let key_like = t.contains('_')
                && t.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            assert!(!key_like, "a string key shows as itself: {t}");
        }
    }
}
