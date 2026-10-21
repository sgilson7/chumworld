//! `dungeon`: the first dungeon's rooms (T21, phase A; the journal's
//! Dungeons tab).
//!
//! **Rooms are spaces of their own.** Each is built far north of the region
//! and high over the land (`data/dungeon.json`'s `origin_m`), so nothing in
//! the overworld is near one: the wanderers, the animals, the water and the
//! grass act only near the player and leave the rooms alone, and the rooms'
//! groups (`places::FADED`, drawn to 3 km) are drawn only from inside. A
//! room is a box: a floor, four walls facing in, a ceiling, and on its floor
//! ledges, six-sided pillars and ramps to stand on (`roadside::Climb`s in the
//! solid map, so a pillar's side is a wall to feet below its top), and doors.
//! Whether the player is in a room is where they stand (`room_at`); a save
//! made at a fountain loads back into the room for free.
//!
//! **Doors.** E at a door (`reach`, `use_door`) fades the view out
//! (`story.veil`), moves the player in front of the door it leads to, facing
//! in, and fades back. A door leads to another room's door, out to the world
//! in front of the great tree's door, or is closed with a line. The player
//! keeps their health and gear through a door (a fresh arm at the new place,
//! as travel's).
//!
//! **Springs and fountains.** Standing in a healing spring restores health
//! a tick up to full; a fountain is a save point (`save::points`, `fountain.<id>`).
//!
//! Every rule here is the core's (CW-1); the page draws the rooms' meshes and
//! eases the veil.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::boom::Solid;
use crate::fx::{self, Fx, ONE};
use crate::geom::{facing, v3, Affine, V3};
use crate::look::Look;
use crate::prim::{push_coloured, Mesh, MeshTable};
use crate::render::Group;
use crate::roadside::Climb;
use crate::solid::Wall;
use crate::world::World;

