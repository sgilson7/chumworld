//! `solid` (M5 Places and story, task S): what a body stands on and what it
//! cannot walk through.
//!
//! Every building in `data/places.json` with a footprint keeps bodies out
//! of it: a body's circle (`RADIUS`) is pushed back out of the footprint's
//! rectangle, turned as the building is, and its speed into the wall is
//! taken away, so it slides along. Fence spans are thin walls of their own.
//! The player's house is four walls with a gap at its door (`Map::home`),
//! since the opening starts inside it.
//!
//! What a body stands on (`surface`): the ground, or the bridge's deck over
//! the river, or the stone floor of the player's house; each with a short
//! ramp at its edge, so walking on is a slope, not a step.
//!
//! This is a rule for every body alike, the player's and each foe's (CW-3 is
//! about decisions: no pilot moves a body; the walls do, the same for all).
//! Steps in `World::step` straight after the feet (`motion::step`), so the
//! arms are carried from where the body really is.

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, V3};
use crate::models::Prop;
use crate::places;
use crate::player;
use crate::world::World;

/// A body's radius against a wall.
pub const RADIUS: Fx = ONE * 35 / 100;
/// How thick a fence is, and the walls of the player's house.
pub const FENCE_THICK: Fx = ONE * 16 / 100;
pub const WALL_THICK: Fx = ONE * 22 / 100;
/// The gap in the player's house's front wall: the door's frame is 1.26 m
/// across (M3's house model), the gap a little narrower.
pub const DOOR_GAP: Fx = ONE * 110 / 100;
/// The bridge's deck stands this far over its foot (M3's planks: 560 mm up,
/// 80 mm thick), and its rails this far either side of the middle.
pub const DECK: Fx = ONE * 600 / 1000;
pub const DECK_HALF: Fx = ONE * 1420 / 1000;
/// The stone floor of the player's house: its footing, 400 mm (M3).
pub const FLOOR: Fx = ONE * 400 / 1000;
/// A ramp from the ground up to a deck or a floor, this long.
pub const RAMP: Fx = ONE * 2;
pub const DOOR_RAMP: Fx = ONE * 9 / 10;

/// A rectangle bodies keep out of: its middle (x, z), its half sizes along
/// its own x and z, and its yaw (`Affine::rot_y`'s).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wall {
    pub x: Fx,
    pub z: Fx,
    pub hx: Fx,
    pub hz: Fx,
    pub yaw: i32,
}

impl Wall {
    /// A point in the wall's own frame: along its x, along its z.
    pub fn local(&self, x: Fx, z: Fx) -> (Fx, Fx) {
        let (dx, dz) = (x - self.x, z - self.z);
        let (s, c) = (fx::sin(self.yaw), fx::cos(self.yaw));
        (fx::mul(dx, c) - fx::mul(dz, s), fx::mul(dx, s) + fx::mul(dz, c))
    }

    /// A direction in the wall's frame, back in the world.
    pub fn world_dir(&self, lx: Fx, lz: Fx) -> (Fx, Fx) {
        let (s, c) = (fx::sin(self.yaw), fx::cos(self.yaw));
        (fx::mul(lx, c) + fx::mul(lz, s), -fx::mul(lx, s) + fx::mul(lz, c))
    }

    /// How far, and which way (a flat unit direction), a circle of radius
    /// `r` at (x, z) must move to stand clear of the wall; `None` when it
    /// is clear already.
    pub fn push(&self, x: Fx, z: Fx, r: Fx) -> Option<(Fx, V3)> {
        let (lx, lz) = self.local(x, z);
        let (ox, oz) = (lx.abs() - self.hx, lz.abs() - self.hz);
        if ox >= r || oz >= r {
            return None;
        }
        let (sx, sz) = (if lx < 0 { -ONE } else { ONE }, if lz < 0 { -ONE } else { ONE });
        let (nx, nz, depth) = if ox > 0 && oz > 0 {
            // past a corner: out along the line from the corner
            let d = fx::length(ox, oz);
            if d >= r {
                return None;
            }
            let d1 = d.max(1);
            (fx::div(fx::mul(ox, sx), d1), fx::div(fx::mul(oz, sz), d1), r - d)
        } else if ox > oz {
            (sx, 0, r - ox)
        } else {
            (0, sz, r - oz)
        };
        let (wx, wz) = self.world_dir(nx, nz);
        Some((depth, v3(wx, 0, wz)))
    }

