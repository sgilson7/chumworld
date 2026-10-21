//! `places`: the village, the bandit camp, the trainer's hut, the shrine,
//! the hollow tree, the scarecrow, the bridge, the signposts and the roads,
//! as `data/places.json` lays them out (M2 World, task W). Names stay
//! descriptive until the person gives them (CW-20).
//!
//! Every built thing goes through M3's door, `models::prop(kind, at, yaw,
//! look, meshes, group)`, standing on the ground `terrain.rs` has flattened
//! under its footprint. Each place is one static `Group` (`World::scene`),
//! with a centre and a radius that holds it.
//!
//! What the rest of the core reads from here: `road()` (the merchant's
//! road as a polyline on the ground, for M5), `roads()` (every road, lane
//! and trail), `at(kind)` (where a place is), `prop_spots(kind)` (where each
//! prop of a kind stands, the bed and the chest among them) and `markers`
//! (the map menu's marks, for M4). `act("places.go", "<kind> [yaw]
//! [pitch]")` puts the player at a place when play next starts, for the
//! screenshot views (`tools/shoot.py`, `?go=` on the address).
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, V3};
use crate::look::Look;
use crate::models::{self, Prop};
use crate::prim::MeshTable;
use crate::render::Group;
use crate::world::World;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PropSpec {
    pub prop: String,
    pub at: [i32; 2],
    pub yaw_deg: i32,
    /// inside another prop (the bed in its house): no footprint of its own
    #[serde(default)]
    pub inside: bool,
    /// M5: drawn by the story each frame, not baked here (the player's
    /// house, cut away while the player is in it; its bed; the chest whose
    /// lid opens)
    #[serde(default)]
    pub moving: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FenceSpec {
    pub from: [i32; 2],
    pub to: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PondSpec {
    pub at: [i32; 2],
    pub radius_m: i32,
    pub depth_cm: i32,
    pub level_cm: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PlaceSpec {
    pub kind: String,
    pub centre: [i32; 2],
    /// a plateau: level within the first radius, back to the land by the second
    #[serde(default)]
    pub flat_m: Option<[i32; 2]>,
    /// no trees within this radius
    #[serde(default)]
    pub clearing_m: i32,
    #[serde(default)]
    pub props: Vec<PropSpec>,
    #[serde(default)]
    pub fences: Vec<FenceSpec>,
    #[serde(default)]
    pub pond: Option<PondSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FieldSpec {
    pub centre: [i32; 2],
    pub half_m: [i32; 2],
    pub yaw_deg: i32,
    pub strip_m: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RoadSpec {
    pub kind: String,
    pub half_cm: i32,
    pub line: Vec<[i32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HillSpec {
    pub at: [i32; 2],
    pub radius_m: i32,
    pub height_m: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ForestSpec {
    pub centre: [i32; 2],
    pub radii_m: [i32; 2],
    pub edge_m: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FlattenSpec {
    pub margin_m: i32,
    pub falloff_m: i32,
}

/// `data/places.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Layout {
    pub footprints_m: BTreeMap<String, [i32; 2]>,
    pub flatten: FlattenSpec,
    pub places: Vec<PlaceSpec>,
    pub fields: Vec<FieldSpec>,
    pub roads: Vec<RoadSpec>,
    pub hills: Vec<HillSpec>,
    pub forest: ForestSpec,
    pub river: crate::water::RiverSpec,
}

impl Layout {
    pub fn parse(json: &str) -> Result<Layout, String> {
        let l: Layout =
            serde_json::from_str(json).map_err(|e| format!("The places file could not be read: {e}"))?;
        for p in &l.places {
            for q in &p.props {
                if prop_kind(&q.prop).is_none() {
                    return Err(format!(
                        "The places file names a prop \"{}\" there is no model for.",
                        q.prop
                    ));
                }
            }
        }
        if l.river.line.len() < 2 {
            return Err("The places file's river has fewer than two points.".into());
        }
        Ok(l)
    }

    /// The layout the game ships with.
    pub fn shipped() -> Layout {
        let mut l = Layout::parse(include_str!("../../../data/places.json")).expect("data/places.json");
        // M7 roadside (T12): the stops beside the ways out, as places
        crate::roadside::lay(&mut l);
        l
    }
}

/// A prop's model by its name in `data/places.json`.
pub fn prop_kind(name: &str) -> Option<Prop> {
    Some(match name {
        "house" => Prop::House,
        "barn" => Prop::Barn,
        "well" => Prop::Well,
        "fence" => Prop::Fence,
        "tent" => Prop::Tent,
        "campfire" => Prop::Campfire,
        "chest" => Prop::Chest,
        "scarecrow" => Prop::Scarecrow,
        "shrine" => Prop::Shrine,
        "hut" => Prop::Hut,
        "bridge" => Prop::Bridge,
        "cart" => Prop::Cart,
        "bed" => Prop::Bed,
        "signpost" => Prop::Signpost,
        "hollow_tree" => Prop::HollowTree,
        "waystone" => Prop::Waystone, // M7 travel
        // M7 roadside (T12)
        "wreck" => Prop::Wreck,
        "wreck_burnt" => Prop::WreckBurnt,
        "watchtower" => Prop::Watchtower,
        "tor" => Prop::Tor,
        "lone_tree" => Prop::LoneTree,
        _ => return None,
    })
}

/// Metres as a world point at height 0.
pub fn metres(p: [i32; 2]) -> V3 {
    v3(p[0] * ONE, 0, p[1] * ONE)
}

/// A prop's place in the world: its kind, its foot, its yaw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spot {
    pub prop: Prop,
    pub at: V3,
    pub yaw: i32,
}

/// Where each prop of a place stands, its foot on the ground.
pub fn spots(w: &World, place: &PlaceSpec) -> Vec<Spot> {
    spots_where(w, place, |_| true)
}

/// The same, of the props `keep` keeps.
fn spots_where(w: &World, place: &PlaceSpec, keep: impl Fn(&PropSpec) -> bool) -> Vec<Spot> {
    let c = metres(place.centre);
    place
        .props
        .iter()
        .filter(|q| keep(q))
        .filter_map(|q| {
            let prop = prop_kind(&q.prop)?;
            let mut at = c.add(v3(q.at[0] * ONE, 0, q.at[1] * ONE));
            at.y = if prop == Prop::Bridge {
                // a bridge stands on its banks: the higher of its two ends,
                // half its span along its length (local z) either way
                let yaw = fx::deg(q.yaw_deg);
                let along = v3(
                    fx::mul(fx::sin(yaw), BRIDGE_SPAN / 2),
                    0,
                    fx::mul(fx::cos(yaw), BRIDGE_SPAN / 2),
                );
                let (a, b) = (at.add(along), at.sub(along));
                w.ground(a.x, a.z).max(w.ground(b.x, b.z))
            } else {
                w.ground(at.x, at.z)
            };
            Some(Spot {
                prop,
                at,
                yaw: fx::deg(q.yaw_deg),
            })
        })
        .collect()
}

/// The bridge's span, end to end (M3's model is about 12 m).
pub const BRIDGE_SPAN: Fx = fx::int(12);

/// The fence posts' spacing along a fence line.
pub const FENCE_SPAN: Fx = fx::int(3);

/// Each fence span along a place's fence lines: its middle on the ground,
/// and the yaw that turns a prop's long side (its local x) along the line.
pub fn fence_spans(w: &World, place: &PlaceSpec) -> Vec<Spot> {
    let c = metres(place.centre);
    let mut out = Vec::new();
    for f in &place.fences {
        let (a, b) = (
            c.add(v3(f.from[0] * ONE, 0, f.from[1] * ONE)),
            c.add(v3(f.to[0] * ONE, 0, f.to[1] * ONE)),
        );
        let d = b.sub(a);
        let len = d.len_xz();
        let n = (len / FENCE_SPAN).max(1);
        // a prop's local x points east at yaw 0; rot_y turns it by yaw
        // counter-clockwise from above, so the yaw of a line is −atan2(dz, dx)
        let yaw = -fx::atan2(d.z, d.x);
        for k in 0..n {
            let mut at = a.lerp(b, (2 * k + 1) * ONE / (2 * n));
            at.y = w.ground(at.x, at.z);
            out.push(Spot {
                prop: Prop::Fence,
                at,
                yaw,
            });
        }
    }
    out
}

/// The name every place's group starts with: the detail levels draw these
/// groups to `PROPS_TO` and hand them over through the dither before it (M3's
/// marks are flicker-safe on props to 3 km, R5.6).
pub const FADED: &str = "place:";
pub const PROPS_TO: (Fx, Fx) = (fx::int(2550), fx::int(3000));

/// Every place's props as groups, one group a place.
pub fn scene(w: &World, look: &Look, meshes: &mut MeshTable, out: &mut Vec<Group>) {
    for place in &w.land.layout.places {
        let c = metres(place.centre);
        // M5: what the story draws itself is left out
        let mut all = spots_where(w, place, |q| !q.moving);
        all.extend(fence_spans(w, place));
        if all.is_empty() {
            continue;
        }
        // the group's radius holds every prop and its own radius (M3's)
        let reach = all
            .iter()
            .map(|s| s.at.sub(c).len_xz() + models::prop_radius(s.prop))
            .max()
            .unwrap_or(0);
        let mut centre = c;
        centre.y = w.ground(c.x, c.z);
        let mut g = Group::new(&format!("{FADED}{}", place.kind), centre, reach);
        for s in &all {
            models::prop(s.prop, s.at, s.yaw, look, meshes, &mut g);
        }
        out.push(g);
    }
}

/// Where a place is, on the ground: its centre.
pub fn at(w: &World, kind: &str) -> Option<V3> {
    w.land.layout.places.iter().find(|p| p.kind == kind).map(|p| {
        let mut c = metres(p.centre);
        c.y = w.ground(c.x, c.z);
        c
    })
}

/// Where each prop of a kind stands, place by place (the bed, the chest,
/// the scarecrow), its foot on the ground.
pub fn prop_spots(w: &World, prop: Prop) -> Vec<Spot> {
    w.land
        .layout
        .places
        .iter()
        .flat_map(|p| spots(w, p))
        .filter(|s| s.prop == prop)
        .collect()
}

/// A road, lane or trail as a polyline on the ground, a point every few
/// metres so it follows the rises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Way {
    pub kind: String,
    pub half: Fx,
    pub points: Vec<V3>,
}

/// The spacing of the points of a way.
pub const WAY_STEP: Fx = fx::int(4);

fn way(w: &World, r: &RoadSpec) -> Way {
    let mut points = Vec::new();
    for k in 0..r.line.len().saturating_sub(1) {
        let (a, b) = (metres(r.line[k]), metres(r.line[k + 1]));
        let n = (b.sub(a).len_xz() / WAY_STEP).max(1);
        for q in 0..n {
            points.push(a.lerp(b, q * ONE / n));
        }
    }
    if let Some(&last) = r.line.last() {
        points.push(metres(last));
    }
    for p in &mut points {
        p.y = w.ground(p.x, p.z);
    }
    Way {
        kind: r.kind.clone(),
        half: r.half_cm * ONE / 100,
        points,
    }
}

/// Every road, lane and trail.
pub fn roads(w: &World) -> Vec<Way> {
    w.land.layout.roads.iter().map(|r| way(w, r)).collect()
}

/// The merchant's road (M5 walks the cart along it), west to east.
pub fn road(w: &World) -> Way {
    w.land
        .layout
        .roads
        .iter()
        .find(|r| r.kind == "road")
        .map(|r| way(w, r))
        .unwrap_or(Way {
            kind: "road".into(),
            half: 0,
            points: Vec::new(),
        })
}

/// The places worth a mark on the map menu (M4 draws them): the kind, a key
/// into `data/strings.json` as `place_<kind>`, and where it is. The village
/// comes first; the crossroads and the trail's end are signposts, not
/// places, and the pond is the village's own.
pub fn markers(w: &World) -> Vec<(&'static str, V3)> {
    // the place's kind in data/places.json, and its key (M4's `place_tree`
    // for the hollow tree)
    const MARKED: [(&str, &str); 7] = [
        ("village", "village"),
        ("camp", "camp"),
        ("hut", "hut"),
        ("shrine", "shrine"),
        ("hollow_tree", "tree"),
        ("scarecrow", "scarecrow"),
        ("bridge", "bridge"),
    ];
    MARKED
        .iter()
        .filter_map(|&(kind, key)| at(w, kind).map(|p| (key, p)))
        // M7 travel: each waystone once it is woken, keyed by its place
        .chain(crate::travel::marks(w))
        // M7 roadside: the stops with models
        .chain(crate::roadside::marks(w))
        // M7 (P9): the held sites seen
        .chain(crate::wanderers::marks(w))
        .collect()
}

/// `places.<name>`: `go` takes a place's kind, or `x,z` in metres, then a
/// yaw and a pitch in degrees, all optional but the place.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    match name {
        "go" => {
            let parts: Vec<&str> = value.split([' ', ',']).filter(|p| !p.is_empty()).collect();
            let refuse = || {
                format!("A place to go is a kind of place, such as bridge, or metres east and south, such as 600,2200; not \"{value}\".")
            };
            let num = |t: &str| t.trim().parse::<i32>().map_err(|_| refuse());
            let (spot, rest) = match parts.first() {
                Some(k) if k.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) => {
                    (at(w, k).ok_or_else(refuse)?, &parts[1..])
                }
                Some(_) if parts.len() >= 2 => {
                    let p = v3(num(parts[0])? * ONE, 0, num(parts[1])? * ONE);
                    (p, &parts[2..])
                }
                _ => return Err(refuse()),
            };
            let yaw = rest.first().map(|t| num(t)).transpose()?.map(fx::deg);
            let pitch = rest.get(1).map(|t| num(t)).transpose()?.map(fx::deg);
            // the player is moved now, a fresh body and fighter there (M1's
            // arm holds world positions, so it is built again, not moved),
            // so a foe asked for after this stands beside the new place
            let mut at = spot;
            at.y = w.ground(at.x, at.z);
            let facing = yaw.unwrap_or(w.aim.yaw);
            w.player = crate::player::Body::new(at, facing);
            let me = &w.me;
            w.me = crate::player::Fighter::new(me.kind, me.held, me.team, at, facing, false);
            // the aim waits for play, which turns the camera to the title's
            w.arrival = Some(Arrival { at, yaw, pitch });
            Ok(())
        }
        _ => Err(format!("Unknown action \"places.{name}\".")),
    }
}

/// Where the player is put when play next starts (`places.go`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrival {
    pub at: V3,
    pub yaw: Option<i32>,
    pub pitch: Option<i32>,
}

/// Turns the view the way `places.go` asked, once: called as play starts.
pub fn arrive(w: &mut World) {
    let Some(a) = w.arrival.take() else {
        return;
    };
    if let Some(y) = a.yaw {
        w.aim.yaw = y;
    }
    if let Some(p) = a.pitch {
        w.aim.pitch = p.clamp(crate::camera::PITCH_MIN, crate::camera::PITCH_MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> World {
        World::new(1)
    }

    #[test]
    fn the_places_file_names_only_props_there_are_models_for() {
        let l = Layout::shipped();
        assert!(l
            .places
            .iter()
            .all(|p| p.props.iter().all(|q| prop_kind(&q.prop).is_some())));
        let mut bad: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/places.json")).unwrap();
        bad["places"][0]["props"][0]["prop"] = serde_json::json!("castle");
        assert_eq!(
            Layout::parse(&bad.to_string()).unwrap_err(),
            "The places file names a prop \"castle\" there is no model for."
        );
    }

    #[test]
    fn the_village_has_eight_houses_the_player_s_house_a_barn_a_well_and_a_bed() {
        let l = Layout::shipped();
        let v = l.places.iter().find(|p| p.kind == "village").unwrap();
        let count = |k: &str| v.props.iter().filter(|q| q.prop == k).count();
        // the journal: about 8 houses, a barn, a well, the player's house with a bed
        assert_eq!(count("house"), 9, "eight houses and the player's");
        assert_eq!((count("barn"), count("well"), count("bed")), (1, 1, 1));
        assert!(!v.fences.is_empty());
        let camp = l.places.iter().find(|p| p.kind == "camp").unwrap();
        let ck = |k: &str| camp.props.iter().filter(|q| q.prop == k).count();
        assert_eq!((ck("tent"), ck("campfire"), ck("chest")), (3, 1, 1));
    }

    #[test]
    fn the_map_s_places_stand_where_the_journal_s_map_puts_them() {
        // the journal's map at 150 px a km, origin (30, 37) px, north up:
        // world x = (px − 30) / 0.15, world z = (py − 37) / 0.15
        let w = world();
        let m = markers(&w);
        let find = |k: &str| m.iter().find(|x| x.0 == k).map(|x| x.1).unwrap();
        let near = |p: V3, x: i32, z: i32, within: i32| {
            let d = v3(p.x - x * ONE, 0, p.z - z * ONE).len_xz();
            assert!(d < within * ONE, "{p:?} is {} m from ({x}, {z})", d / ONE);
        };
        // the village circle at (330, 457) px: (2000, 2800)
        near(find("village"), 2000, 2800, 1);
        // the camp's tents round (512, 185) px: (3213, 987)
        near(find("camp"), 3213, 987, 10);
        // the bridge, the rectangle at (136, 368) px: (707, 2207) on the
        // map, which a road and a river drawn by hand cross at (627, 2248)
        near(find("bridge"), 707, 2207, 100);
        // the boots' marker (150, 187) px is the shrine: (800, 1000), and
        // the river runs 62 m west of it, where the shrine stands on an islet
        near(find("shrine"), 800, 1000, 70);
        // the trousers' (570, 247) px: the hollow tree, (3600, 1400)
        near(find("tree"), 3600, 1400, 1);
        // the jacket's (240, 532) px: the scarecrow, (1400, 3300)
        near(find("scarecrow"), 1400, 3300, 1);
        assert_eq!(m[0].0, "village");
        // every mark has its words for the map menu (M4)
        let strings = crate::page::parse_strings(include_str!("../../../data/strings.json")).unwrap();
        for (k, _) in &m {
            assert!(strings.0.contains_key(&format!("place_{k}")), "no place_{k}");
        }
    }

    #[test]
    fn the_road_runs_from_the_west_edge_to_the_east_edge_over_the_bridge() {
        let w = world();
        let r = road(&w);
        assert!(r.points.first().unwrap().x < 0 && r.points.last().unwrap().x > 4000 * ONE);
        let bridge = at(&w, "bridge").unwrap();
        let closest = r.points.iter().map(|p| p.sub(bridge).len_xz()).min().unwrap();
        assert!(
            closest < WAY_STEP,
            "the road passes the bridge {} m off",
            closest / ONE
        );
        // its points follow the ground
        for p in r.points.iter().step_by(25) {
            assert_eq!(p.y, w.ground(p.x, p.z));
        }
    }

    #[test]
    fn going_to_a_place_puts_the_player_there_when_play_starts() {
        let mut w = world();
        act(&mut w, "go", "bridge 90 -10").unwrap();
        let b = at(&w, "bridge").unwrap();
        assert_eq!(
            (w.player.pos.x, w.player.pos.z),
            (b.x, b.z),
            "the player is moved at once"
        );
        assert_eq!(w.player.pos.y, w.ground(b.x, b.z));
        assert_ne!(w.aim.yaw, fx::deg(90), "the aim waits for play");
        arrive(&mut w);
        assert_eq!((w.aim.yaw, w.aim.pitch), (fx::deg(90), -fx::deg(10)));
        assert_eq!(w.arrival, None, "once");
        act(&mut w, "go", "600,2200").unwrap();
        arrive(&mut w);
        assert_eq!((w.player.pos.x, w.player.pos.z), (600 * ONE, 2200 * ONE));
        assert!(act(&mut w, "go", "castle")
            .unwrap_err()
            .contains("not \"castle\""));
        assert!(act(&mut w, "go", "").is_err());
    }
}