// ---- the file -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Sockets {
    pub side_cm: i32,
    pub up_cm: i32,
    pub catch_cm: i32,
    pub aim_deg: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Bulb {
    pub at_cm: [i32; 3],
    pub range_cm: i32,
    pub every: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Outside {
    pub at_m: [i32; 2],
    pub yaw_deg: i32,
    pub out_cm: i32,
    pub trunk_cm: i32,
    pub tree_cm: i32,
    pub sockets: Sockets,
    pub bulb: Bulb,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum To {
    /// another room's door, by room and door id
    Room { room: String, door: String },
    /// out to the world, in front of the great tree's door
    World(String),
    /// shut, with this line
    Closed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DoorSpec {
    pub id: String,
    pub wall: String,
    pub along_cm: i32,
    pub top_cm: i32,
    pub to: To,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FountainSpec {
    pub id: String,
    pub at_cm: [i32; 2],
    pub top_cm: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SpringSpec {
    pub at_cm: [i32; 2],
    pub top_cm: i32,
    pub radius_cm: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Colours {
    pub floor: [i32; 3],
    pub wall: [i32; 3],
    pub ceiling: [i32; 3],
    pub top: [i32; 3],
    pub side: [i32; 3],
    pub door: [i32; 3],
    pub frame: [i32; 3],
    pub root: [i32; 3],
    pub mushroom: [i32; 3],
}

/// A room's dressing, placed from its seed: roots hanging from the ceiling,
/// ridges of root up its walls, glowing mushrooms at the feet of things.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct Decor {
    pub seed: u64,
    pub hanging: u32,
    pub ridges: u32,
    pub mushrooms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RoomSpec {
    pub id: String,
    pub centre_m: [i32; 2],
    pub size_cm: [i32; 2],
    pub height_cm: i32,
    pub colours: Colours,
    #[serde(default)]
    pub decor: Decor,
    /// x, z, radius, top
    pub pillars: Vec<[i32; 4]>,
    /// x0, z0, x1, z1, top
    pub ledges: Vec<[i32; 5]>,
    /// x0, z0, x1, z1, height at x0, height at x1
    pub ramps: Vec<[i32; 6]>,
    pub doors: Vec<DoorSpec>,
    pub fountains: Vec<FountainSpec>,
    pub springs: Vec<SpringSpec>,
    /// where the floor's ants hatch (`critters.rs`)
    #[serde(default)]
    pub ants: Vec<[i32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub origin_m: [i32; 3],
    pub outside: Outside,
    pub rooms: Vec<RoomSpec>,
    pub door_reach_cm: i32,
    pub door_inside_cm: i32,
    pub dark_ticks: u32,
    pub hold_ticks: u32,
    pub heal_per_tick: i32,
}

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let mut v: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("The dungeon file could not be read: {e}"))?;
        strip(&mut v);
        let s: Spec =
            serde_json::from_value(v).map_err(|e| format!("The dungeon file could not be read: {e}"))?;
        for r in &s.rooms {
            for d in &r.doors {
                if !["n", "s", "e", "w"].contains(&d.wall.as_str()) {
                    return Err(format!("The dungeon file's door {} stands in no wall.", d.id));
                }
                if let To::Room { room, door } = &d.to {
                    let ok = s
                        .rooms
                        .iter()
                        .any(|q| &q.id == room && q.doors.iter().any(|e| &e.id == door));
                    if !ok {
                        return Err(format!("The dungeon file's door {} leads nowhere.", d.id));
                    }
                }
            }
        }
        if s.dark_ticks == 0 || s.hold_ticks == 0 {
            return Err("The dungeon file's fade needs a tick at least.".into());
        }
        Ok(s)
    }
}

/// Notes (`_…`) out of every object, at every depth.
fn strip(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(m) => {
            m.retain(|k, _| !k.starts_with('_'));
            m.values_mut().for_each(strip);
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(strip),
        _ => {}
    }
}

/// The rooms the game ships with, read once.
pub fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| Spec::parse(include_str!("../../../data/dungeon.json")).expect("data/dungeon.json"))
}

fn cm(c: i32) -> Fx {
    c * (ONE / 100)
}

fn rgb(c: [i32; 3]) -> [i32; 3] {
    [c[0] * ONE / 255, c[1] * ONE / 255, c[2] * ONE / 255]
}

// ---- where things are -------------------------------------------------------------

/// A room's floor centre in the world.
pub fn centre(r: &RoomSpec) -> V3 {
    let o = spec().origin_m;
    v3(
        fx::int(o[0] + r.centre_m[0]),
        fx::int(o[1]),
        fx::int(o[2] + r.centre_m[1]),
    )
}

/// A point `x`, `z` centimetres from a room's centre, `y` over its floor.
fn at(r: &RoomSpec, x: i32, y: i32, z: i32) -> V3 {
    centre(r).add(v3(cm(x), cm(y), cm(z)))
}

/// The room the player's feet are in, if any: by where they stand, so a
/// load or a journey needs nothing more.
pub fn room_at(p: V3) -> Option<usize> {
    spec().rooms.iter().position(|r| {
        let c = centre(r);
        let (hx, hz) = (cm(r.size_cm[0]) / 2, cm(r.size_cm[1]) / 2);
        (p.x - c.x).abs() <= hx && (p.z - c.z).abs() <= hz && p.y >= c.y - ONE && p.y <= c.y + cm(r.height_cm)
    })
}

/// A door: where its middle is in the world (its sill's height), the way
/// it faces into its space, and where it leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Door {
    /// the room it stands in, or `None` for the great tree's door outside
    pub room: Option<usize>,
    pub id: String,
    pub at: V3,
    pub yaw: i32,
    pub to: To,
}

/// Every door: the great tree's (first), then each room's in order.
pub fn doors(w: &World) -> Vec<Door> {
    let s = spec();
    let o = &s.outside;
    // on the trunk's face, the way the door faces
    let face = tree(w).add(facing(fx::deg(o.yaw_deg), 0).scale(cm(o.trunk_cm)));
    let mut out = vec![Door {
        room: None,
        id: "outside".into(),
        at: v3(face.x, w.ground(face.x, face.z), face.z),
        yaw: fx::deg(o.yaw_deg),
        to: To::Room {
            room: s.rooms[0].id.clone(),
            door: s.rooms[0].doors[0].id.clone(),
        },
    }];
    for (k, r) in s.rooms.iter().enumerate() {
        let (hx, hz) = (r.size_cm[0] / 2, r.size_cm[1] / 2);
        for d in &r.doors {
            // in the wall, facing into the room
            let (x, z, yaw) = match d.wall.as_str() {
                "s" => (d.along_cm, hz, 0),
                "n" => (d.along_cm, -hz, 180),
                "w" => (-hx, d.along_cm, -90),
                _ => (hx, d.along_cm, 90),
            };
            out.push(Door {
                room: Some(k),
                id: d.id.clone(),
                at: at(r, x, d.top_cm, z),
                yaw: fx::deg(yaw),
                to: d.to.clone(),
            });
        }
    }
    out
}

/// Where a door leaves someone who comes through it: in front of it,
/// facing the way it faces, on whatever stands there.
pub fn arrival(w: &World, d: &Door) -> (V3, i32) {
    let s = spec();
    let out = if d.room.is_none() {
        cm(s.outside.out_cm)
    } else {
        cm(s.door_inside_cm)
    };
    let mut p = d.at.add(facing(d.yaw, 0).scale(out));
    p.y = crate::solid::surface(w, p.x, p.z);
    (p, d.yaw)
}

// ---- the solid map -----------------------------------------------------------------

/// Every room's four walls, as the solid map keeps bodies out of them.
pub fn walls() -> Vec<Wall> {
    let mut out = Vec::new();
    for r in &spec().rooms {
        let c = centre(r);
        let (hx, hz) = (cm(r.size_cm[0]) / 2, cm(r.size_cm[1]) / 2);
        let t = ONE / 4;
        out.push(Wall {
            x: c.x,
            z: c.z - hz - t,
            hx: hx + 2 * t,
            hz: t,
            yaw: 0,
        });
        out.push(Wall {
            x: c.x,
            z: c.z + hz + t,
            hx: hx + 2 * t,
            hz: t,
            yaw: 0,
        });
        out.push(Wall {
            x: c.x - hx - t,
            z: c.z,
            hx: t,
            hz: hz + 2 * t,
            yaw: 0,
        });
        out.push(Wall {
            x: c.x + hx + t,
            z: c.z,
            hx: t,
            hz: hz + 2 * t,
            yaw: 0,
        });
    }
    out
}

/// A pillar's square of feet: its radius times this. A hexagon's sides are
/// 0.87 of its radius from its middle, its corners 1; the square's sides
/// are 0.8 and its corners 1.13.
const PILLAR_SQUARE_PCT: i32 = 80;

/// What a body stands on in the rooms: each floor, ledge, pillar and ramp.
pub fn climbs() -> Vec<Climb> {
    let mut out = Vec::new();
    for r in &spec().rooms {
        let c = centre(r);
        let flat = |x0: i32, z0: i32, x1: i32, z1: i32, top: i32| Climb {
            rect: Wall {
                x: c.x + cm(x0 + x1) / 2,
                z: c.z + cm(z0 + z1) / 2,
                hx: cm((x1 - x0).abs()) / 2,
                hz: cm((z1 - z0).abs()) / 2,
                yaw: 0,
            },
            low: c.y + cm(top),
            high: c.y + cm(top),
        };
        let (hx, hz) = (r.size_cm[0] / 2, r.size_cm[1] / 2);
        out.push(flat(-hx, -hz, hx, hz, 0));
        for l in &r.ledges {
            out.push(flat(l[0], l[1], l[2], l[3], l[4]));
        }
        for p in &r.pillars {
            let h = p[2] * PILLAR_SQUARE_PCT / 100;
            out.push(flat(p[0] - h, p[1] - h, p[0] + h, p[1] + h, p[3]));
        }
        for m in &r.ramps {
            // along world x: a rectangle turned a quarter turn runs its own
            // z east, so its height climbs from its west end to its east
            out.push(Climb {
                rect: Wall {
                    x: c.x + cm(m[0] + m[2]) / 2,
                    z: c.z + cm(m[1] + m[3]) / 2,
                    hx: cm((m[3] - m[1]).abs()) / 2,
                    hz: cm((m[2] - m[0]).abs()) / 2,
                    yaw: fx::deg(90),
                },
                low: c.y + cm(m[4]),
                high: c.y + cm(m[5]),
            });
        }
    }
    out
}

/// What the camera's boom keeps clear of in the rooms: the walls, the
/// ceiling, and each ledge and pillar; and the great tree's trunk.
pub fn blocks(w: &World) -> Vec<Solid> {
    let mut out = vec![Solid::Post {
        at: tree(w),
        r: cm(spec().outside.trunk_cm),
        y1: tree(w).y + cm(spec().outside.tree_cm) / 2,
    }];
    for r in &spec().rooms {
        let c = centre(r);
        let top = c.y + cm(r.height_cm);
        for wall in walls()
            .into_iter()
            .filter(|wl| (wl.x - c.x).abs() < ONE * 400 && (wl.z - c.z).abs() < ONE * 400)
        {
            out.push(Solid::Block {
                wall,
                y0: c.y - ONE,
                y1: top + ONE,
            });
        }
        let (hx, hz) = (cm(r.size_cm[0]) / 2, cm(r.size_cm[1]) / 2);
        out.push(Solid::Block {
            wall: Wall {
                x: c.x,
                z: c.z,
                hx: hx + ONE,
                hz: hz + ONE,
                yaw: 0,
            },
            y0: top,
            y1: top + ONE,
        });
        for l in &r.ledges {
            let wall = Wall {
                x: c.x + cm(l[0] + l[2]) / 2,
                z: c.z + cm(l[1] + l[3]) / 2,
                hx: cm((l[2] - l[0]).abs()) / 2,
                hz: cm((l[3] - l[1]).abs()) / 2,
                yaw: 0,
            };
            out.push(Solid::Block {
                wall,
                y0: c.y - ONE,
                y1: c.y + cm(l[4]),
            });
        }
        for p in &r.pillars {
            out.push(Solid::Post {
                at: at(r, p[0], 0, p[1]),
                r: cm(p[2]),
                y1: c.y + cm(p[3]),
            });
        }
    }
    out
}

// ---- E: doors, the tick ------------------------------------------------------------

/// A journey through a door under way: the door it leads to (by its place
/// in `doors`), and ticks since it began.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transit {
    pub to: usize,
    pub ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    pub transit: Option<Transit>,
    /// the great tree's two sockets, lit by orbs turned into them
    pub sockets: [bool; 2],
    /// both lit: the great tree's door is open
    pub open: bool,
    /// ticks the roots' bulb has had the player in range
    pub bulb: u32,
}