    /// Whether (x, z) lies inside the wall's rectangle grown by `by`.
    pub fn holds(&self, x: Fx, z: Fx, by: Fx) -> bool {
        let (lx, lz) = self.local(x, z);
        lx.abs() <= self.hx + by && lz.abs() <= self.hz + by
    }
}

/// The player's house: which prop of which place it is, its footprint as a
/// wall, its foot on the ground, and the bed in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Home {
    pub place: usize,
    pub prop: usize,
    pub wall: Wall,
    pub foot: V3,
    pub bed: places::Spot,
}

/// Everything in this module that never changes, worked out once from the
/// layout (`World::new`): the walls, the bridges' spans and decks, the
/// player's house.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Map {
    pub walls: Vec<Wall>,
    /// each bridge's deck as a rectangle, and the deck's height
    pub decks: Vec<(Wall, Fx)>,
    /// M7 roadside: the watchtower's and the tor's tops and ramps
    pub climbs: Vec<crate::roadside::Climb>,
    pub home: Option<Home>,
}

impl Map {
    pub fn build(w: &World) -> Map {
        let home = find_home(w);
        let mut m = Map {
            walls: walls(w, home),
            decks: Vec::new(),
            climbs: crate::roadside::climbs(w),
            home,
        };
        for s in places::prop_spots(w, Prop::Bridge) {
            let span = Wall {
                x: s.at.x,
                z: s.at.z,
                hx: DECK_HALF,
                hz: places::BRIDGE_SPAN / 2,
                yaw: s.yaw,
            };
            m.decks.push((span, s.at.y + DECK));
        }
        m
    }
}

/// The house the bed stands in (`data/places.json`: the bed is `inside`).
fn find_home(w: &World) -> Option<Home> {
    let layout = &w.land.layout;
    for (pk, place) in layout.places.iter().enumerate() {
        let c = places::metres(place.centre);
        let Some(bed) = places::spots(w, place).into_iter().find(|s| s.prop == Prop::Bed) else {
            continue;
        };
        for (k, q) in place.props.iter().enumerate() {
            if places::prop_kind(&q.prop) != Some(Prop::House) {
                continue;
            }
            let size = layout.footprints_m.get(&q.prop)?;
            let mut foot = c.add(v3(q.at[0] * ONE, 0, q.at[1] * ONE));
            let wall = Wall {
                x: foot.x,
                z: foot.z,
                hx: size[0] * ONE / 2,
                hz: size[1] * ONE / 2,
                yaw: fx::deg(q.yaw_deg),
            };
            if wall.holds(bed.at.x, bed.at.z, 0) {
                foot.y = w.ground(foot.x, foot.z);
                return Some(Home {
                    place: pk,
                    prop: k,
                    wall,
                    foot,
                    bed,
                });
            }
        }
    }
    None
}

