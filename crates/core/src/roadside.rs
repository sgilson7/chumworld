//! `roadside` (M7 roadside, P7, T12): things to find and do beside the ways
//! out of the village a player most likely runs out by.
//!
//! The person: "make sure there is stuff to do see roads on the sides of the
//! village you are most likely to run out of at the beginning of the game".
//!
//! # Which sides
//!
//! The opening's own data says where a player goes (`data/story.json`): the
//! raid comes in from the south (`raid.comes_from`) and the four villagers
//! flee north-west, north-east, north and west (`villagers[].flee`), so a
//! player running from the orcs runs the villagers' way. Each side in
//! `data/roadside.json` names its villager; its bearing is that villager's
//! last flee point seen from the village centre (`sides`).
//!
//! # Where the stops stand
//!
//! A side's path is the road, lane or trail in `data/places.json` that
//! crosses the opening's boundary (`story::BOUNDARY`, 250 m) within
//! `match_deg` of the side's bearing, followed outward; where none leaves
//! that way, the villager's flee line carried straight on. A stop stands on
//! the path where it is `at_m` from the village centre, `off_m` to the
//! right of it (walking out), facing it. So the stops follow whatever roads
//! the land part lays (T6) without being moved by hand.
//!
//! Every stop with a model joins the layout as a place of its own
//! (`lay`, called from `places::Layout::shipped`), named `roadside_<id>`:
//! its ground is flattened, trees keep off it, its model is baked with the
//! places' props and handed to the detail levels, and its footprint is a
//! wall (`solid.rs`). The watchtower and the tor are climbed (`climbs`):
//! their tops and ramps are surfaces a body stands on, and a body more than
//! `STEP` below one is kept out of it as from a wall (`guard`), so no one
//! walks under the platform or through the rock.
//!
//! # What there is to do
//!
//! - an overturned cart (`cart`, `burning_cart`): E searches it once and
//!   takes what it holds (`gives`); what the player cannot carry is left on
//!   the ground beside it;
//! - a straggler: an orc left behind, which stands by the burning cart and
//!   fights a player who comes near (M1's orc and its tree; it moves only
//!   through its input record, CW-3). It stands only once the opening is
//!   over (`story::Phase::Free`), and drops its coins when down;
//! - a villager who got away, who says where to go (`says`, strings);
//! - the watchtower and the tor, to climb for the view; rocks lie at the
//!   tor's foot;
//! - a well: E drinks, and health is whole again;
//! - a lone tree, the landmark on the north lane.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{v3, yaw_of, Affine, V3};
use crate::input::Input;
use crate::items::Item;
use crate::look::Look;
use crate::models::{self, Prop};
use crate::pilot::Control;
use crate::places::{Layout, PlaceSpec, PropSpec};
use crate::player::Sheath;
use crate::pose::Kind as Figure;
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::solid::{self, Wall};
use crate::story::{self, Gift};
use crate::world::World;

/// The name every roadside place's kind starts with.
pub const PLACE: &str = "roadside_";
/// A path is sampled this often.
pub const PATH_STEP: Fx = fx::int(2);
/// A body more than this below a climbed surface cannot step onto it.
pub const STEP: Fx = ONE / 2;
/// The straggler stands once the player is this near its place.
pub const SPAWN_AT: Fx = fx::int(150);
/// It sees the player this far, and gives up past this far from home.
pub const SIGHT: Fx = fx::int(24);
pub const LEASH: Fx = fx::int(60);
/// E reaches a cart's middle from this far, the well's and a villager's
/// from these.
pub const CART_FROM: Fx = ONE * 24 / 10;
pub const WELL_FROM: Fx = ONE * 19 / 10;
pub const TALK_FROM: Fx = ONE * 22 / 10;
/// The smoke over a burning cart: puffs, each rising for `PUFF_TICKS`,
/// to `SMOKE_RISE` over the cart, drifting `SMOKE_DRIFT` with the wind.
pub const PUFFS: i64 = 24;
pub const PUFF_TICKS: i64 = 720;
pub const SMOKE_RISE: Fx = fx::int(22);
pub const SMOKE_DRIFT: Fx = fx::int(7);
/// How strongly a fresh puff adds to what is behind it.
pub const SMOKE_GLOW: Fx = ONE * 45 / 100;
/// How far a landmark is seen from (`seen_from`): its height over the
/// ground there, over its distance, at least one part in this many (about a
/// degree, 18 pixels of a 1080-pixel view).
pub const SEEN_PARTS: i32 = 60;
/// A stop that draws the eye from the boundary stands at least this tall.
pub const LANDMARK: Fx = fx::int(6);
/// The eye over the ground, looking out from the boundary.
pub const EYE: Fx = ONE * 16 / 10;