/// The orbs the great tree's roots throw are this caster's: no fighter's
/// (`orbs::cast` takes a fighter; the roots throw with `orbs::fire`).
pub const ROOTS: u8 = u8::MAX;

/// The door E would open, and how far beyond `story::REACH`'s edge, as
/// story counts it.
pub fn reach(w: &World) -> Option<(usize, Fx)> {
    let me = w.player.pos;
    let edge = cm(spec().door_reach_cm);
    doors(w)
        .iter()
        .enumerate()
        .map(|(k, d)| (k, d.at.sub(me).len_xz(), (d.at.y - me.y).abs()))
        .filter(|&(_, dxz, dy)| dxz <= edge && dy <= ONE)
        .map(|(k, d, _)| (k, (d - edge + crate::story::REACH).max(0)))
        .min_by_key(|&(_, d)| d)
}

/// The door `k` (by its place in `doors`) at E: through it, or its line.
pub fn use_door(w: &mut World, k: usize) {
    let all = doors(w);
    let Some(d) = all.get(k) else {
        return;
    };
    // the great tree's door is sealed until both its sockets are lit
    if d.room.is_none() && !w.dungeon.open {
        crate::story::banner(w, "dungeon_tree_sealed");
        return;
    }
    let to = match &d.to {
        To::Closed(line) => {
            crate::story::banner(w, line);
            return;
        }
        To::World(_) => 0,
        To::Room { room, door } => {
            let Some(j) = all
                .iter()
                .position(|e| e.room.is_some_and(|r| spec().rooms[r].id == *room) && e.id == *door)
            else {
                return;
            };
            j
        }
    };
    w.dungeon.transit = Some(Transit { to, ticks: 0 });
}

/// While going through a door, the keys and the mouse do nothing.
pub fn filter(w: &World, input: &crate::input::Input) -> crate::input::Input {
    if w.dungeon.transit.is_some() {
        crate::input::Input::default()
    } else {
        *input
    }
}

/// Step 7, with the story: a door's fade and move, and the springs.
pub fn step(w: &mut World) {
    let sp = spec();
    if let Some(mut t) = w.dungeon.transit {
        if t.ticks == 0 {
            w.story.veil = true;
        }
        if t.ticks == sp.dark_ticks {
            let d = doors(w)[t.to].clone();
            put_at(w, &d);
            // the room's ants are back, whatever happened to them before
            if let Some(room) = d.room {
                crate::critters::reset(w, room);
            }
        }
        if t.ticks >= sp.dark_ticks + sp.hold_ticks {
            w.story.veil = false;
            w.dungeon.transit = None;
        } else {
            t.ticks += 1;
            w.dungeon.transit = Some(t);
        }
    }
    roots(w);
    if let Some(k) = spring_at(w.player.pos) {
        let _ = k;
        let h = &mut w.me.health;
        if !h.down() && h.hp < h.most {
            h.hp = healed(h.hp, h.most, sp.heal_per_tick);
        }
    }
}

/// Health after a tick in a spring: `per` more, never past `most` (a rate
/// over 1 a tick would otherwise pass it from just below).
pub fn healed(hp: i32, most: i32, per: i32) -> i32 {
    (hp + per).min(most)
}

/// The spring the feet stand in, if any: within its radius, on its top.
pub fn spring_at(p: V3) -> Option<usize> {
    let mut k = 0;
    for r in &spec().rooms {
        for s in &r.springs {
            let c = at(r, s.at_cm[0], s.top_cm, s.at_cm[1]);
            if c.sub(p).len_xz() <= cm(s.radius_cm) && (p.y - c.y).abs() <= ONE / 4 {
                return Some(k);
            }
            k += 1;
        }
    }
    None
}

/// The player stands in front of door `d`, facing in, with their health,
/// gear and wounds as they were (a fresh arm at the new place, as travel's).
fn put_at(w: &mut World, d: &Door) {
    let (p, yaw) = arrival(w, d);
    w.player = crate::player::Body::new(p, yaw);
    let held = crate::items::held(w);
    let health = w.me.health;
    let me = &w.me;
    w.me = crate::player::Fighter::new(me.kind, held, me.team, p, yaw, false);
    w.me.health = health;
    w.aim.yaw = yaw;
    w.aim.pitch = 0;
    w.aim.rest = (0, 0);
}

/// Test acts for views and checks (`dungeon.<name>`): `go:<door>` stands
/// the player in front of a door (`outside`, or a room's door id; a
/// world set up from the address never saves, `save.staged`), `open` lights
/// both of the great tree's sockets.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    match name {
        "go" => {
            let all = doors(w);
            let d = all
                .iter()
                .find(|d| d.id == value)
                .cloned()
                .ok_or_else(|| format!("There is no door \"{value}\"."))?;
            put_at(w, &d);
            if let Some(room) = d.room {
                crate::critters::reset(w, room);
            }
            // play opens from the title's camera unless an arrival says
            // otherwise (`places::arrive`): look the way the door faces
            let (at, yaw) = arrival(w, &d);
            w.arrival = Some(crate::places::Arrival {
                at,
                yaw: Some(yaw),
                pitch: None,
            });
            Ok(())
        }
        "open" => {
            w.dungeon.sockets = [true, true];
            w.dungeon.open = true;
            Ok(())
        }
        other => Err(format!("There is no dungeon act \"{other}\".")),
    }
}

// ---- the great tree outside ---------------------------------------------------------

/// The great tree's trunk's middle, on the ground.
pub fn tree(w: &World) -> V3 {
    let o = &spec().outside;
    let (x, z) = (fx::int(o.at_m[0]), fx::int(o.at_m[1]));
    v3(x, w.ground(x, z), z)
}

/// The trunk as bodies keep out of it: its middle and radius, when `p` is
/// within `reach` of it.
pub fn trunk_near(w: &World, p: V3, reach: Fx) -> Option<(V3, Fx)> {
    let c = tree(w);
    let r = cm(spec().outside.trunk_cm);
    (c.sub(p).len_xz() <= reach + r).then_some((c, r))
}

/// The two sockets' middles: either side of the door on the trunk's face.
pub fn sockets(w: &World) -> [V3; 2] {
    let o = &spec().outside;
    let yaw = fx::deg(o.yaw_deg);
    let face = tree(w).add(facing(yaw, 0).scale(cm(o.trunk_cm)));
    let side = crate::geom::right_of(yaw).scale(cm(o.sockets.side_cm));
    let up = v3(0, cm(o.sockets.up_cm), 0);
    [face.sub(side).add(up), face.add(side).add(up)]
}

/// The roots' bulb, where its orbs set off.
pub fn bulb(w: &World) -> V3 {
    let b = &spec().outside.bulb;
    tree(w).add(v3(cm(b.at_cm[0]), cm(b.at_cm[1]), cm(b.at_cm[2])))
}

