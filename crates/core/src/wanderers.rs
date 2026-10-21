//! `wanderers` (M7 polish, P9, T15): foes met between the major areas.
//!
//! The person: "there needs to be a bit more in the overworld in between
//! these major areas ... plus, add more random enemies in the overworld".
//!
//! # Where they are
//!
//! `data/wanderers.json` names the ways between the major areas (the
//! bridge, the crossroads, the trail's end, the camp, the hut, the shrine,
//! the hollow tree, the scarecrow), each along the road, a trail or across
//! the open. A way counts only where it is clear of the keep distances: the
//! village's 700 m (the roadside part's band, T12), the camp's 400 m, a
//! waystone's, the trainer's and the traveller's, and every other place's.
//! That run is cut into equal parts no longer than `every_m` (a run of
//! `least_m` or more into two at least), and each cut is a slot, moved up
//! to `jitter_m` either way. The kinds then take slots in the file's order,
//! each a count drawn from its range and slots drawn from those it fits
//! (`place`): one or two ambushes beside a trail, one or two bandit patrols
//! on the east road or a trail (the merchant's beat has the west road), two
//! to four held sites (an orc fire, a bandit lean-to), and orc pairs roaming
//! loops in the rest. All
//! of it comes from the world's seed through `rng.rs` and the land, so the
//! same seed gives the same world (CW-4); it is worked out the first time
//! it is needed (`ensure`), and `scene` works the same list out for the
//! held sites' props.
//!
//! # When they stand (CW-11)
//!
//! Only once the opening is over (the player has crossed the boundary) and
//! never while the player is down. An encounter stands when the player is
//! within `wake_m` of it, if the foes standing within `wake_m` of the
//! player, every hostile foe counted, stay at `most_near` or under; past
//! `park_m` it is parked: its foes are taken out of the world (the last
//! fighters are dropped from `World::foes`; one with a later fighter after
//! it is held still and its place reused by the next to stand), so a parked
//! encounter costs a distance a tick. A roaming group parked stands again
//! somewhere on its round, by the tick (`start_at`).
//!
//! # What they do (CW-3)
//!
//! Every foe is M1's, with its own tree. Until it is up, `steer` gives it
//! the keys to walk its round, its patrol or back home, or to stand; once
//! up (it saw the player, or one of its group is hurt) it runs at the
//! player and draws, and its tree fights. It stops and goes back past
//! `leash_m` from its round or home, or within the `live` distances of the
//! village, the trainer, the traveller or the merchant's cart. It moves
//! only through those records.
//!
//! # What is kept (for the save, T16)
//!
//! `State::records` holds one `Record` per encounter, by its `id`: beaten,
//! when, which of its foes are down, whether a held site has been seen (its
//! map mark) and whether its reward was dropped. An id is the encounter's
//! place in `State::list`, the same for the same seed and the same data
//! file. A beaten encounter stays beaten; a roaming group or patrol stands
//! again `returns_after_s` after it was beaten, once the player is away
//! (`returns`); a held site or an ambush does not.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::fx::{self, Fx, ONE};
use crate::geom::{facing, right_of, v3, yaw_of, V3};
use crate::input::{self, Input};
use crate::items::{self, Item};
use crate::look::Look;
use crate::models::{self, Prop};
use crate::pilot::Control;
use crate::player::{self, Sheath};
use crate::pose::Kind;
use crate::prim::MeshTable;
use crate::render::Group;
use crate::rng::Rng;
use crate::story::{self, Gift, Phase};
use crate::world::World;

/// While parked, a round's walkers move on a point of it this often (15 s).
pub const POINT_TICKS: u64 = 900;
/// The spacing of the points a way is measured along.
pub const STEP: Fx = fx::int(4);
/// A foe this near its home is home.
pub const HOME: Fx = ONE * 15 / 10;
/// Ticks in a second.
const SECOND: u64 = 60;

// ---- data/wanderers.json ------------------------------------------------------

/// How a kind moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Moves {
    /// a loop beside and across its way
    Roam,
    /// along its way and back
    Patrol,
    /// hidden by a tree or a rock beside a trail until the player passes
    Ambush,
    /// at a site of its own: a fire, a lean-to
    Hold,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Keep {
    pub village_m: i32,
    pub camp_m: i32,
    pub place_m: i32,
    pub waystone_m: i32,
    pub people_m: i32,
    pub beat_m: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Live {
    pub village_m: i32,
    pub people_m: i32,
}

/// Something left: coins, or a count of an item.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Reward {
    #[serde(default)]
    pub coins: Option<u32>,
    #[serde(default)]
    pub item: Option<String>,
    #[serde(default)]
    pub count: Option<u32>,
}

