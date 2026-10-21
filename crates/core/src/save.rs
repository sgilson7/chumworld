//! `save` (M7 part P10, triage T16): what a save holds, how the core writes
//! it and reads it back, the save points, the autosave, and a fall after
//! the opening.
//!
//! The person: "add areas to save your game to browser / to a save file
//! json so you stop reviving in the village, the game should auto save as
//! well in the browser".
//!
//! # What a save is
//!
//! Progress, not a snapshot of the world (`Progress`): the seed, the
//! inventory (counts, the sword hand, the armour slots, the quick keys,
//! coins), the merchant's one-of-a-kind stock, the quest flags (`Flag`), the
//! woken and reached waystones, what was taken (`taken`: the story's pickups
//! and the drops by stable key) and who was beaten (`beaten`: the raid's
//! rounds and the camp's people by stable key), the save point, the
//! settings, the walked map and the ticks played. `taken` and `beaten` are
//! keyed sets: a later part adds its keys to `known_taken` and `known_foe`
//! and a save without them has them absent, so no new version is needed.
//!
//! The text (`text`) is JSON: a format name and version, a checksum over the
//! `game` object as written (FNV-1a, 64 bits, over its compact text with
//! sorted keys), and `game` itself. `read` checks every field: its type, its
//! range, that each id is known, that each count is within its stack, that
//! nothing is held without the route that gives it (CW-6, `routes`), and the
//! checksum, so a hand edit or a corrupt file is refused whole (`Refusal`).
//! Loading (`world`) rebuilds the world from the seed as the opening, ends
//! the opening, applies the progress and stands the player at the save
//! point. The same save gives the same world (CW-4).
//!
//! The core does no input or output: it returns the text (`text_of`), and
//! the page (`web/save.js`, the one file that names browser storage) keeps
//! it under one key and offers it as a download.
//!
//! # When the game saves
//!
//! Never before the opening ends, never mid-duel (`travel::danger`), never
//! mid-dialogue or in the shop, never while down or travelling
//! (`refusal`). E at a save point (each waystone, and the lantern posts in
//! `data/save.json`) saves there and makes it the save point. The game
//! saves itself in the browser when the opening ends (at the save point
//! nearest the crossing), when a waystone is first woken, when the quest's
//! line moves on, when the player walks into or out of a major area, and
//! after `autosave_ticks` of play outside a duel; a save that comes due
//! while it may not be written waits until it may. Each save shows a small
//! Saved mark (`hud`).
//!
//! # A fall after the opening
//!
//! The player comes to at the last save point (`fall`, from `story::come_to`)
//! and keeps everything they had when they fell: the inventory, the coins,
//! what they took, the flags, the waystones. Foes beaten since the last save
//! stand again (`beaten` goes back to the save's), and a drop already taken
//! never lies again (`drop`), so no item comes twice. The opening's dream
//! is unchanged.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::api::{fill, Action, Panel};
use crate::fx::{Fx, ONE};
use crate::game::{Game, Screen};
use crate::geom::{facing, V3};
use crate::items::{Hand, Item, Kind, Slot};
use crate::models::Prop;
use crate::page::Strings;
use crate::story::{self, Gift, Phase};
use crate::world::{HudItem, World};

// ---- data/save.json -----------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PostSpec {
    pub id: String,
    pub place: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AreaSpec {
    pub place: String,
    pub radius_m: i32,
}

/// `data/save.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub format: String,
    pub version: u64,
    pub posts: Vec<PostSpec>,
    pub use_cm: i32,
    pub arrive_cm: i32,
    pub areas: Vec<AreaSpec>,
    pub autosave_ticks: u32,
    pub mark_ticks: u32,
    pub coins_most: u32,
    pub playtime_most: u64,
    pub text_most: usize,
}

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let mut v: Value = serde_json::from_str(json)
            .map_err(|e| format!("The save file's rules could not be read: {e}"))?;
        if let Some(m) = v.as_object_mut() {
            m.retain(|k, _| !k.starts_with('_'));
        }
        let s: Spec =
            serde_json::from_value(v).map_err(|e| format!("The save file's rules could not be read: {e}"))?;
        if s.autosave_ticks == 0 || s.mark_ticks == 0 {
            return Err("The save file's rules need an autosave and a mark of a tick at least.".into());
        }
        Ok(s)
    }
}

/// The rules the game ships with, read once.
pub fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| Spec::parse(include_str!("../../../data/save.json")).expect("data/save.json"))
}

/// The numbers the glossary names.
pub fn values() -> Vec<(&'static str, String)> {
    let ticks = spec().autosave_ticks as u64;
    vec![(
        "autosave.minutes",
        (ticks / crate::ticks::TICKS_PER_SECOND as u64 / 60).to_string(),
    )]
}

// ---- the save points ------------------------------------------------------------

fn waystones() -> usize {
    crate::travel::spec().points.len()
}

/// How many save points there are: the waystones, then the posts.
pub fn points() -> usize {
    waystones() + spec().posts.len()
}

/// A save point's id in a save: `waystone.<id>` or `post.<id>`.
pub fn point_id(k: usize) -> String {
    let n = waystones();
    if k < n {
        format!("waystone.{}", crate::travel::spec().points[k].id)
    } else {
        format!(
            "post.{}",
            spec().posts.get(k - n).map(|p| p.id.as_str()).unwrap_or("")
        )
    }
}

pub fn point_of(id: &str) -> Option<usize> {
    (0..points()).find(|&k| point_id(k) == id)
}

/// A post's foot and the way it faces.
fn post(w: &World, j: usize) -> Option<(V3, i32)> {
    let p = spec().posts.get(j)?;
    let place = w.land.layout.places.iter().find(|q| q.kind == p.place)?;
    let s = crate::places::spots(w, place)
        .into_iter()
        .find(|s| s.prop == Prop::SavePost)?;
    Some((s.at, s.yaw))
}

/// A save point's foot and the way it faces.
pub fn foot(w: &World, k: usize) -> Option<(V3, i32)> {
    let n = waystones();
    if k < n {
        crate::travel::stone(w, k)
    } else {
        post(w, k - n)
    }
}

/// Where the player stands at save point `k` after a load or a fall: in
/// front of it, facing the way it faces.
pub fn arrival(w: &World, k: usize) -> Option<(V3, i32)> {
    let n = waystones();
    if k < n {
        return crate::travel::arrival(w, k);
    }
    let (at, yaw) = post(w, k - n)?;
    let mut p = at.add(facing(yaw, 0).scale(spec().arrive_cm * ONE / 100));
    p.y = crate::solid::surface(w, p.x, p.z);
    Some((p, yaw))
}

/// The save point nearest the player.
pub fn nearest(w: &World) -> usize {
    let me = w.player.pos;
    (0..points())
        .filter_map(|k| foot(w, k).map(|(at, _)| (k, at.sub(me).len_xz())))
        .min_by_key(|&(_, d)| d)
        .map(|(k, _)| k)
        .unwrap_or(0)
}

/// The save point E would save at, and how far, as story counts it: a woken
/// waystone or a post (a dormant waystone is woken first, `travel`).
pub fn reach(w: &World) -> Option<(usize, Fx)> {
    let me = w.player.pos;
    let n = waystones();
    let edge = ONE / 2 + spec().use_cm * ONE / 100;
    (0..points())
        .filter(|&k| k >= n || w.travel.points.get(k).is_some_and(|p| p.woken))
        .filter_map(|k| foot(w, k).map(|(at, _)| (k, at.sub(me).len_xz())))
        .filter(|&(_, d)| d <= edge)
        .map(|(k, d)| (k, (d - edge + story::REACH).max(0)))
        .min_by_key(|&(_, d)| d)
}

/// The map's marks for the posts (M4's `places::markers`), each keyed by
/// its place, so `place_<place>` names it.
pub fn marks(w: &World) -> Vec<(&'static str, V3)> {
    spec()
        .posts
        .iter()
        .enumerate()
        .filter_map(|(j, p)| post(w, j).map(|(at, _)| (p.place.as_str(), at)))
        .collect()
}

/// The major area the player stands in, by its index in the file.
pub fn area_of(w: &World) -> Option<usize> {
    let me = w.player.pos;
    spec()
        .areas
        .iter()
        .position(|a| crate::places::at(w, &a.place).is_some_and(|c| c.sub(me).len_xz() < a.radius_m * ONE))
}

// ---- stable keys --------------------------------------------------------------

/// The raid's rounds: `raid.<round>`, beaten once every orc on it is down.
pub fn raid_key(route: usize) -> String {
    format!("raid.{route}")
}

/// The camp's people in the order the camp stands them (`camp::spawn`): the
/// lookout, the bandits by the order of `data/story.json`, the leader.
pub fn camp_keys() -> Vec<String> {
    let sp = &story::spec().camp;
    let mut out = vec!["camp.lookout".to_string()];
    out.extend((0..sp.bandits.len()).map(|k| format!("camp.bandit.{k}")));
    out.push("camp.leader".into());
    out
}

/// What each camp member drops, by slot (`camp::drops` lays them in this
/// order): the lookout the gloves and coins, each bandit coins and knives,
/// the leader the sword and coins.
pub const DROPS_EACH: usize = 2;
pub const KEY_DROP: &str = "drop.camp.key";

pub fn drop_key(foe: &str, slot: usize) -> String {
    format!("drop.{foe}.{slot}")
}

/// The roadside's stops (P7, T12) of a kind, by id, in the file's order.
pub fn roadside_stops(kind: crate::roadside::Kind) -> Vec<&'static crate::roadside::StopSpec> {
    crate::roadside::spec()
        .sides
        .iter()
        .flat_map(|s| s.stops.iter())
        .filter(|s| crate::roadside::Kind::named(&s.kind) == Some(kind))
        .collect()
}

/// The roadside's straggler at stop `id`: `roadside.<id>`.
pub fn roadside_key(id: &str) -> String {
    format!("roadside.{id}")
}