/// After the orbs (step 7): the bulb throws while the player is near and
/// the door shut; an orb of the roots' turned toward a dark socket flies
/// straight to it, and lights it as it passes.
fn roots(w: &mut World) {
    let o = &spec().outside;
    if w.dungeon.open {
        return;
    }
    // a world held for a view or a check throws nothing (R9.24)
    let near = bulb(w).sub(w.player.pos).len_xz() <= cm(o.bulb.range_cm)
        && !w.story.opening()
        && !crate::story::holding(w);
    if near {
        w.dungeon.bulb += 1;
        if w.dungeon.bulb % o.bulb.every == 1 {
            let v = &crate::orbs::spec().pattern(crate::orbs::Pattern::PracticeOne)[0];
            let mut next = w.orbs.next;
            let thrown = crate::orbs::fire(v, bulb(w), crate::orbs::target(w), 0, ROOTS, &mut next);
            w.orbs.next = next;
            w.orbs.flying.extend(thrown);
        }
    } else {
        w.dungeon.bulb = 0;
    }
    let socks = sockets(w);
    // within `aim_deg`: the turned way's dot with the way to the socket at
    // least the angle's cosine (both unit)
    let aim = fx::cos(fx::deg(o.sockets.aim_deg));
    let catch = cm(o.sockets.catch_cm);
    let mut lit = Vec::new();
    for orb in w
        .orbs
        .flying
        .iter_mut()
        .filter(|b| b.caster == ROOTS && b.turned.is_some())
    {
        // turned this tick: toward the dark socket it was knocked nearest
        if orb.turned == Some(orb.age) {
            let best = (0..2)
                .filter(|&k| !w.dungeon.sockets[k])
                .map(|k| (k, socks[k].sub(orb.pos).norm()))
                .map(|(k, d)| (k, d, orb.dir.norm().dot(d)))
                .filter(|&(_, _, c)| c >= aim)
                .max_by_key(|&(_, _, c)| c);
            if let Some((_, d, _)) = best {
                orb.dir = d;
            }
        }
        for (k, sock) in socks.iter().enumerate() {
            if !w.dungeon.sockets[k] && to_segment(orb.prev, orb.pos, *sock) <= catch {
                lit.push((orb.id, k));
            }
        }
    }
    for (id, k) in lit {
        w.orbs.flying.retain(|b| b.id != id);
        if !w.dungeon.sockets[k] {
            w.dungeon.sockets[k] = true;
            crate::story::banner(w, "dungeon_socket_lit");
        }
    }
    if w.dungeon.sockets.iter().all(|&b| b) {
        w.dungeon.open = true;
        crate::story::banner(w, "dungeon_tree_open");
    }
}

/// How far `p` is from the segment `a`–`b` (in i64, so long segments and
/// far points do not overflow).
fn to_segment(a: V3, b: V3, p: V3) -> Fx {
    let d = [(b.x - a.x) as i64, (b.y - a.y) as i64, (b.z - a.z) as i64];
    let q = [(p.x - a.x) as i64, (p.y - a.y) as i64, (p.z - a.z) as i64];
    let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    let t = if dd == 0 {
        0
    } else {
        ((q[0] * d[0] + q[1] * d[1] + q[2] * d[2]).clamp(0, dd) * ONE as i64) / dd
    };
    let c = v3(
        a.x + (d[0] * t / ONE as i64) as Fx,
        a.y + (d[1] * t / ONE as i64) as Fx,
        a.z + (d[2] * t / ONE as i64) as Fx,
    );
    p.dist(c)
}

// ---- the save (`save::PARTS`) ---------------------------------------------------------

/// A dungeon id the save may hold: a socket lit, the tree's door open.
pub fn save_known(id: &str) -> bool {
    matches!(id, "socket.0" | "socket.1" | "open")
}

pub fn save_ids(w: &World) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for (k, &b) in w.dungeon.sockets.iter().enumerate() {
        if b {
            out.insert(format!("socket.{k}"));
        }
    }
    if w.dungeon.open {
        out.insert("open".into());
    }
    out
}

pub fn save_apply(w: &mut World, ids: &std::collections::BTreeSet<String>) {
    for k in 0..2 {
        w.dungeon.sockets[k] = ids.contains(&format!("socket.{k}"));
    }
    w.dungeon.open = ids.contains("open");
}

// ---- the save fountains (`save::points`) -----------------------------------------

/// Every fountain, in order: its id, its foot and the way it faces.
pub fn fountains() -> Vec<(String, V3, i32)> {
    let mut out = Vec::new();
    for r in &spec().rooms {
        for f in &r.fountains {
            out.push((f.id.clone(), at(r, f.at_cm[0], f.top_cm, f.at_cm[1]), 0));
        }
    }
    out
}

// ---- the rooms, drawn ---------------------------------------------------------------

/// A flat quad facing `n` (a unit vector), its middle `m`, `u` a unit
/// vector across it, half sizes `hu` along `u` and `hv` along `n × u`.
fn quad(o: &mut Vec<i32>, m: V3, n: V3, u: V3, hu: Fx, hv: Fx, col: [i32; 3]) {
    let v = n.cross(u).norm();
    let (a, b) = (u.scale(hu), v.scale(hv));
    let p = [m.sub(a).sub(b), m.add(a).sub(b), m.add(a).add(b), m.sub(a).add(b)];
    push_coloured(o, p[0], p[1], p[2], col);
    push_coloured(o, p[0], p[2], p[3], col);
}

const UP: V3 = v3(0, ONE, 0);
const DOWN: V3 = v3(0, -ONE, 0);
const EAST: V3 = v3(ONE, 0, 0);
const SOUTH: V3 = v3(0, 0, ONE);

/// A box with no bottom: x0..x1, z0..z1 (cm, room frame), from the floor up
/// to `top`.
fn ledge(o: &mut Vec<i32>, l: [i32; 5], top: [i32; 3], side: [i32; 3]) {
    let (x0, z0, x1, z1, h) = (cm(l[0]), cm(l[1]), cm(l[2]), cm(l[3]), cm(l[4]));
    let (mx, mz, hx, hz) = (
        (x0 + x1) / 2,
        (z0 + z1) / 2,
        (x1 - x0).abs() / 2,
        (z1 - z0).abs() / 2,
    );
    quad(o, v3(mx, h, mz), UP, EAST, hx, hz, top);
    quad(
        o,
        v3(mx, h / 2, mz + hz),
        SOUTH,
        EAST.scale(-ONE),
        hx,
        h / 2,
        side,
    );
    quad(
        o,
        v3(mx, h / 2, mz - hz),
        SOUTH.scale(-ONE),
        EAST,
        hx,
        h / 2,
        side,
    );
    quad(o, v3(mx + hx, h / 2, mz), EAST, SOUTH, hz, h / 2, side);
    quad(
        o,
        v3(mx - hx, h / 2, mz),
        EAST.scale(-ONE),
        SOUTH.scale(-ONE),
        hz,
        h / 2,
        side,
    );
}

/// A six-sided pillar: its middle (cm), radius and top.
fn pillar(o: &mut Vec<i32>, p: [i32; 4], top: [i32; 3], side: [i32; 3]) {
    let (cx, cz, r, h) = (cm(p[0]), cm(p[1]), cm(p[2]), cm(p[3]));
    // corners anticlockwise seen from above: (cos θ, −sin θ)
    let corner = |k: i32| {
        let a = fx::deg(60 * k);
        v3(cx + fx::mul(fx::cos(a), r), 0, cz - fx::mul(fx::sin(a), r))
    };
    let mid = v3(cx, h, cz);
    for k in 0..6 {
        let (a, b) = (corner(k), corner(k + 1));
        push_coloured(o, mid, v3(a.x, h, a.z), v3(b.x, h, b.z), top);
        // the side between, facing out
        let m = v3((a.x + b.x) / 2, h / 2, (a.z + b.z) / 2);
        let n = v3(m.x - cx, 0, m.z - cz).norm();
        let u = UP.cross(n).norm();
        quad(o, m, n, u, b.sub(a).len_xz() / 2, h / 2, side);
    }
}