impl Reward {
    pub fn gift(&self) -> Option<Gift> {
        match (&self.item, self.coins) {
            (Some(id), None) => Some(Gift::Item(Item::from_id(id)?, self.count.unwrap_or(1))),
            (None, Some(n)) => Some(Gift::Coins(n)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PropAt {
    pub prop: String,
    pub at: [i32; 2],
    pub yaw_deg: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KindSpec {
    pub name: String,
    pub moves: Moves,
    pub foes: Vec<String>,
    pub on: Vec<String>,
    /// how many, drawn from this range; none: the rest of the slots
    #[serde(default)]
    pub count: Option<[i32; 2]>,
    #[serde(default)]
    pub cover_m: [i32; 2],
    #[serde(default)]
    pub along_m: i32,
    #[serde(default)]
    pub side_m: [i32; 2],
    #[serde(default)]
    pub clear_m: i32,
    #[serde(default)]
    pub flat_cm: i32,
    #[serde(default)]
    pub props: Vec<PropAt>,
    #[serde(default)]
    pub stand: Vec<[i32; 2]>,
    #[serde(default)]
    pub per_foe: Vec<Reward>,
    #[serde(default)]
    pub cleared: Vec<Reward>,
    #[serde(default)]
    pub mark: Option<String>,
    #[serde(default)]
    pub half_m: i32,
    #[serde(default)]
    pub loop_m: [i32; 2],
    pub returns_after_s: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WaySpec {
    pub from: String,
    pub to: String,
    pub by: String,
}

/// `data/wanderers.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub wake_m: i32,
    pub park_m: i32,
    pub most_near: usize,
    pub keep: Keep,
    pub live: Live,
    pub every_m: i32,
    pub least_m: i32,
    pub jitter_m: i32,
    pub sight_m: i32,
    pub lookout_m: i32,
    pub notice_m: i32,
    pub spring_m: i32,
    pub calm_m: i32,
    pub leash_m: i32,
    pub kinds: Vec<KindSpec>,
    pub ways: Vec<WaySpec>,
}

/// A foe's name in the file: its kind and the look it wears.
pub fn foe_named(name: &str) -> Option<(Kind, Kind)> {
    match name {
        "orc" => Some((Kind::Orc, Kind::Orc)),
        "bandit" => Some((Kind::Bandit, Kind::Bandit)),
        "lookout" => Some((Kind::Bandit, Kind::Lookout)),
        _ => None,
    }
}

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let s: Spec =
            serde_json::from_str(json).map_err(|e| format!("The wanderers file could not be read: {e}"))?;
        let fills = s.kinds.iter().filter(|k| k.count.is_none()).count();
        if fills != 1 {
            return Err("The wanderers file needs one kind without a count, to fill the rest.".into());
        }
        for k in &s.kinds {
            if k.foes.is_empty() || k.foes.len() > 8 {
                return Err(format!(
                    "The wanderers kind \"{}\" needs one to eight foes.",
                    k.name
                ));
            }
            if let Some(f) = k.foes.iter().find(|f| foe_named(f).is_none()) {
                return Err(format!(
                    "The wanderers kind \"{}\" names a foe \"{f}\"; a foe is orc, bandit or lookout.",
                    k.name
                ));
            }
            if let Some(b) =
                k.on.iter()
                    .find(|b| !matches!(b.as_str(), "road" | "trail" | "open"))
            {
                return Err(format!(
                    "The wanderers kind \"{}\" goes on \"{b}\"; a way is road, trail or open.",
                    k.name
                ));
            }
            if let Some(p) = k
                .props
                .iter()
                .find(|p| crate::places::prop_kind(&p.prop).is_none())
            {
                return Err(format!(
                    "The wanderers kind \"{}\" names a prop \"{}\" there is no model for.",
                    k.name, p.prop
                ));
            }
            if k.moves == Moves::Hold && k.stand.len() != k.foes.len() {
                return Err(format!(
                    "The wanderers kind \"{}\" needs a place to stand for each foe.",
                    k.name
                ));
            }
            // CW-6: what an encounter leaves is a consumable, never a
            // weapon or armour piece, and the items file names the route
            for r in k.per_foe.iter().chain(&k.cleared) {
                let Some(g) = r.gift() else {
                    return Err(format!(
                        "The wanderers kind \"{}\" leaves something that is neither coins nor an item: {r:?}.",
                        k.name
                    ));
                };
                if let Gift::Item(i, _) = g {
                    let d = i.def();
                    let ok = matches!(d.kind, items::Kind::Throwable | items::Kind::Supply)
                        && d.routes.iter().any(|r| r == "wanderers");
                    if !ok {
                        return Err(format!(
                            "The wanderers kind \"{}\" leaves {}, which is not a consumable with the wanderers route.",
                            k.name, d.id
                        ));
                    }
                }
            }
        }
        if let Some(w) = s
            .ways
            .iter()
            .find(|w| !matches!(w.by.as_str(), "road" | "trail" | "open"))
        {
            return Err(format!(
                "A wanderers way goes by \"{}\"; a way is road, trail or open.",
                w.by
            ));
        }
        if s.park_m <= s.wake_m || s.every_m <= 0 || s.most_near == 0 {
            return Err(
                "The wanderers file's park_m must pass wake_m, and every_m and most_near be above 0.".into(),
            );
        }
        Ok(s)
    }
}

/// The file the game ships with, read once.
pub fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| {
        Spec::parse(include_str!("../../../data/wanderers.json")).expect("data/wanderers.json")
    })
}

fn m(v: i32) -> Fx {
    fx::int(v)
}

// ---- state --------------------------------------------------------------------

/// One encounter as placed: the same for the same seed and data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encounter {
    /// its place in `State::list`
    pub id: u16,
    /// its kind, by its place in the file's `kinds`
    pub kind: usize,
    pub moves: Moves,
    /// where it is, on the ground: a round's or patrol's middle, an ambush's
    /// cover, a held site's centre
    pub anchor: V3,
    /// a round (a loop) or a patrol's way (walked there and back)
    pub route: Vec<V3>,
    /// each foe's home and heading (ambush and held sites)
    pub homes: Vec<(V3, i32)>,
    /// a held site's props, on the ground
    pub props: Vec<(Prop, V3, i32)>,
    /// which way of the file it is met on, and how far along its run
    pub way: usize,
    pub along: Fx,
}

/// What is kept of an encounter (P10 saves these, by `id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    pub id: u16,
    /// every one of its foes is down; it stays so (`returns` excepted)
    pub beaten: bool,
    /// the tick it was beaten
    pub beaten_at: u64,
    /// which of its foes are down, a bit each in the file's order: one
    /// that is down does not stand again with the rest
    pub fallen: u8,
    /// a held site has been seen: its mark is on the map
    pub seen: bool,
    /// its reward for being beaten lies on the ground
    pub rewarded: bool,
}

/// A foe of an encounter that stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Member {
    /// which of the encounter's foes, and its place in `World::foes`
    pub slot: usize,
    pub foe: usize,
    /// the point of its round it walks to, and which way along a patrol
    pub next: usize,
    pub dir: i32,
    /// ticks left to wait at a point
    pub wait: u32,
    /// the share carried between two key mixes (`story::keys_toward`)
    pub acc: Fx,
    /// what it carried lies on the ground
    pub dropped: bool,
}

/// An encounter that stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// its place in `State::list`
    pub enc: usize,
    pub members: Vec<Member>,
    /// it has seen the player and fights
    pub alarm: bool,
    /// an ambush that has sprung, or a lookout that has shouted
    pub sprung: bool,
    /// chance for its pauses, from the seed, its id and the tick it stood
    pub rng: Rng,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// the world's seed, which places everything
    pub seed: u64,
    /// `list` and `records` are worked out (`ensure`)
    pub placed: bool,
    pub list: Vec<Encounter>,
    pub records: Vec<Record>,
    pub standing: Vec<Standing>,
    /// places in `World::foes` left by a parked encounter, held still until
    /// dropped from the end or reused
    pub free: Vec<usize>,
    /// the cap on foes standing near the player (`most_near`)
    pub most: usize,
}

impl State {
    pub fn new(seed: u64) -> State {
        State {
            seed,
            placed: false,
            list: Vec::new(),
            records: Vec::new(),
            standing: Vec::new(),
            free: Vec::new(),
            most: spec().most_near,
        }
    }
}

impl Default for State {
    fn default() -> State {
        State::new(0)
    }
}

// ---- the ways -----------------------------------------------------------------

/// A way as measured: its points every `STEP` along the part clear of the
/// keep distances, and how far along each is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub way: usize,
    pub by: String,
    pub pts: Vec<V3>,
    pub arc: Vec<Fx>,
}

impl Run {
    pub fn len(&self) -> Fx {
        self.arc.last().copied().unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.pts.len() < 2
    }