/// A foe or encounter a save may name as beaten. A later part adds its
/// keys here (and stands a beaten one down when it stands, as
/// `camp::spawn` and `roadside::step` do with `beaten` and `lay_down`).
pub fn known_foe(key: &str) -> bool {
    camp_keys().iter().any(|k| k == key)
        || (0..story::spec().raid.orcs.len()).any(|r| raid_key(r) == key)
        || roadside_stops(crate::roadside::Kind::Straggler)
            .iter()
            .any(|s| roadside_key(&s.id) == key)
}

/// The story's pickup `k` (by its order in `data/story.json`).
pub fn pickup_key(k: usize) -> String {
    format!("pickup.{k}")
}

/// Something a save may name as taken. A later part adds its keys here,
/// and lays its drops through `drop` with them.
pub fn known_taken(key: &str) -> bool {
    use crate::roadside::Kind as R;
    key == KEY_DROP
        || (0..story::spec().pickups.len()).any(|k| pickup_key(k) == key)
        || camp_keys()
            .iter()
            .any(|f| (0..DROPS_EACH).any(|s| drop_key(f, s) == key))
        || roadside_stops(R::Rocks)
            .iter()
            .any(|s| (0..s.gives.len()).any(|n| drop_key(&roadside_key(&s.id), n) == key))
        || roadside_stops(R::Straggler)
            .iter()
            .any(|s| drop_key(&roadside_key(&s.id), 0) == key)
}

// ---- a part's own progress ----------------------------------------------------

/// A part's progress that is neither a foe beaten nor a thing taken (the
/// roadside's carts searched; P11's rumours heard, quest steps and revealed
/// marks): a set of ids under the part's name in the save's `parts`. A part
/// joins by adding one `Part` to `PARTS`: the ids it may hold, the ids the
/// world holds now, and how a loaded world takes them back. A save without
/// the part has none of its ids, so adding a part needs no new version.
pub struct Part {
    pub name: &'static str,
    pub known: fn(&str) -> bool,
    pub ids: fn(&World) -> BTreeSet<String>,
    pub apply: fn(&mut World, &BTreeSet<String>),
}

pub const PARTS: &[Part] = &[
    Part {
        name: "roadside",
        known: roadside_known,
        ids: roadside_ids,
        apply: roadside_apply,
    },
    // M7 rumours (T17): what was heard, who was met, the quest lines' steps
    // and the places marked
    Part {
        name: "rumours",
        known: crate::rumours::save_known,
        ids: crate::rumours::save_ids,
        apply: crate::rumours::save_apply,
    },
];

/// The roadside's carts searched: `searched.<stop id>`.
fn searched_key(id: &str) -> String {
    format!("searched.{id}")
}

fn roadside_known(id: &str) -> bool {
    crate::roadside::spec()
        .sides
        .iter()
        .flat_map(|s| s.stops.iter())
        .filter(|s| crate::roadside::Kind::named(&s.kind).is_some_and(|k| k.searched()))
        .any(|s| searched_key(&s.id) == id)
}

fn roadside_ids(w: &World) -> BTreeSet<String> {
    w.roadside
        .stops
        .iter()
        .zip(&w.roadside.searched)
        .filter(|(_, on)| **on)
        .map(|((s, _), _)| searched_key(&s.id))
        .collect()
}

fn roadside_apply(w: &mut World, ids: &BTreeSet<String>) {
    for k in 0..w.roadside.stops.len() {
        if ids.contains(&searched_key(&w.roadside.stops[k].0.id)) {
            if let Some(s) = w.roadside.searched.get_mut(k) {
                *s = true;
            }
        }
    }
}

/// Whether a part's ids name a searched cart that gives `item`.
fn searched_gives(parts: &BTreeMap<String, BTreeSet<String>>, item: Item) -> bool {
    let Some(ids) = parts.get("roadside") else {
        return false;
    };
    crate::roadside::spec()
        .sides
        .iter()
        .flat_map(|s| s.stops.iter())
        .filter(|s| ids.contains(&searched_key(&s.id)))
        .any(|s| s.gives.iter().any(|g| g.item.as_deref() == Some(item.id())))
}

/// The story's pickup that gives `item`, if one does.
fn pickup_of(item: Item) -> Option<usize> {
    story::spec()
        .pickups
        .iter()
        .position(|p| p.item.as_deref() == Some(item.id()))
}

// ---- progress -------------------------------------------------------------------

/// A quest flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Flag {
    CampCleared,
    ChestOpened,
    MerchantMet,
    PracticePassed,
    ImbueLearned,
    FluteGiven,
}

impl Flag {
    pub const ALL: [Flag; 6] = [
        Flag::CampCleared,
        Flag::ChestOpened,
        Flag::MerchantMet,
        Flag::PracticePassed,
        Flag::ImbueLearned,
        Flag::FluteGiven,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Flag::CampCleared => "camp_cleared",
            Flag::ChestOpened => "chest_opened",
            Flag::MerchantMet => "merchant_met",
            Flag::PracticePassed => "practice_passed",
            Flag::ImbueLearned => "imbue_learned",
            Flag::FluteGiven => "flute_given",
        }
    }

    pub fn from_id(id: &str) -> Option<Flag> {
        Flag::ALL.iter().copied().find(|f| f.id() == id)
    }
}

/// The settings a save keeps: the reader's settings (`ui::Settings`) and
/// the look speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kept {
    pub settings: crate::ui::Settings,
    pub look_pct: i32,
}

const N: usize = Item::ALL.len();

/// What a save holds: progress, decided here (CW-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub seed: u32,
    pub playtime: u64,
    pub point: usize,
    pub count: [u32; N],
    pub weapon: Option<Item>,
    pub worn: [Option<Item>; 5],
    pub quick: [Option<Item>; 3],
    pub coins: u32,
    pub stock: [u32; N],
    pub flags: BTreeSet<Flag>,
    pub woken: Vec<bool>,
    pub reached: Vec<bool>,
    pub taken: BTreeSet<String>,
    pub beaten: BTreeSet<String>,
    pub kept: Kept,
    pub map: Vec<u64>,
    /// each part's own ids, by part (`PARTS`); a part with none is absent
    pub parts: BTreeMap<String, BTreeSet<String>>,
}

/// Who and what is beaten in `w`: the save's own, then each camp member
/// down and each raid round whose every orc is down.
fn beaten_in(w: &World) -> BTreeSet<String> {
    let mut out = w.save.beaten.clone();
    let keys = camp_keys();
    for (k, m) in w.camp.members.iter().enumerate() {
        if w.foes.get(m.foe).is_some_and(|f| f.fighter.health.down()) {
            if let Some(key) = keys.get(k) {
                out.insert(key.clone());
            }
        }
    }
    if let Some(s) = w.roadside.straggler {
        if w.foes.get(s.foe).is_some_and(|f| f.fighter.health.down()) {
            if let Some((stop, _)) = w.roadside.stops.get(s.stop) {
                out.insert(roadside_key(&stop.id));
            }
        }
    }
    if !w.story.opening() {
        for r in 0..story::spec().raid.orcs.len() {
            let mut on = w.story.raid.iter().filter(|x| x.route == r).peekable();
            if on.peek().is_some() && on.all(|x| w.foes.get(x.foe).is_none_or(|f| f.fighter.health.down())) {
                out.insert(raid_key(r));
            }
        }
    }
    out
}

/// What is taken in `w`: the save's own, the story's pickups taken, and each
/// keyed drop taken.
fn taken_in(w: &World) -> BTreeSet<String> {
    let mut out = w.save.taken.clone();
    let n = story::spec().pickups.len();
    for (k, p) in w.story.pickups.iter().enumerate().take(n) {
        if p.taken {
            out.insert(pickup_key(k));
        }
    }
    for (k, key) in &w.save.keyed {
        if w.story.pickups.get(*k).is_some_and(|p| p.taken) {
            out.insert(key.clone());
        }
    }
    out
}

/// The progress of the world as it stands.
pub fn progress(w: &World) -> Progress {
    let inv = &w.inventory;
    let beaten = beaten_in(w);
    let mut flags = BTreeSet::new();
    if camp_keys().iter().all(|k| beaten.contains(k)) {
        flags.insert(Flag::CampCleared);
    }
    if w.camp.open {
        flags.insert(Flag::ChestOpened);
    }
    if w.merchant.hits > 0 {
        flags.insert(Flag::MerchantMet);
    }
    if w.trainer.taught > 0 {
        flags.insert(Flag::PracticePassed);
    }
    if inv.knows_imbue {
        flags.insert(Flag::ImbueLearned);
    }
    if w.travel.given {
        flags.insert(Flag::FluteGiven);
    }
    Progress {
        seed: w.story.seed as u32,
        playtime: w.save.playtime,
        point: w.save.point.unwrap_or_else(|| nearest(w)),
        count: inv.count,
        weapon: inv.weapon,
        worn: inv.worn,
        quick: inv.quick,
        coins: inv.coins,
        stock: w.ui.stock,
        flags,
        woken: w.travel.points.iter().map(|p| p.woken).collect(),
        reached: w.travel.points.iter().map(|p| p.reached).collect(),
        taken: taken_in(w),
        beaten,
        kept: Kept {
            settings: w.ui.settings,
            look_pct: w.aim.pct,
        },
        map: w.ui.fog.bits().to_vec(),
        parts: PARTS
            .iter()
            .map(|p| (p.name.to_string(), (p.ids)(w)))
            .filter(|(_, ids)| !ids.is_empty())
            .collect(),
    }
}

// ---- writing --------------------------------------------------------------------