/// Every wall in the region: each building's footprint (but the player's
/// house, which is four walls and a door), each fence span, and the
/// bridges' rails.
fn walls(w: &World, home: Option<Home>) -> Vec<Wall> {
    let layout = &w.land.layout;
    let mut out = Vec::new();
    for (pk, place) in layout.places.iter().enumerate() {
        let c = places::metres(place.centre);
        for (k, q) in place.props.iter().enumerate() {
            let Some(prop) = places::prop_kind(&q.prop) else {
                continue;
            };
            // M7 roadside: what is climbed has walls of its own (below)
            if q.inside || matches!(prop, Prop::Bridge | Prop::Watchtower | Prop::Tor) {
                continue;
            }
            let Some(size) = layout.footprints_m.get(&q.prop) else {
                continue;
            };
            let at = c.add(v3(q.at[0] * ONE, 0, q.at[1] * ONE));
            let wall = Wall {
                x: at.x,
                z: at.z,
                hx: size[0] * ONE / 2,
                hz: size[1] * ONE / 2,
                yaw: fx::deg(q.yaw_deg),
            };
            if home.is_some_and(|h| h.place == pk && h.prop == k) {
                out.extend(house_walls(&wall));
            } else {
                out.push(small(prop, wall));
            }
        }
        for f in places::fence_spans(w, place) {
            out.push(Wall {
                x: f.at.x,
                z: f.at.z,
                hx: places::FENCE_SPAN / 2,
                hz: FENCE_THICK / 2,
                yaw: f.yaw,
            });
        }
    }
    // the bed in the player's house
    if let Some(h) = home {
        if let Some(size) = layout.footprints_m.get("bed") {
            out.push(Wall {
                x: h.bed.at.x,
                z: h.bed.at.z,
                hx: size[0] * ONE / 2,
                hz: size[1] * ONE / 2,
                yaw: h.bed.yaw,
            });
        }
    }
    out.extend(crate::roadside::walls(w));
    for s in places::prop_spots(w, Prop::Bridge) {
        for side in [-1, 1] {
            let (dx, dz) = rot(s.yaw, side * DECK_HALF, 0);
            out.push(Wall {
                x: s.at.x + dx,
                z: s.at.z + dz,
                hx: ONE / 10,
                hz: places::BRIDGE_SPAN / 2,
                yaw: s.yaw,
            });
        }
    }
    out
}

/// A footprint as the thing in it stands: a post, a well or a fire is
/// smaller than the ground flattened for it.
fn small(prop: Prop, mut wall: Wall) -> Wall {
    let shrink = match prop {
        Prop::Signpost | Prop::Scarecrow | Prop::SavePost => ONE * 38 / 100,
        Prop::Well => ONE * 15 / 100,
        Prop::Campfire => ONE * 25 / 100,
        Prop::Chest => ONE * 5 / 100,
        // M7 roadside: a lone tree's trunk, not its crown
        Prop::LoneTree => ONE * 115 / 100,
        _ => 0,
    };
    wall.hx -= shrink;
    wall.hz -= shrink;
    wall
}

/// A point `(lx, lz)` in a frame turned by `yaw`, in the world.
fn rot(yaw: i32, lx: Fx, lz: Fx) -> (Fx, Fx) {
    let (s, c) = (fx::sin(yaw), fx::cos(yaw));
    (fx::mul(lx, c) + fx::mul(lz, s), -fx::mul(lx, s) + fx::mul(lz, c))
}

/// The player's house as four walls inside its footprint, the front (its
/// local −z, where M3's door is) split round the door.
pub fn house_walls(f: &Wall) -> Vec<Wall> {
    let t = WALL_THICK / 2;
    let at = |lx: Fx, lz: Fx, hx: Fx, hz: Fx| {
        let (dx, dz) = rot(f.yaw, lx, lz);
        Wall {
            x: f.x + dx,
            z: f.z + dz,
            hx,
            hz,
            yaw: f.yaw,
        }
    };
    let side = (f.hx - DOOR_GAP / 2) / 2;
    vec![
        at(0, f.hz - t, f.hx, t),
        at(-f.hx + t, 0, t, f.hz),
        at(f.hx - t, 0, t, f.hz),
        at(-(DOOR_GAP / 2 + side), -f.hz + t, side, t),
        at(DOOR_GAP / 2 + side, -f.hz + t, side, t),
    ]
}