    /// The point `s` along, and the way's heading there (over a few points).
    pub fn at(&self, s: Fx) -> (V3, V3) {
        let n = self.pts.len();
        let i = self
            .arc
            .partition_point(|&a| a <= s)
            .saturating_sub(1)
            .min(n.saturating_sub(2));
        let span = (self.arc[i + 1] - self.arc[i]).max(1);
        let t = (((s - self.arc[i]).clamp(0, span) as i64 * ONE as i64) / span as i64) as Fx;
        let p = self.pts[i].lerp(self.pts[i + 1], t);
        let (a, b) = (self.pts[i.saturating_sub(3)], self.pts[(i + 4).min(n - 1)]);
        let d = b.sub(a);
        (p, v3(d.x, 0, d.z).norm())
    }

    /// The nearest of its points to `p`: how far along, how far off.
    pub fn nearest(&self, p: V3) -> (Fx, Fx) {
        self.pts
            .iter()
            .zip(&self.arc)
            .map(|(q, &a)| (q.sub(p).len_xz(), a))
            .min()
            .map(|(d, a)| (a, d))
            .unwrap_or((0, Fx::MAX))
    }
}

/// A chunk of the land, by its index, and its things.
type Chunk = ((i32, i32), Vec<crate::scatter::Item>);

/// What nothing is placed near.
struct Zones {
    village: V3,
    camp: Option<V3>,
    places: Vec<V3>,
    waystones: Vec<V3>,
    people: Vec<V3>,
    beat: Vec<V3>,
    /// every road, lane and trail, worked out once
    roads: Vec<crate::places::Way>,
    /// the land's things by chunk, as each is first asked for
    chunks: std::cell::RefCell<Vec<Chunk>>,
}

fn flat(p: V3) -> V3 {
    v3(p.x, 0, p.z)
}

fn zones(w: &World) -> Zones {
    let layout = &w.land.layout;
    let centre = |k: &str| {
        layout
            .places
            .iter()
            .find(|p| p.kind == k)
            .map(|p| crate::places::metres(p.centre))
    };
    let village = centre("village").unwrap_or(flat(crate::world::START));
    let mut places = Vec::new();
    let mut waystones = Vec::new();
    for p in &layout.places {
        let c = crate::places::metres(p.centre);
        if p.kind.starts_with("waystone") {
            waystones.push(c);
        } else if p.kind != "village" && p.kind != "camp" {
            places.push(c);
        }
    }
    let mut people = Vec::new();
    if let Some(hut) = centre("hut") {
        let t = story::spec().trainer.at;
        people.push(hut.add(v3(m(t[0]), 0, m(t[1]))));
    }
    if let Some(((t, _), _)) = crate::travel::traveller(w) {
        people.push(flat(t));
    }
    let ms = &story::spec().merchant;
    let roads = crate::places::roads(w);
    let beat = roads
        .iter()
        .find(|r| r.kind == "road")
        .map(|r| r.points.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.x >= m(ms.west_x) && p.x <= m(ms.east_x))
        .map(flat)
        .collect();
    Zones {
        village,
        camp: centre("camp"),
        places,
        waystones,
        people,
        beat,
        roads,
        chunks: std::cell::RefCell::new(Vec::new()),
    }
}

fn far_from(p: V3, list: &[V3], d: Fx) -> bool {
    list.iter().all(|q| q.sub(p).len_xz() >= d)
}

/// Clear of the keep distances (but for the merchant's beat).
fn zone_ok(z: &Zones, p: V3) -> bool {
    let k = &spec().keep;
    let p = flat(p);
    p.sub(z.village).len_xz() >= m(k.village_m)
        && z.camp.is_none_or(|c| p.sub(c).len_xz() >= m(k.camp_m))
        && far_from(p, &z.places, m(k.place_m))
        && far_from(p, &z.waystones, m(k.waystone_m))
        && far_from(p, &z.people, m(k.people_m))
}

/// Where a foe, a route or a prop may be: clear of every keep distance and
/// out of the water.
fn allowed(w: &World, z: &Zones, p: V3) -> bool {
    zone_ok(z, p) && far_from(flat(p), &z.beat, m(spec().keep.beat_m)) && !w.land.is_water(p.x, p.z)
}

/// A straight line from `a` to `b` is allowed every few metres.
fn line_ok(w: &World, z: &Zones, a: V3, b: V3) -> bool {
    let n = (b.sub(a).len_xz() / fx::int(8)).max(1);
    (0..=n).all(|k| allowed(w, z, a.lerp(b, k * ONE / n)))
}

fn on_ground(w: &World, p: V3) -> V3 {
    v3(p.x, w.ground(p.x, p.z), p.z)
}

/// The points of a way, before it is cut to its clear run.
fn way_points(w: &World, z: &Zones, sp: &WaySpec) -> Vec<V3> {
    let centre = |k: &str| {
        w.land
            .layout
            .places
            .iter()
            .find(|p| p.kind == k)
            .map(|p| crate::places::metres(p.centre))
    };
    let (Some(a), Some(b)) = (centre(&sp.from), centre(&sp.to)) else {
        return Vec::new();
    };
    if sp.by == "open" {
        let n = (b.sub(a).len_xz() / STEP).max(1);
        return (0..=n).map(|k| on_ground(w, a.lerp(b, k * ONE / n))).collect();
    }
    // the road or trail that passes nearest both ends, between them
    let mut best: Option<(Fx, Vec<V3>)> = None;
    for r in z.roads.iter().filter(|r| r.kind == sp.by) {
        let near = |c: V3| {
            r.points
                .iter()
                .enumerate()
                .map(|(i, p)| (flat(*p).sub(c).len_xz(), i))
                .min()
                .unwrap_or((Fx::MAX, 0))
        };
        let ((da, ia), (db, ib)) = (near(a), near(b));
        let score = da.saturating_add(db);
        if best.as_ref().is_none_or(|(s, _)| score < *s) {
            let mut pts: Vec<V3> = r.points[ia.min(ib)..=ia.max(ib)].to_vec();
            if ia > ib {
                pts.reverse();
            }
            best = Some((score, pts));
        }
    }
    best.map(|(_, p)| p).unwrap_or_default()
}

/// Every way of the file as measured: the longest stretch of it clear of
/// the keep distances.
pub fn runs(w: &World) -> Vec<Run> {
    runs_in(w, &zones(w))
}

fn runs_in(w: &World, z: &Zones) -> Vec<Run> {
    let mut out = Vec::new();
    for (k, sp) in spec().ways.iter().enumerate() {
        let pts = way_points(w, z, sp);
        let mut best: (usize, usize) = (0, 0);
        let mut start = None;
        for i in 0..=pts.len() {
            let ok = i < pts.len() && zone_ok(z, pts[i]);
            match (ok, start) {
                (true, None) => start = Some(i),
                (false, Some(s)) => {
                    if i - s > best.1 - best.0 {
                        best = (s, i);
                    }
                    start = None;
                }
                _ => {}
            }
        }
        let pts: Vec<V3> = pts[best.0..best.1].to_vec();
        let mut arc = Vec::with_capacity(pts.len());
        let mut s = 0;
        for i in 0..pts.len() {
            if i > 0 {
                s += pts[i].sub(pts[i - 1]).len_xz();
            }
            arc.push(s);
        }
        out.push(Run {
            way: k,
            by: sp.by.clone(),
            pts,
            arc,
        });
    }
    out
}