/// FNV-1a, 64 bits: the checksum over the game object's compact text.
pub fn checksum(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn slot_of(k: usize) -> Slot {
    Slot::ALL[k]
}

/// The `game` object of a save. Its keys are exactly `GAME_KEYS`.
pub fn game_value(p: &Progress) -> Value {
    let items: Map<String, Value> = Item::ALL
        .iter()
        .filter(|i| p.count[**i as usize] > 0)
        .map(|i| (i.id().to_string(), json!(p.count[*i as usize])))
        .collect();
    let stock: Map<String, Value> = Item::ALL
        .iter()
        .filter(|i| p.stock[**i as usize] > 0)
        .map(|i| (i.id().to_string(), json!(p.stock[*i as usize])))
        .collect();
    let worn: Map<String, Value> = p
        .worn
        .iter()
        .enumerate()
        .filter_map(|(k, i)| i.map(|i| (slot_of(k).id().to_string(), json!(i.id()))))
        .collect();
    let ids = |list: &[bool]| -> Vec<&str> {
        list.iter()
            .enumerate()
            .filter(|(_, on)| **on)
            .map(|(k, _)| crate::travel::spec().points[k].id.as_str())
            .collect()
    };
    let set = &p.kept.settings;
    json!({
        "beaten": p.beaten.iter().map(|k| (k.clone(), json!(true))).collect::<Map<String, Value>>(),
        "coins": p.coins,
        "flags": p.flags.iter().map(|f| f.id()).collect::<Vec<_>>(),
        "items": items,
        "map": p.map.iter().map(|w| format!("{w:016x}")).collect::<String>(),
        "parts": p.parts.iter().map(|(k, ids)| (k.clone(), json!(ids))).collect::<Map<String, Value>>(),
        "phase": "free",
        "playtime": p.playtime,
        "point": point_id(p.point),
        "quick": p.quick.iter().map(|q| q.map(|i| i.id())).collect::<Vec<_>>(),
        "reached": ids(&p.reached),
        "seed": p.seed,
        "settings": {
            "blood": set.blood,
            "cue": set.cue,
            "hand_pct": set.hand_pct,
            "invert_y": set.invert_y,
            "look_pct": p.kept.look_pct,
            "volume_pct": set.volume_pct,
        },
        "stock": stock,
        "taken": p.taken.iter().collect::<Vec<_>>(),
        "weapon": p.weapon.map(|i| i.id()),
        "woken": ids(&p.woken),
        "worn": worn,
    })
}

/// The keys of a save's `game` object: what a save holds, and nothing else
/// (no name, no date or clock reading, nothing about the device).
pub const GAME_KEYS: [&str; 18] = [
    "beaten", "coins", "flags", "items", "map", "parts", "phase", "playtime", "point", "quick", "reached",
    "seed", "settings", "stock", "taken", "weapon", "woken", "worn",
];
pub const SETTING_KEYS: [&str; 6] = ["blood", "cue", "hand_pct", "invert_y", "look_pct", "volume_pct"];
pub const TOP_KEYS: [&str; 4] = ["checksum", "format", "game", "version"];

/// A save's text: the format, the version, the checksum, the game.
pub fn text(p: &Progress) -> String {
    let game = game_value(p);
    let sum = checksum(&game.to_string());
    let top = json!({
        "format": spec().format,
        "version": spec().version,
        "checksum": sum,
        "game": game,
    });
    serde_json::to_string_pretty(&top).unwrap_or_default()
}

/// The last save's text, for the page to store or download; empty before
/// the first save.
pub fn text_of(w: &World) -> String {
    w.save.last.as_ref().map(text).unwrap_or_default()
}

// ---- reading --------------------------------------------------------------------

/// Why a save was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// not JSON, empty, or far too long
    Unreadable,
    /// JSON, but not this game's save
    Format,
    /// this game's save, of another version
    Version,
    /// changed since it was written
    Checksum,
    /// a field missing, of the wrong type, out of range or unknown
    Field(String),
    /// an item held without the way to get it (CW-6)
    Route(Item),
}

impl Refusal {
    /// The reason, as the refusal line's `{why}`.
    pub fn why(&self, s: &Strings) -> String {
        match self {
            Refusal::Unreadable => s.get("save_why_unreadable"),
            Refusal::Format => s.get("save_why_format"),
            Refusal::Version => s.get("save_why_version"),
            Refusal::Checksum => s.get("save_why_checksum"),
            Refusal::Field(f) => fill(&s.get("save_why_field"), &[("field", f.clone())]),
            Refusal::Route(i) => fill(
                &s.get("save_why_route"),
                &[("item", s.get(&format!("item_{}", i.id())))],
            ),
        }
    }
}

fn bad(field: &str) -> Refusal {
    Refusal::Field(field.into())
}

fn uint(v: &Value, field: &str, most: u64) -> Result<u64, Refusal> {
    v.as_u64().filter(|n| *n <= most).ok_or_else(|| bad(field))
}

fn flag(v: &Value, field: &str) -> Result<bool, Refusal> {
    v.as_bool().ok_or_else(|| bad(field))
}

fn item(v: &Value, field: &str) -> Result<Item, Refusal> {
    v.as_str().and_then(Item::from_id).ok_or_else(|| bad(field))
}

fn object<'a>(v: &'a Value, field: &str) -> Result<&'a Map<String, Value>, Refusal> {
    v.as_object().ok_or_else(|| bad(field))
}

fn strings<'a>(v: &'a Value, field: &str) -> Result<Vec<&'a str>, Refusal> {
    v.as_array()
        .ok_or_else(|| bad(field))?
        .iter()
        .map(|x| x.as_str().ok_or_else(|| bad(field)))
        .collect()
}

/// Exactly these keys, each once (a JSON object holds a key once).
fn keys_are(m: &Map<String, Value>, want: &[&str], field: &str) -> Result<(), Refusal> {
    for k in m.keys() {
        if !want.contains(&k.as_str()) {
            return Err(bad(&format!("{field}{k}")));
        }
    }
    for k in want {
        if !m.contains_key(*k) {
            return Err(bad(&format!("{field}{k}")));
        }
    }
    Ok(())
}

/// A list of distinct ids, each known.
fn id_set(v: &Value, field: &str, known: impl Fn(&str) -> bool) -> Result<BTreeSet<String>, Refusal> {
    let list = strings(v, field)?;
    let set: BTreeSet<String> = list.iter().map(|s| s.to_string()).collect();
    if set.len() != list.len() || list.iter().any(|s| !known(s)) {
        return Err(bad(field));
    }
    Ok(set)
}

/// The waystones a list names, by point.
fn stones(v: &Value, field: &str) -> Result<Vec<bool>, Refusal> {
    let pts = &crate::travel::spec().points;
    let set = id_set(v, field, |id| pts.iter().any(|p| p.id == id))?;
    Ok(pts.iter().map(|p| set.contains(&p.id)).collect())
}

/// Item counts by id, each known and within its stack.
fn counts(v: &Value, field: &str) -> Result<[u32; N], Refusal> {
    let mut out = [0; N];
    for (id, n) in object(v, field)? {
        let f = format!("{field}.{id}");
        let i = Item::from_id(id).ok_or_else(|| bad(&f))?;
        let n = uint(n, &f, i.def().stack as u64)?;
        if n == 0 {
            return Err(bad(&f));
        }
        out[i as usize] = n as u32;
    }
    Ok(out)
}

/// Reads a save's text, checking everything; the progress, or why not.
pub fn read(text: &str) -> Result<Progress, Refusal> {
    let sp = spec();
    if text.trim().is_empty() || text.len() > sp.text_most {
        return Err(Refusal::Unreadable);
    }
    let top: Value = serde_json::from_str(text).map_err(|_| Refusal::Unreadable)?;
    let top = top.as_object().ok_or(Refusal::Format)?;
    if top.get("format").and_then(|f| f.as_str()) != Some(sp.format.as_str()) {
        return Err(Refusal::Format);
    }
    if top.get("version").and_then(|v| v.as_u64()) != Some(sp.version) {
        return Err(Refusal::Version);
    }
    keys_are(top, &TOP_KEYS, "")?;
    let game = &top["game"];
    let sum = top["checksum"].as_str().ok_or_else(|| bad("checksum"))?;
    if sum != checksum(&game.to_string()) {
        return Err(Refusal::Checksum);
    }
    let g = object(game, "game")?;
    keys_are(g, &GAME_KEYS, "")?;
    if g["phase"].as_str() != Some("free") {
        return Err(bad("phase"));
    }
    let seed = uint(&g["seed"], "seed", u32::MAX as u64)? as u32;
    let playtime = uint(&g["playtime"], "playtime", sp.playtime_most)?;
    let point = g["point"]
        .as_str()
        .and_then(point_of)
        .ok_or_else(|| bad("point"))?;
    let coins = uint(&g["coins"], "coins", sp.coins_most as u64)? as u32;
    let count = counts(&g["items"], "items")?;
    let stock = counts(&g["stock"], "stock")?;
    let weapon = match &g["weapon"] {
        Value::Null => None,
        v => Some(item(v, "weapon")?),
    };
    let mut worn = [None; 5];
    for (slot, v) in object(&g["worn"], "worn")? {
        let f = format!("worn.{slot}");
        let k = Slot::ALL
            .iter()
            .position(|s| s.id() == slot)
            .ok_or_else(|| bad(&f))?;
        worn[k] = Some(item(v, &f)?);
    }
    let q = g["quick"]
        .as_array()
        .filter(|a| a.len() == 3)
        .ok_or_else(|| bad("quick"))?;
    let mut quick = [None; 3];
    for (k, v) in q.iter().enumerate() {
        quick[k] = match v {
            Value::Null => None,
            v => Some(item(v, "quick")?),
        };
    }
    let flags: BTreeSet<Flag> = id_set(&g["flags"], "flags", |id| Flag::from_id(id).is_some())?
        .iter()
        .filter_map(|id| Flag::from_id(id))
        .collect();
    let woken = stones(&g["woken"], "woken")?;
    let reached = stones(&g["reached"], "reached")?;
    let taken = id_set(&g["taken"], "taken", known_taken)?;
    let mut beaten = BTreeSet::new();
    for (k, v) in object(&g["beaten"], "beaten")? {
        if !known_foe(k) || v.as_bool() != Some(true) {
            return Err(bad(&format!("beaten.{k}")));
        }
        beaten.insert(k.clone());
    }
    let s = object(&g["settings"], "settings")?;
    keys_are(s, &SETTING_KEYS, "settings.")?;
    let range = |key: &str, (lo, hi, _): (i32, i32, i32)| -> Result<i32, Refusal> {
        let f = format!("settings.{key}");
        s[key]
            .as_i64()
            .filter(|n| (lo as i64..=hi as i64).contains(n))
            .map(|n| n as i32)
            .ok_or_else(|| bad(&f))
    };
    let kept = Kept {
        settings: crate::ui::Settings {
            invert_y: flag(&s["invert_y"], "settings.invert_y")?,
            hand_pct: range("hand_pct", crate::ui::HAND_RANGE)?,
            volume_pct: range("volume_pct", crate::ui::VOLUME_RANGE)?,
            cue: flag(&s["cue"], "settings.cue")?,
            blood: flag(&s["blood"], "settings.blood")?,
        },
        look_pct: range(
            "look_pct",
            (crate::camera::LOOK_PCT_MIN, crate::camera::LOOK_PCT_MAX, 1),
        )?,
    };
    let map = map_bits(&g["map"])?;
    let mut parts = BTreeMap::new();
    for (name, ids) in object(&g["parts"], "parts")? {
        let f = format!("parts.{name}");
        let part = PARTS.iter().find(|p| p.name == name).ok_or_else(|| bad(&f))?;
        let set = id_set(ids, &f, part.known)?;
        if set.is_empty() {
            return Err(bad(&f));
        }
        parts.insert(name.clone(), set);
    }
    let p = Progress {
        seed,
        playtime,
        point,
        count,
        weapon,
        worn,
        quick,
        coins,
        stock,
        flags,
        woken,
        reached,
        taken,
        beaten,
        kept,
        map,
        parts,
    };
    hands(&p)?;
    routes(&p)?;
    Ok(p)
}

