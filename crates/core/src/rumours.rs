//! `rumours` (M7 rumours, P11, T17): people who tell you where things are,
//! and the quest lines what they tell you starts.
//!
//! The person: "you should also be able to talk to people to get rumors /
//! start quest lines, so you should be able to talk to some people who point
//! out the speedsters outfit, where to get the long sword, boomerang, imbue,
//! etc".
//!
//! # Rumours
//!
//! `data/rumours.json` lists each rumour: what it is about (`of`, an item,
//! the imbue, the traveller or the waystones) and the one route it names
//! (`route`, which must be that thing's route in `data/items.json` or what
//! `data/travel.json` says of it: the tests hold it), the quest line it
//! starts or moves on, the place it marks on the map, who tells it, and what
//! makes it moot (`until`). No rumour is told while the opening lasts.
//!
//! # Who tells them
//!
//! A teller (`tellers` in the file) is one of the roadside villagers (after
//! their own lines, `roadside::use_stop`), the merchant (after the greeting,
//! before the shop), the traveller (after the greeting, before the rides),
//! the trainer (once the practice is passed), or a person of this module's
//! own: a woodcutter at the crossroads, a fisher by the bridge and a herder on
//! the trainer's hill, each beside a woodpile so the ground under them is
//! flattened (`lay`, called from `places::Layout::shipped`). E talks to them
//! (`story::Use::Teller`).
//!
//! A talk tells the first rumour the teller knows that is neither heard nor
//! moot, in the file's order, so talking again moves on to the next; once
//! nothing is left the teller says its closing line. A rumour heard from one
//! person is heard: another who knows it skips it.
//!
//! # Quest lines
//!
//! A line opens when one of its rumours is heard, and keeps that rumour as
//! its latest. It moves on as its steps come to hold (an item picked up, the
//! imbue learned, a waystone woken) and is done when they all hold (`step`).
//! A step once held stays held: a piece sold is still counted.
//!
//! # The state, for the save (P10, T16)
//!
//! Everything this module keeps is `State`: lists of ids from
//! `data/rumours.json` (rumours, tellers, lines) and place kinds from
//! `data/places.json`. They are stable names, not indexes, so a save written
//! by one build reads in the next as long as the ids stay.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{v3, yaw_of, V3};
use crate::items::Item;
use crate::look::Look;
use crate::models;
use crate::page::Strings;
use crate::places::{Layout, PlaceSpec, PropSpec};
use crate::pose::Kind as Figure;
use crate::render::Shapes;
use crate::world::World;

/// The name every teller's place starts with.
pub const PLACE: &str = "teller_";
/// The prop each of this module's tellers stands beside.
pub const LOGS: &str = "logs";