// ---- placing ------------------------------------------------------------------

/// A place for an encounter on a run.
#[derive(Debug, Clone, Copy)]
struct Slot {
    run: usize,
    s: Fx,
    p: V3,
    /// along the way, and to its right
    t: V3,
    n: V3,
    /// which side a site or a one-sided loop goes: 1 right, −1 left
    side: i32,
}

/// The things of the land within `r` of `c` (`scatter::items`, chunk by chunk).
fn things_near(w: &World, z: &Zones, c: V3, r: Fx) -> Vec<crate::scatter::Item> {
    let side = w.land.spec.scatter.chunk_m * ONE;
    let (i0, i1) = ((c.x - r).div_euclid(side), (c.x + r).div_euclid(side));
    let (j0, j1) = ((c.z - r).div_euclid(side), (c.z + r).div_euclid(side));
    let mut out = Vec::new();
    for j in j0..=j1 {
        for i in i0..=i1 {
            let mut memo = z.chunks.borrow_mut();
            let k = match memo.iter().position(|(key, _)| *key == (i, j)) {
                Some(k) => k,
                None => {
                    memo.push(((i, j), crate::scatter::items(&w.land, i, j)));
                    memo.len() - 1
                }
            };
            out.extend(memo[k].1.iter().copied().filter(|t| t.at.sub(c).len_xz() <= r));
        }
    }
    out
}

/// A point `x` metres right and `z` toward `yaw` of `c` (Q16).
fn local(c: V3, yaw: i32, x: Fx, z: Fx) -> V3 {
    c.add(right_of(yaw).scale(x)).add(facing(yaw, 0).scale(z))
}

fn build_roam(w: &World, z: &Zones, k: &KindSpec, sl: &Slot) -> Option<Encounter> {
    let (h, o) = (m(k.loop_m[0]), m(k.loop_m[1]));
    let side = sl.side;
    // across the way first; beside it, one side then the other, if that
    // comes too near the merchant's beat
    for (o1, o2) in [
        (-o, o),
        (m(70) * side, m(110) * side),
        (-m(70) * side, -m(110) * side),
    ] {
        let corner = |a: Fx, b: Fx| on_ground(w, sl.p.add(sl.t.scale(a)).add(sl.n.scale(b)));
        let route = vec![corner(-h, o1), corner(h, o1), corner(h, o2), corner(-h, o2)];
        let ok = (0..route.len()).all(|i| line_ok(w, z, route[i], route[(i + 1) % route.len()]));
        if ok {
            return Some(Encounter {
                id: 0,
                kind: 0,
                moves: Moves::Roam,
                anchor: on_ground(w, sl.p.add(sl.n.scale((o1 + o2) / 2))),
                route,
                homes: Vec::new(),
                props: Vec::new(),
                way: 0,
                along: sl.s,
            });
        }
    }
    None
}

fn build_patrol(w: &World, z: &Zones, k: &KindSpec, run: &Run, sl: &Slot) -> Option<Encounter> {
    let h = m(k.half_m);
    let (mid, _) = run.nearest(sl.p);
    let i0 = run.arc.partition_point(|&a| a < mid);
    // the stretch round the slot that is clear, within `half_m` either way
    let ok = |i: usize| (run.arc[i] - mid).abs() <= h && allowed(w, z, run.pts[i]);
    if i0 >= run.pts.len() || !ok(i0) {
        return None;
    }
    let (mut a, mut b) = (i0, i0);
    while a > 0 && ok(a - 1) {
        a -= 1;
    }
    while b + 1 < run.pts.len() && ok(b + 1) {
        b += 1;
    }
    if run.arc[b] - run.arc[a] < m(100) {
        return None;
    }
    let mut route: Vec<V3> = (a..=b).step_by(6).map(|i| run.pts[i]).collect();
    if route.last() != Some(&run.pts[b]) {
        route.push(run.pts[b]);
    }
    Some(Encounter {
        id: 0,
        kind: 0,
        moves: Moves::Patrol,
        anchor: run.pts[i0],
        route,
        homes: Vec::new(),
        props: Vec::new(),
        way: 0,
        along: sl.s,
    })
}

fn build_ambush(w: &World, z: &Zones, k: &KindSpec, run: &Run, sl: &Slot) -> Option<Encounter> {
    let along = m(k.along_m);
    let (near, far) = (m(k.cover_m[0]), m(k.cover_m[1]));
    let cover = things_near(w, z, sl.p, along + far)
        .into_iter()
        .filter(|t| t.thing != crate::scatter::Thing::Rock || t.size >= 140)
        .filter_map(|t| {
            let (s, d) = run.nearest(t.at);
            ((s - sl.s).abs() <= along && d >= near && d <= far).then_some((
                (s - sl.s).abs(),
                t.at.x,
                t.at.z,
                t,
                s,
            ))
        })
        .min_by_key(|c| (c.0, c.1, c.2))?;
    let (t, s) = (cover.3, cover.4);
    let (q, _) = run.at(s);
    let away = flat(t.at.sub(q)).norm();
    let yaw = yaw_of(q.sub(t.at));
    let across = right_of(yaw);
    let behind = t.at.add(away.scale(ONE * 18 / 10));
    let mut homes = Vec::new();
    for j in 0..k.foes.len() as i32 {
        let off = (2 * j - (k.foes.len() as i32 - 1)) * ONE * 13 / 10;
        let p = on_ground(w, behind.add(across.scale(off)));
        if !allowed(w, z, p) {
            return None;
        }
        homes.push((p, yaw));
    }
    Some(Encounter {
        id: 0,
        kind: 0,
        moves: Moves::Ambush,
        anchor: on_ground(w, t.at),
        route: Vec::new(),
        homes,
        props: Vec::new(),
        way: 0,
        along: s,
    })
}

/// A held site's ground: allowed, clear of trees, rocks and roads, level.
fn site_ok(w: &World, z: &Zones, k: &KindSpec, c: V3) -> bool {
    let clear = m(k.clear_m);
    let h = w.ground(c.x, c.z);
    let ring = (0..8).map(|i| c.add(facing(fx::deg(45 * i), 0).scale(clear)));
    let level = (0..8)
        .map(|i| c.add(facing(fx::deg(45 * i), 0).scale(ONE * 6)))
        .all(|p| (w.ground(p.x, p.z) - h).abs() * 100 <= ONE * k.flat_cm);
    let roads = &z.roads;
    allowed(w, z, c)
        && ring.into_iter().all(|p| allowed(w, z, p))
        && level
        && roads.iter().all(|r| {
            r.points
                .iter()
                .all(|p| flat(*p).sub(flat(c)).len_xz() >= r.half + clear)
        })
        && things_near(w, z, c, clear + ONE * 2).is_empty()
}