// ---- data/roadside.json ------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GiftSpec {
    #[serde(default)]
    pub item: Option<String>,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub coins: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StopSpec {
    pub id: String,
    pub kind: String,
    pub at_m: i32,
    pub off_m: i32,
    #[serde(default)]
    pub gives: Vec<GiftSpec>,
    #[serde(default)]
    pub says: Vec<String>,
    #[serde(default)]
    pub coins: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SideSpec {
    pub side: String,
    pub villager: usize,
    pub stops: Vec<StopSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ClearSpec {
    pub at: [i32; 2],
    pub m: i32,
}

/// `data/roadside.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub match_deg: i32,
    pub path_to_m: i32,
    pub keep_clear: Vec<ClearSpec>,
    pub footprints_m: BTreeMap<String, [i32; 2]>,
    pub sides: Vec<SideSpec>,
}

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let s: Spec =
            serde_json::from_str(json).map_err(|e| format!("The roadside file could not be read: {e}"))?;
        let villagers = story::spec().villagers.len();
        for side in &s.sides {
            if side.villager >= villagers {
                return Err(format!(
                    "The roadside file's side \"{}\" names a villager the story file does not have.",
                    side.side
                ));
            }
            for stop in &side.stops {
                let Some(kind) = Kind::named(&stop.kind) else {
                    return Err(format!(
                        "The roadside file's stop \"{}\" is of a kind there is none of: \"{}\".",
                        stop.id, stop.kind
                    ));
                };
                for g in &stop.gives {
                    if gift(g).is_none() {
                        return Err(format!(
                            "The roadside file's stop \"{}\" gives something that is neither an item nor coins.",
                            stop.id
                        ));
                    }
                }
                if kind == Kind::Villager && stop.says.is_empty() {
                    return Err(format!(
                        "The roadside file's villager \"{}\" says nothing.",
                        stop.id
                    ));
                }
                if let Some(p) = kind.prop_name() {
                    if !s.footprints_m.contains_key(p) && !["well"].contains(&p) {
                        return Err(format!("The roadside file has no footprint for \"{p}\"."));
                    }
                }
            }
        }
        Ok(s)
    }
}

/// The roadside file the game ships with, read once.
pub fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| Spec::parse(include_str!("../../../data/roadside.json")).expect("data/roadside.json"))
}

fn gift(g: &GiftSpec) -> Option<Gift> {
    match (&g.item, g.coins) {
        (Some(id), None) => Item::from_id(id).map(|i| Gift::Item(i, g.count.unwrap_or(1))),
        (None, Some(n)) => Some(Gift::Coins(n)),
        _ => None,
    }
}

/// What a stop is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Cart,
    BurningCart,
    Straggler,
    Villager,
    Tower,
    Rocks,
    Well,
    Tree,
}

impl Kind {
    pub fn named(name: &str) -> Option<Kind> {
        Some(match name {
            "cart" => Kind::Cart,
            "burning_cart" => Kind::BurningCart,
            "straggler" => Kind::Straggler,
            "villager" => Kind::Villager,
            "tower" => Kind::Tower,
            "rocks" => Kind::Rocks,
            "well" => Kind::Well,
            "tree" => Kind::Tree,
            _ => return None,
        })
    }

    /// The prop it stands as, by its name in the places' layout.
    pub fn prop_name(self) -> Option<&'static str> {
        Some(match self {
            Kind::Cart => "wreck",
            Kind::BurningCart => "wreck_burnt",
            Kind::Tower => "watchtower",
            Kind::Rocks => "tor",
            Kind::Well => "well",
            Kind::Tree => "lone_tree",
            Kind::Straggler | Kind::Villager => return None,
        })
    }

    pub fn prop(self) -> Option<Prop> {
        self.prop_name().and_then(crate::places::prop_kind)
    }

    /// Searched with E.
    pub fn searched(self) -> bool {
        matches!(self, Kind::Cart | Kind::BurningCart)
    }
}

// ---- the sides and their paths ---------------------------------------------------

/// A side a player leaves the village by: its name, its bearing (a heading,
/// `geom` conventions), and its path outward from the boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Side {
    pub name: String,
    pub yaw: i32,
    /// the path is a road, lane or trail (not the flee line)
    pub on_road: bool,
    pub path: Vec<V3>,
}

/// Where a stop stands (whole metres, height 0 until placed on the ground),
/// facing its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stop {
    pub id: String,
    pub kind: Kind,
    pub side: usize,
    /// its index in its side's `stops`
    pub index: usize,
    pub at: V3,
    pub yaw_deg: i32,
    /// the point on the path it stands beside
    pub on_path: V3,
}

impl Stop {
    pub fn spec(&self) -> &'static StopSpec {
        &spec().sides[self.side].stops[self.index]
    }

    pub fn yaw(&self) -> i32 {
        deg(self.yaw_deg)
    }

    /// Its place's kind in the layout.
    pub fn place(&self) -> String {
        format!("{PLACE}{}", self.id)
    }
}

/// The village centre in a layout (the place of kind "village").
pub fn centre(layout: &Layout) -> V3 {
    layout
        .places
        .iter()
        .find(|p| p.kind == "village")
        .map(|p| crate::places::metres(p.centre))
        .unwrap_or(crate::world::START)
}

