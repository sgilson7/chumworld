//! `travel` (M7 part P6, triage T11): the flute, the waystones, the travel
//! song and the paid traveller.
//!
//! The person: "you should definitly be able to get an instrument you can
//! play to return to fast travel points like ocarina of time / majoras mask,
//! or certain npcs can fast travel you to far off places for the first time
//! for a price".
//!
//! # The flute
//!
//! The trainer gives it once the practice is passed (its one route,
//! `trainer_gift`, CW-6), and in the same talk teaches the travel song: the
//! line `TEACH_LINE` shows the song's notes on a staff. Played from a quick
//! key or from Select Item, it opens the song screen and the world holds
//! still (`Screen::Song`). Five keys play five notes (`Note`, the arrows and
//! Space, as Ocarina of Time's C buttons and A), each shown on the staff as
//! it sounds. Once as many notes as the song has are played, the phrase is
//! judged: the travel song holds on the staff for `right_ticks`, then the
//! map shows the waystones the player has woken to choose from; anything
//! else fades over `wrong_ticks` and the staff clears. Esc puts the flute
//! away.
//!
//! # The waystones
//!
//! Five carved waystones (`Prop::Waystone`, places `waystone_<id>` in
//! `data/places.json`): by the village (outside the opening's boundary), by
//! the bridge, on the trainer's hill, on the shrine's islet, and down the
//! trail from the bandit camp (outside the lookout's sight). Each starts
//! dormant; E beside it wakes it, and from then on its marks glow and the
//! map marks it. Travel fades the view out, puts the player in front of the
//! chosen waystone facing the way it faces, and fades back in.
//!
//! Travel is refused, with a line that says why: during the opening
//! (before the boundary), while a foe with its blade out is within
//! `danger_m` or a foe holds on the player, and while someone is talking.
//!
//! # The traveller
//!
//! A traveller with a pack mule waits at the crossroads. E: a greeting, then
//! a list of the waystones the player has not yet been near, each priced by
//! its distance from the crossroads. A ride takes exactly the price, fades
//! like the song, and leaves the player at that waystone already woken. A
//! place the player has been near is never offered: the flute covers it.
//!
//! # Sound
//!
//! Every sound this module wants is an event (`State::sound`: a count and
//! an id) that the view model carries (`ui().travel_sound`), for M7's sound
//! part to play from its table: `note_<note>`, `song_right`, `song_wrong`,
//! `flute_away`, `waystone_wake`, `travel_out`, `travel_in`, `ride_pay`.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::api::Action;
use crate::fx::{self, Fx, ONE};
use crate::game::{Game, Screen};
use crate::geom::{facing, v3, Affine, V3};
use crate::input::Input;
use crate::items::Item;
use crate::look::Look;
use crate::models::{self, Prop};
use crate::page::Strings;
use crate::pose::Kind;
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::world::World;

/// The dialogue line that teaches the song: the dialogue box shows the
/// song's notes under it.
pub const TEACH_LINE: &str = "trainer_flute_2";

// ---- notes -----------------------------------------------------------------

/// The flute's five notes, by the key that plays each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Note {
    A,
    Down,
    Right,
    Left,
    Up,
}

impl Note {
    pub const ALL: [Note; 5] = [Note::A, Note::Down, Note::Right, Note::Left, Note::Up];

    /// The id in `data/travel.json`, the word `data/keys.json` sends, and
    /// the string and icon keys (`note_<id>`).
    pub fn id(self) -> &'static str {
        match self {
            Note::A => "a",
            Note::Down => "down",
            Note::Right => "right",
            Note::Left => "left",
            Note::Up => "up",
        }
    }

    pub fn from_id(id: &str) -> Option<Note> {
        Note::ALL.iter().copied().find(|n| n.id() == id)
    }

    /// The note's line on the staff, 0 at the bottom (`staff` in the file).
    pub fn row(self) -> usize {
        spec().staff.iter().position(|n| *n == self).unwrap_or(0)
    }
}