// ---- data/rumours.json -----------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LineSpec {
    pub id: String,
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RevealSpec {
    /// a place's kind in data/places.json
    pub place: String,
    /// the string that names it on the map
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RumourSpec {
    pub id: String,
    pub of: String,
    pub route: String,
    pub line: String,
    #[serde(default)]
    pub reveals: Option<RevealSpec>,
    pub told_by: Vec<String>,
    pub until: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TellerSpec {
    pub id: String,
    /// a roadside villager: its stop's id in data/roadside.json
    #[serde(default)]
    pub stop: Option<String>,
    /// a person of this module's own: who they are (`who_<who>`)
    #[serde(default)]
    pub who: Option<String>,
    #[serde(default)]
    pub at: Option<[i32; 2]>,
    #[serde(default)]
    pub yaw_deg: i32,
    #[serde(default)]
    pub logs: [i32; 2],
    #[serde(default)]
    pub logs_yaw_deg: i32,
    #[serde(default)]
    pub look: u8,
    #[serde(default)]
    pub hello: Option<String>,
    #[serde(default)]
    pub closing: Option<String>,
}

/// `data/rumours.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub lines: Vec<LineSpec>,
    pub rumours: Vec<RumourSpec>,
    pub tellers: Vec<TellerSpec>,
    pub footprints_m: BTreeMap<String, [i32; 2]>,
    pub talk_cm: i32,
    pub keep_clear_m: i32,
}

/// The tellers who are not people of this module's own.
const OTHERS: [&str; 3] = ["merchant", "traveller", "trainer"];

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let s: Spec =
            serde_json::from_str(json).map_err(|e| format!("The rumours file could not be read: {e}"))?;
        for l in &s.lines {
            for c in &l.steps {
                check_condition(c).map_err(|e| format!("The rumours file's line \"{}\" {e}", l.id))?;
            }
        }
        for r in &s.rumours {
            if !s.lines.iter().any(|l| l.id == r.line) {
                return Err(format!(
                    "The rumours file's rumour \"{}\" moves on a line there is none of: \"{}\".",
                    r.id, r.line
                ));
            }
            for t in &r.told_by {
                if !s.tellers.iter().any(|k| &k.id == t) {
                    return Err(format!(
                        "The rumours file's rumour \"{}\" is told by \"{t}\", who is not among its tellers.",
                        r.id
                    ));
                }
            }
            if r.told_by.is_empty() {
                return Err(format!("The rumours file's rumour \"{}\" is told by no one.", r.id));
            }
            for c in &r.until {
                check_condition(c).map_err(|e| format!("The rumours file's rumour \"{}\" {e}", r.id))?;
            }
        }
        for t in &s.tellers {
            let own = t.who.is_some() && t.at.is_some();
            let stop = t.stop.is_some();
            if !(own || stop || OTHERS.contains(&t.id.as_str())) {
                return Err(format!(
                    "The rumours file's teller \"{}\" is neither a roadside villager, the merchant, the traveller, the trainer, nor a person with a place.",
                    t.id
                ));
            }
        }
        Ok(s)
    }
}

/// The rumours file the game ships with, read once.
pub fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| Spec::parse(include_str!("../../../data/rumours.json")).expect("data/rumours.json"))
}

// ---- conditions ---------------------------------------------------------------------

/// What makes a step hold or a rumour moot: `has:<item>`, `knows:imbue`,
/// `woke:<point>` or `woke:any`, `reached:all`; `a|b` holds when either does.
fn check_condition(c: &str) -> Result<(), String> {
    for one in c.split('|') {
        let ok = match one.split_once(':') {
            Some(("has", id)) => Item::from_id(id).is_some(),
            Some(("knows", "imbue")) => true,
            Some(("woke", id)) => id == "any" || crate::travel::spec().points.iter().any(|p| p.id == id),
            Some(("reached", "all")) => true,
            _ => false,
        };
        if !ok {
            return Err(format!("names a condition there is none of: \"{one}\"."));
        }
    }
    Ok(())
}

/// Whether condition `c` holds in `w`.
pub fn holds(w: &World, c: &str) -> bool {
    c.split('|').any(|one| match one.split_once(':') {
        Some(("has", id)) => Item::from_id(id).is_some_and(|i| w.inventory.has(i)),
        Some(("knows", "imbue")) => crate::imbue::learned(w),
        Some(("woke", "any")) => w.travel.points.iter().any(|p| p.woken),
        Some(("woke", id)) => crate::travel::spec()
            .points
            .iter()
            .position(|p| p.id == id)
            .and_then(|k| w.travel.points.get(k))
            .is_some_and(|p| p.woken),
        Some(("reached", "all")) => w.travel.points.iter().all(|p| p.reached),
        _ => false,
    })
}

// ---- the state ------------------------------------------------------------------------

/// One quest line, opened by a rumour.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Line {
    /// its id in data/rumours.json's `lines`
    pub id: String,
    /// the id of the last rumour heard for it
    pub latest: String,
    /// the steps (their conditions, as data/rumours.json writes them) that
    /// have held, in the order they came to: a step once held stays held
    pub held: Vec<String>,
    pub done: bool,
}

/// Everything the rumours keep, by stable ids (P10's save writes it).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct State {
    /// rumour ids heard, in the order heard
    pub heard: Vec<String>,
    /// teller ids who have said their first lines
    pub met: Vec<String>,
    /// quest lines open or done, in the order opened
    pub lines: Vec<Line>,
    /// place kinds (data/places.json) a rumour has marked on the map
    pub marks: Vec<String>,
}