/// A villager's flee bearing: its last flee point seen from the village
/// centre (both in the story file's metres round the centre).
pub fn flee_yaw(villager: usize) -> i32 {
    let v = &story::spec().villagers[villager];
    let last = v.flee.last().copied().unwrap_or(v.at);
    yaw_of(v3(last[0] * ONE, 0, last[1] * ONE))
}

/// The raid's bearing: the middle of its entry points seen from the centre.
pub fn raid_yaw() -> i32 {
    let from = &story::spec().raid.comes_from;
    let n = from.len().max(1) as i32;
    let (x, z) = from.iter().fold((0, 0), |(x, z), p| (x + p[0], z + p[1]));
    yaw_of(v3(x / n * ONE, 0, z / n * ONE))
}

fn dense(line: &[[i32; 2]]) -> Vec<V3> {
    let mut out = Vec::new();
    for k in 0..line.len().saturating_sub(1) {
        let (a, b) = (crate::places::metres(line[k]), crate::places::metres(line[k + 1]));
        let n = (b.sub(a).len_xz() / PATH_STEP).max(1);
        for q in 0..n {
            out.push(a.lerp(b, q * ONE / n));
        }
    }
    if let Some(&last) = line.last() {
        out.push(crate::places::metres(last));
    }
    out
}

/// A flat unit direction from `a` to `b`.
fn dir(a: V3, b: V3) -> V3 {
    let d = b.sub(a);
    let d = v3(d.x, 0, d.z);
    if d.len_xz() == 0 {
        v3(0, 0, -ONE)
    } else {
        d.norm()
    }
}

/// Every side in the roadside file, its path found in `layout`.
pub fn sides(layout: &Layout) -> Vec<Side> {
    let sp = spec();
    let c = centre(layout);
    let r = story::BOUNDARY;
    let to = fx::int(sp.path_to_m);
    let radial = |p: V3| p.sub(c).len_xz();
    sp.sides
        .iter()
        .map(|s| {
            let yaw = flee_yaw(s.villager);
            // the road crossing the boundary nearest the side's bearing
            let mut best: Option<(i32, Vec<V3>)> = None;
            for road in &layout.roads {
                let pts = dense(&road.line);
                for k in 0..pts.len().saturating_sub(1) {
                    let (a, b) = (radial(pts[k]) < r, radial(pts[k + 1]) < r);
                    if a == b {
                        continue;
                    }
                    let off = fx::wrap(yaw_of(pts[k].sub(c)) - yaw).abs();
                    if off > deg(sp.match_deg) || best.as_ref().is_some_and(|(o, _)| *o <= off) {
                        continue;
                    }
                    let path: Vec<V3> = if a {
                        pts[k..].to_vec()
                    } else {
                        pts[..=k + 1].iter().rev().copied().collect()
                    };
                    best = Some((off, path));
                }
            }
            let on_road = best.is_some();
            let mut path = best.map(|(_, p)| p).unwrap_or_else(|| {
                let d = crate::geom::facing(yaw, 0);
                vec![c.add(d.scale(r))]
            });
            // carried straight on past the road's end (or from the boundary,
            // the flee line) to `path_to_m`
            let mut guard = 0;
            while path.last().is_some_and(|p| radial(*p) < to) && guard < 2000 {
                let n = path.len();
                let d = if n >= 2 {
                    dir(path[n - 2], path[n - 1])
                } else {
                    crate::geom::facing(yaw, 0)
                };
                let next = path[n - 1].add(d.scale(PATH_STEP));
                path.push(next);
                guard += 1;
            }
            Side {
                name: s.side.clone(),
                yaw,
                on_road,
                path,
            }
        })
        .collect()
}

/// Where every stop stands in `layout`.
pub fn stops(layout: &Layout) -> Vec<Stop> {
    let sp = spec();
    let c = centre(layout);
    let mut out = Vec::new();
    for (si, side) in sides(layout).iter().enumerate() {
        for (k, s) in sp.sides[si].stops.iter().enumerate() {
            let want = fx::int(s.at_m);
            let path = &side.path;
            let i = path
                .iter()
                .position(|p| p.sub(c).len_xz() >= want)
                .unwrap_or(path.len() - 1);
            let d = dir(path[i.saturating_sub(1)], path[(i + 1).min(path.len() - 1)]);
            let right = v3(-d.z, 0, d.x);
            let p = path[i].add(right.scale(fx::int(s.off_m)));
            let round = |v: Fx| (v + ONE / 2).div_euclid(ONE);
            let at = v3(round(p.x) * ONE, 0, round(p.z) * ONE);
            let face = if s.off_m == 0 { d } else { dir(at, path[i]) };
            let yaw_deg =
                ((yaw_of(face) as i64 * 360 + fx::TURN as i64 / 2).div_euclid(fx::TURN as i64)) as i32;
            out.push(Stop {
                id: s.id.clone(),
                kind: Kind::named(&s.kind).expect("checked"),
                side: si,
                index: k,
                at,
                yaw_deg,
                on_path: path[i],
            });
        }
    }
    out
}