// ---- data/travel.json -------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PointSpec {
    pub id: String,
    pub place: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PriceSpec {
    pub base: u32,
    pub per_m: i32,
    pub most: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TravellerSpec {
    pub place: String,
    pub at: [i32; 2],
    pub yaw_deg: i32,
    pub mule_at: [i32; 2],
    pub mule_yaw_deg: i32,
    pub talk_cm: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SpecFile {
    song: Vec<String>,
    staff: Vec<String>,
    points: Vec<PointSpec>,
    reach_m: i32,
    wake_cm: i32,
    arrive_cm: i32,
    danger_m: i32,
    price: PriceSpec,
    traveller: TravellerSpec,
    dark_ticks: u32,
    hold_ticks: u32,
    right_ticks: u32,
    wrong_ticks: u32,
    glow_far_m: i32,
}

/// `data/travel.json`, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    pub song: Vec<Note>,
    pub staff: Vec<Note>,
    pub points: Vec<PointSpec>,
    pub reach: Fx,
    pub wake: Fx,
    pub arrive: Fx,
    pub danger: Fx,
    pub price: PriceSpec,
    pub traveller: TravellerSpec,
    pub dark_ticks: u32,
    pub hold_ticks: u32,
    pub right_ticks: u32,
    pub wrong_ticks: u32,
    pub glow_far: Fx,
}

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let mut v: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("The travel file could not be read: {e}"))?;
        if let Some(m) = v.as_object_mut() {
            m.retain(|k, _| !k.starts_with('_'));
        }
        let f: SpecFile =
            serde_json::from_value(v).map_err(|e| format!("The travel file could not be read: {e}"))?;
        let notes = |list: &[String]| -> Result<Vec<Note>, String> {
            list.iter()
                .map(|n| {
                    Note::from_id(n)
                        .ok_or_else(|| format!("The travel file names a note \"{n}\" the flute has not."))
                })
                .collect()
        };
        let song = notes(&f.song)?;
        let staff = notes(&f.staff)?;
        if song.is_empty() {
            return Err("The travel file's song has no notes.".into());
        }
        if Note::ALL.iter().any(|n| !staff.contains(n)) || staff.len() != Note::ALL.len() {
            return Err("The travel file's staff must give each of the five notes a line.".into());
        }
        if f.price.per_m <= 0 {
            return Err("The travel file's price needs a whole number of metres a coin.".into());
        }
        let m = |v: i32| v * ONE;
        let cm = |v: i32| v * ONE / 100;
        Ok(Spec {
            song,
            staff,
            points: f.points,
            reach: m(f.reach_m),
            wake: cm(f.wake_cm),
            arrive: cm(f.arrive_cm),
            danger: m(f.danger_m),
            price: f.price,
            traveller: f.traveller,
            dark_ticks: f.dark_ticks,
            hold_ticks: f.hold_ticks,
            right_ticks: f.right_ticks,
            wrong_ticks: f.wrong_ticks,
            glow_far: m(f.glow_far_m),
        })
    }
}

/// The numbers the glossary names: the flute's keys, the song's notes, the
/// rides' least and most prices.
pub fn values() -> Vec<(&'static str, String)> {
    let sp = spec();
    vec![
        ("flute.keys", Note::ALL.len().to_string()),
        ("song.notes", sp.song.len().to_string()),
        ("ride.least", sp.price.base.to_string()),
        ("ride.most", sp.price.most.to_string()),
    ]
}

/// The travel file the game ships with, read once.
pub fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| Spec::parse(include_str!("../../../data/travel.json")).expect("data/travel.json"))
}

// ---- state ------------------------------------------------------------------

/// One waystone: whether it is woken, and whether the player has been near it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub woken: bool,
    pub reached: bool,
}

/// What the song screen shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// the staff, notes being played
    Notes,
    /// the waystones to choose from: woken ones for the flute, ones not yet
    /// reached for the traveller's ride
    Pick { ride: bool },
}

/// A played phrase being judged: right or wrong, ticks since.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Judged {
    pub right: bool,
    pub ticks: u32,
}

/// The song screen while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Song {
    pub phase: Phase,
    pub played: Vec<Note>,
    pub judged: Option<Judged>,
    /// the chosen row of the list
    pub cursor: usize,
    /// a refusal under the list, by string key, and the coins it names
    pub note: Option<(&'static str, u32)>,
}

impl Song {
    fn new(phase: Phase) -> Song {
        Song {
            phase,
            played: Vec::new(),
            judged: None,
            cursor: 0,
            note: None,
        }
    }
}

/// A journey under way: to which point, ticks since it began, and whether
/// it is the traveller's ride.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Warp {
    pub to: usize,
    pub ticks: u32,
    pub ride: bool,
}

/// The last sound this module asked for, and how many it has asked for, so
/// the page plays each once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sound {
    pub seq: u32,
    pub id: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// one for each point in `data/travel.json`, in its order
    pub points: Vec<Point>,
    /// the trainer has given the flute (and taught the song)
    pub given: bool,
    /// the song screen, open or asked for (`ui::wanted` opens it)
    pub song: Option<Song>,
    pub warp: Option<Warp>,
    pub sound: Sound,
}

impl Default for State {
    fn default() -> State {
        State {
            points: vec![Point::default(); spec().points.len()],
            given: false,
            song: None,
            warp: None,
            sound: Sound::default(),
        }
    }
}

fn sound(w: &mut World, id: &'static str) {
    w.travel.sound = Sound {
        seq: w.travel.sound.seq.wrapping_add(1),
        id,
    };
}

// ---- where things are -------------------------------------------------------