impl State {
    pub fn heard(&self, id: &str) -> bool {
        self.heard.iter().any(|h| h == id)
    }

    pub fn line(&self, id: &str) -> Option<&Line> {
        self.lines.iter().find(|l| l.id == id)
    }
}

/// Whether rumour `r` is moot: something it lists has come to hold, or its
/// line is done.
pub fn moot(w: &World, r: &RumourSpec) -> bool {
    r.until.iter().any(|c| holds(w, c)) || w.rumours.line(&r.line).is_some_and(|l| l.done)
}

/// The rumours `teller` would tell now, in order: none during the opening;
/// otherwise those it knows that are neither heard nor moot.
pub fn offered(w: &World, teller: &str) -> Vec<&'static RumourSpec> {
    if w.story.opening() {
        return Vec::new();
    }
    spec()
        .rumours
        .iter()
        .filter(|r| r.told_by.iter().any(|t| t == teller))
        .filter(|r| !w.rumours.heard(&r.id) && !moot(w, r))
        .collect()
}

/// Hears rumour `r`: its line opens (or takes it as its latest), its place
/// is marked, and a line over the game says where it went.
pub fn hear(w: &mut World, r: &RumourSpec) {
    let st = &mut w.rumours;
    if !st.heard(&r.id) {
        st.heard.push(r.id.clone());
    }
    let opened = match st.lines.iter_mut().find(|l| l.id == r.line) {
        Some(l) => {
            l.latest = r.id.clone();
            false
        }
        None => {
            st.lines.push(Line {
                id: r.line.clone(),
                latest: r.id.clone(),
                held: Vec::new(),
                done: false,
            });
            true
        }
    };
    let marked = match &r.reveals {
        Some(m) if !st.marks.contains(&m.place) => {
            st.marks.push(m.place.clone());
            true
        }
        _ => false,
    };
    advance(w);
    if marked {
        crate::story::banner(w, "rumour_marked");
    } else if opened {
        crate::story::banner(w, "rumour_noted");
    }
}

/// The next rumour `teller` tells, heard as it is told: its string key.
pub fn tell(w: &mut World, teller: &str) -> Option<String> {
    let r = *offered(w, teller).first()?;
    hear(w, r);
    Some(format!("rumour_{}", r.id))
}

fn teller(id: &str) -> Option<&'static TellerSpec> {
    spec().tellers.iter().find(|t| t.id == id)
}

/// What `teller` says when talked to: its first lines (`first`, the first
/// time only), then the next rumour; with no rumour left, its closing line
/// (once it has said anything at all). The opening tells no rumour.
pub fn talk_lines(w: &mut World, id: &str, first: &[&str]) -> Vec<String> {
    let met = w.rumours.met.iter().any(|m| m == id);
    let mut out: Vec<String> = Vec::new();
    if !met {
        out.extend(first.iter().map(|l| l.to_string()));
        if let Some(h) = teller(id).and_then(|t| t.hello.as_ref()) {
            out.push(h.clone());
        }
    }
    let opening = w.story.opening();
    match tell(w, id) {
        Some(line) => out.push(line),
        None if met || out.is_empty() => {
            if let Some(c) = teller(id).and_then(|t| t.closing.as_ref()) {
                if !opening {
                    out.push(c.clone());
                }
            }
        }
        None => {}
    }
    if !opening && !met {
        w.rumours.met.push(id.to_string());
    }
    out
}

/// A roadside villager talked to (`roadside::use_stop`): its own lines the
/// first time, then a rumour, then the next, then its closing line.
pub fn talk_roadside(w: &mut World, stop: &str, says: &[&str]) {
    let Some(t) = spec().tellers.iter().find(|t| t.stop.as_deref() == Some(stop)) else {
        crate::ui::say(w, "villager", says);
        return;
    };
    let lines = talk_lines(w, &t.id, says);
    let refs: Vec<&str> = lines.iter().map(|l| l.as_str()).collect();
    crate::ui::say(w, "villager", &refs);
}

/// The rumour the merchant, the traveller or the trainer adds after its own
/// line, if it has one to tell.
pub fn after(w: &mut World, id: &str) -> Option<String> {
    tell(w, id)
}