/// What a body at (x, z) stands on: the ground, the bridge's deck, or the
/// floor of the player's house.
pub fn surface(w: &World, x: Fx, z: Fx) -> Fx {
    let ground = w.ground(x, z);
    let mut best = ground;
    for (span, top) in &w.solid.decks {
        if let Some(h) = raised(span, *top, ground, x, z, RAMP, false) {
            best = best.max(h);
        }
    }
    if let Some(h) = w.solid.home {
        let floor = Wall {
            hx: h.wall.hx + ONE * 15 / 100,
            hz: h.wall.hz + ONE * 15 / 100,
            ..h.wall
        };
        if let Some(y) = raised(&floor, h.foot.y + FLOOR, ground, x, z, DOOR_RAMP, true) {
            best = best.max(y);
        }
    }
    // M7 roadside: the watchtower's and the tor's tops and ramps
    if let Some(y) = crate::roadside::surface(&w.solid.climbs, x, z) {
        best = best.max(y);
    }
    best
}

/// The height of a raised rectangle at (x, z), `top` within it, easing
/// down to `ground` over `ramp` past its ends (its local z ends, or with
/// `front` only its local −z end); `None` outside.
fn raised(r: &Wall, top: Fx, ground: Fx, x: Fx, z: Fx, ramp: Fx, front: bool) -> Option<Fx> {
    let (lx, lz) = r.local(x, z);
    if lx.abs() > r.hx {
        return None;
    }
    let past = if front {
        if lz > r.hz {
            return None;
        }
        (-lz - r.hz).max(0)
    } else {
        lz.abs() - r.hz
    };
    if past <= 0 {
        return Some(top);
    }
    if past >= ramp || top <= ground {
        return None;
    }
    let t = ONE - fx::div(past, ramp);
    Some(ground + fx::mul(top - ground, t))
}

/// Pushes the circle at `pos` out of every wall; the share of `vel` into a
/// wall is taken away. Returns whether it was pushed.
pub fn keep_out(walls: &[Wall], pos: &mut V3, vel: &mut V3, r: Fx) -> bool {
    let mut pushed = false;
    // twice, so a body in a corner between two walls leaves both
    for _ in 0..2 {
        for wall in walls {
            if let Some((depth, n)) = wall.push(pos.x, pos.z, r) {
                pos.x += fx::mul(n.x, depth);
                pos.z += fx::mul(n.z, depth);
                let into = vel.dot(n);
                if into < 0 {
                    *vel = vel.sub(n.scale(into));
                }
                pushed = true;
            }
        }
    }
    pushed
}

/// The walls near (x, z), within `reach`: a body moves a few centimetres a
/// tick, so only these can touch it.
pub fn near(walls: &[Wall], x: Fx, z: Fx, reach: Fx) -> Vec<Wall> {
    walls.iter().filter(|k| k.holds(x, z, reach)).copied().collect()
}