fn build_hold(w: &World, z: &Zones, k: &KindSpec, sl: &Slot) -> Option<Encounter> {
    let (d0, d1) = (m(k.side_m[0]), m(k.side_m[1]));
    for side in [sl.side, -sl.side] {
        for d in [d0, (d0 + d1) / 2, d1] {
            let c = on_ground(w, sl.p.add(sl.n.scale(d * side)));
            if !site_ok(w, z, k, c) {
                continue;
            }
            // the site faces the way
            let yaw = yaw_of(sl.p.sub(c));
            let homes: Vec<(V3, i32)> = k
                .stand
                .iter()
                .map(|s| (on_ground(w, local(c, yaw, m(s[0]), m(s[1]))), yaw))
                .collect();
            if !homes.iter().all(|(p, _)| allowed(w, z, *p)) {
                continue;
            }
            let props = k
                .props
                .iter()
                .filter_map(|p| {
                    let at = on_ground(w, local(c, yaw, m(p.at[0]), m(p.at[1])));
                    Some((
                        crate::places::prop_kind(&p.prop)?,
                        at,
                        fx::wrap(yaw + fx::deg(p.yaw_deg)),
                    ))
                })
                .collect();
            return Some(Encounter {
                id: 0,
                kind: 0,
                moves: Moves::Hold,
                anchor: c,
                route: Vec::new(),
                homes,
                props,
                way: 0,
                along: sl.s,
            });
        }
    }
    None
}

fn build(w: &World, z: &Zones, kind: usize, run: &Run, sl: &Slot) -> Option<Encounter> {
    let k = &spec().kinds[kind];
    if !k.on.contains(&run.by) {
        return None;
    }
    let mut e = match k.moves {
        Moves::Roam => build_roam(w, z, k, sl),
        Moves::Patrol => build_patrol(w, z, k, run, sl),
        Moves::Ambush => build_ambush(w, z, k, run, sl),
        Moves::Hold => build_hold(w, z, k, sl),
    }?;
    e.kind = kind;
    e.way = run.way;
    Some(e)
}

/// Every encounter for `seed`: the slots along each run, then the kinds in
/// the file's order (module doc).
pub fn place(w: &World, seed: u64) -> Vec<Encounter> {
    let sp = spec();
    let z = zones(w);
    let runs = runs_in(w, &z);
    let mut rng = Rng::new(seed ^ 0x3a4d_e7e5);
    let mut slots = Vec::new();
    for (r, run) in runs.iter().enumerate() {
        if run.is_empty() {
            continue;
        }
        let len = run.len();
        let every = m(sp.every_m);
        // the fewest equal parts no longer than `every_m`: one slot between two
        let mut parts = ((len + every - 1) / every).max(1);
        if parts == 1 && len >= m(sp.least_m) {
            parts = 2;
        }
        for k in 1..parts {
            let jitter = rng.between(-sp.jitter_m, sp.jitter_m) * ONE;
            let s = ((len as i64 * k as i64 / parts as i64) as Fx + jitter).clamp(0, len);
            let (p, t) = run.at(s);
            let n = right_of(yaw_of(t));
            let side = if rng.chance(1, 2) { 1 } else { -1 };
            slots.push(Slot {
                run: r,
                s,
                p,
                t,
                n,
                side,
            });
        }
    }
    let mut taken: Vec<Option<Encounter>> = vec![None; slots.len()];
    for (kind, k) in sp.kinds.iter().enumerate() {
        let Some([lo, hi]) = k.count else {
            continue;
        };
        let mut want = rng.between(lo, hi);
        // the open slots in an order drawn from the seed
        let mut order: Vec<usize> = (0..slots.len()).filter(|&i| taken[i].is_none()).collect();
        for i in (1..order.len()).rev() {
            let j = rng.below(i as u32 + 1) as usize;
            order.swap(i, j);
        }
        for i in order {
            if want == 0 {
                break;
            }
            if let Some(e) = build(w, &z, kind, &runs[slots[i].run], &slots[i]) {
                taken[i] = Some(e);
                want -= 1;
            }
        }
    }
    if let Some(fill) = sp.kinds.iter().position(|k| k.count.is_none()) {
        for i in 0..slots.len() {
            if taken[i].is_none() {
                taken[i] = build(w, &z, fill, &runs[slots[i].run], &slots[i]);
            }
        }
    }
    let mut out: Vec<Encounter> = taken.into_iter().flatten().collect();
    for (i, e) in out.iter_mut().enumerate() {
        e.id = i as u16;
    }
    out
}

/// The list is worked out once, the first time it is needed.
pub fn ensure(w: &mut World) {
    if w.wanderers.placed {
        return;
    }
    let list = place(w, w.wanderers.seed);
    w.wanderers.records = list
        .iter()
        .map(|e| Record {
            id: e.id,
            beaten: false,
            beaten_at: 0,
            fallen: 0,
            seen: false,
            rewarded: false,
        })
        .collect();
    w.wanderers.list = list;
    w.wanderers.placed = true;
}

/// A world made again from the same seed (a dream, `story::wake`) keeps the
/// placing, with every record new.
pub fn carry(old: &State, new: &mut State) {
    if old.placed && old.seed == new.seed {
        new.list = old.list.clone();
        new.records = old
            .records
            .iter()
            .map(|r| Record {
                id: r.id,
                beaten: false,
                beaten_at: 0,
                fallen: 0,
                seen: false,
                rewarded: false,
            })
            .collect();
        new.placed = true;
    }
}

// ---- the tick -----------------------------------------------------------------

/// Encounters stand only once the opening is over and the player is up.
pub fn open(w: &World) -> bool {
    w.story.phase == Phase::Free && w.story.crossed.is_some() && !w.player_down()
}

fn all(n: usize) -> u8 {
    ((1u16 << n) - 1) as u8
}

/// No state of the tick names fighter `id`: it can be dropped or reused.
fn clear(w: &World, id: usize) -> bool {
    let id8 = id as u8;
    let b = &w.blades;
    !(w.wounds.pools.iter().any(|p| p.0 == id8)
        || w.throwables.flying.iter().any(|f| f.by == id8)
        || w.throwables.landed.iter().any(|f| f.by == id8)
        || w.orbs.casts.iter().any(|c| c.caster == id8)
        || w.orbs.flying.iter().any(|o| o.caster == id8)
        || b.clashing
            .iter()
            .chain(&b.touching)
            .any(|p| p.0 == id8 || p.1 == id8)
        || b.hits.iter().any(|h| h.by == id8 || h.on == id8)
        || b.clashes.iter().any(|c| c.a == id8 || c.b == id8)
        || w.feel.downed.contains(&id8)
        || w.me.engaged == Some(id8)
        || w.foes.iter().any(|f| f.fighter.engaged == Some(id8)))
}