// ---- quest lines ----------------------------------------------------------------------

fn line_spec(id: &str) -> Option<&'static LineSpec> {
    spec().lines.iter().find(|l| l.id == id)
}

/// Each open line's steps counted again; a line whose steps all hold is
/// done, and a line over the game says so.
fn advance(w: &mut World) {
    let mut closed = None;
    for k in 0..w.rumours.lines.len() {
        let l = &w.rumours.lines[k];
        if l.done {
            continue;
        }
        let Some(ls) = line_spec(&l.id) else {
            continue;
        };
        let now: Vec<&String> = ls
            .steps
            .iter()
            .filter(|c| !l.held.contains(c) && holds(w, c))
            .collect();
        let l = &mut w.rumours.lines[k];
        l.held.extend(now.into_iter().cloned());
        if l.held.len() >= ls.steps.len() {
            l.done = true;
            closed = Some(l.id.clone());
        }
    }
    if let Some(id) = closed {
        crate::story::banner(w, &format!("line_{id}_done"));
    }
}

/// Step 7, after travel: the open lines move on as their steps come to hold.
pub fn step(w: &mut World) {
    advance(w);
}

// ---- the people of this module's own -----------------------------------------------

/// One of this module's tellers, standing in the world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub teller: &'static TellerSpec,
    /// where they stand, on the ground
    pub at: V3,
    pub yaw: i32,
}

/// The tellers with places of their own.
pub fn own() -> impl Iterator<Item = &'static TellerSpec> {
    spec().tellers.iter().filter(|t| t.who.is_some() && t.at.is_some())
}

/// Each person standing, their feet on the ground.
pub fn people(w: &World) -> Vec<Person> {
    own()
        .map(|t| {
            let a = t.at.expect("own");
            let mut at = v3(a[0] * ONE, 0, a[1] * ONE);
            at.y = w.ground(at.x, at.z);
            Person {
                teller: t,
                at,
                yaw: deg(t.yaw_deg),
            }
        })
        .collect()
}

/// Adds each person to the layout as a place of their own (`teller_<id>`),
/// the woodpile beside them its one prop, so the ground is flattened under
/// it and them, trees keep off and the pile is a wall.
pub fn lay(layout: &mut Layout) {
    let sp = spec();
    for (k, f) in &sp.footprints_m {
        layout.footprints_m.entry(k.clone()).or_insert(*f);
    }
    for t in own() {
        layout.places.push(PlaceSpec {
            kind: format!("{PLACE}{}", t.id),
            centre: t.at.expect("own"),
            flat_m: None,
            clearing_m: 5,
            props: vec![PropSpec {
                prop: LOGS.into(),
                at: t.logs,
                yaw_deg: t.logs_yaw_deg,
                inside: false,
                moving: false,
            }],
            fences: Vec::new(),
            pond: None,
        });
    }
}

/// How far E reaches a person from.
pub fn talk_from() -> Fx {
    spec().talk_cm * ONE / 100
}

/// The nearest person E would talk to, and how far as `story::nearest_use`
/// weighs it (from their edge).
pub fn reach(w: &World) -> Option<(usize, Fx)> {
    let me = w.player.pos;
    let from = talk_from();
    people(w)
        .iter()
        .enumerate()
        .map(|(k, p)| (k, p.at.sub(me).len_xz()))
        .filter(|&(_, d)| d <= from)
        .map(|(k, d)| (k, (d - from + crate::story::REACH).max(0)))
        .min_by_key(|&(_, d)| d)
}

/// The prompt's key for E by person `k`.
pub fn prompt_key(k: usize) -> String {
    let who = own().nth(k).and_then(|t| t.who.clone()).unwrap_or_default();
    format!("does_talk_{who}")
}

/// E by person `k`.
pub fn talk(w: &mut World, k: usize) {
    let Some(t) = own().nth(k) else {
        return;
    };
    let lines = talk_lines(w, &t.id, &[]);
    let refs: Vec<&str> = lines.iter().map(|l| l.as_str()).collect();
    crate::ui::say(w, t.who.as_deref().unwrap_or("villager"), &refs);
}