/// A waystone's foot (on the ground) and the way it faces.
pub fn stone(w: &World, k: usize) -> Option<(V3, i32)> {
    let p = spec().points.get(k)?;
    let place = w.land.layout.places.iter().find(|q| q.kind == p.place)?;
    let s = crate::places::spots(w, place)
        .into_iter()
        .find(|s| s.prop == Prop::Waystone)?;
    Some((s.at, s.yaw))
}

/// Point `k`'s place centre in the layout, at height 0: where its waystone
/// stands (at [0, 0] in its place), without reading the ground, for the
/// tick's nearness check.
fn centre_xz(w: &World, k: usize) -> Option<V3> {
    let p = spec().points.get(k)?;
    let place = w.land.layout.places.iter().find(|q| q.kind == p.place)?;
    Some(crate::places::metres(place.centre))
}

/// Where the player arrives at point `k`: `arrive` in front of the
/// waystone, on what is underfoot there, facing the way the waystone faces.
pub fn arrival(w: &World, k: usize) -> Option<(V3, i32)> {
    let (at, yaw) = stone(w, k)?;
    let mut p = at.add(facing(yaw, 0).scale(spec().arrive));
    p.y = crate::solid::surface(w, p.x, p.z);
    Some((p, yaw))
}

/// The traveller's feet and heading, and the mule's.
pub fn traveller(w: &World) -> Option<((V3, i32), (V3, i32))> {
    let t = &spec().traveller;
    let c = crate::places::at(w, &t.place)?;
    let put = |at: [i32; 2]| {
        let mut p = c.add(v3(at[0] * ONE, 0, at[1] * ONE));
        p.y = w.ground(p.x, p.z);
        p
    };
    Some((
        (put(t.at), fx::deg(t.yaw_deg)),
        (put(t.mule_at), fx::deg(t.mule_yaw_deg)),
    ))
}

/// The map's marks for the woken waystones (M4's `places::markers`): each
/// keyed by its place, so `place_<place>` names it.
pub fn marks(w: &World) -> Vec<(&'static str, V3)> {
    let sp = spec();
    (0..sp.points.len())
        .filter(|&k| w.travel.points.get(k).is_some_and(|p| p.woken))
        .filter_map(|k| stone(w, k).map(|(at, _)| (sp.points[k].place.as_str(), at)))
        .collect()
}

/// The points the flute lists (woken) or the traveller offers (not yet
/// reached), in the file's order.
pub fn listed(w: &World, ride: bool) -> Vec<usize> {
    (0..w.travel.points.len())
        .filter(|&k| {
            let p = w.travel.points[k];
            if ride {
                !p.reached && !p.woken
            } else {
                p.woken
            }
        })
        .collect()
}

/// A ride's price for a journey of `d` (metres, Q16): `base`, and a coin a
/// `per_m` metres, held to `most`.
pub fn price_for(d: Fx) -> u32 {
    let p = &spec().price;
    let metres = (d / ONE).max(0);
    (p.base + (metres / p.per_m) as u32).min(p.most)
}

/// The ride's price to point `k`, by its distance from the traveller.
pub fn price(w: &World, k: usize) -> u32 {
    match (traveller(w), stone(w, k)) {
        (Some(((t, _), _)), Some((s, _))) => price_for(s.sub(t).len_xz()),
        _ => spec().price.most,
    }
}

// ---- the rules ----------------------------------------------------------------

/// Why travel is refused now, as a string key, or `None` when it may go.
pub fn refusal(w: &World) -> Option<&'static str> {
    if w.story.opening() {
        return Some("travel_refuse_opening");
    }
    if w.ui.dialogue.is_some() || w.ui.shop_open {
        return Some("travel_refuse_talk");
    }
    if danger(w) {
        return Some("travel_refuse_duel");
    }
    if w.travel.warp.is_some() || w.player_down() {
        return Some("travel_refuse_busy");
    }
    None
}

/// A foe is on the player: one with its blade out within `danger`, or one
/// holding on the player wherever it is.
pub fn danger(w: &World) -> bool {
    let me = w.player.pos;
    (1..crate::player::count(w)).any(|id| {
        let f = crate::player::fighter(w, id);
        if f.team == w.me.team || f.health.down() {
            return false;
        }
        let near = crate::player::body(w, id).pos.sub(me).len_xz() < spec().danger;
        (f.drawn() && near) || f.engaged == Some(0)
    })
}

/// The flute played in play (its quick key, `inventory::step`): the song
/// screen opens after the tick, or a line over the game says why not.
pub fn request(w: &mut World) {
    if !w.inventory.has(Item::Flute) {
        return;
    }
    match refusal(w) {
        Some(r) => crate::story::banner(w, r),
        None => {
            w.travel.song = Some(Song::new(Phase::Notes));
        }
    }
}

/// Whether the song screen is open or asked for (`ui::wanted`).
pub fn wanted(w: &World) -> bool {
    w.travel.song.is_some()
}