/// Every room as a group of its own (`place:dungeon_<id>`, faded to 3 km
/// with the places), and the room's furniture.
pub fn scene(w: &World, look: &Look, meshes: &mut MeshTable, out: &mut Vec<Group>) {
    for r in &spec().rooms {
        let col = &r.colours;
        let (floor, wall, ceil, top, side) = (
            rgb(col.floor),
            rgb(col.wall),
            rgb(col.ceiling),
            rgb(col.top),
            rgb(col.side),
        );
        let (hx, hz, h) = (cm(r.size_cm[0]) / 2, cm(r.size_cm[1]) / 2, cm(r.height_cm));
        let mut o = Vec::new();
        quad(&mut o, V3::ZERO, UP, EAST, hx, hz, floor);
        quad(&mut o, v3(0, h, 0), DOWN, EAST, hx, hz, ceil);
        // the walls, facing in
        quad(&mut o, v3(0, h / 2, -hz), SOUTH, EAST, hx, h / 2, wall);
        quad(
            &mut o,
            v3(0, h / 2, hz),
            SOUTH.scale(-ONE),
            EAST.scale(-ONE),
            hx,
            h / 2,
            wall,
        );
        quad(
            &mut o,
            v3(-hx, h / 2, 0),
            EAST,
            SOUTH.scale(-ONE),
            hz,
            h / 2,
            wall,
        );
        quad(&mut o, v3(hx, h / 2, 0), EAST.scale(-ONE), SOUTH, hz, h / 2, wall);
        for l in &r.ledges {
            ledge(&mut o, *l, top, side);
        }
        for p in &r.pillars {
            pillar(&mut o, *p, top, side);
        }
        for m in &r.ramps {
            // a slope from (x0, h0) to (x1, h1), its top only
            let (x0, z0, x1, z1, h0, h1) = (cm(m[0]), cm(m[1]), cm(m[2]), cm(m[3]), cm(m[4]), cm(m[5]));
            let a = v3(x0, h0, z0);
            let b = v3(x0, h0, z1);
            let c = v3(x1, h1, z1);
            let d = v3(x1, h1, z0);
            push_coloured(&mut o, a, b, c, top);
            push_coloured(&mut o, a, c, d, top);
        }
        // the doors: a dark doorway a hand in front of its wall, a frame round it
        let (door, frame) = (rgb(col.door), rgb(col.frame));
        for d in &r.doors {
            let (x, z, n) = match d.wall.as_str() {
                "s" => (cm(d.along_cm), hz - ONE * 4 / 100, SOUTH.scale(-ONE)),
                "n" => (cm(d.along_cm), -hz + ONE * 4 / 100, SOUTH),
                "w" => (-hx + ONE * 4 / 100, cm(d.along_cm), EAST),
                _ => (hx - ONE * 4 / 100, cm(d.along_cm), EAST.scale(-ONE)),
            };
            let u = UP.cross(n).norm();
            let sill = cm(d.top_cm);
            let m = v3(x, sill + ONE, z);
            quad(&mut o, m.add(n.scale(ONE / 50)), n, u, ONE * 7 / 10, ONE, door);
            quad(&mut o, m, n, u, ONE * 9 / 10, ONE * 12 / 10, frame);
        }
        let mesh = meshes.add(o);
        let c = centre(r);
        let reach = fx::length(hx, hz) + h;
        let mut g = Group::new(&format!("{}dungeon_{}", crate::places::FADED, r.id), c, reach);
        g.shapes.solid(mesh, Affine::translate(c), [ONE; 3], 0);
        // a fountain: a stone basin and a glow of water over it
        for f in &r.fountains {
            let p = at(r, f.at_cm[0], f.top_cm, f.at_cm[1]);
            let basin = Affine::translate(p.add(v3(0, ONE * 35 / 100, 0))).then(&Affine::scale(v3(
                ONE * 9 / 10,
                ONE * 7 / 10,
                ONE * 9 / 10,
            )));
            g.shapes.inked(
                Mesh::Cube,
                basin,
                [ONE * 72 / 100, ONE * 70 / 100, ONE * 66 / 100],
                look.ink(),
            );
            let water = Affine::translate(p.add(v3(0, ONE * 80 / 100, 0))).then(&Affine::scale(v3(
                ONE * 6 / 10,
                ONE * 3 / 10,
                ONE * 6 / 10,
            )));
            g.shapes.glow(
                Mesh::Sphere,
                water,
                [ONE * 78 / 100, ONE * 70 / 100, ONE * 94 / 100],
                ONE * 7 / 10,
            );
        }
        dress(r, look, &mut g);
        // a spring: a glow of green water just over the pillar's top
        for s in &r.springs {
            let p = at(r, s.at_cm[0], s.top_cm, s.at_cm[1]);
            let pool = Affine::translate(p.add(v3(0, ONE * 3 / 100, 0))).then(&Affine::scale(v3(
                cm(s.radius_cm) * 2,
                ONE,
                cm(s.radius_cm) * 2,
            )));
            g.shapes.glow(
                Mesh::Square,
                pool,
                [ONE * 62 / 100, ONE * 89 / 100, ONE * 78 / 100],
                ONE * 8 / 10,
            );
        }
        out.push(g);
    }
    out.push(tree_group(w, look));
}

/// The lowest a hanging root comes, over the room's floor: above every hop
/// from a 3 m top (feet 3.5 m, head 5.3 m) and the camera's boom over it.
pub const HANG_CLEAR_CM: i32 = 750;

/// A room's dressing (`Decor`), the same every run from its seed (CW-4).
fn dress(r: &RoomSpec, look: &Look, g: &mut Group) {
    let d = &r.decor;
    let mut rng = crate::rng::Rng::new(d.seed);
    let (hx, hz) = (r.size_cm[0] / 2, r.size_cm[1] / 2);
    let ink = look.ink();
    let root = rgb(r.colours.root);
    let lit = rgb(r.colours.mushroom);
    // hanging roots: tapering down from the ceiling
    for _ in 0..d.hanging {
        let x = rng.between(-hx + 60, hx - 60);
        let z = rng.between(-hz + 60, hz - 60);
        let len = rng.between(150, r.height_cm - HANG_CLEAR_CM);
        let thick = rng.between(12, 30);
        let top = at(r, x, r.height_cm - len / 2, z);
        let a = Affine::translate(top)
            .then(&Affine::rot_x(fx::deg(180)))
            .then(&Affine::scale(v3(cm(thick), cm(len), cm(thick))));
        g.shapes.inked(Mesh::Taper, a, root, ink);
    }
    // ridges of root up the walls, half in them
    for k in 0..d.ridges {
        let wall = k % 4;
        let t = rng.between(-90, 90);
        let (x, z) = match wall {
            0 => (hx * t / 100, -hz),
            1 => (hx * t / 100, hz),
            2 => (-hx, hz * t / 100),
            _ => (hx, hz * t / 100),
        };
        let w = rng.between(40, 70);
        let a = Affine::translate(at(r, x, r.height_cm / 2, z)).then(&Affine::scale(v3(
            cm(w),
            cm(r.height_cm),
            cm(w),
        )));
        g.shapes.inked(Mesh::Taper, a, root, ink);
    }
    // mushrooms: at the feet of the walls and the pillars, glowing softly
    for k in 0..d.mushrooms {
        let (x, z) = if k % 2 == 0 && !r.pillars.is_empty() {
            let p = r.pillars[(k as usize / 2) % r.pillars.len()];
            let a = fx::deg(rng.between(0, 359));
            let rr = p[2] + 25;
            (
                p[0] + fx::mul(fx::cos(a), rr * ONE) / ONE,
                p[1] - fx::mul(fx::sin(a), rr * ONE) / ONE,
            )
        } else {
            let t = rng.between(-85, 85);
            if k % 4 == 1 {
                (hx * t / 100, -hz + 35)
            } else {
                (-hx + 35, hz * t / 100)
            }
        };
        let s = rng.between(10, 22);
        let cap = Affine::translate(at(r, x, s, z)).then(&Affine::scale(v3(cm(s * 2), cm(s), cm(s * 2))));
        g.shapes.glow(Mesh::Sphere, cap, lit, ONE * 6 / 10);
    }
}