/// The walked map: one hex word of 16 digits for each 64 cells, the last
/// word's cells past the map's edge clear.
fn map_bits(v: &Value) -> Result<Vec<u64>, Refusal> {
    let words = crate::ui::Fog::words();
    let hex = v
        .as_str()
        .filter(|h| h.len() == words * 16)
        .ok_or_else(|| bad("map"))?;
    if !hex
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(bad("map"));
    }
    let out: Vec<u64> = (0..words)
        .map(|k| u64::from_str_radix(&hex[k * 16..k * 16 + 16], 16).unwrap_or(u64::MAX))
        .collect();
    let used = crate::ui::Fog::cells() % 64;
    if used > 0 && out[words - 1] >> used != 0 {
        return Err(bad("map"));
    }
    Ok(out)
}

/// The hands, the slots and the keys hold only what is carried and fits.
fn hands(p: &Progress) -> Result<(), Refusal> {
    let has = |i: Item| p.count[i as usize] > 0;
    if let Some(i) = p.weapon {
        let d = i.def();
        if !has(i) || d.kind != Kind::Weapon || d.hand != Some(Hand::Sword) {
            return Err(bad("weapon"));
        }
    }
    for (k, w) in p.worn.iter().enumerate() {
        if let Some(i) = w {
            if !has(*i) || i.def().slot != Some(slot_of(k)) {
                return Err(bad(&format!("worn.{}", slot_of(k).id())));
            }
        }
    }
    let mut seen = Vec::new();
    for i in p.quick.iter().flatten() {
        if !has(*i) || !i.def().quick || seen.contains(i) {
            return Err(bad("quick"));
        }
        seen.push(*i);
    }
    for i in Item::ALL {
        let d = i.def();
        if p.stock[i as usize] > 0 && (d.stack != 1 || d.keep) {
            return Err(bad(&format!("stock.{}", i.id())));
        }
    }
    if p.woken.iter().zip(&p.reached).any(|(w, r)| *w && !*r) {
        return Err(bad("woken"));
    }
    Ok(())
}

/// CW-6: nothing is held (or in the merchant's stock) without the route
/// that gives it, and the flags agree with each other and with the keys.
fn routes(p: &Progress) -> Result<(), Refusal> {
    let has = |i: Item| p.count[i as usize] > 0 || p.stock[i as usize] > 0;
    let took = |k: &str| p.taken.contains(k);
    let on = |f: Flag| p.flags.contains(&f);
    let camp = camp_keys();
    let leader = camp.last().cloned().unwrap_or_default();
    let bandits: Vec<&String> = camp.iter().filter(|k| k.starts_with("camp.bandit.")).collect();
    // one of each one-of-a-kind item, wherever it is
    for i in Item::ALL {
        if i.def().stack == 1 && p.count[i as usize] + p.stock[i as usize] > 1 {
            return Err(Refusal::Route(i));
        }
    }
    let route_ok = |i: Item| -> bool {
        match i {
            Item::ShortSword | Item::Rock => true,
            Item::Sword => took(&drop_key(&leader, 0)),
            Item::Boomerang => on(Flag::ChestOpened),
            Item::ChestKey => took(KEY_DROP) && !on(Flag::ChestOpened),
            Item::Hood => on(Flag::MerchantMet) || p.count[i as usize] == 0,
            Item::Jacket | Item::Trousers | Item::Boots => pickup_of(i).is_some_and(|k| took(&pickup_key(k))),
            Item::Gloves => took(&drop_key("camp.lookout", 0)),
            Item::Flute => on(Flag::FluteGiven),
            Item::Knife => {
                on(Flag::MerchantMet)
                    || bandits.iter().any(|b| took(&drop_key(b, 1)))
                    || searched_gives(&p.parts, i)
            }
            Item::Bandage => {
                on(Flag::MerchantMet)
                    || pickup_of(i).is_some_and(|k| took(&pickup_key(k)))
                    || searched_gives(&p.parts, i)
            }
        }
    };
    for i in Item::ALL {
        if has(i) && !route_ok(i) {
            return Err(Refusal::Route(i));
        }
    }
    // the hood is the merchant's until bought: in the stock or carried
    if p.count[Item::Hood as usize] + p.stock[Item::Hood as usize] != 1 {
        return Err(Refusal::Route(Item::Hood));
    }
    // a stocked item other than the hood was sold to the merchant
    if Item::ALL
        .iter()
        .any(|i| *i != Item::Hood && p.stock[*i as usize] > 0 && !on(Flag::MerchantMet))
    {
        return Err(bad("stock"));
    }
    // the key, once taken, is carried until it opens the chest
    if took(KEY_DROP) && !on(Flag::ChestOpened) && p.count[Item::ChestKey as usize] == 0 {
        return Err(Refusal::Route(Item::ChestKey));
    }
    // the flute is the trainer's gift and cannot be sold
    if on(Flag::FluteGiven) != (p.count[Item::Flute as usize] > 0) {
        return Err(Refusal::Route(Item::Flute));
    }
    let flags_ok = on(Flag::ImbueLearned) == on(Flag::PracticePassed)
        && (!on(Flag::FluteGiven) || on(Flag::PracticePassed))
        && (!on(Flag::ChestOpened) || took(KEY_DROP))
        && on(Flag::CampCleared) == camp.iter().all(|k| p.beaten.contains(k));
    if !flags_ok {
        return Err(bad("flags"));
    }
    Ok(())
}

// ---- loading --------------------------------------------------------------------

/// The world a save gives: the opening from its seed, the opening ended,
/// the progress applied, the player at the save point.
pub fn world(p: &Progress) -> World {
    let mut w = story::opening(p.seed as u64);
    let on = |f: Flag| p.flags.contains(&f);
    // the opening is over: the villagers have fled, the orcs keep the
    // village, those beaten lie where they stood
    w.story.phase = Phase::Free;
    w.story.lift = 0;
    for v in &mut w.story.villagers {
        v.gone = true;
    }
    for k in 0..w.story.raid.len() {
        let r = w.story.raid[k].clone();
        if p.beaten.contains(&raid_key(r.route)) {
            lay_down(&mut w, r.foe);
        }
    }
    for (k, pk) in w.story.pickups.iter_mut().enumerate() {
        if p.taken.contains(&pickup_key(k)) {
            pk.taken = true;
        }
    }
    // the drops the opening lays by key (the tor's rocks)
    let keyed = std::mem::take(&mut w.save.keyed);
    for (k, key) in &keyed {
        if p.taken.contains(key) {
            if let Some(pk) = w.story.pickups.get_mut(*k) {
                pk.taken = true;
            }
        }
    }
    for part in PARTS {
        if let Some(ids) = p.parts.get(part.name) {
            (part.apply)(&mut w, ids);
        }
    }
    // what the player carries
    let inv = &mut w.inventory;
    inv.count = p.count;
    inv.weapon = p.weapon;
    inv.worn = p.worn;
    inv.quick = p.quick;
    inv.ready = None;
    inv.coins = p.coins;
    inv.knows_imbue = on(Flag::ImbueLearned);
    w.ui.stock = p.stock;
    w.ui.settings = p.kept.settings;
    w.ui.fog = crate::ui::Fog::from_bits(p.map.clone());
    w.aim.pct = p.kept.look_pct;
    // the quest
    if on(Flag::MerchantMet) {
        w.merchant.hits = w.merchant.hits.max(1);
    }
    if on(Flag::PracticePassed) {
        w.trainer.stage = crate::trainer::Stage::Done;
        w.trainer.taught = 1;
    }
    w.travel.given = on(Flag::FluteGiven);
    for (k, pt) in w.travel.points.iter_mut().enumerate() {
        pt.woken = p.woken.get(k).copied().unwrap_or(false);
        pt.reached = p.reached.get(k).copied().unwrap_or(false);
    }
    if on(Flag::ChestOpened) {
        w.camp.open = true;
        w.camp.lid = crate::camp::LID_TICKS;
    }
    // the save's own record
    w.save = State {
        on: true,
        last: Some(p.clone()),
        point: Some(p.point),
        playtime: p.playtime,
        taken: p.taken.clone(),
        beaten: p.beaten.clone(),
        keyed,
        ..State::default()
    };
    stand_at(&mut w, p.point);
    w
}