/// Step 3b: every body kept out of the walls and stood on what is under it.
pub fn step(w: &mut World) {
    let map = w.solid.clone();
    for id in 0..player::count(w) {
        let b = player::body(w, id);
        let (mut pos, mut vel) = (b.pos, b.vel);
        // the player in bed lies on it (`story::lift`), and the bed is no wall to them
        let in_bed = id == 0 && crate::story::in_bed(w);
        if !in_bed {
            let close = near(&map.walls, pos.x, pos.z, ONE * 2);
            keep_out(&close, &mut pos, &mut vel, RADIUS);
            // M7 roadside: no one walks under a platform or into a rock
            crate::roadside::guard(&map.climbs, b.prev_pos.y, &mut pos, &mut vel);
        }
        pos.y = surface(w, pos.x, pos.z) + if in_bed { w.story.lift } else { 0 };
        let b = player::parts_mut(w, id).0;
        b.pos = pos;
        b.vel = vel;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall() -> Wall {
        // 6 m by 5 m, turned a quarter turn: its x runs north (−z), its z east
        Wall {
            x: fx::int(100),
            z: fx::int(200),
            hx: fx::int(3),
            hz: fx::int(5) / 2,
            yaw: fx::deg(90),
        }
    }

    #[test]
    fn a_wall_turned_a_quarter_turn_runs_its_own_x_north() {
        let k = wall();
        // 2 m north of the middle is +2 along its x; 1 m east is +1 along its z
        assert_eq!(k.local(fx::int(100), fx::int(198)), (fx::int(2), 0));
        assert_eq!(k.local(fx::int(101), fx::int(200)), (0, fx::int(1)));
    }

    #[test]
    fn a_body_against_a_wall_is_pushed_out_to_its_radius_and_stops_going_in() {
        let k = wall();
        // the wall's east face is 2.5 m east of its middle, at x = 102.5; a
        // body at x = 102.6 with radius 0.35 overlaps by 0.25 m and goes
        // back to 102.85, its eastward speed kept, its westward speed gone
        let mut p = v3(fx::int(1026) / 10, 0, fx::int(200));
        let mut vel = v3(-ONE / 10, 0, ONE / 20);
        assert!(keep_out(&[k], &mut p, &mut vel, RADIUS));
        assert!((p.x - fx::int(10285) / 100).abs() <= 2, "{}", p.x);
        assert_eq!(p.z, fx::int(200));
        assert_eq!(vel.x, 0);
        assert_eq!(vel.z, ONE / 20, "it slides along");
        // one 0.36 m clear is left alone
        let mut q = v3(fx::int(10286) / 100, 0, fx::int(200));
        let mut v = V3::ZERO;
        assert!(!keep_out(&[k], &mut q, &mut v, RADIUS));
    }

    #[test]
    fn the_player_cannot_walk_into_a_house_but_walks_out_of_their_own_door() {
        let w = World::new(1);
        let all = &w.solid.walls;
        // the second house in the village, at (-46, -10) from (2000, 2800):
        // its middle (1954, 2790) is inside a wall
        assert!(all.iter().any(|k| k.holds(fx::int(1954), fx::int(2790), 0)));
        // the player's house: its middle is clear (a room), and so is its
        // doorway; a point inside its back wall is not
        let h = w.solid.home.expect("the player's house");
        let mid = h.wall;
        assert!(!all.iter().any(|k| k.holds(mid.x, mid.z, 0)), "a room");
        let (dx, dz) = rot(mid.yaw, 0, -mid.hz + WALL_THICK / 2);
        assert!(
            !all.iter().any(|k| k.holds(mid.x + dx, mid.z + dz, 0)),
            "the doorway"
        );
        let (bx, bz) = rot(mid.yaw, 0, mid.hz - WALL_THICK / 2);
        assert!(
            all.iter().any(|k| k.holds(mid.x + bx, mid.z + bz, 0)),
            "the back wall"
        );
    }

    #[test]
    fn a_body_on_the_bridge_stands_on_its_deck_and_ramps_down_off_its_ends() {
        let w = World::new(1);
        let s = places::prop_spots(&w, Prop::Bridge)[0];
        // over the middle of the river the deck is 0.6 m over the bridge's
        // foot, whatever the channel under it
        assert_eq!(surface(&w, s.at.x, s.at.z), s.at.y + DECK);
        assert!(w.ground(s.at.x, s.at.z) < s.at.y - ONE, "the river runs under it");
        // a metre past an end, half way down the 2 m ramp
        let (dx, dz) = rot(s.yaw, 0, places::BRIDGE_SPAN / 2 + ONE);
        let (x, z) = (s.at.x + dx, s.at.z + dz);
        let g = w.ground(x, z);
        let want = g + (s.at.y + DECK - g) / 2;
        assert!(
            (surface(&w, x, z) - want).abs() <= 2,
            "{} {}",
            surface(&w, x, z),
            want
        );
    }
}