/// Wakes point `k`: its glow and its mark on the map.
pub fn wake(w: &mut World, k: usize) {
    if let Some(p) = w.travel.points.get_mut(k) {
        if !p.woken {
            p.woken = true;
            p.reached = true;
            sound(w, "waystone_wake");
        }
    }
}

/// How far E is from waking the nearest dormant waystone, for story's E
/// (`story::nearest_use`): the point and the distance as story counts it.
pub fn wake_reach(w: &World) -> Option<(usize, Fx)> {
    let me = w.player.pos;
    let edge = ONE / 2 + spec().wake;
    (0..w.travel.points.len())
        .filter(|&k| !w.travel.points[k].woken)
        .filter_map(|k| stone(w, k).map(|(at, _)| (k, at.sub(me).len_xz())))
        .filter(|&(_, d)| d <= edge)
        .map(|(k, d)| (k, (d - edge + crate::story::REACH).max(0)))
        .min_by_key(|&(_, d)| d)
}

/// E by a dormant waystone.
pub fn use_waystone(w: &mut World, k: usize) {
    wake(w, k);
    crate::story::banner(w, "travel_woke");
}

/// How far E is from talking to the traveller.
pub fn talk_reach(w: &World) -> Option<Fx> {
    let ((at, _), _) = traveller(w)?;
    let from = spec().traveller.talk_cm * ONE / 100;
    let d = at.sub(w.player.pos).len_xz();
    (d <= from).then(|| (d - from + crate::story::REACH).max(0))
}

/// E by the traveller: a greeting, then the list of rides (the dialogue
/// shows first, the list the tick after it closes, as the merchant's shop).
pub fn talk(w: &mut World) {
    let line = if listed(w, true).is_empty() {
        "traveller_nowhere"
    } else {
        "traveller_hello"
    };
    // M7 rumours (T17): a rumour after the greeting, while there is one
    match crate::rumours::after(w, "traveller") {
        Some(r) => crate::ui::say(w, "traveller", &[line, &r]),
        None => crate::ui::say(w, "traveller", &[line]),
    }
    if !listed(w, true).is_empty() {
        w.travel.song = Some(Song::new(Phase::Pick { ride: true }));
    }
}

/// The quest moves on to the flute once it is given, until a waystone is
/// woken (`story::task`); then on to the camp.
pub fn quest(w: &World) -> bool {
    w.inventory.has(Item::Flute) && w.travel.points.iter().all(|p| !p.woken)
}

/// Step 0: during a journey the player's keys, buttons and mouse do nothing.
pub fn filter(w: &World, input: &Input) -> Input {
    if w.travel.warp.is_some() {
        Input::default()
    } else {
        *input
    }
}

/// Step 7, after the story: the points the player comes near, the flute
/// given once the trainer's practice is passed, and a journey under way.
pub fn step(w: &mut World) {
    let sp = spec();
    let me = w.player.pos;
    for k in 0..w.travel.points.len() {
        if !w.travel.points[k].reached && centre_xz(w, k).is_some_and(|at| at.sub(me).len_xz() < sp.reach) {
            w.travel.points[k].reached = true;
        }
    }
    if !w.travel.given && w.trainer.taught > 0 && w.ui.dialogue.is_none() {
        w.travel.given = true;
        crate::inventory::give(w, Item::Flute, 1);
        crate::ui::say(w, "trainer", &["trainer_flute_1", TEACH_LINE, "trainer_flute_3"]);
    }
    journey(w);
}

fn journey(w: &mut World) {
    let sp = spec();
    let Some(mut j) = w.travel.warp else {
        return;
    };
    if j.ticks == 0 {
        w.story.veil = true;
        sound(w, "travel_out");
    }
    if j.ticks == sp.dark_ticks {
        put_at(w, j.to);
        sound(w, "travel_in");
    }
    if j.ticks >= sp.dark_ticks + sp.hold_ticks {
        w.story.veil = false;
        w.travel.warp = None;
        if j.ride {
            crate::story::banner(w, "traveller_arrived");
        }
        return;
    }
    j.ticks += 1;
    w.travel.warp = Some(j);
}

/// The player stands at point `k`'s arrival, facing out, a fresh body and
/// fighter there (as `places.go`), and the point is woken.
pub fn put_at(w: &mut World, k: usize) {
    let Some((at, yaw)) = arrival(w, k) else {
        return;
    };
    w.player = crate::player::Body::new(at, yaw);
    let held = crate::items::held(w);
    let me = &w.me;
    w.me = crate::player::Fighter::new(me.kind, held, me.team, at, yaw, false);
    w.aim.yaw = yaw;
    w.aim.pitch = 0;
    w.aim.rest = (0, 0);
    wake(w, k);
    if let Some(p) = w.travel.points.get_mut(k) {
        p.reached = true;
    }
}