/// The room a stop takes round its middle: its footprint's half diagonal,
/// or a person's.
pub fn room(stop: &Stop, layout: &Layout) -> Fx {
    match stop.kind.prop_name().and_then(|p| layout.footprints_m.get(p)) {
        Some(f) => fx::length(f[0] * ONE / 2, f[1] * ONE / 2),
        None => ONE,
    }
}

/// Adds every stop to the layout as a place of its own (`roadside_<id>`),
/// and the stops' footprints.
pub fn lay(layout: &mut Layout) {
    let sp = spec();
    for (k, f) in &sp.footprints_m {
        layout.footprints_m.entry(k.clone()).or_insert(*f);
    }
    for stop in stops(layout) {
        let props = match stop.kind.prop_name() {
            Some(name) => vec![PropSpec {
                prop: name.into(),
                at: [0, 0],
                yaw_deg: stop.yaw_deg,
                inside: false,
                moving: false,
            }],
            None => Vec::new(),
        };
        let clear = match stop.kind.prop() {
            Some(p) => models::prop_radius(p) / ONE + 2,
            None => 3,
        };
        layout.places.push(PlaceSpec {
            kind: stop.place(),
            centre: [stop.at.x / ONE, stop.at.z / ONE],
            flat_m: None,
            clearing_m: clear,
            props,
            fences: Vec::new(),
            pond: None,
        });
    }
}

// ---- what bodies climb and meet ---------------------------------------------------

/// A surface to climb: a rectangle (a `Wall`'s), its height rising evenly
/// along its own z from `low` at −hz to `high` at +hz (a flat top when the
/// two are equal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Climb {
    pub rect: Wall,
    pub low: Fx,
    pub high: Fx,
}

impl Climb {
    /// Its height along its own z, `lz` held to its ends.
    fn along(&self, lz: Fx) -> Fx {
        let lz = lz.clamp(-self.rect.hz, self.rect.hz);
        let t = fx::div(lz + self.rect.hz, (2 * self.rect.hz).max(1));
        self.low + fx::mul(self.high - self.low, t)
    }

    /// Its height at (x, z), inside it.
    pub fn height(&self, x: Fx, z: Fx) -> Option<Fx> {
        if !self.rect.holds(x, z, 0) {
            return None;
        }
        Some(self.along(self.rect.local(x, z).1))
    }
}

fn mm(v: i32) -> Fx {
    models::mm(v)
}

/// A rectangle in a stop's own frame (metres from its middle, mm), in the
/// world.
fn rect(at: V3, yaw: i32, x: i32, z: i32, hx: i32, hz: i32) -> Wall {
    let (s, c) = (fx::sin(yaw), fx::cos(yaw));
    let (lx, lz) = (mm(x), mm(z));
    Wall {
        x: at.x + fx::mul(lx, c) + fx::mul(lz, s),
        z: at.z - fx::mul(lx, s) + fx::mul(lz, c),
        hx: mm(hx),
        hz: mm(hz),
        yaw,
    }
}

/// Each stop of the layout placed on the ground: its foot and its yaw.
fn feet(w: &World) -> Vec<(Stop, V3)> {
    stops(&w.land.layout)
        .into_iter()
        .map(|s| {
            let mut at = s.at;
            at.y = w.ground(at.x, at.z);
            (s, at)
        })
        .collect()
}

/// The surfaces bodies climb: the watchtower's platform and stair, the
/// tor's top and ramp (the numbers of `models/props.rs`).
pub fn climbs(w: &World) -> Vec<Climb> {
    use crate::models::props_numbers as n;
    let mut out = Vec::new();
    for (s, at) in feet(w) {
        let yaw = s.yaw();
        match s.kind {
            Kind::Tower => {
                let top = at.y + mm(n::WATCH_TOP);
                out.push(Climb {
                    rect: rect(at, yaw, 0, n::WATCH_Z, n::WATCH_HALF, n::WATCH_HALF),
                    low: top,
                    high: top,
                });
                let (z0, z1) = (n::WATCH_FOOT, n::WATCH_Z - n::WATCH_HALF);
                out.push(Climb {
                    rect: rect(at, yaw, 0, (z0 + z1) / 2, n::WATCH_STAIR_HALF, (z1 - z0) / 2),
                    low: at.y,
                    high: top,
                });
            }
            Kind::Rocks => {
                let top = at.y + mm(n::TOR_TOP);
                for (x, z, hx, hz) in [(0, 2500, 1600, 1050), (0, 2900, 2200, 900), (0, 1100, 750, 350)] {
                    out.push(Climb {
                        rect: rect(at, yaw, x, z, hx, hz),
                        low: top,
                        high: top,
                    });
                }
                let (z0, z1) = (n::TOR_FOOT, n::TOR_EDGE);
                out.push(Climb {
                    rect: rect(at, yaw, 0, (z0 + z1) / 2, n::TOR_RAMP_HALF, (z1 - z0) / 2),
                    low: at.y,
                    high: top,
                });
            }
            _ => {}
        }
    }
    out
}