/// The great tree in the forest, a place's group of its own
/// (`place:great_tree`): its trunk, its crown, its roots and its doorway.
/// The sockets, the bulb and the door's leaf change, so `draw` draws them.
fn tree_group(w: &World, look: &Look) -> Group {
    let o = &spec().outside;
    let c = tree(w);
    let (r, h) = (cm(o.trunk_cm), cm(o.tree_cm));
    let mut g = Group::new(
        &format!("{}great_tree", crate::places::FADED),
        c.add(v3(0, h / 2, 0)),
        h,
    );
    let bark = [ONE * 42 / 100, ONE * 29 / 100, ONE * 18 / 100];
    let leaf = [ONE * 31 / 100, ONE * 48 / 100, ONE * 22 / 100];
    let leaf2 = [ONE * 40 / 100, ONE * 58 / 100, ONE * 26 / 100];
    let ink = look.ink();
    // the trunk: an eight-sided column narrowing upward, a little into the ground
    let trunk = Affine::translate(c.add(v3(0, h * 3 / 10 - ONE, 0))).then(&Affine::scale(v3(
        r * 2,
        h * 6 / 10,
        r * 2,
    )));
    g.shapes.inked(Mesh::Taper, trunk, bark, ink);
    // the crown: faceted balls round the top
    let crown = c.add(v3(0, h * 7 / 10, 0));
    for (k, (dx, dy, dz, s)) in [
        (0, 6, 0, 26),
        (-11, 0, 4, 18),
        (10, 2, -5, 19),
        (3, -2, 11, 17),
        (-4, 3, -11, 17),
    ]
    .iter()
    .enumerate()
    {
        let at = crown.add(v3(fx::int(*dx), fx::int(*dy), fx::int(*dz)));
        let col = if k % 2 == 0 { leaf } else { leaf2 };
        g.shapes.inked(
            Mesh::Facet,
            Affine::translate(at).then(&Affine::scale(v3(fx::int(*s), fx::int(*s) * 7 / 10, fx::int(*s)))),
            col,
            ink,
        );
    }
    // the roots: low wedges splaying out from the foot, between the door's
    for k in 0..7 {
        let a = fx::deg(25 + 50 * k);
        let d = facing(a, 0);
        let at = c.add(d.scale(r + ONE * 2)).add(v3(0, ONE * 4 / 10, 0));
        let roo = Affine::translate(at)
            .then(&Affine::rot_y(a))
            .then(&Affine::scale(v3(ONE * 18 / 10, ONE, ONE * 6)));
        g.shapes.inked(Mesh::Wedge, roo, bark, ink);
    }
    // the doorway: a dark arch on the trunk's face, the way the door faces
    let yaw = fx::deg(o.yaw_deg);
    let face = c
        .add(facing(yaw, 0).scale(r - ONE / 4))
        .add(v3(0, ONE * 16 / 10, 0));
    let arch = Affine::translate(face)
        .then(&Affine::rot_y(yaw))
        .then(&Affine::scale(v3(ONE * 22 / 10, ONE * 32 / 10, ONE * 8 / 10)));
    g.shapes.solid(
        Mesh::Cube,
        arch,
        [ONE * 12 / 100, ONE * 8 / 100, ONE * 5 / 100],
        0,
    );
    g
}