// ---- the song screen (Game: it changes screens) --------------------------------

/// Plays the flute from Select Item (Enter on it): the song screen, or the
/// refusal under the item.
pub fn play(g: &mut Game) {
    match refusal(&g.world) {
        Some(r) => g.menu.note = Some(g.strings.get(r)),
        None => {
            g.world.travel.song = Some(Song::new(Phase::Notes));
            g.show(Screen::Song);
        }
    }
}

/// Esc on the song screen: the flute is put away, the world goes on.
pub fn cancel(g: &mut Game) {
    g.world.travel.song = None;
    sound(&mut g.world, "flute_away");
    g.show(Screen::Play);
}

/// A key on the song screen (`data/keys.json`'s `travel` keymap, words
/// `note_<note>` and `choose`): a note on the staff, or the cursor and the
/// choice on the list.
pub fn key(g: &mut Game, word: &str) -> Result<(), String> {
    let word = word.strip_prefix("note_").unwrap_or(word);
    let Some(song) = g.world.travel.song.clone() else {
        return Ok(());
    };
    match song.phase {
        Phase::Notes => {
            if word == "choose" {
                return Ok(());
            }
            let n = Note::from_id(word).ok_or_else(|| format!("The flute has no key \"{word}\"."))?;
            note(&mut g.world, n);
        }
        Phase::Pick { ride } => {
            let n = listed(&g.world, ride).len();
            let s = g.world.travel.song.as_mut().expect("open");
            match word {
                "up" | "left" if n > 0 => s.cursor = (s.cursor + n - 1) % n,
                "down" | "right" if n > 0 => s.cursor = (s.cursor + 1) % n,
                "a" | "choose" => {
                    let k = s.cursor;
                    choose(g, k)?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// A note played: it sounds and shows; with the phrase as long as the song,
/// the phrase is judged.
pub fn note(w: &mut World, n: Note) {
    let len = spec().song.len();
    let Some(s) = w.travel.song.as_mut() else {
        return;
    };
    if s.phase != Phase::Notes || s.judged.is_some() {
        return;
    }
    s.played.push(n);
    let id = match n {
        Note::A => "note_a",
        Note::Down => "note_down",
        Note::Right => "note_right",
        Note::Left => "note_left",
        Note::Up => "note_up",
    };
    let done = s.played.len() >= len;
    let right = s.played == spec().song;
    sound(w, id);
    if done {
        let s = w.travel.song.as_mut().expect("open");
        s.judged = Some(Judged { right, ticks: 0 });
        sound(w, if right { "song_right" } else { "song_wrong" });
    }
}

/// Time on the song screen while the world holds still: a judged phrase
/// holds or fades, then the list shows or the staff clears. Whether what
/// the screen shows changed.
pub fn song_ticks(w: &mut World, ticks: u32) -> bool {
    let sp = spec();
    let Some(s) = w.travel.song.as_mut() else {
        return false;
    };
    let Some(mut j) = s.judged else {
        return false;
    };
    j.ticks = j.ticks.saturating_add(ticks);
    if j.right && j.ticks >= sp.right_ticks {
        *s = Song::new(Phase::Pick { ride: false });
    } else if !j.right && j.ticks >= sp.wrong_ticks {
        s.played.clear();
        s.judged = None;
    } else {
        s.judged = Some(j);
        return false;
    }
    true
}

/// Choosing row `row` of the list: the flute's journey, or the traveller's
/// ride for its price.
pub fn choose(g: &mut Game, row: usize) -> Result<(), String> {
    let Some(song) = g.world.travel.song.clone() else {
        return Ok(());
    };
    let Phase::Pick { ride } = song.phase else {
        return Ok(());
    };
    let list = listed(&g.world, ride);
    let Some(&k) = list.get(row) else {
        return Err(format!("There is no waystone {row} on the list."));
    };
    if let Some(s) = g.world.travel.song.as_mut() {
        s.cursor = row;
    }
    if ride {
        let cost = price(&g.world, k);
        let have = g.world.inventory.coins;
        if have < cost {
            if let Some(s) = g.world.travel.song.as_mut() {
                s.note = Some(("traveller_short", cost - have));
            }
            return Ok(());
        }
        g.world.inventory.coins -= cost;
        sound(&mut g.world, "ride_pay");
        g.world.travel.song = None;
        g.world.travel.warp = Some(Warp {
            to: k,
            ticks: 0,
            ride: true,
        });
        crate::ui::say(&mut g.world, "traveller", &["traveller_ride"]);
        g.show(Screen::Dialogue);
        return Ok(());
    }
    if let Some(r) = refusal(&g.world) {
        if let Some(s) = g.world.travel.song.as_mut() {
            s.note = Some((r, 0));
        }
        return Ok(());
    }
    g.world.travel.song = None;
    g.world.travel.warp = Some(Warp {
        to: k,
        ticks: 0,
        ride: false,
    });
    g.show(Screen::Play);
    Ok(())
}

/// The traveller's list closed without a ride.
pub fn leave(g: &mut Game) {
    g.world.travel.song = None;
    g.show(Screen::Play);
}

/// One value for what this module shows during play, so the page reads the
/// view model again when it changes (`Game::advance`).
pub fn stamp(w: &World) -> (u32, bool, usize) {
    (
        w.travel.sound.seq,
        w.travel.warp.is_some(),
        w.travel.points.iter().filter(|p| p.woken).count(),
    )
}

// ---- view models ----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoteView {
    pub id: String,
    pub label: String,
    pub icon: String,
    /// the note's place on the staff, as CSS percentages
    pub left: String,
    pub top: String,
}

/// The song as the dialogue, the quest screen and the song screen show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SongCard {
    pub name: String,
    pub about: String,
    pub notes: Vec<NoteView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyView {
    pub key: String,
    pub note: NoteView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PickRow {
    pub index: usize,
    pub id: String,
    pub label: String,
    /// empty for the flute's journeys
    #[serde(skip_serializing_if = "String::is_empty")]
    pub price: String,
    pub left: String,
    pub top: String,
    pub chosen: bool,
    pub enabled: bool,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PickView {
    pub ride: bool,
    pub heading: String,
    /// the map image's version and side (`ui::map_pixels`)
    pub version: u32,
    pub size: i32,
    pub you: crate::ui::Mark,
    pub rows: Vec<PickRow>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub empty: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub note: String,
    pub coins: Option<crate::ui::Count>,
    pub hint: String,
}

/// The song screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SongView {
    /// "notes" or "pick"
    pub phase: String,
    pub heading: String,
    pub hint: String,
    /// the staff's five lines, bottom to top, as CSS percentages from the top
    pub lines: Vec<String>,
    pub keys: Vec<KeyView>,
    /// the notes played so far
    pub played: Vec<NoteView>,
    /// "", "right" or "wrong"
    pub state: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub said: String,
    /// the song the player knows, as a reminder
    pub song: Option<SongCard>,
    pub pick: Option<PickView>,
    pub leave: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SoundView {
    pub seq: u32,
    pub id: String,
}

/// A share as a CSS percentage with two decimals, from hundredths.
fn pct(h: i64) -> String {
    format!("{}.{:02}%", h / 100, h % 100)
}

/// The staff's line `row` (0 at the bottom), from the top.
fn line_top(row: usize) -> String {
    pct(8000 - 1500 * row as i64)
}

/// Note `i` of a phrase `of` long, from the left.
fn note_left(i: usize, of: usize) -> String {
    pct(1000 + 8000 * (2 * i as i64 + 1) / (2 * of.max(1) as i64))
}

fn note_view(s: &Strings, n: Note, i: usize, of: usize) -> NoteView {
    NoteView {
        id: n.id().into(),
        label: s.get(&format!("note_{}", n.id())),
        icon: format!("data/icons/note_{}.svg", n.id()),
        left: note_left(i, of),
        top: line_top(n.row()),
    }
}

/// The travel song with its notes.
pub fn song_card(s: &Strings) -> SongCard {
    let song = &spec().song;
    SongCard {
        name: s.get("travel_song"),
        about: s.get("travel_song_about"),
        notes: song
            .iter()
            .enumerate()
            .map(|(i, n)| note_view(s, *n, i, song.len()))
            .collect(),
    }
}

/// The song card, once the trainer has taught it (the quest screen).
pub fn known(w: &World, s: &Strings) -> Option<SongCard> {
    w.travel.given.then(|| song_card(s))
}

pub fn sound_view(w: &World) -> SoundView {
    SoundView {
        seq: w.travel.sound.seq,
        id: w.travel.sound.id.into(),
    }
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

pub fn view(g: &Game) -> Option<SongView> {
    if g.screen != Screen::Song {
        return None;
    }
    let (s, w) = (&g.strings, &g.world);
    let song = w.travel.song.as_ref()?;
    let len = spec().song.len();
    let keys = [Note::Up, Note::Left, Note::Down, Note::Right, Note::A]
        .iter()
        .enumerate()
        .map(|(i, n)| KeyView {
            key: s.get(&format!("key_note_{}", n.id())),
            note: note_view(s, *n, i, Note::ALL.len()),
        })
        .collect();
    let state = match song.judged {
        Some(Judged { right: true, .. }) => "right",
        Some(Judged { right: false, .. }) => "wrong",
        None => "",
    };
    let pick = match song.phase {
        Phase::Notes => None,
        Phase::Pick { ride } => Some(pick_view(g, song, ride)),
    };
    let ride = matches!(song.phase, Phase::Pick { ride: true });
    Some(SongView {
        phase: if pick.is_some() { "pick" } else { "notes" }.into(),
        heading: s.get(if ride {
            "traveller_heading"
        } else {
            "flute_heading"
        }),
        hint: s.get("flute_hint"),
        lines: (0..Note::ALL.len()).map(line_top).collect(),
        keys,
        played: song
            .played
            .iter()
            .enumerate()
            .map(|(i, n)| note_view(s, *n, i, len))
            .collect(),
        state: state.into(),
        said: match state {
            "right" => s.get("song_right"),
            "wrong" => s.get("song_wrong"),
            _ => String::new(),
        },
        song: w.travel.given.then(|| song_card(s)),
        pick,
        leave: action(
            s,
            if ride { "travel.leave" } else { "travel.cancel" },
            "",
            if ride { "leave" } else { "flute_away" },
            true,
        ),
    })
}

fn pick_view(g: &Game, song: &Song, ride: bool) -> PickView {
    let (s, w) = (&g.strings, &g.world);
    let list = listed(w, ride);
    let rows = list
        .iter()
        .enumerate()
        .map(|(row, &k)| {
            let id = &spec().points[k].id;
            let at = stone(w, k).map(|(a, _)| a).unwrap_or(V3::ZERO);
            let cost = price(w, k);
            PickRow {
                index: row,
                id: id.clone(),
                label: s.get(&format!("waystone_{id}")),
                price: if ride {
                    crate::ui::coins_text(s, "price", cost, &[])
                } else {
                    String::new()
                },
                left: crate::ui::map_pct(at.x),
                top: crate::ui::map_pct(at.z),
                chosen: row == song.cursor,
                enabled: !ride || w.inventory.coins >= cost,
                action: action(s, "travel.choose", &row.to_string(), "", true),
            }
        })
        .collect();
    let p = w.player.pos;
    PickView {
        ride,
        heading: s.get(if ride { "traveller_where" } else { "flute_where" }),
        version: w.ui.fog.version,
        size: crate::ui::MAP_PX,
        you: crate::ui::Mark {
            kind: "you".into(),
            label: s.get("you"),
            icon: String::new(),
            left: crate::ui::map_pct(p.x),
            top: crate::ui::map_pct(p.z),
            quiet: false,
        },
        rows,
        empty: if list.is_empty() {
            s.get(if ride {
                "traveller_nowhere"
            } else {
                "flute_none_woken"
            })
        } else {
            String::new()
        },
        note: match song.note {
            Some(("traveller_short", n)) => crate::ui::coins_text(s, "traveller_short", n, &[]),
            Some((k, _)) => s.get(k),
            None => String::new(),
        },
        coins: ride.then(|| crate::ui::coins_view(s, w)),
        hint: s.get(if ride { "traveller_hint" } else { "flute_pick_hint" }),
    }
}

// ---- drawing --------------------------------------------------------------------

/// The woken waystones' light (their marks lit, a halo at the eye, a ring
/// of dithered light on the ground round the foot: no blending to sort,
/// CW-18) and the traveller with the mule. Nothing here moves on its own, so
/// a still view draws the same frame tick after tick.
pub fn draw(w: &World, _alpha: Fx, look: &Look, out: &mut Shapes) {
    let sp = spec();
    let glow = models::waystone_glow();
    let colour = models::table().c("waystone_glow");
    let eye = w.player.pos;
    for k in 0..w.travel.points.len() {
        if !w.travel.points[k].woken {
            continue;
        }
        let Some((at, yaw)) = stone(w, k) else {
            continue;
        };
        if at.sub(eye).len_xz() > sp.glow_far {
            continue;
        }
        let place = Affine::translate(at).then(&Affine::rot_y(yaw));
        glow.placed(&place).emit(look.ink(), out);
        let ring = place
            .then(&Affine::translate(v3(0, ONE * 4 / 100, 0)))
            .then(&Affine::scale(v3(ONE * 26 / 10, ONE * 2 / 100, ONE * 26 / 10)));
        out.dithered(Mesh::Tube, ring, colour, ONE / 2);
    }
    if w.story.phase != crate::story::Phase::Off {
        draw_traveller(w, look, out);
    }
}

fn draw_traveller(w: &World, look: &Look, out: &mut Shapes) {
    let Some(((feet, yaw), (mule, mule_yaw))) = traveller(w) else {
        return;
    };
    if feet.sub(w.player.pos).len_xz() > models::FIGURE_FAR {
        return;
    }
    let pose = models::rest_pose(Kind::Villager, feet, yaw);
    let dress = models::Dress { worn: 0, variant: 2 };
    models::figure_dressed(Kind::Villager, &pose, dress, look, out);
    let t = models::table();
    let ink = look.ink();
    let f = facing(yaw, 0);
    let r = crate::geom::right_of(yaw);
    // a pack on the back with a bedroll on top, and a staff in the right hand
    let back = pose
        .chest
        .add(f.scale(-ONE * 26 / 100))
        .add(v3(0, -ONE * 8 / 100, 0));
    let frame = Affine::translate(back).then(&Affine::rot_y(yaw));
    out.inked(
        Mesh::Cube,
        frame.then(&Affine::scale(v3(ONE * 40 / 100, ONE * 46 / 100, ONE * 22 / 100))),
        t.c("traveller_pack"),
        ink,
    );
    let roll = frame
        .then(&Affine::translate(v3(0, ONE * 30 / 100, 0)))
        .then(&Affine::rot_z(fx::QUARTER_TURN))
        .then(&Affine::scale(v3(ONE * 14 / 100, ONE * 52 / 100, ONE * 14 / 100)));
    out.inked(Mesh::Cyl, roll, t.c("traveller_roll"), ink);
    let hand = pose.hand[crate::pose::RIGHT];
    let foot = v3(hand.x, feet.y, hand.z)
        .add(f.scale(ONE * 6 / 100))
        .add(r.scale(ONE * 4 / 100));
    let top = hand.add(v3(0, ONE * 45 / 100, 0));
    let staff = models::bar(foot, top, (ONE * 4 / 100, ONE * 4 / 100), r);
    out.inked(Mesh::Cyl, staff, t.c("traveller_staff"), ink);
    // the mule, packed: two bags over its back
    models::donkey(mule, mule_yaw, 0, look, out);
    let mf = facing(mule_yaw, 0);
    let mr = crate::geom::right_of(mule_yaw);
    for side in [-1, 1] {
        let at = mule
            .add(v3(0, ONE * 95 / 100, 0))
            .add(mr.scale(side * ONE * 30 / 100))
            .add(mf.scale(-ONE * 5 / 100));
        let bag = Affine::translate(at)
            .then(&Affine::rot_y(mule_yaw))
            .then(&Affine::scale(v3(ONE * 18 / 100, ONE * 36 / 100, ONE * 46 / 100)));
        out.inked(Mesh::Facet, bag, t.c("saddle_bag"), ink);
    }
}

// ---- acts ---------------------------------------------------------------------

/// `travel.<name>`: `key` (the song screen's keys), `choose` (a row of the
/// list), `cancel` (put the flute away), `leave` (the traveller's list),
/// `play` (the flute, as its quick key would), and, test-only, `give` (the
/// flute and its song), `wake` (a point's id, or `all`), `reach` (a point's
/// id, or `all`: been near), `song` (play the travel song at once, for
/// views of the list).
pub fn act(g: &mut Game, name: &str, value: &str) -> Result<(), String> {
    let screen = g.screen;
    let point = |w: &World, v: &str| -> Result<Vec<usize>, String> {
        if v.trim() == "all" {
            return Ok((0..w.travel.points.len()).collect());
        }
        spec()
            .points
            .iter()
            .position(|p| p.id == v.trim())
            .map(|k| vec![k])
            .ok_or_else(|| {
                format!(
                    "There is no waystone \"{v}\"; the waystones are village, bridge, hill, shrine and camp."
                )
            })
    };
    match name {
        "travel.key" if screen == Screen::Song => key(g, value.trim())?,
        "travel.choose" if screen == Screen::Song => {
            let row = value
                .trim()
                .parse::<usize>()
                .map_err(|_| format!("A row is a whole number, not \"{value}\"."))?;
            choose(g, row)?;
        }
        "travel.cancel" if screen == Screen::Song => cancel(g),
        "travel.leave" if screen == Screen::Song => leave(g),
        "travel.key" | "travel.choose" | "travel.cancel" | "travel.leave" => {}
        "travel.play" => match screen {
            Screen::Play => {
                request(&mut g.world);
                if wanted(&g.world) {
                    g.show(Screen::Song);
                }
            }
            Screen::Menu => play(g),
            // mid-dialogue or in the shop: the refusal, over the game
            _ => {
                let r = refusal(&g.world).unwrap_or("travel_refuse_talk");
                crate::story::banner(&mut g.world, r);
            }
        },
        "travel.give" => {
            crate::inventory::give(&mut g.world, Item::Flute, 1);
            g.world.travel.given = true;
        }
        "travel.wake" => {
            for k in point(&g.world, value)? {
                wake(&mut g.world, k);
            }
        }
        "travel.reach" => {
            for k in point(&g.world, value)? {
                g.world.travel.points[k].reached = true;
            }
        }
        "travel.song" if screen == Screen::Song => {
            if let Some(s) = g.world.travel.song.as_mut() {
                *s = Song::new(Phase::Pick { ride: false });
            }
        }
        "travel.song" => {}
        _ => return Err(format!("Unknown action \"{name}\".")),
    }
    g.ui_version += 1;
    Ok(())
}

#[cfg(test)]
mod tests;