/// The walls of what is climbed: posts, rails and boulders, which keep
/// every body out whatever its height (`solid::walls` adds them; the two
/// props' footprints are no walls of their own).
pub fn walls(w: &World) -> Vec<Wall> {
    use crate::models::props_numbers as n;
    let mut out = Vec::new();
    for (s, at) in feet(w) {
        let yaw = s.yaw();
        match s.kind {
            Kind::Tower => {
                let (z0, p) = (n::WATCH_Z, 1450);
                for (x, z) in [(-p, z0 - p), (p, z0 - p), (-p, z0 + p), (p, z0 + p)] {
                    out.push(rect(at, yaw, x, z, 110, 110));
                }
                out.push(rect(at, yaw, 0, z0 + p, 1550, 60));
                for x in [-p, p] {
                    out.push(rect(at, yaw, x, z0, 60, 1550));
                }
                for x in [-1125, 1125] {
                    out.push(rect(at, yaw, x, z0 - p, 380, 60));
                }
                let (f0, f1) = (n::WATCH_FOOT, n::WATCH_Z - n::WATCH_HALF);
                for x in [-n::WATCH_STAIR_HALF - 40, n::WATCH_STAIR_HALF + 40] {
                    out.push(rect(at, yaw, x, (f0 + f1) / 2, 40, (f1 - f0) / 2));
                }
            }
            Kind::Rocks => {
                for (x, z, hx, hz) in [
                    (-2900, 3400, 1000, 1250),
                    (2800, 2200, 900, 1100),
                    (1600, 4900, 1100, 850),
                    (-1500, -4800, 450, 400),
                    (1700, -3600, 600, 550),
                    (300, 4300, 650, 550),
                    (-1900, 1400, 700, 750),
                    (2300, 900, 650, 650),
                ] {
                    out.push(rect(at, yaw, x, z, hx, hz));
                }
                // the rubble along the ramp's sides (as `props.rs` lays it)
                for (i, z) in [-4600, -3100, -1600, -200].iter().enumerate() {
                    let x = if i % 2 == 0 { 1150 } else { -1150 };
                    out.push(rect(at, yaw, x, *z, 300, 600));
                    out.push(rect(at, yaw, -x * 95 / 100, z + 650, 300, 500));
                }
            }
            _ => {}
        }
    }
    out
}

/// Keeps a body whose feet stood at `feet_y` last tick out of every climbed
/// surface more than `STEP` above it, as from a wall: a body walks up a
/// stair or a ramp from its foot and onto the top from its head, but not
/// under a platform or through a rock's side.
pub fn guard(climbs: &[Climb], feet_y: Fx, pos: &mut V3, vel: &mut V3) -> bool {
    let high: Vec<Wall> = climbs
        .iter()
        .filter(|c| c.rect.holds(pos.x, pos.z, solid::RADIUS + ONE))
        .filter(|c| {
            let lz = c.rect.local(pos.x, pos.z).1;
            feet_y + STEP < c.along(lz)
        })
        .map(|c| c.rect)
        .collect();
    solid::keep_out(&high, pos, vel, solid::RADIUS)
}

/// The highest climbed surface at (x, z), if one is there.
pub fn surface(climbs: &[Climb], x: Fx, z: Fx) -> Option<Fx> {
    climbs.iter().filter_map(|c| c.height(x, z)).max()
}

// ---- state ------------------------------------------------------------------------

/// The orc left behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Straggler {
    pub foe: usize,
    pub stop: usize,
    pub home: V3,
    pub yaw: i32,
    pub acc: Fx,
    pub chasing: bool,
    pub dropped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    /// every stop, its foot on the ground
    pub stops: Vec<(Stop, V3)>,
    /// each stop searched (a cart), by index
    pub searched: Vec<bool>,
    pub straggler: Option<Straggler>,
}

/// With the opening (`story::begin`): the stops on the ground, nothing
/// searched, the rocks at the tor's foot; the straggler not yet standing.
pub fn begin(w: &mut World) {
    let stops = feet(w);
    w.roadside = State {
        searched: vec![false; stops.len()],
        stops,
        straggler: None,
    };
    for k in 0..w.roadside.stops.len() {
        let (s, at) = w.roadside.stops[k].clone();
        if s.kind != Kind::Rocks {
            continue;
        }
        // beside the ramp's foot, toward the path
        let yaw = s.yaw();
        let (sn, cs) = (fx::sin(yaw), fx::cos(yaw));
        for (n, g) in s.spec().gives.iter().enumerate() {
            let (lx, lz) = (mm(-1500 + 900 * n as i32), mm(-6400));
            let p = v3(
                at.x + fx::mul(lx, cs) + fx::mul(lz, sn),
                0,
                at.z - fx::mul(lx, sn) + fx::mul(lz, cs),
            );
            if let Some(gift) = gift(g) {
                // M7 save: by stable key, so a load knows them taken
                let key = crate::save::drop_key(&crate::save::roadside_key(&s.id), n);
                crate::save::drop(w, &key, gift, p);
            }
        }
    }
}