/// What changes at the great tree: the door's leaf while it is shut, each
/// socket (dark, or lit and glowing), and the roots' bulb.
pub fn draw(w: &World, _alpha: Fx, look: &Look, out: &mut crate::render::Shapes) {
    if w.dungeon.open && w.player.pos.sub(tree(w)).len_xz() > ONE * 300 {
        return;
    }
    if tree(w).sub(w.player.pos).len_xz() > ONE * 400 {
        return;
    }
    let o = &spec().outside;
    let yaw = fx::deg(o.yaw_deg);
    let c = tree(w);
    let ink = look.ink();
    if !w.dungeon.open {
        let face = c
            .add(facing(yaw, 0).scale(cm(o.trunk_cm) + ONE / 5))
            .add(v3(0, ONE * 16 / 10, 0));
        let leaf = Affine::translate(face)
            .then(&Affine::rot_y(yaw))
            .then(&Affine::scale(v3(ONE * 2, ONE * 3, ONE / 5)));
        out.inked(
            Mesh::Cube,
            leaf,
            [ONE * 47 / 100, ONE * 33 / 100, ONE * 21 / 100],
            ink,
        );
    }
    for (k, at) in sockets(w).iter().enumerate() {
        let ring = Affine::translate(*at)
            .then(&Affine::rot_y(yaw))
            .then(&Affine::rot_x(fx::deg(90)))
            .then(&Affine::scale(v3(ONE * 7 / 10, ONE / 4, ONE * 7 / 10)));
        out.inked(
            Mesh::Tube,
            ring,
            [ONE * 62 / 100, ONE * 60 / 100, ONE * 55 / 100],
            ink,
        );
        if w.dungeon.sockets[k] {
            out.glow(
                Mesh::Sphere,
                Affine::translate(*at).then(&Affine::scale(v3(ONE / 2, ONE / 2, ONE / 2))),
                [ONE / 2, ONE * 82 / 100, ONE],
                ONE * 9 / 10,
            );
        }
    }
    if !w.dungeon.open {
        let b = bulb(w);
        out.inked(
            Mesh::Facet,
            Affine::translate(b).then(&Affine::scale(v3(ONE * 9 / 10, ONE * 8 / 10, ONE * 9 / 10))),
            [ONE * 72 / 100, ONE * 42 / 100, ONE * 66 / 100],
            ink,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_dungeon_file_reads_and_a_bad_one_is_refused() {
        let s = spec();
        assert!(!s.rooms.is_empty());
        let bad =
            include_str!("../../../data/dungeon.json").replacen("\"wall\": \"w\"", "\"wall\": \"up\"", 1);
        assert!(Spec::parse(&bad).unwrap_err().contains("stands in no wall"));
    }

    #[test]
    fn every_room_floats_well_over_the_land_under_it() {
        let w = crate::story::opening(9);
        for r in &spec().rooms {
            let c = centre(r);
            let (hx, hz) = (cm(r.size_cm[0]) / 2, cm(r.size_cm[1]) / 2);
            for (x, z) in [(-hx, -hz), (hx, -hz), (-hx, hz), (hx, hz), (0, 0)] {
                let g = w.ground(c.x + x, c.z + z);
                assert!(
                    c.y - g >= ONE * 100,
                    "room {} is {} m over the land",
                    r.id,
                    (c.y - g) / ONE
                );
            }
        }
    }

    #[test]
    fn in_the_hub_a_body_stands_on_the_floor_a_ledge_a_pillar_and_the_ramp() {
        let w = crate::story::opening(9);
        let r = &spec().rooms[0];
        let floor = centre(r).y;
        let s = |x: i32, z: i32| {
            let p = at(r, x, 0, z);
            crate::solid::surface(&w, p.x, p.z) - floor
        };
        // the floor between pillars
        assert_eq!(s(-600, -700), 0);
        // the entrance's ledge, a pillar's top
        assert_eq!(s(0, 800), cm(300));
        assert_eq!(s(-450, 400), cm(300));
        // the ramp: 3 m at its west end (x 2 m), 0 at its east (x 10 m), and
        // halfway (x 6 m) half: 1.5 m, within a unit's rounding
        // (within a few units: the climb's share is rounded in fixed point)
        assert!(
            (s(250, 950) - (cm(300) - cm(300) * 50 / 800)).abs() <= 8,
            "5% of the way along"
        );
        assert!((s(600, 950) - cm(150)).abs() <= 8, "{}", s(600, 950));
        assert!(s(990, 950) <= cm(5));
        // the player is in the hub only inside it
        assert_eq!(room_at(at(r, 0, 300, 800)), Some(0));
        assert_eq!(room_at(crate::world::START), None);
    }

    #[test]
    fn a_hop_from_the_entrance_s_ledge_lands_on_the_next_pillar_and_a_walk_off_falls_to_the_floor() {
        use crate::input::{Input, FORWARD, RUN};
        let r = &spec().rooms[0];
        let floor = centre(r).y;
        // run from the ledge's west end toward the pillar at (-4.5, 4): a
        // 2.1 m gap, inside a hop's 4.3 m
        let mut w = crate::story::opening(9);
        crate::story::step_aside(&mut w);
        let from = at(r, -150, 300, 700);
        let goal = at(r, -450, 300, 400);
        let yaw = crate::geom::yaw_of(goal.sub(from));
        w.player = crate::player::Body::new(from, yaw);
        w.me = crate::player::Fighter::new(w.me.kind, crate::items::held(&w), 0, from, yaw, false);
        w.aim.yaw = yaw;
        let mut flew = false;
        for _ in 0..90 {
            w.step(&Input {
                keys: FORWARD | RUN,
                ..Input::default()
            });
            flew |= w.me.gait.air;
            if w.player.pos.sub(goal).len_xz() < ONE / 2 {
                break;
            }
        }
        // let go of the keys and come down
        for _ in 0..30 {
            w.step(&Input::default());
            if !w.me.gait.air {
                break;
            }
        }
        assert!(flew, "it hopped");
        assert!(w.player.pos.sub(goal).len_xz() < ONE, "it reached the pillar");
        assert!(
            (w.player.pos.y - (floor + cm(300))).abs() < ONE / 10,
            "on its top"
        );
        // walking off the same edge falls to the floor below
        let mut w = crate::story::opening(9);
        crate::story::step_aside(&mut w);
        w.player = crate::player::Body::new(from, yaw);
        w.me = crate::player::Fighter::new(w.me.kind, crate::items::held(&w), 0, from, yaw, false);
        w.aim.yaw = yaw;
        for _ in 0..120 {
            w.step(&Input {
                keys: FORWARD,
                ..Input::default()
            });
        }
        assert!(
            w.player.pos.y - floor < ONE / 10,
            "down on the floor, {}",
            (w.player.pos.y - floor) / (ONE / 100)
        );
    }

    #[test]
    fn a_door_fades_out_moves_the_player_through_and_fades_back_and_keeps_their_health() {
        let mut w = crate::story::opening(9);
        crate::story::step_aside(&mut w);
        let all = doors(&w);
        // sealed, the great tree's door says so and stays shut
        w.player = crate::player::Body::new(all[0].at.add(facing(all[0].yaw, 0).scale(ONE)), all[0].yaw);
        use_door(&mut w, 0);
        assert!(w.dungeon.transit.is_none());
        assert_eq!(
            w.story.banner.as_ref().map(|b| b.key.as_str()),
            Some("dungeon_tree_sealed")
        );
        w.dungeon.open = true;
        // open, the great tree's door outside leads to the hub's entrance
        let (p, yaw) = arrival(&w, &all[0]);
        w.player = crate::player::Body::new(p, yaw);
        w.me.health.hp = 40;
        let (k, _) = reach(&w).unwrap_or((usize::MAX, 0));
        assert_eq!(k, usize::MAX, "4.5 m out is past a door's reach");
        w.player = crate::player::Body::new(all[0].at.add(facing(all[0].yaw, 0).scale(ONE)), yaw);
        let (k, _) = reach(&w).expect("a metre out, the door is in reach");
        assert_eq!(k, 0);
        use_door(&mut w, k);
        for _ in 0..spec().dark_ticks {
            w.step(&crate::input::Input::default());
        }
        assert!(w.story.veil, "dark");
        for _ in 0..spec().hold_ticks + 2 {
            w.step(&crate::input::Input::default());
        }
        assert!(!w.story.veil && w.dungeon.transit.is_none(), "back in the light");
        assert_eq!(room_at(w.player.pos), Some(0), "in the hub");
        assert!(
            w.me.health.hp >= 40 && w.me.health.hp < 100,
            "the health it had: {}",
            w.me.health.hp
        );
        // a closed door says why, and nothing moves
        let left = all.iter().position(|d| d.id == "left").unwrap();
        let before = w.player.pos;
        use_door(&mut w, left);
        assert!(w.dungeon.transit.is_none());
        assert_eq!(w.player.pos, before);
        assert_eq!(
            w.story.banner.as_ref().map(|b| b.key.as_str()),
            Some("dungeon_door_later")
        );
    }

    /// A world past the opening, the player before the great tree's door.
    fn at_the_tree() -> World {
        let mut w = crate::story::opening(9);
        crate::story::step_aside(&mut w);
        let d = doors(&w)[0].clone();
        let p = d.at.add(facing(d.yaw, 0).scale(ONE * 6));
        w.player = crate::player::Body::new(v3(p.x, w.ground(p.x, p.z), p.z), d.yaw + fx::deg(180));
        w
    }

    /// An orb of the roots' at `at`, going `dir`, turned on the tick just run
    /// (as `orbs::turn` leaves it).
    fn turned_orb(w: &mut World, at: V3, dir: V3) {
        let v = &crate::orbs::spec().pattern(crate::orbs::Pattern::PracticeOne)[0];
        let mut next = w.orbs.next;
        let mut o = crate::orbs::fire(v, at, at.add(dir), 0, ROOTS, &mut next).remove(0);
        w.orbs.next = next;
        o.dir = dir.norm();
        o.hold = 0;
        o.age = 5;
        o.turned = Some(5);
        w.orbs.flying.push(o);
    }

    #[test]
    fn an_orb_turned_toward_a_dark_socket_lights_it_and_both_lit_open_the_tree_s_door() {
        let mut w = at_the_tree();
        let socks = sockets(&w);
        // knocked 30° off the line to the left socket: within 60° of both
        // dark sockets, so it flies straight to whichever its line is nearer
        let from = socks[0].add(v3(ONE * 4, -ONE, ONE * 3));
        let off = crate::orbs::turn_y(socks[0].sub(from).norm(), fx::deg(30));
        let ways = [socks[0].sub(from).norm(), socks[1].sub(from).norm()];
        let first = if off.dot(ways[0]) >= off.dot(ways[1]) {
            0
        } else {
            1
        };
        turned_orb(&mut w, from, off);
        roots(&mut w);
        let mine = w.orbs.flying.iter().find(|o| o.turned.is_some()).map(|o| o.dir);
        assert_eq!(mine, Some(ways[first]), "aimed at the socket nearer its line");
        // 5.1 m at the practice orb's 3 m/s: about 102 ticks
        for _ in 0..200 {
            w.step(&crate::input::Input::default());
            if w.dungeon.sockets[first] {
                break;
            }
        }
        assert!(
            w.dungeon.sockets[first] && !w.dungeon.sockets[1 - first],
            "one lit, one dark"
        );
        assert!(!w.dungeon.open);
        // an orb not turned passes by the other socket and lights nothing
        let other = 1 - first;
        let mut w2 = at_the_tree();
        let v = &crate::orbs::spec().pattern(crate::orbs::Pattern::PracticeOne)[0];
        let mut next = 0;
        let mut o = crate::orbs::fire(
            v,
            socks[other].add(v3(-ONE, 0, 0)),
            socks[other].add(v3(ONE, 0, 0)),
            0,
            ROOTS,
            &mut next,
        )
        .remove(0);
        o.hold = 0;
        w2.orbs.flying.push(o);
        for _ in 0..30 {
            w2.step(&crate::input::Input::default());
        }
        assert!(
            !w2.dungeon.sockets[other],
            "an orb that was never turned lights nothing"
        );
        // the other turned into it too: the door opens
        let from = socks[other].add(v3(0, 0, ONE * 3));
        turned_orb(&mut w, from, socks[other].sub(from));
        for _ in 0..200 {
            w.step(&crate::input::Input::default());
            if w.dungeon.open {
                break;
            }
        }
        assert!(w.dungeon.open, "both lit, open");
        assert_eq!(
            w.story.banner.as_ref().map(|b| b.key.as_str()),
            Some("dungeon_tree_open")
        );
    }

    #[test]
    fn the_bulb_throws_only_while_the_player_is_near_and_the_door_is_shut() {
        let mut w = at_the_tree();
        let count = |w: &World| w.orbs.flying.iter().filter(|o| o.caster == ROOTS).count();
        w.step(&crate::input::Input::default());
        assert_eq!(count(&w), 1, "the first tick in range throws one");
        // far off, nothing more
        let mut far = at_the_tree();
        let p = tree(&far).add(v3(ONE * 60, 0, ONE * 60));
        far.player = crate::player::Body::new(v3(p.x, far.ground(p.x, p.z), p.z), 0);
        for _ in 0..200 {
            far.step(&crate::input::Input::default());
        }
        assert_eq!(count(&far), 0);
        // open, the bulb is quiet
        let mut open = at_the_tree();
        open.dungeon.open = true;
        for _ in 0..200 {
            open.step(&crate::input::Input::default());
        }
        assert_eq!(count(&open), 0);
    }

    #[test]
    fn a_body_cannot_run_through_the_great_tree_s_trunk() {
        use crate::input::{Input, FORWARD, RUN};
        let mut w = at_the_tree();
        // run north at the trunk from 12 m south of its middle
        let c = tree(&w);
        let p = c.add(v3(ONE, 0, ONE * 12));
        w.player = crate::player::Body::new(v3(p.x, w.ground(p.x, p.z), p.z), 0);
        w.aim.yaw = 0;
        let r = cm(spec().outside.trunk_cm);
        let mut nearest = i32::MAX;
        for _ in 0..240 {
            w.step(&Input {
                keys: FORWARD | RUN,
                ..Input::default()
            });
            nearest = nearest.min(w.player.pos.sub(c).len_xz());
        }
        // it slides round the trunk and runs on north, its middle never
        // nearer the trunk's than the trunk's radius (and a hair for the
        // push-out's rounding)
        assert!(nearest >= r - ONE / 50, "{} cm in", (r - nearest) * 100 / ONE);
        assert!(nearest < r + ONE, "it did meet the trunk");
    }

    #[test]
    fn the_tree_s_door_opened_stays_open_through_a_save_and_a_load() {
        let mut w = at_the_tree();
        w.dungeon.sockets = [true, true];
        w.dungeon.open = true;
        let p = crate::save::progress(&w);
        let back = crate::save::read(&crate::save::text(&p)).expect("reads");
        let a = crate::save::world(&back);
        assert!(a.dungeon.open && a.dungeon.sockets == [true, true]);
        // and a save from before stays shut
        let shut = crate::save::world(
            &crate::save::read(&crate::save::text(&crate::save::progress(&at_the_tree()))).unwrap(),
        );
        assert!(!shut.dungeon.open);
    }

    #[test]
    fn every_room_door_leaves_you_inside_its_room_on_its_ledge() {
        let w = crate::story::opening(9);
        for d in doors(&w).iter().filter(|d| d.room.is_some()) {
            let (p, _) = arrival(&w, d);
            let r = &spec().rooms[d.room.unwrap()];
            let q = p.sub(centre(r));
            assert_eq!(
                room_at(p),
                d.room,
                "door {} leaves you at x {} z {} y {} cm",
                d.id,
                q.x * 100 / ONE,
                q.z * 100 / ONE,
                q.y * 100 / ONE
            );
            assert_eq!(p.y - centre(r).y, cm(300), "door {} on its sill's ledge", d.id);
        }
    }

    #[test]
    fn standing_in_the_healing_spring_heals_a_point_a_tick_up_to_full() {
        let mut w = crate::story::opening(9);
        crate::story::step_aside(&mut w);
        let r = &spec().rooms[0];
        let p = at(r, 0, 300, 250);
        w.player = crate::player::Body::new(p, 0);
        w.me = crate::player::Fighter::new(w.me.kind, crate::items::held(&w), 0, p, 0, false);
        w.me.health.hp = 90;
        for _ in 0..5 {
            w.step(&crate::input::Input::default());
        }
        assert_eq!(w.me.health.hp, 95, "five ticks, five points");
        for _ in 0..20 {
            w.step(&crate::input::Input::default());
        }
        assert_eq!(w.me.health.hp, w.me.health.most, "never past full");
        // at a faster rate, the last tick stops at full: 98 + 5 is 100, not 103
        assert_eq!(healed(98, 100, 5), 100);
        assert_eq!(healed(90, 100, 5), 95);
        // a step off the spring heals nothing
        let off = at(r, -450, 300, 400);
        w.player = crate::player::Body::new(off, 0);
        w.me.health.hp = 50;
        w.step(&crate::input::Input::default());
        assert_eq!(w.me.health.hp, 50);
    }
}