/// Step 7 (after travel): who has fallen, the alarms, what is seen; then
/// parking, the ones that return, and the ones that stand.
pub fn step(w: &mut World) {
    if !open(w) {
        return;
    }
    ensure(w);
    tend(w);
    seen(w);
    park(w);
    trim(w);
    returns(w);
    wake(w);
}

fn me(w: &World) -> V3 {
    w.player.pos
}

/// Each standing encounter: its fallen drop what they carry, a whole group
/// down is beaten, and the alarm.
fn tend(w: &mut World) {
    let sp = spec();
    for li in 0..w.wanderers.standing.len() {
        let st = w.wanderers.standing[li].clone();
        let enc = w.wanderers.list[st.enc].clone();
        let k = &sp.kinds[enc.kind];
        let mut last = enc.anchor;
        for (mi, mb) in st.members.iter().enumerate() {
            let Some(foe) = w.foes.get(mb.foe) else {
                continue;
            };
            if !foe.fighter.health.down() || mb.dropped {
                continue;
            }
            let (at, yaw) = (foe.body.pos, foe.body.facing);
            last = at;
            for (j, r) in k.per_foe.iter().enumerate() {
                if let Some(g) = r.gift() {
                    let turn = fx::deg(90 + 60 * j as i32);
                    story::drop(w, g, at.add(facing(yaw + turn, 0).scale(ONE * 8 / 10)));
                }
            }
            w.wanderers.standing[li].members[mi].dropped = true;
            w.wanderers.records[st.enc].fallen |= 1 << mb.slot;
        }
        let rec = w.wanderers.records[st.enc];
        if rec.fallen == all(k.foes.len()) && !rec.beaten {
            let r = &mut w.wanderers.records[st.enc];
            r.beaten = true;
            r.beaten_at = w.tick;
            if !r.rewarded {
                r.rewarded = true;
                let at = if enc.moves == Moves::Hold {
                    enc.anchor
                } else {
                    last
                };
                for (j, g) in k.cleared.iter().filter_map(Reward::gift).enumerate() {
                    let turn = fx::deg(40 + 75 * j as i32);
                    story::drop(w, g, at.add(facing(turn, 0).scale(ONE * 14 / 10)));
                }
                if enc.moves == Moves::Hold {
                    story::banner(w, "wanderer_cleared");
                }
            }
        }
        alarm(w, li, &enc, k);
    }
}

/// Up once the player is seen (by the kind's way of seeing) or one of the
/// group is hurt; calm once the player is `calm_m` from all of it.
fn alarm(w: &mut World, li: usize, enc: &Encounter, k: &KindSpec) {
    let sp = spec();
    let me = me(w);
    let st = &w.wanderers.standing[li];
    let mut seen = false;
    let mut near = false;
    for mb in &st.members {
        let Some(foe) = w.foes.get(mb.foe) else {
            continue;
        };
        let f = &foe.fighter;
        if f.health.down() {
            continue;
        }
        let gap = foe.body.pos.sub(me).len_xz();
        let sight = match enc.moves {
            Moves::Roam | Moves::Patrol => m(sp.sight_m),
            Moves::Ambush => m(sp.spring_m),
            Moves::Hold if k.foes[mb.slot.min(k.foes.len() - 1)] == "lookout" => m(sp.lookout_m),
            Moves::Hold => m(sp.notice_m),
        };
        seen |= gap < sight || f.health.hp < f.health.most;
        near |= gap < m(sp.calm_m);
    }
    let was = st.alarm;
    let down = w.player_down();
    let st = &mut w.wanderers.standing[li];
    if seen && !down {
        st.alarm = true;
    } else if !near {
        st.alarm = false;
    }
    if st.alarm && !was && !st.sprung {
        st.sprung = true;
        match enc.moves {
            Moves::Ambush => story::banner(w, "wanderer_ambush"),
            Moves::Hold if k.foes.iter().any(|f| f == "lookout") => story::banner(w, "wanderer_lookout"),
            _ => {}
        }
    }
}

/// A held site comes onto the map once the player has been within
/// `lookout_m` twice over of it.
fn seen(w: &mut World) {
    let me = me(w);
    let d = m(spec().lookout_m * 2);
    for i in 0..w.wanderers.list.len() {
        if !w.wanderers.records[i].seen && w.wanderers.list[i].anchor.sub(me).len_xz() < d {
            w.wanderers.records[i].seen = true;
        }
    }
}

/// Past `park_m`, an encounter's foes leave the world: each is held still
/// and its place freed (`trim` drops it from the end).
fn park(w: &mut World) {
    let far = m(spec().park_m);
    let me = me(w);
    let mut li = 0;
    while li < w.wanderers.standing.len() {
        let enc = w.wanderers.standing[li].enc;
        if w.wanderers.list[enc].anchor.sub(me).len_xz() <= far {
            li += 1;
            continue;
        }
        let st = w.wanderers.standing.remove(li);
        for mb in st.members {
            if let Some(f) = w.foes.get_mut(mb.foe) {
                f.brain.control = Control::Still;
                w.wanderers.free.push(mb.foe);
            }
        }
    }
}

/// Freed places at the end of `World::foes` are dropped, once nothing
/// names them.
fn trim(w: &mut World) {
    while let Some(k) = w.foes.len().checked_sub(1) {
        if !w.wanderers.free.contains(&k) || !clear(w, k + 1) {
            break;
        }
        w.foes.pop();
        w.wanderers.free.retain(|&f| f != k);
    }
    // a place past the end (a test duel cleared the foes) is gone already
    let n = w.foes.len();
    w.wanderers.free.retain(|&f| f < n);
    for st in &mut w.wanderers.standing {
        st.members.retain(|mb| mb.foe < n);
    }
}

/// A beaten roaming group or patrol stands again `returns_after_s` after
/// it was beaten, once the player is past `park_m` from it.
fn returns(w: &mut World) {
    let sp = spec();
    let me = me(w);
    for i in 0..w.wanderers.list.len() {
        let e = &w.wanderers.list[i];
        let after = sp.kinds[e.kind].returns_after_s as u64 * SECOND;
        let r = w.wanderers.records[i];
        if r.beaten
            && after > 0
            && w.tick >= r.beaten_at + after
            && e.anchor.sub(me).len_xz() > m(sp.park_m)
            && !w.wanderers.standing.iter().any(|s| s.enc == i)
        {
            let r = &mut w.wanderers.records[i];
            r.beaten = false;
            r.fallen = 0;
            r.rewarded = false;
        }
    }
}

/// The hostile foes standing within `wake_m` of the player.
pub fn near_count(w: &World) -> usize {
    let me = me(w);
    let d = m(spec().wake_m);
    w.foes
        .iter()
        .enumerate()
        .filter(|(k, f)| {
            !w.wanderers.free.contains(k)
                && f.fighter.team != w.me.team
                && !f.fighter.health.down()
                && f.body.pos.sub(me).len_xz() < d
        })
        .count()
}