/// A foe of the story lies where it stands, down (a beaten foe in a loaded
/// world).
pub fn lay_down(w: &mut World, foe: usize) {
    if let Some(f) = w.foes.get_mut(foe) {
        f.fighter.health.hp = 0;
        f.fighter.health.down_ticks = crate::health::FALL_TICKS;
    }
}

/// The player stands at save point `k`'s arrival, a fresh body and fighter
/// there, facing out.
fn stand_at(w: &mut World, k: usize) {
    let Some((at, yaw)) = arrival(w, k) else {
        return;
    };
    w.player = crate::player::Body::new(at, yaw);
    let held = crate::items::held(w);
    w.me = crate::player::Fighter::new(crate::pose::Kind::Player, held, 0, at, yaw, false);
    w.aim.yaw = yaw;
    w.aim.pitch = 0;
    w.aim.rest = (0, 0);
    w.story.crossed = Some(at);
}

/// Whether a camp member (by its order, `camp_keys`) was beaten in the save
/// this world came from: the camp stands it down (`camp::spawn`).
pub fn beaten(w: &World, key: &str) -> bool {
    w.save.beaten.contains(key)
}

/// Whether the whole camp was beaten in the save this world came from: the
/// camp's line is not said again.
pub fn was_cleared(w: &World) -> bool {
    camp_keys().iter().all(|k| w.save.beaten.contains(k))
}

/// A drop by stable key: it lies for E to take, unless it was taken
/// already (in this world or the save it came from).
pub fn drop(w: &mut World, key: &str, gift: Gift, at: V3) {
    if w.save.taken.contains(key) || w.save.keyed.iter().any(|(_, k)| k == key) {
        return;
    }
    story::drop(w, gift, at);
    let k = w.story.pickups.len() - 1;
    w.save.keyed.push((k, key.to_string()));
}

// ---- state ------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    /// the game saves itself: from the end of the opening, or a load
    pub on: bool,
    /// the last save written or loaded: a fall returns to it
    pub last: Option<Progress>,
    /// saves written, so the page stores each once
    pub seq: u32,
    /// a save is due, and waits until one may be written
    pub due: bool,
    /// the save point a load stands the player at
    pub point: Option<usize>,
    /// ticks of play outside a duel since the last save
    pub since: u32,
    /// what the autosave watches: set on the first tick it looks
    pub placed: bool,
    pub area: Option<usize>,
    pub task: &'static str,
    pub woken: Vec<bool>,
    /// ticks the Saved mark has left
    pub mark: u32,
    /// ticks played, carried from the save this world came from
    pub playtime: u64,
    /// taken and beaten in the save this world came from
    pub taken: BTreeSet<String>,
    pub beaten: BTreeSet<String>,
    /// drops laid by stable key: the pickup's index and its key
    pub keyed: Vec<(usize, String)>,
}

/// Why a save may not be written now, as a string key.
pub fn refusal(w: &World) -> Option<&'static str> {
    if w.story.phase != Phase::Free {
        return Some(if w.story.opening() || w.story.phase == Phase::Off {
            "save_refuse_opening"
        } else {
            "save_refuse_busy"
        });
    }
    if w.ui.dialogue.is_some() || w.ui.shop_open {
        return Some("save_refuse_talk");
    }
    if crate::travel::danger(w) {
        return Some("save_refuse_duel");
    }
    if w.travel.warp.is_some() || w.player_down() || w.story.veil {
        return Some("save_refuse_busy");
    }
    None
}

/// Writes a save of the world as it stands: its progress becomes the last
/// save and the page is told to store it. A save the core would itself
/// refuse (a world staged by test acts) is not written.
pub fn write(w: &mut World) -> bool {
    let p = progress(w);
    if read(&text(&p)).is_err() {
        return false;
    }
    let st = &mut w.save;
    st.point = Some(p.point);
    st.last = Some(p);
    st.seq = st.seq.wrapping_add(1);
    st.due = false;
    st.since = 0;
    st.mark = spec().mark_ticks;
    true
}

/// The opening ended at the boundary (`story::boundary`): the game saves
/// itself from now on, first at the save point nearest the crossing.
pub fn crossed(w: &mut World) {
    w.save.on = true;
    w.save.point = Some(nearest(w));
    w.save.due = true;
}

/// Step 7, after travel: the autosave's watch, and a save that is due.
pub fn step(w: &mut World) {
    w.save.playtime = w.save.playtime.saturating_add(1);
    w.save.mark = w.save.mark.saturating_sub(1);
    if !w.save.on || w.story.phase != Phase::Free {
        return;
    }
    let area = area_of(w);
    let task = story::task(w);
    let woken: Vec<bool> = w.travel.points.iter().map(|p| p.woken).collect();
    let st = &mut w.save;
    if !st.placed {
        st.placed = true;
        st.area = area;
        st.task = task;
        st.woken = woken;
    } else {
        if area != st.area || task != st.task {
            st.due = true;
        }
        if let Some(k) = (0..woken.len()).find(|&k| woken[k] && !st.woken.get(k).copied().unwrap_or(false)) {
            st.point = Some(k);
            st.due = true;
        }
        st.area = area;
        st.task = task;
        st.woken = woken;
    }
    if !crate::travel::danger(w) {
        w.save.since = w.save.since.saturating_add(1);
    }
    if w.save.since >= spec().autosave_ticks {
        w.save.due = true;
    }
    if w.save.due && refusal(w).is_none() {
        write(w);
    }
}

/// E at save point `k`: it becomes the save point and the game is saved,
/// with a line; or a line says why not.
pub fn use_point(w: &mut World, k: usize) {
    if let Some(r) = refusal(w) {
        story::banner(w, r);
        return;
    }
    let before = w.save.point;
    w.save.point = Some(k);
    if write(w) {
        w.save.on = true;
        story::banner(w, "saved");
    } else {
        w.save.point = before;
        story::banner(w, "save_refuse_staged");
    }
}

/// After the opening, down (`story::come_to`): the player comes to at the
/// last save point with everything they had, and the foes beaten since the
/// save stand again. Whether there was a save to come to.
pub fn fall(w: &mut World) -> bool {
    let Some(last) = w.save.last.clone() else {
        return false;
    };
    let mut p = progress(w);
    p.point = last.point;
    p.beaten = last.beaten.clone();
    p.flags.remove(&Flag::CampCleared);
    if last.flags.contains(&Flag::CampCleared) {
        p.flags.insert(Flag::CampCleared);
    }
    if read(&text(&p)).is_err() {
        p = last;
    }
    let seq = w.save.seq;
    *w = world(&p);
    w.save.seq = seq;
    write(w);
    story::banner(w, "save_came_to");
    true
}

// ---- screen ---------------------------------------------------------------------

/// The small Saved mark over the game.
pub fn hud(w: &World, s: &Strings, out: &mut Vec<HudItem>) {
    out.push(HudItem {
        id: "saved".into(),
        text: s.get("saved_mark"),
        shown: w.save.mark > 0,
    });
}

/// One value for what this module shows, so the page reads the view model
/// again when a save is written or the mark goes (`Game::advance`).
pub fn stamp(w: &World) -> (u32, bool) {
    (w.save.seq, w.save.mark > 0)
}

/// The page's side of saving: a text kept in the browser at the start (if
/// it was a save the core reads), the refusal or confirmation line, the
/// new-game confirm step, and the counts the page acts on once each.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Shelf {
    pub stored: Option<Progress>,
    pub note: Option<String>,
    pub confirm: bool,
    pub export: u32,
    pub wipe: u32,
}

/// What the page needs to store, download or forget a save, in the view
/// model (`ui().save`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct View {
    /// saves written: the page stores `save_text()` when it changes
    pub seq: u32,
    /// downloads asked for: the page offers `save_text()` as `file_name`
    pub export: u32,
    pub file_name: String,
    /// new games confirmed: the page forgets its save when it changes
    pub wipe: u32,
    /// the action whose button asks for a file and sends its text
    pub file_action: String,
}

pub fn view(g: &Game) -> View {
    View {
        seq: g.world.save.seq,
        export: g.saves.export,
        file_name: g.strings.get("save_file_name"),
        wipe: g.saves.wipe,
        file_action: "save.load".into(),
    }
}

fn button(s: &Strings, name: &str, label: &str, engage: bool) -> Action {
    Action {
        name: name.into(),
        value: String::new(),
        label: s.get(label),
        enabled: true,
        engage,
    }
}

/// The title's first button: Continue when the browser holds a save.
pub fn title_button(g: &Game) -> Option<Action> {
    g.saves
        .stored
        .as_ref()
        .map(|_| button(&g.strings, "save.continue", "continue", true))
}

/// The save's panels on the title (New game, Load save file, the confirm
/// step) and on the pause screen (Download save, Load save file).
pub fn panels(g: &Game, out: &mut Vec<Panel>) {
    let s = &g.strings;
    let (id, actions, text) = match g.screen {
        Screen::Title if g.saves.confirm => (
            "title-save",
            vec![
                button(s, "save.new_yes", "new_game_yes", true),
                button(s, "save.back", "back", false),
            ],
            s.get("new_game_warn"),
        ),
        Screen::Title => {
            let mut a = Vec::new();
            if g.saves.stored.is_some() {
                a.push(button(s, "save.new", "new_game", false));
            }
            a.push(button(s, "save.load", "load_file", false));
            ("title-save", a, g.saves.note.clone().unwrap_or_default())
        }
        Screen::Paused => (
            "paused-save",
            vec![
                button(s, "save.export", "download_save", false),
                button(s, "save.load", "load_file", false),
            ],
            g.saves.note.clone().unwrap_or_default(),
        ),
        _ => return,
    };
    out.push(Panel {
        id: id.into(),
        heading: String::new(),
        text,
        rows_heading: String::new(),
        rows: Vec::new(),
        actions,
    });
}