/// A test duel forgets the straggler: its foe is gone with the duel's reset.
pub fn forget(w: &mut World) {
    w.roadside.straggler = None;
}

/// Step 2b: the straggler, sheathed at home until it sees the player; then
/// it runs at them, draws and its own tree fights; past its leash, or with
/// the player down, it walks home and puts its blade away. Only keys (CW-3).
pub fn steer(w: &mut World, inputs: &mut [Input]) {
    let Some(mut s) = w.roadside.straggler else {
        return;
    };
    let Some(foe) = w.foes.get(s.foe) else {
        return;
    };
    if foe.brain.control != Control::Tree || foe.fighter.health.down() {
        return;
    }
    let me = w.player.pos;
    let f = &foe.fighter;
    let at = foe.body.pos;
    let gap = me.sub(at).len_xz();
    let leashed = at.sub(s.home).len_xz() > LEASH;
    s.chasing = !w.player_down() && !leashed && (gap < SIGHT || (s.chasing && gap < SIGHT * 2));
    let input = if s.chasing {
        let drawn = f.drawn();
        let far = if drawn {
            gap > story::SHEATHE_AT
        } else {
            gap > story::DRAW_AT
        };
        if far {
            if f.sheath == Sheath::Drawn {
                Input {
                    keys: story::sheath_keys(f, false),
                    ..Input::default()
                }
            } else if f.sheath == Sheath::Sheathed {
                let run = (w.tick + s.foe as u64) % story::RUN_OF < story::RUN_TICKS;
                let keys = story::keys_toward(f.look_yaw, me.sub(at), &mut s.acc);
                Input {
                    keys: keys | if run { crate::input::RUN } else { 0 },
                    ..Input::default()
                }
            } else {
                Input::default()
            }
        } else if f.sheath == Sheath::Sheathed {
            Input {
                keys: story::sheath_keys(f, true) | story::keys_toward(f.look_yaw, me.sub(at), &mut s.acc),
                ..Input::default()
            }
        } else {
            // drawn and near: its own tree fights
            inputs[s.foe]
        }
    } else if f.drawn() {
        Input {
            keys: story::sheath_keys(f, false),
            ..Input::default()
        }
    } else if at.sub(s.home).len_xz() > ONE * 15 / 10 {
        Input {
            keys: story::keys_toward(f.look_yaw, s.home.sub(at), &mut s.acc),
            ..Input::default()
        }
    } else {
        Input::default()
    };
    inputs[s.foe] = input;
    w.roadside.straggler = Some(s);
}

/// Step 7: the straggler stands once the opening is over and the player
/// comes near; down, it drops its coins.
pub fn step(w: &mut World) {
    if w.roadside.straggler.is_none() && w.story.phase == story::Phase::Free {
        let me = w.player.pos;
        let near = w
            .roadside
            .stops
            .iter()
            .position(|(s, at)| s.kind == Kind::Straggler && at.sub(me).len_xz() < SPAWN_AT);
        if let Some(k) = near {
            let (s, at) = w.roadside.stops[k].clone();
            let mut home = at;
            home.y = solid::surface(w, at.x, at.z);
            let yaw = s.yaw();
            let foe = story::sheathed_foe(w, Figure::Orc, Figure::Orc, home, yaw);
            // M7 save: beaten in the save this world came from, it lies down
            if crate::save::beaten(w, &crate::save::roadside_key(&s.id)) {
                crate::save::lay_down(w, foe);
            }
            w.roadside.straggler = Some(Straggler {
                foe,
                stop: k,
                home,
                yaw,
                acc: 0,
                chasing: false,
                dropped: false,
            });
        }
    }
    if let Some(s) = w.roadside.straggler {
        let down = w.foes.get(s.foe).is_some_and(|f| f.fighter.health.down());
        if down && !s.dropped {
            let f = &w.foes[s.foe];
            let at = f
                .body
                .pos
                .add(crate::geom::facing(f.body.facing + deg(90), 0).scale(ONE * 8 / 10));
            let coins = w.roadside.stops[s.stop].0.spec().coins;
            if coins > 0 {
                // M7 save: by stable key, never laid again once taken
                let key =
                    crate::save::drop_key(&crate::save::roadside_key(&w.roadside.stops[s.stop].0.id), 0);
                crate::save::drop(w, &key, Gift::Coins(coins), at);
            }
            if let Some(t) = &mut w.roadside.straggler {
                t.dropped = true;
            }
        }
    }
}

// ---- E ---------------------------------------------------------------------------

/// The nearest stop E acts on, and how far it is as `story::nearest_use`
/// weighs it (a thing's edge, not its middle).
pub fn reach(w: &World) -> Option<(usize, Fx)> {
    let me = w.player.pos;
    let mut best: Option<(usize, Fx)> = None;
    for (k, (s, at)) in w.roadside.stops.iter().enumerate() {
        let from = match s.kind {
            Kind::Cart | Kind::BurningCart if !w.roadside.searched.get(k).copied().unwrap_or(true) => {
                CART_FROM
            }
            Kind::Well => WELL_FROM,
            Kind::Villager => TALK_FROM,
            _ => continue,
        };
        let d = at.sub(me).len_xz();
        if d > from {
            continue;
        }
        let d = (d - from + story::REACH).max(0);
        if best.is_none_or(|(_, b)| d < b) {
            best = Some((k, d));
        }
    }
    best
}