/// Within `wake_m`, nearest first, an encounter not beaten stands, if the
/// cap allows its foes.
fn wake(w: &mut World) {
    let me = me(w);
    let d = m(spec().wake_m);
    let mut due: Vec<(Fx, usize)> = (0..w.wanderers.list.len())
        .filter(|&i| !w.wanderers.records[i].beaten && !w.wanderers.standing.iter().any(|s| s.enc == i))
        .map(|i| (w.wanderers.list[i].anchor.sub(me).len_xz(), i))
        .filter(|(g, _)| *g < d)
        .collect();
    due.sort();
    let mut near = near_count(w);
    for (_, i) in due {
        let k = &spec().kinds[w.wanderers.list[i].kind];
        let fallen = w.wanderers.records[i].fallen;
        let need = (0..k.foes.len()).filter(|j| fallen & (1 << j) == 0).count();
        if near + need > w.wanderers.most {
            continue;
        }
        near += need;
        stand(w, i);
    }
}

/// Where a round's walkers are when it stands: a point of its round by the
/// tick, so a parked group has walked on (`POINT_TICKS` a point).
pub fn start_at(e: &Encounter, tick: u64) -> (usize, i32) {
    let n = e.route.len().max(1) as u64;
    let q = tick / POINT_TICKS;
    match e.moves {
        Moves::Patrol if n > 1 => {
            let period = 2 * (n - 1);
            let q = q % period;
            if q < n - 1 {
                (q as usize, 1)
            } else {
                ((period - q) as usize, -1)
            }
        }
        _ => ((q % n) as usize, 1),
    }
}

/// A freed place nothing names, or none.
fn take_free(w: &mut World) -> Option<usize> {
    let mut free = w.wanderers.free.clone();
    free.sort();
    let k = free.into_iter().find(|&k| k < w.foes.len() && clear(w, k + 1))?;
    w.wanderers.free.retain(|&f| f != k);
    Some(k)
}

fn stand(w: &mut World, i: usize) {
    let e = w.wanderers.list[i].clone();
    let k = &spec().kinds[e.kind];
    let fallen = w.wanderers.records[i].fallen;
    let (first, dir) = start_at(&e, w.tick);
    let mut members = Vec::new();
    for (j, name) in k.foes.iter().enumerate() {
        if fallen & (1 << j) != 0 {
            continue;
        }
        let (kind, look) = foe_named(name).unwrap_or((Kind::Orc, Kind::Orc));
        let (at, yaw, next) = match e.moves {
            Moves::Hold | Moves::Ambush => {
                let (p, y) = e.homes[j.min(e.homes.len() - 1)];
                (p, y, 0)
            }
            Moves::Roam | Moves::Patrol => {
                let n = e.route.len();
                let next = match e.moves {
                    Moves::Patrol => (first as i32 + dir).clamp(0, n as i32 - 1) as usize,
                    _ => (first + 1) % n,
                };
                let ahead = e.route[next].sub(e.route[first]);
                let yaw = yaw_of(ahead);
                let p = e.route[first]
                    .add(right_of(yaw).scale(ONE * 25 / 10 * j as i32))
                    .sub(facing(yaw, 0).scale(ONE * 2 * j as i32));
                (p, yaw, next)
            }
        };
        let mut at = at;
        at.y = crate::solid::surface(w, at.x, at.z);
        let mut foe = story::sheathed_foe(w, kind, look, at, yaw);
        if let Some(slot) = take_free(w) {
            let last = w.foes.len() - 1;
            w.foes.swap(slot, last);
            w.foes.pop();
            foe = slot;
        }
        members.push(Member {
            slot: j,
            foe,
            next,
            dir,
            wait: 0,
            acc: 0,
            dropped: false,
        });
    }
    let seed = w.wanderers.seed ^ ((e.id as u64) << 24) ^ w.tick;
    w.wanderers.standing.push(Standing {
        enc: i,
        members,
        alarm: false,
        sprung: false,
        rng: Rng::new(seed),
    });
}

// ---- steering (CW-3) ------------------------------------------------------------

/// How far `p` is from the line `a`–`b`, across the ground.
fn seg_dist(p: V3, a: V3, b: V3) -> Fx {
    let (dx, dz) = ((b.x - a.x) as i128, (b.z - a.z) as i128);
    let (px, pz) = ((p.x - a.x) as i128, (p.z - a.z) as i128);
    let dd = dx * dx + dz * dz;
    let t = (px * dx + pz * dz).clamp(0, dd.max(1));
    let (qx, qz) = if dd == 0 {
        (0, 0)
    } else {
        (dx * t / dd, dz * t / dd)
    };
    v3((px - qx) as Fx, 0, (pz - qz) as Fx).len_xz()
}

/// How far a foe has strayed: from its home, or from its round's line.
fn strayed(e: &Encounter, slot: usize, at: V3) -> Fx {
    match e.moves {
        Moves::Hold | Moves::Ambush => e.homes[slot.min(e.homes.len() - 1)].0.sub(at).len_xz(),
        Moves::Roam => (0..e.route.len())
            .map(|i| seg_dist(at, e.route[i], e.route[(i + 1) % e.route.len()]))
            .min()
            .unwrap_or(0),
        Moves::Patrol => e
            .route
            .windows(2)
            .map(|p| seg_dist(at, p[0], p[1]))
            .min()
            .unwrap_or(0),
    }
}

/// Where a foe may fight: clear of the `live` distances of the village, the
/// trainer, the traveller and the merchant's cart.
fn fights_here(w: &World, at: V3) -> bool {
    let l = &spec().live;
    let near = |p: V3| p.sub(at).len_xz() < m(l.people_m);
    let trainer = crate::trainer::id(w).map(|id| player::body(w, id).pos);
    let traveller = crate::travel::traveller(w).map(|((t, _), _)| t);
    let cart = w.merchant.active().then_some(w.merchant.at);
    story::centre(w).sub(at).len_xz() >= m(l.village_m)
        && ![trainer, traveller, cart].into_iter().flatten().any(near)
}