/// `save.<name>` (`Game::act`): `stored` takes the browser's text at the
/// start; `continue` plays it; `new` asks first when there is a save, and
/// `new_yes` forgets it and plays the opening; `back` leaves the confirm
/// step; `load` takes a file's text; `export` asks the page to download the
/// last save; `now` (test-only) saves at once where saving is allowed.
pub fn act(g: &mut Game, name: &str, value: &str) -> Result<(), String> {
    let s = g.strings.clone();
    match name {
        "save.stored" => {
            if value.trim().is_empty() {
                return Ok(());
            }
            match read(value) {
                Ok(p) => g.saves.stored = Some(p),
                Err(r) => {
                    g.saves.stored = None;
                    g.saves.note = Some(fill(&s.get("save_refused_start"), &[("why", r.why(&s))]));
                }
            }
        }
        "save.continue" => {
            let Some(p) = g.saves.stored.clone() else {
                return Err(s.get("save_none_stored"));
            };
            let seq = g.world.save.seq;
            g.world = world(&p);
            g.world.save.seq = seq;
            g.saves.note = None;
            let aim = g.world.aim;
            g.show(Screen::Play);
            g.world.aim = aim;
        }
        "save.new" => {
            if g.saves.stored.is_some() {
                g.saves.confirm = true;
            } else {
                g.show(Screen::Play);
            }
        }
        "save.new_yes" => {
            g.saves.stored = None;
            g.saves.confirm = false;
            g.saves.note = None;
            g.saves.wipe = g.saves.wipe.wrapping_add(1);
            g.show(Screen::Play);
        }
        "save.back" => g.saves.confirm = false,
        "save.load" => match read(value) {
            Ok(p) => {
                let seq = g.world.save.seq;
                let aim_pct = p.kept.look_pct;
                g.world = world(&p);
                // the loaded save is the browser's save from now on
                g.world.save.seq = seq.wrapping_add(1);
                g.world.aim.pct = aim_pct;
                g.saves.stored = Some(p);
                g.saves.confirm = false;
                g.saves.note = Some(s.get(if g.screen == Screen::Title {
                    "save_loaded_title"
                } else {
                    "save_loaded"
                }));
            }
            Err(r) => {
                g.saves.note = Some(fill(&s.get("save_refused_file"), &[("why", r.why(&s))]));
            }
        },
        "save.export" => {
            if g.world.save.last.is_none() {
                return Err(s.get("save_none"));
            }
            g.saves.export = g.saves.export.wrapping_add(1);
        }
        "save.now" => {
            if let Some(r) = refusal(&g.world) {
                return Err(s.get(r));
            }
            g.world.save.on = true;
            if !write(&mut g.world) {
                return Err(s.get("save_refuse_staged"));
            }
        }
        _ => return Err(format!("Unknown action \"{name}\".")),
    }
    g.ui_version += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;

    /// A bad save: what is wrong, the edit that makes it, the refusal.
    type Case = (
        &'static str,
        Box<dyn Fn(&mut Map<String, Value>)>,
        Result<Progress, Refusal>,
    );

    fn idle(w: &mut World, n: u32) {
        for _ in 0..n {
            w.step(&Input::default());
        }
    }

    /// A world past the opening, standing by the village's waystone, the
    /// autosave on and saved once.
    fn saved() -> World {
        let mut w = story::opening(9);
        story::step_aside(&mut w);
        stand_at(&mut w, 0);
        w.save.on = true;
        assert!(write(&mut w), "a plain world past the opening saves");
        w
    }

    /// Some progress: the jacket and the boots taken from where they lie,
    /// coins, the trainer's practice passed with the flute, a waystone
    /// woken, the lookout beaten and its gloves worn.
    fn some_progress() -> Progress {
        let mut p = progress(&saved());
        let jacket = pickup_of(Item::Jacket).unwrap();
        let boots = pickup_of(Item::Boots).unwrap();
        p.taken.insert(pickup_key(jacket));
        p.taken.insert(pickup_key(boots));
        p.taken.insert(drop_key("camp.lookout", 0));
        p.beaten.insert("camp.lookout".into());
        p.beaten.insert(raid_key(2));
        p.count[Item::Jacket as usize] = 1;
        p.count[Item::Boots as usize] = 1;
        p.count[Item::Gloves as usize] = 1;
        p.count[Item::Flute as usize] = 1;
        p.count[Item::Rock as usize] = 7;
        p.worn[Slot::Body as usize] = Some(Item::Jacket);
        p.worn[Slot::Hands as usize] = Some(Item::Gloves);
        p.quick = [Some(Item::Rock), None, Some(Item::Flute)];
        p.coins = 36;
        p.flags
            .extend([Flag::PracticePassed, Flag::ImbueLearned, Flag::FluteGiven]);
        p.woken[2] = true;
        p.reached[2] = true;
        p.point = 2;
        p.playtime = 54_321;
        p.kept.settings.blood = false;
        p.kept.settings.hand_pct = 135;
        p.kept.look_pct = 60;
        p.map[3] = 0x00f0;
        // the roadside (P7): the north-east cart searched (its knives
        // carried), the straggler beaten, the tor's first rocks taken
        p.parts
            .insert("roadside".into(), ["searched.north_east_cart".to_string()].into());
        p.count[Item::Knife as usize] = 2;
        p.beaten.insert(roadside_key("north_east_orc"));
        p.taken.insert(drop_key(&roadside_key("west_rocks"), 0));
        p
    }

    /// The game object of a text, changed by `edit`, sealed again with a
    /// right checksum: a well-formed save with one field made bad.
    fn sealed(p: &Progress, edit: impl Fn(&mut Map<String, Value>)) -> String {
        let mut game = game_value(p);
        edit(game.as_object_mut().unwrap());
        let sum = checksum(&game.to_string());
        json!({"format": "chumworld-save", "version": 1, "checksum": sum, "game": game}).to_string()
    }

    #[test]
    fn a_save_round_trips_to_the_same_progress_and_the_same_world() {
        let p = some_progress();
        let t = text(&p);
        let back = read(&t).expect("the core reads its own save");
        assert_eq!(back, p);
        let (a, b) = (world(&back), world(&p));
        assert_eq!(a.hash(), b.hash(), "the same save, the same world (CW-4)");
        // the loaded world's own progress is the save, and writes the same text
        assert_eq!(text(&progress(&a)), t);
        assert_eq!(a.inventory.worn[Slot::Hands as usize], Some(Item::Gloves));
        assert_eq!(a.inventory.coins, 36);
        assert!(a.inventory.knows_imbue && a.travel.given && a.travel.points[2].woken);
        assert_eq!(a.trainer.stage, crate::trainer::Stage::Done);
        assert!(a.story.pickups[pickup_of(Item::Jacket).unwrap()].taken);
        assert!(!a.ui.settings.blood);
        let cart = a
            .roadside
            .stops
            .iter()
            .position(|(s, _)| s.id == "north_east_cart")
            .unwrap();
        assert!(a.roadside.searched[cart], "the searched cart stays searched");
        let rocks = a
            .save
            .keyed
            .iter()
            .find(|(_, k)| k == "drop.roadside.west_rocks.0")
            .unwrap()
            .0;
        assert!(a.story.pickups[rocks].taken, "the tor's rocks stay taken");
        assert_eq!(a.aim.pct, 60);
        assert_eq!(a.story.phase, Phase::Free, "a load is past the opening");
        // standing at the hill's waystone, 4 m in front of it
        let (stone, _) = crate::travel::stone(&a, 2).unwrap();
        let d = stone.sub(a.player.pos).len_xz();
        assert!((d - 4 * ONE).abs() < ONE / 10, "{d}");
        // the beaten round's orc lies down; the others stand
        let r = a.story.raid.iter().find(|r| r.route == 2).unwrap();
        assert!(a.foes[r.foe].fighter.health.down());
        let other = a.story.raid.iter().find(|r| r.route == 1).unwrap();
        assert!(!a.foes[other.foe].fighter.health.down());
        // a different seed is a different world
        let mut q = p.clone();
        q.seed += 1;
        assert_ne!(world(&q).hash(), a.hash());
    }

    #[test]
    fn a_save_changed_by_hand_fails_its_checksum() {
        let t = text(&some_progress());
        let edited = t.replacen("\"coins\": 36", "\"coins\": 9000", 1);
        assert_ne!(edited, t, "the edit found its field");
        assert_eq!(read(&edited), Err(Refusal::Checksum));
        // the same edit with the checksum made again reads
        let mut p = some_progress();
        p.coins = 9000;
        assert_eq!(read(&sealed(&p, |_| {})).map(|q| q.coins), Ok(9000));
    }

    #[test]
    fn a_save_that_is_not_a_save_is_refused() {
        assert_eq!(read(""), Err(Refusal::Unreadable));
        assert_eq!(read("{broken"), Err(Refusal::Unreadable));
        assert_eq!(read("[1, 2]"), Err(Refusal::Format));
        let p = some_progress();
        let other = text(&p).replacen("chumworld-save", "another-game", 1);
        assert_eq!(read(&other), Err(Refusal::Format));
        let newer = text(&p).replacen("\"version\": 1", "\"version\": 2", 1);
        assert_eq!(read(&newer), Err(Refusal::Version));
        let long = " ".repeat(spec().text_most + 1);
        assert_eq!(read(&long), Err(Refusal::Unreadable));
    }

    #[test]
    fn each_bad_field_is_refused_by_name() {
        let p = some_progress();
        let field = |f: &str| Err(Refusal::Field(f.into()));
        let cases: Vec<Case> = vec![
            (
                "coins as text",
                Box::new(|g| {
                    g.insert("coins".into(), json!("36"));
                }),
                field("coins"),
            ),
            (
                "coins over the most",
                Box::new(|g| {
                    g.insert("coins".into(), json!(100_000));
                }),
                field("coins"),
            ),
            (
                "coins negative",
                Box::new(|g| {
                    g.insert("coins".into(), json!(-1));
                }),
                field("coins"),
            ),
            (
                "a fraction",
                Box::new(|g| {
                    g.insert("playtime".into(), serde_json::from_str("1.5").unwrap());
                }),
                field("playtime"),
            ),
            (
                "seed past 32 bits",
                Box::new(|g| {
                    g.insert("seed".into(), json!(1u64 << 32));
                }),
                field("seed"),
            ),
            (
                "an unknown item",
                Box::new(|g| {
                    g["items"]["lute"] = json!(1);
                }),
                field("items.lute"),
            ),
            (
                "rocks over the stack",
                Box::new(|g| {
                    g["items"]["rock"] = json!(21);
                }),
                field("items.rock"),
            ),
            (
                "a count of none",
                Box::new(|g| {
                    g["items"]["rock"] = json!(0);
                }),
                field("items.rock"),
            ),
            (
                "an unknown field",
                Box::new(|g| {
                    g.insert("name".into(), json!("Sam"));
                }),
                field("name"),
            ),
            (
                "a missing field",
                Box::new(|g| {
                    g.remove("point");
                }),
                field("point"),
            ),
            (
                "an unknown save point",
                Box::new(|g| {
                    g.insert("point".into(), json!("post.moon"));
                }),
                field("point"),
            ),
            (
                "the opening's phase",
                Box::new(|g| {
                    g.insert("phase".into(), json!("raid"));
                }),
                field("phase"),
            ),
            (
                "a weapon not carried",
                Box::new(|g| {
                    g.insert("weapon".into(), json!("sword"));
                }),
                field("weapon"),
            ),
            (
                "a throwable in the sword hand",
                Box::new(|g| {
                    g.insert("weapon".into(), json!("rock"));
                }),
                field("weapon"),
            ),
            (
                "armour in the wrong slot",
                Box::new(|g| {
                    g["worn"]["head"] = json!("jacket");
                }),
                field("worn.head"),
            ),
            (
                "an unknown slot",
                Box::new(|g| {
                    g["worn"]["tail"] = json!("jacket");
                }),
                field("worn.tail"),
            ),
            (
                "armour on a quick key",
                Box::new(|g| {
                    g.insert("quick".into(), json!(["jacket", null, null]));
                }),
                field("quick"),
            ),
            (
                "an item on two keys",
                Box::new(|g| {
                    g.insert("quick".into(), json!(["rock", "rock", null]));
                }),
                field("quick"),
            ),
            (
                "four quick keys",
                Box::new(|g| {
                    g.insert("quick".into(), json!([null, null, null, null]));
                }),
                field("quick"),
            ),
            (
                "an unknown flag",
                Box::new(|g| {
                    g.insert("flags".into(), json!(["dragon_slain"]));
                }),
                field("flags"),
            ),
            (
                "an unknown foe",
                Box::new(|g| {
                    g["beaten"]["camp.dragon"] = json!(true);
                }),
                field("beaten.camp.dragon"),
            ),
            (
                "a foe beaten as false",
                Box::new(|g| {
                    g["beaten"]["camp.leader"] = json!(false);
                }),
                field("beaten.camp.leader"),
            ),
            (
                "an unknown pickup",
                Box::new(|g| {
                    g.insert("taken".into(), json!(["pickup.99"]));
                }),
                field("taken"),
            ),
            (
                "a pickup twice",
                Box::new(|g| {
                    g.insert("taken".into(), json!(["pickup.5", "pickup.5"]));
                }),
                field("taken"),
            ),
            (
                "an unknown waystone",
                Box::new(|g| {
                    g.insert("woken".into(), json!(["moon"]));
                }),
                field("woken"),
            ),
            (
                "woken but never reached",
                Box::new(|g| {
                    g.insert("reached".into(), json!([]));
                }),
                field("woken"),
            ),
            (
                "a setting out of range",
                Box::new(|g| {
                    g["settings"]["hand_pct"] = json!(9000);
                }),
                field("settings.hand_pct"),
            ),
            (
                "a setting missing",
                Box::new(|g| {
                    g["settings"].as_object_mut().unwrap().remove("blood");
                }),
                field("settings.blood"),
            ),
            (
                "a setting of the wrong type",
                Box::new(|g| {
                    g["settings"]["invert_y"] = json!(1);
                }),
                field("settings.invert_y"),
            ),
            (
                "a short map",
                Box::new(|g| {
                    g.insert("map".into(), json!("00ff"));
                }),
                field("map"),
            ),
            (
                "cells past the map's edge",
                Box::new(|g| {
                    let m = g["map"].as_str().unwrap().to_string();
                    let n = m.len();
                    g.insert("map".into(), json!(format!("{}ffffffffffffffff", &m[..n - 16])));
                }),
                field("map"),
            ),
            (
                "the imbue without the practice",
                Box::new(|g| {
                    g.insert("flags".into(), json!(["flute_given", "imbue_learned"]));
                }),
                field("flags"),
            ),
            (
                "a cleared camp with its foes standing",
                Box::new(|g| {
                    g.insert(
                        "flags".into(),
                        json!(["camp_cleared", "flute_given", "imbue_learned", "practice_passed"]),
                    );
                }),
                field("flags"),
            ),
            (
                "a stocked item never sold",
                Box::new(|g| {
                    g["items"].as_object_mut().unwrap().remove("boots");
                    g["stock"]["boots"] = json!(1);
                }),
                field("stock"),
            ),
            (
                "an unknown part",
                Box::new(|g| {
                    g["parts"]["dragons"] = json!(["slain"]);
                }),
                field("parts.dragons"),
            ),
            (
                "an unknown id of a part",
                Box::new(|g| {
                    g["parts"]["roadside"] = json!(["searched.moon"]);
                }),
                field("parts.roadside"),
            ),
            (
                "a part with no ids",
                Box::new(|g| {
                    g["parts"]["roadside"] = json!([]);
                }),
                field("parts.roadside"),
            ),
            (
                "an unknown straggler",
                Box::new(|g| {
                    g["beaten"]["roadside.moon"] = json!(true);
                }),
                field("beaten.roadside.moon"),
            ),
            (
                "a stocked stack",
                Box::new(|g| {
                    g["stock"]["rock"] = json!(1);
                }),
                field("stock.rock"),
            ),
        ];
        for (what, edit, want) in cases {
            assert_eq!(read(&sealed(&p, edit)), want, "{what}");
        }
    }

    #[test]
    fn nothing_is_held_without_the_route_that_gives_it() {
        let p = some_progress();
        let route = |i: Item| Err(Refusal::Route(i));
        let cases: Vec<Case> = vec![
            (
                "the sword with the leader standing",
                Box::new(|g| {
                    g["items"]["sword"] = json!(1);
                }),
                route(Item::Sword),
            ),
            (
                "the boomerang with the chest shut",
                Box::new(|g| {
                    g["items"]["boomerang"] = json!(1);
                }),
                route(Item::Boomerang),
            ),
            (
                "the chest key from nowhere",
                Box::new(|g| {
                    g["items"]["chest_key"] = json!(1);
                }),
                route(Item::ChestKey),
            ),
            (
                "the hood never bought",
                Box::new(|g| {
                    g["items"]["hood"] = json!(1);
                    g["stock"].as_object_mut().unwrap().remove("hood");
                }),
                route(Item::Hood),
            ),
            (
                "the hood twice",
                Box::new(|g| {
                    g["items"]["hood"] = json!(1);
                    g.insert(
                        "flags".into(),
                        json!(["flute_given", "imbue_learned", "merchant_met", "practice_passed"]),
                    );
                }),
                route(Item::Hood),
            ),
            (
                "the trousers not taken",
                Box::new(|g| {
                    g["items"]["trousers"] = json!(1);
                }),
                route(Item::Trousers),
            ),
            (
                "knives from no one",
                Box::new(|g| {
                    g.insert("parts".into(), json!({}));
                }),
                route(Item::Knife),
            ),
            (
                "a flute the trainer never gave",
                Box::new(|g| {
                    g.insert("flags".into(), json!(["imbue_learned", "practice_passed"]));
                }),
                route(Item::Flute),
            ),
            (
                "a taken key that vanished",
                Box::new(|g| {
                    g["taken"].as_array_mut().unwrap().push(json!("drop.camp.key"));
                }),
                route(Item::ChestKey),
            ),
        ];
        for (what, edit, want) in cases {
            assert_eq!(read(&sealed(&p, edit)), want, "{what}");
        }
        // with its route, each reads: the leader's sword taken
        let ok = sealed(&p, |g| {
            g["items"]["sword"] = json!(1);
            g["taken"]
                .as_array_mut()
                .unwrap()
                .push(json!("drop.camp.leader.0"));
            let mut t: Vec<Value> = g["taken"].as_array().unwrap().clone();
            t.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
            g.insert("taken".into(), json!(t));
        });
        assert!(read(&ok).is_ok(), "{:?}", read(&ok));
    }

    #[test]
    fn the_save_holds_nothing_but_the_documented_fields() {
        let t = text(&some_progress());
        let v: Value = serde_json::from_str(&t).unwrap();
        let keys = |v: &Value| -> Vec<String> { v.as_object().unwrap().keys().cloned().collect() };
        assert_eq!(keys(&v), ["checksum", "format", "game", "version"]);
        assert_eq!(
            keys(&v["game"]),
            [
                "beaten", "coins", "flags", "items", "map", "parts", "phase", "playtime", "point", "quick",
                "reached", "seed", "settings", "stock", "taken", "weapon", "woken", "worn"
            ]
        );
        assert_eq!(
            keys(&v["game"]["settings"]),
            ["blood", "cue", "hand_pct", "invert_y", "look_pct", "volume_pct"]
        );
        // every string in it is an id the game knows, the map's hex, the
        // format and the checksum: no name, no date, no device
        let g = &v["game"];
        assert_eq!(g["point"], "waystone.hill");
        assert_eq!(g["playtime"], 54_321, "ticks played, not a clock");
        assert_eq!(v["format"], "chumworld-save");
        // the same progress gives the same text, byte for byte
        assert_eq!(text(&some_progress()), t);
    }

    #[test]
    fn no_save_is_written_during_the_opening() {
        let mut w = story::opening(4);
        idle(&mut w, 200);
        assert!(w.story.opening());
        assert_eq!(refusal(&w), Some("save_refuse_opening"));
        // two minutes and more in the raid, standing by the bed
        w.save.since = spec().autosave_ticks;
        w.save.due = true;
        idle(&mut w, 30);
        assert_eq!(w.save.seq, 0, "nothing written");
        assert!(w.save.last.is_none());
        use_point(&mut w, 0);
        assert_eq!(w.save.seq, 0);
        assert_eq!(
            w.story.banner.as_ref().map(|b| b.key.as_str()),
            Some("save_refuse_opening")
        );
        // crossing the boundary writes the first save, at the save point
        // nearest the crossing: the village's waystone on the north lane
        let c = story::centre(&w);
        let mut at = c.add(crate::geom::v3(0, 0, -(story::BOUNDARY + ONE)));
        at.y = w.ground(at.x, at.z);
        w.player = crate::player::Body::new(at, 0);
        w.player.prev_pos = c;
        idle(&mut w, 1);
        assert_eq!(w.story.phase, Phase::Free);
        assert_eq!(w.save.seq, 1, "saved as the opening ends");
        assert_eq!(
            w.save.last.as_ref().map(|p| point_id(p.point)),
            Some("waystone.village".into())
        );
    }

    #[test]
    fn the_autosave_never_fires_mid_duel_or_mid_dialogue() {
        let mut w = saved();
        let seq = w.save.seq;
        idle(&mut w, 2);
        // a drawn orc 3 m off: due, but not written while it stands
        crate::player::act(&mut w, "spawn", "orc").unwrap();
        crate::player::act(&mut w, "gap", "300").unwrap();
        crate::player::act(&mut w, "pilot", "still").unwrap();
        w.foes[0].fighter.sheath = crate::player::Sheath::Drawn;
        w.save.since = spec().autosave_ticks;
        assert_eq!(refusal(&w), Some("save_refuse_duel"));
        idle(&mut w, 5);
        assert_eq!(w.save.seq, seq, "not mid-duel");
        assert!(w.save.due, "it waits");
        // the duel over (the orc down), mid-dialogue: still not
        w.foes[0].fighter.health.hp = 0;
        crate::ui::say(&mut w, "trainer", &["trainer_1"]);
        w.step(&Input::default());
        assert_eq!(w.save.seq, seq, "not mid-dialogue");
        // the dialogue read: written at once
        w.ui.dialogue = None;
        w.step(&Input::default());
        assert_eq!(w.save.seq, seq + 1);
        assert!(w.save.mark > 0, "the Saved mark shows");
        assert!(!w.save.due);
    }

    #[test]
    fn two_minutes_of_play_save_the_game_and_walking_into_an_area_does_too() {
        let mut w = saved();
        let seq = w.save.seq;
        idle(&mut w, spec().autosave_ticks - 1);
        assert_eq!(w.save.seq, seq, "not before two minutes");
        idle(&mut w, 2);
        assert_eq!(w.save.seq, seq + 1, "after two minutes");
        // put at the crossroads (an area), the game saves
        let mut at = crate::places::at(&w, "crossroads").unwrap();
        at.x += ONE * 5;
        w.player = crate::player::Body::new(at, 0);
        idle(&mut w, 1);
        assert_eq!(w.save.seq, seq + 2, "walking into the crossroads");
    }

    #[test]
    fn e_at_a_post_saves_there_with_a_line() {
        let mut w = saved();
        let k = points() - spec().posts.len();
        let (foot, yaw) = foot(&w, k).unwrap();
        w.player = crate::player::Body::new(foot.add(facing(yaw, 0).scale(ONE * 15 / 10)), 0);
        assert_eq!(
            reach(&w).map(|(p, _)| p),
            Some(k),
            "the crossroads' post is in reach"
        );
        let seq = w.save.seq;
        use_point(&mut w, k);
        assert_eq!(w.save.seq, seq + 1);
        assert_eq!(w.story.banner.as_ref().map(|b| b.key.as_str()), Some("saved"));
        assert_eq!(
            w.save.last.as_ref().map(|p| point_id(p.point)),
            Some("post.crossroads".into())
        );
        // a dormant waystone is not a save point until it is woken
        assert!(w.travel.points.iter().all(|p| !p.woken));
        let (stone, _) = crate::travel::stone(&w, 1).unwrap();
        w.player = crate::player::Body::new(stone.add(crate::geom::v3(ONE, 0, 0)), 0);
        assert_eq!(reach(&w), None);
    }

    #[test]
    fn a_fall_after_the_opening_returns_to_the_last_save_point_with_everything_kept() {
        let mut w = saved();
        let k = points() - 1; // the trail's post
        let (at, _) = arrival(&w, k).unwrap();
        w.player = crate::player::Body::new(at, 0);
        use_point(&mut w, k);
        idle(&mut w, 2);
        // since the save: coins found, the lookout beaten and its gloves
        // taken; then walked off and fell
        w.inventory.coins += 20;
        w.save.beaten.insert("camp.lookout".into());
        w.save.taken.insert(drop_key("camp.lookout", 0));
        crate::inventory::give(&mut w, Item::Gloves, 1);
        let mut far = at;
        far.x -= fx_m(300);
        w.player = crate::player::Body::new(far, 0);
        w.me.health.hp = 0;
        for _ in 0..(story::FALL_SHOW + story::FADE_TICKS + 5) {
            w.step(&Input::default());
        }
        assert_eq!(w.story.phase, Phase::Free);
        assert!(!w.player_down(), "come to, whole");
        let (want, _) = arrival(&w, k).unwrap();
        assert!(
            want.sub(w.player.pos).len_xz() < ONE / 10,
            "at the trail's post, not the village"
        );
        assert_eq!(w.inventory.coins, 20, "the coins kept");
        assert!(w.inventory.has(Item::Gloves), "the gloves kept");
        assert!(
            !w.save.beaten.contains("camp.lookout"),
            "the lookout stands again"
        );
        assert!(
            w.save.taken.contains(&drop_key("camp.lookout", 0)),
            "and will not drop the gloves twice"
        );
        assert_eq!(
            w.story.banner.as_ref().map(|b| b.key.as_str()),
            Some("save_came_to")
        );
    }

    fn fx_m(m: i32) -> Fx {
        m * ONE
    }

    #[test]
    fn a_drop_taken_once_never_lies_again() {
        let mut w = saved();
        let at = w.player.pos;
        let n = w.story.pickups.len();
        drop(&mut w, "drop.camp.leader.0", Gift::Item(Item::Sword, 1), at);
        assert_eq!(w.story.pickups.len(), n + 1);
        drop(&mut w, "drop.camp.leader.0", Gift::Item(Item::Sword, 1), at);
        assert_eq!(w.story.pickups.len(), n + 1, "laid once");
        w.save.taken.insert("drop.camp.leader.1".into());
        drop(&mut w, "drop.camp.leader.1", Gift::Coins(24), at);
        assert_eq!(w.story.pickups.len(), n + 1, "taken in the save: not laid");
    }

    #[test]
    fn a_stored_save_continues_and_a_bad_one_starts_a_new_game_with_a_line() {
        let mut g = crate::game::tests::game();
        let t = text(&some_progress());
        g.act("save.stored", &t).unwrap();
        let u = crate::api::ui(&g);
        assert_eq!(u.panels[0].actions[0].name, "save.continue");
        assert_eq!(u.panels[0].actions[0].label, "Continue");
        g.act("save.continue", "").unwrap();
        assert_eq!(g.screen, Screen::Play);
        assert_eq!(g.world.inventory.coins, 36);
        // a corrupt stored save: a new game, the Play button, and the line
        let mut g = crate::game::tests::game();
        g.act("save.stored", "{\"format\": \"chumworld-save\"").unwrap();
        let u = crate::api::ui(&g);
        assert_eq!(u.panels[0].actions[0].label, "Play");
        let line = &u.panels.iter().find(|p| p.id == "title-save").unwrap().text;
        assert!(line.starts_with("The saved game was refused"), "{line}");
        assert!(line.ends_with("A new game starts."), "{line}");
        assert_eq!(g.world.story.phase, Phase::Bed(0), "the opening, untouched");
        // a refused file leaves the game as it was, with its line
        let before = g.world.clone();
        g.act("save.load", &t.replacen("\"coins\": 36", "\"coins\": 37", 1))
            .unwrap();
        assert_eq!(g.world, before, "never a half-loaded world");
        assert!(g
            .saves
            .note
            .as_deref()
            .unwrap_or("")
            .contains("changed since it was written"));
    }

    #[test]
    fn a_new_game_over_a_save_asks_first_and_then_forgets_it() {
        let mut g = crate::game::tests::game();
        g.act("save.stored", &text(&some_progress())).unwrap();
        g.act("save.new", "").unwrap();
        assert_eq!(g.screen, Screen::Title, "it asks first");
        let u = crate::api::ui(&g);
        let p = u.panels.iter().find(|p| p.id == "title-save").unwrap();
        assert_eq!(p.actions[0].name, "save.new_yes");
        let wipe = g.saves.wipe;
        g.act("save.new_yes", "").unwrap();
        assert_eq!(g.screen, Screen::Play);
        assert_eq!(g.saves.wipe, wipe + 1, "the page is told to forget the save");
        assert!(g.saves.stored.is_none());
    }
}