/// The prompt's key for E at stop `k`.
pub fn prompt_key(w: &World, k: usize) -> &'static str {
    match w.roadside.stops[k].0.kind {
        Kind::Well => "does_drink",
        Kind::Villager => "does_talk_villager",
        _ => "does_search_cart",
    }
}

/// E at stop `k`: a cart is searched (once), the well drunk from, a
/// villager talked to.
pub fn use_stop(w: &mut World, k: usize) {
    let (s, at) = w.roadside.stops[k].clone();
    match s.kind {
        Kind::Cart | Kind::BurningCart => {
            if w.roadside.searched[k] {
                return;
            }
            w.roadside.searched[k] = true;
            let mut left = 0;
            for (n, g) in s.spec().gives.iter().enumerate() {
                match gift(g) {
                    Some(Gift::Coins(c)) => w.inventory.coins += c,
                    Some(Gift::Item(i, count)) => {
                        let took = crate::inventory::give(w, i, count);
                        if took < count {
                            // what does not fit is left beside the cart
                            let p = at.add(
                                crate::geom::facing(s.yaw() + deg(60 * n as i32 - 30), 0)
                                    .scale(ONE * 22 / 10),
                            );
                            story::drop(w, Gift::Item(i, count - took), p);
                            left += 1;
                        }
                    }
                    None => {}
                }
            }
            story::banner(
                w,
                if left > 0 {
                    "story_full"
                } else {
                    "roadside_searched"
                },
            );
        }
        Kind::Well => {
            let h = &mut w.me.health;
            h.hp = h.most;
            story::banner(w, "roadside_drink");
        }
        Kind::Villager => {
            let lines: Vec<&str> = s.spec().says.iter().map(|l| l.as_str()).collect();
            crate::ui::say(w, "villager", &lines);
        }
        _ => {}
    }
}

// ---- drawing -----------------------------------------------------------------------

/// The villagers who got away, standing by their paths, and the smoke over
/// each burning cart (its puffs in a fixed order, CW-18).
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    for (s, at) in &w.roadside.stops {
        match s.kind {
            Kind::Villager => {
                let variant = story::spec().villagers[spec().sides[s.side].villager].look;
                let pose = models::rest_pose(Figure::Villager, *at, s.yaw());
                let dress = models::Dress { worn: 0, variant };
                models::figure_dressed(Figure::Villager, &pose, dress, look, out);
            }
            Kind::BurningCart => smoke(w, *at, alpha, out),
            _ => {}
        }
    }
}

/// One puff of smoke `k` at tick `t` (with `alpha` of the next): its middle
/// over the cart's foot and its size.
pub fn puff(foot: V3, k: i64, tick: u64, alpha: Fx) -> (V3, Fx) {
    let life = PUFF_TICKS * ONE as i64;
    let t = (tick as i64 * ONE as i64 + alpha as i64 + k * life / PUFFS).rem_euclid(life);
    let s = (t / PUFF_TICKS) as Fx;
    // it rises fast at first and slows (s (3 − s) / 2), swells, drifts
    // more as it rises, and each puff wanders a little to its own side
    let rise = fx::mul(SMOKE_RISE, fx::mul(s, 3 * ONE - s) / 2);
    let drift = fx::mul(SMOKE_DRIFT, fx::mul(s, s));
    let wander = fx::mul(fx::sin(deg(137 * k as i32)), fx::mul(ONE * 6 / 10, s));
    let mut size = ONE * 3 / 2 + fx::mul(ONE * 9 / 2, s);
    // the last eighth of its life it thins away
    let end = ONE * 7 / 8;
    if s > end {
        size = fx::mul(size, fx::div(ONE - s, ONE - end));
    }
    let at = foot.add(v3(
        fx::mul(drift, ONE * 8 / 10) + wander,
        ONE + rise,
        -fx::mul(drift, ONE * 6 / 10) + wander / 2,
    ));
    (at, size)
}

fn smoke(w: &World, foot: V3, alpha: Fx, out: &mut Shapes) {
    let t = models::table();
    let (dark, light) = (t.c("smoke"), t.c("smoke_light"));
    for k in 0..PUFFS {
        // a thinned puff is still drawn, so every frame draws as many; the
        // story held still (`story.still`) holds the smoke still too
        let (tick, alpha) = if w.story.still { (0, 0) } else { (w.tick, alpha) };
        let (at, size) = puff(foot, k, tick, alpha);
        let share = fx::div(at.y - foot.y, SMOKE_RISE + ONE).clamp(0, ONE);
        let rgb = [0, 1, 2].map(|i| dark[i] + fx::mul(light[i] - dark[i], share));
        let place = Affine::translate(at)
            .then(&Affine::rot_y(deg(67 * k as i32)))
            .then(&Affine::scale(v3(size, fx::mul(size, ONE * 8 / 10), size)));
        // a soft halo added to what is behind it, thinning as it rises: no
        // depth written, so the order they are drawn in changes no sum
        out.glow(Mesh::Dot, place, rgb, SMOKE_GLOW - fx::mul(SMOKE_GLOW / 2, share));
    }
}