/// Step 2b, after the story's: each standing foe that its tree is not
/// fighting with gets its record from here (CW-3: still only a record).
pub fn steer(w: &mut World, inputs: &mut [Input]) {
    if w.story.still || w.wanderers.standing.is_empty() {
        return;
    }
    let sp = spec();
    let me = me(w);
    let down = w.player_down();
    for li in 0..w.wanderers.standing.len() {
        let enc = w.wanderers.list[w.wanderers.standing[li].enc].clone();
        let alarm = w.wanderers.standing[li].alarm;
        for mi in 0..w.wanderers.standing[li].members.len() {
            let mut mb = w.wanderers.standing[li].members[mi];
            let Some(foe) = w.foes.get(mb.foe) else {
                continue;
            };
            if foe.brain.control != Control::Tree || foe.fighter.health.down() {
                continue;
            }
            let f = &foe.fighter;
            let at = foe.body.pos;
            let gap = me.sub(at).len_xz();
            let fight = alarm
                && !down
                && gap < player::FOE_RANGE
                && strayed(&enc, mb.slot, at) < m(sp.leash_m)
                && fights_here(w, at);
            let sheathe = || Input {
                keys: story::sheath_keys(f, false),
                ..Input::default()
            };
            let input = if fight {
                let drawn = f.drawn();
                let far = if drawn {
                    gap > story::SHEATHE_AT
                } else {
                    gap > story::DRAW_AT
                };
                if far {
                    if f.sheath == Sheath::Drawn {
                        sheathe()
                    } else if f.sheath == Sheath::Sheathed {
                        let run = (w.tick + mb.foe as u64) % story::RUN_OF < story::RUN_TICKS;
                        let keys = story::keys_toward(f.look_yaw, me.sub(at), &mut mb.acc);
                        Input {
                            keys: keys | if run { input::RUN } else { 0 },
                            ..Input::default()
                        }
                    } else {
                        Input::default()
                    }
                } else if f.sheath == Sheath::Sheathed {
                    Input {
                        keys: story::sheath_keys(f, true)
                            | story::keys_toward(f.look_yaw, me.sub(at), &mut mb.acc),
                        ..Input::default()
                    }
                } else {
                    // drawn and near: its own tree fights
                    inputs.get(mb.foe).copied().unwrap_or_default()
                }
            } else if f.drawn() {
                sheathe()
            } else {
                let goal = match enc.moves {
                    Moves::Hold | Moves::Ambush => Some(enc.homes[mb.slot.min(enc.homes.len() - 1)].0),
                    Moves::Roam | Moves::Patrol => {
                        let g = enc.route[mb.next.min(enc.route.len() - 1)];
                        // the second of a pair walks a little to the side
                        let side = right_of(f.look_yaw).scale(ONE * 2 * mb.slot as i32);
                        Some(g.add(side))
                    }
                };
                match goal {
                    Some(g)
                        if g.sub(at).len_xz() > HOME || matches!(enc.moves, Moves::Roam | Moves::Patrol) =>
                    {
                        if g.sub(at).len_xz() <= story::ARRIVED * 2
                            && matches!(enc.moves, Moves::Roam | Moves::Patrol)
                        {
                            if mb.wait > 0 {
                                mb.wait -= 1;
                            } else {
                                let n = enc.route.len();
                                mb.next = match enc.moves {
                                    Moves::Patrol => {
                                        let next = mb.next as i32 + mb.dir;
                                        if next < 0 || next >= n as i32 {
                                            mb.dir = -mb.dir;
                                        }
                                        (mb.next as i32 + mb.dir).clamp(0, n as i32 - 1) as usize
                                    }
                                    _ => (mb.next + 1) % n,
                                };
                                let st = &mut w.wanderers.standing[li];
                                mb.wait = st.rng.between(story::PAUSE.0, story::PAUSE.1) as u32;
                            }
                            Input::default()
                        } else {
                            Input {
                                keys: story::keys_toward(f.look_yaw, g.sub(at), &mut mb.acc),
                                ..Input::default()
                            }
                        }
                    }
                    _ => Input::default(),
                }
            };
            if let Some(i) = inputs.get_mut(mb.foe) {
                *i = input;
            }
            w.wanderers.standing[li].members[mi] = mb;
        }
    }
}

// ---- the map and the scene --------------------------------------------------------

/// The map's marks for the held sites seen (M4's `places::markers`): each
/// keyed by its kind's `mark`, so `place_<mark>` names it.
pub fn marks(w: &World) -> Vec<(&'static str, V3)> {
    let sp = spec();
    w.wanderers
        .list
        .iter()
        .zip(&w.wanderers.records)
        .filter(|(_, r)| r.seen)
        .filter_map(|(e, _)| sp.kinds[e.kind].mark.as_deref().map(|k| (k, e.anchor)))
        .collect()
}

/// The held sites' props, a group a site, drawn by distance and faded like
/// the places' (`places::FADED`).
pub fn scene(w: &World, look: &Look, meshes: &mut MeshTable, out: &mut Vec<Group>) {
    let list = if w.wanderers.placed {
        w.wanderers.list.clone()
    } else {
        place(w, w.wanderers.seed)
    };
    for e in list.iter().filter(|e| !e.props.is_empty()) {
        let reach = e
            .props
            .iter()
            .map(|(p, at, _)| at.sub(e.anchor).len_xz() + models::prop_radius(*p))
            .max()
            .unwrap_or(0);
        let mut g = Group::new(
            &format!("{}wanderer_{}", crate::places::FADED, e.id),
            e.anchor,
            reach,
        );
        for (p, at, yaw) in &e.props {
            models::prop(*p, *at, *yaw, look, meshes, &mut g);
        }
        out.push(g);
    }
}

// ---- acts -----------------------------------------------------------------------

/// `wanderers.<name>`, for views and tests: `near` takes a kind's name and
/// an optional count (`orc_fire 1`, the second) and puts the player on the
/// way 45 m short of that encounter, facing it, when play starts (as
/// `places.go`); `seen` puts every held site on the map.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    ensure(w);
    match name {
        "near" => {
            let mut parts = value.split([' ', ',']).filter(|p| !p.is_empty());
            let kind = parts.next().unwrap_or("");
            let nth: usize = match parts.next() {
                Some(t) => t
                    .parse()
                    .map_err(|_| format!("Which one is a whole number from 0, not \"{t}\"."))?,
                None => 0,
            };
            let Some(ki) = spec().kinds.iter().position(|k| k.name == kind) else {
                let names: Vec<&str> = spec().kinds.iter().map(|k| k.name.as_str()).collect();
                return Err(format!(
                    "There is no kind of encounter \"{kind}\"; the kinds are {}.",
                    names.join(", ")
                ));
            };
            let Some(e) = w.wanderers.list.iter().filter(|e| e.kind == ki).nth(nth).cloned() else {
                return Err(format!("This world has no {kind} {nth}."));
            };
            let runs = runs(w);
            let run = &runs[e.way];
            let (s, _) = run.nearest(e.anchor);
            let (p, _) = run.at((s - m(45)).max(0));
            let yaw = yaw_of(e.anchor.sub(p));
            let deg = (fx::wrap(yaw) as i64 * 360 / fx::TURN as i64) as i32;
            crate::places::act(
                w,
                "go",
                &format!("{},{} {deg} -8", fx::floor(p.x), fx::floor(p.z)),
            )
        }
        "seen" => {
            for r in &mut w.wanderers.records {
                r.seen = true;
            }
            Ok(())
        }
        _ => Err(format!("Unknown action \"wanderers.{name}\".")),
    }
}

#[cfg(test)]
mod tests;