/// The people, standing by their woodpiles in villagers' clothes.
pub fn draw(w: &World, _alpha: Fx, look: &Look, out: &mut Shapes) {
    for p in people(w) {
        let pose = models::rest_pose(Figure::Villager, p.at, p.yaw);
        let dress = models::Dress {
            worn: 0,
            variant: p.teller.look % models::VILLAGER_VARIANTS,
        };
        models::figure_dressed(Figure::Villager, &pose, dress, look, out);
    }
}

// ---- the screens --------------------------------------------------------------------

/// A mark a rumour put on the map: its id (`rumour_<place>`), the string
/// that names it, where it is, and whether the map round it is still
/// unwalked (a question mark until it is).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RumourMark {
    pub place: String,
    pub label: String,
    pub at: V3,
    pub unseen: bool,
}

/// The places rumours have marked, in the order marked.
pub fn marks(w: &World) -> Vec<RumourMark> {
    w.rumours
        .marks
        .iter()
        .filter_map(|place| {
            let r = spec()
                .rumours
                .iter()
                .find_map(|r| r.reveals.as_ref().filter(|m| &m.place == place))?;
            let at = crate::places::at(w, place)?;
            let (cx, cz) = crate::ui::Fog::cell_of(at.x, at.z);
            Some(RumourMark {
                place: place.clone(),
                label: r.label.clone(),
                at,
                unseen: !w.ui.fog.seen(cx, cz),
            })
        })
        .collect()
}

/// One quest line as the Quest screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineView {
    pub id: String,
    pub title: String,
    /// "2 of 5" for a line of more than one step, else empty
    pub progress: String,
    pub text: String,
    pub done: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub done_label: String,
}

/// The quest lines, open ones first in the order opened, then those done.
pub fn lines(w: &World, s: &Strings) -> Vec<LineView> {
    let view = |l: &Line| {
        let steps = line_spec(&l.id).map(|ls| ls.steps.len()).unwrap_or(1);
        LineView {
            id: l.id.clone(),
            title: s.get(&format!("line_{}", l.id)),
            progress: if steps > 1 {
                crate::api::fill(
                    &s.get("line_progress"),
                    &[("n", l.held.len().to_string()), ("of", steps.to_string())],
                )
            } else {
                String::new()
            },
            text: if l.done {
                s.get(&format!("line_{}_done", l.id))
            } else {
                s.get(&format!("rumour_{}", l.latest))
            },
            done: l.done,
            done_label: if l.done { s.get("line_done") } else { String::new() },
        }
    };
    let st = &w.rumours.lines;
    st.iter()
        .filter(|l| !l.done)
        .chain(st.iter().filter(|l| l.done))
        .map(view)
        .collect()
}

// ---- act ------------------------------------------------------------------------------

/// `rumours.<name>`, for tests and views: `hear <id>` hears a rumour as if
/// told; `look <teller> [metres]` stands the player before one of this
/// module's people, looking at them.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    match name {
        "hear" => {
            let id = value.trim();
            let r = spec()
                .rumours
                .iter()
                .find(|r| r.id == id)
                .ok_or_else(|| format!("There is no rumour \"{id}\"."))?;
            hear(w, r);
            Ok(())
        }
        "look" => {
            let mut parts = value.split_whitespace();
            let id = parts.next().unwrap_or("");
            let back: i32 = parts.next().and_then(|t| t.parse().ok()).unwrap_or(4);
            let p = people(w)
                .into_iter()
                .find(|p| p.teller.id == id)
                .ok_or_else(|| format!("There is no one to talk to called \"{id}\"."))?;
            let eye = p.at.add(crate::geom::facing(p.yaw, 0).scale(fx::int(back)));
            let yaw = yaw_of(p.at.sub(eye));
            let yaw_deg = (yaw as i64 * 360).div_euclid(fx::TURN as i64);
            crate::places::act(w, "go", &format!("{},{} {} -6", eye.x / ONE, eye.z / ONE, yaw_deg))
        }
        _ => Err(format!("Unknown action \"rumours.{name}\".")),
    }
}

#[cfg(test)]
mod tests;