/// How tall a stop stands over its foot, smoke and all.
pub fn top(stop: &Stop) -> Fx {
    match stop.kind {
        Kind::BurningCart => SMOKE_RISE + ONE,
        _ => stop.kind.prop().map(models::prop_top).unwrap_or(0),
    }
}

/// Whether a stop's top is seen from `eye`: tall enough in the view
/// (`SEEN_PARTS`) and over the ground all the way (sampled every 4 m).
pub fn seen_from(w: &World, eye: V3, foot: V3, height: Fx) -> bool {
    let tip = v3(foot.x, foot.y + height, foot.z);
    let d = tip.sub(eye).len_xz();
    if d == 0 || height * SEEN_PARTS < d {
        return false;
    }
    let n = (d / fx::int(4)).max(1);
    (1..n).all(|q| {
        let p = eye.lerp(tip, q * ONE / n);
        w.ground(p.x, p.z) < p.y
    })
}

// ---- the map and acts ----------------------------------------------------------------

/// The map's marks for the landmarks and the well (the overturned carts
/// are left off, so the marks round the village stay readable): (key,
/// where), the key read as `place_<key>` in the strings.
pub fn marks(w: &World) -> Vec<(&'static str, V3)> {
    stops(&w.land.layout)
        .into_iter()
        .filter_map(|s| {
            let key = match s.kind {
                Kind::BurningCart => "wreck_burnt",
                Kind::Tower => "watchtower",
                Kind::Rocks => "tor",
                Kind::Well => "roadside_well",
                Kind::Tree => "lone_tree",
                _ => return None,
            };
            let mut at = s.at;
            at.y = w.ground(at.x, at.z);
            Some((key, at))
        })
        .collect()
}

/// `roadside.<name>`, for the views: `line <side>` stands the player on a
/// side's path at the boundary's line, looking out along it; `look <id>
/// [metres]` before a stop, looking at it; `top <id>` on the watchtower's
/// platform or the tor's top, looking out along its side.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    match name {
        "line" => {
            let side = sides(&w.land.layout)
                .into_iter()
                .find(|s| s.name == value.trim())
                .ok_or_else(|| format!("There is no way out of the village named \"{}\".", value.trim()))?;
            let p = side.path[0];
            let ahead = side.path[(25).min(side.path.len() - 1)];
            let yaw = yaw_of(ahead.sub(p));
            let yaw_deg = (yaw as i64 * 360).div_euclid(fx::TURN as i64);
            crate::places::act(w, "go", &format!("{},{} {} 4", p.x / ONE, p.z / ONE, yaw_deg))
        }
        "look" | "top" => {
            let mut parts = value.split_whitespace();
            let id = parts.next().unwrap_or("");
            let back: i32 = parts.next().and_then(|t| t.parse().ok()).unwrap_or(12);
            let (s, foot) = feet(w)
                .into_iter()
                .find(|(s, _)| s.id == id)
                .ok_or_else(|| format!("There is no roadside stop \"{id}\"."))?;
            let deg_of = |yaw: i32| (yaw as i64 * 360).div_euclid(fx::TURN as i64);
            if name == "look" {
                // `back` metres in front of it (toward its path), looking at it
                let eye = foot.add(crate::geom::facing(s.yaw(), 0).scale(fx::int(back)));
                let yaw = yaw_of(foot.sub(eye));
                return crate::places::act(
                    w,
                    "go",
                    &format!("{},{} {} -4", eye.x / ONE, eye.z / ONE, deg_of(yaw)),
                );
            }
            // on its top, looking out along its side's path
            use crate::models::props_numbers as n;
            let z = if s.kind == Kind::Tower { n::WATCH_Z } else { 2500 };
            let r = rect(foot, s.yaw(), 0, z, 0, 0);
            let side = &sides(&w.land.layout)[s.side];
            let ahead = side.path[(side.path.len() - 1).min(200)];
            let yaw = yaw_of(ahead.sub(v3(r.x, 0, r.z)));
            crate::places::act(
                w,
                "go",
                &format!("{},{} {} -6", r.x / ONE, r.z / ONE, deg_of(yaw)),
            )?;
            let mut at = v3(r.x, 0, r.z);
            at.y = solid::surface(w, at.x, at.z);
            w.player = crate::player::Body::new(at, w.player.facing);
            let me = &w.me;
            w.me = crate::player::Fighter::new(me.kind, me.held, me.team, at, w.player.facing, false);
            if let Some(a) = &mut w.arrival {
                a.at = at;
            }
            Ok(())
        }
        _ => Err(format!("Unknown action \"roadside.{name}\".")),
    }
}

#[cfg(test)]
mod tests;
