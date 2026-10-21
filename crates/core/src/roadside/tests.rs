//! T12's tests: the sides come from the opening's data, every side has a
//! stop soon after the boundary and a landmark seen from it, the stops are
//! no more than a minute's run apart and stand clear of everything, the
//! straggler waits for the opening's end, and each stop does what it says.

use super::*;
use crate::input::{FORWARD, USE};
use crate::places::{Layout, RoadSpec};
use crate::story::Phase;

fn the(w: &World, id: &str) -> (Stop, V3) {
    w.roadside
        .stops
        .iter()
        .find(|(s, _)| s.id == id)
        .cloned()
        .unwrap_or_else(|| panic!("no stop {id}"))
}

/// A fresh world with the roadside's stops on the ground (no opening).
fn world() -> World {
    let mut w = World::new(1);
    begin(&mut w);
    w
}

fn press(w: &mut World, keys: u32) {
    w.step(&Input {
        keys,
        ..Input::default()
    });
    w.step(&Input::default());
}

/// The player stood at `at`, looking along `yaw`.
fn stand(w: &mut World, at: V3, yaw: i32) {
    let mut at = at;
    at.y = solid::surface(w, at.x, at.z);
    w.player = crate::player::Body::new(at, yaw);
    w.me = crate::player::Fighter::new(w.me.kind, w.me.held, w.me.team, at, yaw, false);
    w.aim.yaw = yaw;
}

/// A point in a stop's own frame (mm), in the world.
fn local(s: &Stop, foot: V3, x: i32, z: i32) -> V3 {
    let r = rect(foot, s.yaw(), x, z, 0, 0);
    v3(r.x, foot.y, r.z)
}

fn near_deg(yaw: i32, want_deg: i32) {
    let off = fx::wrap(yaw - deg(want_deg)).abs();
    assert!(
        off <= deg(1),
        "{} degrees, not {want_deg}",
        yaw as i64 * 360 / fx::TURN as i64
    );
}

#[test]
fn the_sides_are_the_ways_the_villagers_flee_and_none_faces_the_raid() {
    // by hand from data/story.json (headings turn left from north): the
    // villagers' last flee points (2, −230), (−215, −180), (200, −150),
    // (−240, 50) are north (−0.5°), north-west (50.1°), north-east (−53.1°)
    // and west (101.8°); the raid's entry points average (63, 170), heading
    // −159.6°, south by south-east
    let sp = spec();
    let mut villagers: Vec<usize> = sp.sides.iter().map(|s| s.villager).collect();
    villagers.sort();
    assert_eq!(villagers, vec![0, 1, 2, 3], "one side for each villager's way");
    let by = |name: &str| {
        sides(&Layout::shipped())
            .into_iter()
            .find(|s| s.name == name)
            .unwrap()
            .yaw
    };
    near_deg(by("north"), 0);
    near_deg(by("north_west"), 50);
    near_deg(by("north_east"), -53);
    near_deg(by("west"), 102);
    near_deg(raid_yaw(), -160);
    for s in sides(&Layout::shipped()) {
        assert!(
            fx::wrap(s.yaw - raid_yaw()).abs() >= deg(90),
            "{} faces the raid",
            s.name
        );
    }
}

/// The shipped places, without the roadside's own.
fn bare() -> Layout {
    Layout::parse(include_str!("../../../../data/places.json")).unwrap()
}

#[test]
fn a_stop_stands_beside_its_road_or_else_its_villager_s_flee_line() {
    // no road leaves to the north-east on main: the well stands 310 m out on
    // the flee line (0.8, −0.6), 10 m to its left: (2000 + 248 − 6,
    // 2800 − 186 − 8) = (2242, 2606)
    let mut l = bare();
    l.roads.retain(|r| !r.line.contains(&[2250, 2500]));
    let ne = sides(&l).into_iter().find(|s| s.name == "north_east").unwrap();
    assert!(!ne.on_road);
    let well = stops(&l).into_iter().find(|s| s.id == "north_east_well").unwrap();
    let (x, z) = (well.at.x / ONE, well.at.z / ONE);
    assert!(
        (x - 2242).abs() <= 2 && (z - 2606).abs() <= 2,
        "({x}, {z}), sampled every 2 m"
    );
    // a road out to the north-east (the land part's, R9.41): the well moves
    // beside it, 310 m out on (1996, 2700)–(2250, 2500) at t = 0.744, which
    // is (2185, 2551), and 10 m to its left: (2179, 2543)
    l.roads.push(RoadSpec {
        kind: "road".into(),
        half_cm: 225,
        line: vec![[1996, 2700], [2250, 2500], [2560, 2190], [2870, 1888]],
    });
    let ne = sides(&l).into_iter().find(|s| s.name == "north_east").unwrap();
    assert!(ne.on_road);
    let well = stops(&l).into_iter().find(|s| s.id == "north_east_well").unwrap();
    let (x, z) = (well.at.x / ONE, well.at.z / ONE);
    assert!((x - 2179).abs() <= 2 && (z - 2543).abs() <= 2, "({x}, {z})");
    // and it faces the road: its front points back across the 10 m
    let face = crate::geom::facing(well.yaw(), 0);
    let to_road = dir(well.at, well.on_path);
    assert!(face.dot(to_road) > ONE * 99 / 100, "{face:?} {to_road:?}");
    // the north lane runs from the crossroads into the village: it is found
    // the other way round, outward from the boundary
    let n = sides(&Layout::shipped())
        .into_iter()
        .find(|s| s.name == "north")
        .unwrap();
    assert!(n.on_road, "the lane");
    assert!(n.path[0].z > n.path[20].z, "outward is north");
}

/// Each side's stops, nearest the village first.
fn by_side(l: &Layout) -> Vec<(Side, Vec<Stop>)> {
    let c = centre(l);
    let all = stops(l);
    sides(l)
        .into_iter()
        .enumerate()
        .map(|(k, side)| {
            let mut mine: Vec<Stop> = all.iter().filter(|s| s.side == k).cloned().collect();
            mine.sort_by_key(|s| s.at.sub(c).len_xz());
            (side, mine)
        })
        .collect()
}

#[test]
fn every_side_has_a_stop_from_250_to_400_m_and_a_landmark_seen_from_the_boundary() {
    let w = world();
    let l = &w.land.layout;
    let c = centre(l);
    for (side, mine) in by_side(l) {
        let first = mine[0].at.sub(c).len_xz();
        assert!(
            first >= fx::int(250) && first <= fx::int(400),
            "{}: the first stop is {} m out",
            side.name,
            first / ONE
        );
        // from the boundary's line on the path, at eye height, a stop at
        // least 6 m tall stands a degree or more over the ground, and the
        // ground between hides none of it
        let mut eye = side.path[0];
        eye.y = w.ground(eye.x, eye.z) + EYE;
        let seen: Vec<&Stop> = mine
            .iter()
            .filter(|s| {
                let mut foot = s.at;
                foot.y = w.ground(foot.x, foot.z);
                top(s) >= LANDMARK && seen_from(&w, eye, foot, top(s))
            })
            .collect();
        assert!(
            !seen.is_empty(),
            "{}: nothing tall is seen from the boundary",
            side.name
        );
    }
}

#[test]
fn a_landmark_too_low_or_behind_a_rise_is_not_seen() {
    let w = world();
    // a 6 m thing 300 m off is 1/50 of the distance: seen; 400 m off, 1/67: not
    let eye = v3(
        fx::int(2000),
        w.ground(fx::int(2000), fx::int(2500)) + EYE,
        fx::int(2500),
    );
    let at = |d: i32| {
        let mut p = v3(fx::int(2000), 0, fx::int(2500 - d));
        p.y = w.ground(p.x, p.z);
        p
    };
    assert!(seen_from(&w, eye, at(300), LANDMARK));
    assert!(!seen_from(&w, eye, at(400), LANDMARK));
    // a thing 30 m below the ground's line is behind it
    let mut sunk = at(300);
    sunk.y -= fx::int(30);
    assert!(!seen_from(&w, eye, sunk, LANDMARK));
}

#[test]
fn no_stop_is_more_than_a_minute_s_run_from_the_last_along_its_side() {
    // a minute at T7's run of 7 m/s is 420 m; from the boundary's line on
    // the path to the first stop, then stop to stop
    let l = Layout::shipped();
    for (side, mine) in by_side(&l) {
        let mut last = side.path[0];
        for s in &mine {
            let gap = s.at.sub(last).len_xz();
            assert!(
                gap <= fx::int(420),
                "{}: {} is {} m from the last",
                side.name,
                s.id,
                gap / ONE
            );
            last = s.at;
        }
    }
}

#[test]
fn no_stop_stands_on_a_building_a_road_the_river_the_edge_or_another_stop() {
    let w = world();
    let l = &w.land.layout;
    let c = centre(l);
    let all = stops(l);
    let room_of = |s: &Stop| room(s, l);
    for s in &all {
        let r = room_of(s);
        let (x, z) = (s.at.x, s.at.z);
        // outside the village's boundary
        assert!(
            s.at.sub(c).len_xz() >= story::BOUNDARY + r,
            "{} is in the village",
            s.id
        );
        // clear of the square's edge by 50 m
        let edge = fx::int(50) + r;
        assert!(
            x > edge && z > edge && x < fx::int(4000) - edge && z < fx::int(4000) - edge,
            "{} is at the edge",
            s.id
        );
        // clear of every footprint of every other place (circles round each)
        for p in l.places.iter().filter(|p| !p.kind.starts_with(PLACE)) {
            for q in &p.props {
                let Some(f) = l.footprints_m.get(&q.prop) else {
                    continue;
                };
                let at = crate::places::metres([p.centre[0] + q.at[0], p.centre[1] + q.at[1]]);
                let other = fx::length(f[0] * ONE / 2, f[1] * ONE / 2);
                assert!(
                    at.sub(s.at).len_xz() > r + other + fx::int(2),
                    "{} stands on {}'s {}",
                    s.id,
                    p.kind,
                    q.prop
                );
            }
        }
        // off every road's surface, with a metre to spare
        for road in &l.roads {
            for p in dense(&road.line) {
                assert!(
                    p.sub(s.at).len_xz() > road.half_cm * ONE / 100 + r + ONE,
                    "{} stands on a {}",
                    s.id,
                    road.kind
                );
            }
        }
        // away from the river and its banks
        assert!(
            w.land.river.near(x, z, fx::int(100)).is_none(),
            "{} is by the river",
            s.id
        );
        assert!(!crate::terrain::is_water(&w, x, z), "{} is in water", s.id);
        // the room left for other parts' things
        for k in &spec().keep_clear {
            let at = crate::places::metres(k.at);
            assert!(
                at.sub(s.at).len_xz() > fx::int(k.m) + r,
                "{} is in the gap at {:?}",
                s.id,
                k.at
            );
        }
        // and clear of the other stops
        for o in all.iter().filter(|o| o.id != s.id) {
            assert!(
                o.at.sub(s.at).len_xz() > r + room_of(o) + ONE,
                "{} and {} overlap",
                s.id,
                o.id
            );
        }
    }
}

#[test]
fn every_stop_with_a_model_is_a_place_of_its_own_on_flattened_ground() {
    let w = world();
    for (s, foot) in &w.roadside.stops {
        let place = crate::places::at(&w, &s.place()).unwrap_or_else(|| panic!("{} is no place", s.id));
        assert_eq!((place.x, place.z), (foot.x, foot.z));
        if let Some(p) = s.kind.prop() {
            let spots = crate::places::prop_spots(&w, p);
            assert!(
                spots.iter().any(|q| q.at.x == foot.x && q.at.z == foot.z),
                "{}",
                s.id
            );
        }
    }
    // the map marks the stops with models
    let marks = crate::places::markers(&w);
    for key in ["wreck_burnt", "watchtower", "tor", "roadside_well", "lone_tree"] {
        assert!(marks.iter().any(|(k, _)| *k == key), "no mark {key}");
    }
}

#[test]
fn the_straggler_stands_only_once_the_opening_is_over() {
    let mut w = story::opening(3);
    let (_, home) = the(&w, "north_east_orc");
    let foes = w.foes.len();
    // the player put 60 m from its home in every phase of the opening
    let here = home.add(v3(fx::int(60), 0, 0));
    for phase in [Phase::Bed(0), Phase::Rising(3), Phase::Raid, Phase::Dream(10)] {
        w.story.phase = phase;
        w.player.pos = here;
        step(&mut w);
        assert_eq!(w.roadside.straggler, None, "{phase:?}");
        assert_eq!(w.foes.len(), foes);
    }
    // once free, it stands at its home, an orc with its blade away
    w.story.phase = Phase::Free;
    step(&mut w);
    let s = w.roadside.straggler.expect("it stands");
    assert_eq!(w.foes.len(), foes + 1);
    let f = &w.foes[s.foe];
    assert_eq!(f.brain.kind, Figure::Orc);
    assert_eq!(f.fighter.sheath, Sheath::Sheathed);
    assert_eq!((f.body.pos.x, f.body.pos.z), (home.x, home.z));
    // but not while the player is far off
    let mut far = story::opening(3);
    far.story.phase = Phase::Free;
    far.player.pos = home.add(v3(fx::int(200), 0, 0));
    step(&mut far);
    assert_eq!(far.roadside.straggler, None);
}

/// An opening world with the player put `gap` metres from the straggler's
/// home, toward the village, and play run until it stands.
fn by_the_straggler(seed: u64, gap: i32) -> World {
    let mut w = story::opening(seed);
    let (_, home) = the(&w, "north_east_orc");
    let c = story::centre(&w);
    let back = dir(home, c);
    let at = home.add(back.scale(fx::int(gap)));
    crate::places::act(&mut w, "go", &format!("{},{}", at.x / ONE, at.z / ONE)).unwrap();
    crate::places::arrive(&mut w);
    w.aim.yaw = yaw_of(home.sub(at));
    for _ in 0..3 {
        w.step(&Input::default());
    }
    w
}

#[test]
fn the_straggler_runs_at_a_player_it_sees_draws_and_fights() {
    let mut w = by_the_straggler(4, 18);
    assert_eq!(w.story.phase, Phase::Free);
    let s = w.roadside.straggler.expect("it stands");
    let start = w.foes[s.foe].body.pos;
    let mut drawn = false;
    for _ in 0..240 {
        w.step(&Input::default());
        drawn |= w.foes[s.foe].fighter.sheath == Sheath::Drawn;
    }
    let f = &w.foes[s.foe];
    let gap = f.body.pos.sub(w.player.pos).len_xz();
    assert!(gap < fx::int(4), "it came to {} m", gap / ONE);
    assert!(f.body.pos.sub(start).len_xz() > fx::int(12), "it ran");
    assert!(drawn, "it drew");
    // a player it cannot see is left alone: from 40 m it stays home
    let mut w = by_the_straggler(4, 40);
    let s = w.roadside.straggler.expect("it stands");
    let home = w.foes[s.foe].body.pos;
    for _ in 0..120 {
        w.step(&Input::default());
    }
    assert!(w.foes[s.foe].body.pos.sub(home).len_xz() < ONE, "it stayed");
}

#[test]
fn a_fallen_straggler_drops_its_coins() {
    let mut w = by_the_straggler(4, 40);
    let s = w.roadside.straggler.expect("it stands");
    let before = w.story.pickups.len();
    w.foes[s.foe].fighter.health.hp = 0;
    w.step(&Input::default());
    w.step(&Input::default());
    let new: Vec<_> = w.story.pickups[before..].to_vec();
    assert_eq!(new.len(), 1, "once");
    assert_eq!(new[0].gift, Gift::Coins(10));
    assert!(new[0].at.sub(w.foes[s.foe].body.pos).len_xz() < ONE * 2);
}

#[test]
fn e_searches_a_cart_once_and_takes_what_it_holds() {
    let mut w = world();
    let (s, foot) = the(&w, "north_cart");
    // in front of its open bed, toward the lane
    let at = local(&s, foot, 0, -1900);
    stand(&mut w, at, s.yaw());
    let strings = crate::page::parse_strings(include_str!("../../../../data/strings.json")).unwrap();
    assert_eq!(story::prompt(&w, &strings).unwrap(), "E · Search the cart");
    let coins = w.inventory.coins;
    press(&mut w, USE);
    // the file: 6 coins and a bandage
    assert_eq!(w.inventory.coins, coins + 6);
    assert_eq!(w.inventory.count(Item::Bandage), 1);
    assert_eq!(w.story.banner.as_ref().unwrap().key, "roadside_searched");
    press(&mut w, USE);
    assert_eq!(w.inventory.coins, coins + 6, "once");
    assert_eq!(w.inventory.count(Item::Bandage), 1);
    assert_ne!(
        story::prompt(&w, &strings).as_deref(),
        Some("E · Search the cart")
    );
}

#[test]
fn what_a_full_player_cannot_carry_is_left_beside_the_cart() {
    let mut w = world();
    let (s, foot) = the(&w, "north_west_cart");
    let room = crate::items::Item::Rock.def().stack;
    crate::inventory::give(&mut w, Item::Rock, room);
    stand(&mut w, local(&s, foot, 0, -1900), s.yaw());
    let before = w.story.pickups.len();
    press(&mut w, USE);
    // its 3 rocks lie on the ground; its 8 coins are taken
    let left = &w.story.pickups[before..];
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].gift, Gift::Item(Item::Rock, 3));
    assert!(left[0].at.sub(foot).len_xz() < fx::int(3));
    assert_eq!(w.story.banner.as_ref().unwrap().key, "story_full");
}

#[test]
fn the_well_makes_health_whole() {
    let mut w = world();
    let (s, foot) = the(&w, "north_east_well");
    stand(&mut w, local(&s, foot, 0, -1500), s.yaw());
    w.me.health.hp = 30;
    let strings = crate::page::parse_strings(include_str!("../../../../data/strings.json")).unwrap();
    assert_eq!(story::prompt(&w, &strings).unwrap(), "E · Drink from the well");
    press(&mut w, USE);
    assert_eq!(w.me.health.hp, crate::health::PLAYER);
}

#[test]
fn a_villager_by_the_way_says_where_to_go() {
    let mut w = world();
    let (s, foot) = the(&w, "north_villager");
    stand(&mut w, local(&s, foot, 0, -1500), s.yaw());
    press(&mut w, USE);
    let d = w.ui.dialogue.as_ref().expect("a dialogue");
    assert_eq!(d.who, "villager");
    assert_eq!(
        d.lines,
        // M7 rumours (T17): then the first rumour it knows
        vec![
            "roadside_north_1".to_string(),
            "roadside_north_2".to_string(),
            "rumour_hood".to_string()
        ]
    );
    // every villager's lines are in the strings
    let strings = crate::page::parse_strings(include_str!("../../../../data/strings.json")).unwrap();
    for side in &spec().sides {
        for stop in &side.stops {
            for l in &stop.says {
                assert!(strings.0.contains_key(l), "no string {l}");
            }
        }
    }
}

#[test]
fn a_body_climbs_the_watchtower_s_stair_and_cannot_walk_under_its_platform() {
    use crate::models::props_numbers as n;
    let mut w = world();
    let (s, foot) = the(&w, "north_west_tower");
    // a metre before the stair's foot, walking up it (local +z, away from
    // the way it faces)
    let up = fx::wrap(s.yaw() + fx::HALF_TURN);
    stand(&mut w, local(&s, foot, 0, n::WATCH_FOOT - 1000), up);
    for _ in 0..600 {
        w.step(&Input {
            keys: FORWARD,
            ..Input::default()
        });
    }
    let top = foot.y + mm(n::WATCH_TOP);
    assert_eq!(w.player.pos.y, top, "on the platform, 4.9 m up");
    // from the ground at the platform's side, walking in, a body is kept out
    let mut w = world();
    let side = local(&s, foot, 3500, n::WATCH_Z);
    stand(&mut w, side, fx::wrap(s.yaw() + fx::QUARTER_TURN));
    for _ in 0..300 {
        w.step(&Input {
            keys: FORWARD,
            ..Input::default()
        });
    }
    let r = rect(foot, s.yaw(), 0, n::WATCH_Z, n::WATCH_HALF, n::WATCH_HALF);
    assert!(!r.holds(w.player.pos.x, w.player.pos.z, 0), "under the platform");
    assert!(w.player.pos.y < foot.y + ONE, "on the ground");
}

#[test]
fn a_body_on_the_ground_cannot_walk_into_the_tor_s_side() {
    // from 5 m off its right side, between the boulders there, walking in
    // toward its middle: kept out at the top's edge (2.2 m out, and a
    // body's radius), on the ground; walked up its ramp, the same body
    // stands on its top
    let mut w = world();
    let (s, foot) = the(&w, "west_rocks");
    let left = fx::wrap(s.yaw() + fx::QUARTER_TURN);
    stand(&mut w, local(&s, foot, 5000, 3700), left);
    for _ in 0..300 {
        w.step(&Input {
            keys: FORWARD,
            ..Input::default()
        });
    }
    let r = rect(foot, s.yaw(), 0, 2900, 2200, 900);
    assert!(!r.holds(w.player.pos.x, w.player.pos.z, 0), "inside the rock");
    assert!(w.player.pos.y < foot.y + ONE, "on the ground");
    let mut w = world();
    let up = fx::wrap(s.yaw() + fx::HALF_TURN);
    stand(
        &mut w,
        local(&s, foot, 0, crate::models::props_numbers::TOR_FOOT - 1000),
        up,
    );
    for _ in 0..300 {
        w.step(&Input {
            keys: FORWARD,
            ..Input::default()
        });
    }
    assert_eq!(
        w.player.pos.y,
        foot.y + mm(crate::models::props_numbers::TOR_TOP),
        "on its top"
    );
}

#[test]
fn the_tor_s_ramp_rises_evenly_to_its_top() {
    use crate::models::props_numbers as n;
    let w = world();
    let (s, foot) = the(&w, "west_rocks");
    let at = |z: i32| {
        let p = local(&s, foot, 0, z);
        solid::surface(&w, p.x, p.z) - foot.y
    };
    // the ramp runs from 5.6 m before the middle (the ground) to 0.75 m
    // past it (3.25 m up): half way, at −2.425 m, it is 1.625 m up
    assert!((at(-2425) - ONE * 1625 / 1000).abs() < ONE / 50, "{}", at(-2425));
    assert_eq!(at(2500), mm(n::TOR_TOP));
    assert!(at(n::TOR_FOOT + 100).abs() < ONE / 10);
}

#[test]
fn a_climbed_surface_more_than_a_step_up_is_a_wall() {
    let c = Climb {
        rect: Wall {
            x: 0,
            z: 0,
            hx: ONE,
            hz: fx::int(4),
            yaw: 0,
        },
        low: 0,
        high: fx::int(4),
    };
    // at its low end, from the ground: walked onto
    let (mut p, mut v) = (v3(0, 0, -fx::int(4)), V3::ZERO);
    assert!(!guard(&[c], 0, &mut p, &mut v));
    // at its middle (2 m up), from the ground: pushed out of its side
    let (mut p, mut v) = (v3(ONE * 9 / 10, 0, 0), v3(-ONE / 10, 0, 0));
    assert!(guard(&[c], 0, &mut p, &mut v));
    assert!(p.x >= ONE + solid::RADIUS - 2, "{}", p.x);
    assert_eq!(v.x, 0);
    // from 1.8 m up (on it), the same place is walked on
    let (mut p, mut v) = (v3(ONE * 9 / 10, 0, 0), V3::ZERO);
    assert!(!guard(&[c], ONE * 18 / 10, &mut p, &mut v));
}

#[test]
fn smoke_rises_from_the_burning_cart_in_a_fixed_order() {
    let foot = v3(0, 0, 0);
    // a puff starts a metre over the cart, 1.5 m across; half its life on
    // (360 ticks) it is 22 × ½ × 2.5 / 2 = 13.75 m higher and
    // 1.5 + 4.5 / 2 = 3.75 m across
    let (a, s) = puff(foot, 0, 0, 0);
    assert_eq!((a.y, s), (ONE, ONE * 3 / 2));
    let (b, s) = puff(foot, 0, 360, 0);
    assert_eq!(b.y, ONE + ONE * 1375 / 100);
    assert_eq!(s, ONE * 375 / 100);
    // the shapes drawn: as many every frame, in the same order
    let mut w = story::opening(2);
    let l = crate::look::tests::look();
    let count = |w: &World| {
        let mut out = Shapes::default();
        draw(w, ONE / 3, &l, &mut out);
        out.list.len()
    };
    let n = count(&w);
    let shapes = |w: &World| {
        let mut out = Shapes::default();
        draw(w, ONE / 3, &l, &mut out);
        out.list
    };
    let before = shapes(&w);
    for _ in 0..PUFF_TICKS as usize / 3 {
        w.step(&Input::default());
        assert_eq!(count(&w), n);
    }
    assert_ne!(shapes(&w), before, "the smoke moves");
    // held still for a picture (`story.still`), it holds still
    w.story.still = true;
    let held = shapes(&w);
    for _ in 0..10 {
        w.step(&Input::default());
    }
    assert_eq!(shapes(&w), held);
}

#[test]
fn the_same_seed_gives_the_same_roadside_tick_for_tick() {
    let run = || {
        let mut w = by_the_straggler(9, 20);
        for k in 0..400 {
            w.step(&Input {
                keys: if k % 120 < 60 { FORWARD } else { 0 },
                dx: if k % 40 < 4 { 300 } else { -20 },
                ..Input::default()
            });
        }
        w
    };
    let (a, b) = (run(), run());
    assert!(a.roadside.straggler.is_some());
    assert_eq!(a.hash(), b.hash());
    assert_eq!(a.roadside, b.roadside);
}

#[test]
fn the_roadside_file_is_refused_when_it_names_what_is_not_there() {
    let good: serde_json::Value =
        serde_json::from_str(include_str!("../../../../data/roadside.json")).unwrap();
    let mut bad = good.clone();
    bad["sides"][0]["stops"][0]["kind"] = serde_json::json!("castle");
    assert!(Spec::parse(&bad.to_string()).unwrap_err().contains("\"castle\""));
    let mut bad = good.clone();
    bad["sides"][0]["villager"] = serde_json::json!(9);
    assert!(Spec::parse(&bad.to_string()).unwrap_err().contains("villager"));
    let mut bad = good.clone();
    bad["sides"][0]["stops"][0]["gives"] = serde_json::json!([{ "item": "crown" }]);
    assert!(Spec::parse(&bad.to_string()).unwrap_err().contains("neither"));
}

/// The static scene round each stop with a model, as the page receives it
/// (the static array, and the terrain and scatter the detail levels draw
/// round an eye there): no two surfaces in one plane (CW-18).
#[test]
fn the_roadside_s_static_scene_draws_no_two_surfaces_in_one_plane() {
    use crate::flicker::{fights, instances, Inst, Seen};
    let mut g = crate::game::tests::game();
    let w = g.world.clone();
    let stops: Vec<(Stop, V3)> = feet(&w)
        .into_iter()
        .filter(|(s, _)| s.kind.prop().is_some())
        .collect();
    assert!(stops.len() >= 8);
    let base = instances(&g.scene);
    for (s, foot) in &stops {
        let mut e = local(s, *foot, 0, -14_000);
        e.y = w.ground(e.x, e.z) + fx::int(2);
        let mut insts: Vec<Inst> = base.clone();
        let mut shapes = Shapes::default();
        let look = g.look.clone();
        g.detail.frame(&w, &look, e, &mut shapes);
        for sh in shapes.list {
            insts.push(Inst {
                mesh: sh.mesh,
                at: sh.at,
                rgb: sh.rgb,
                value: sh.value,
                pass: sh.pass,
            });
        }
        // only what stands round this stop
        insts.retain(|i| i.at.t.sub(*foot).len_xz() < fx::int(40));
        let table = g.detail.scan_table(&g.meshes);
        let seen = Seen {
            bottom: foot.y - fx::int(5),
            eyes: vec![
                e,
                local(s, *foot, 9_000, 0),
                v3(foot.x, foot.y + fx::int(12), foot.z),
            ],
            far: crate::render::FAR,
        };
        let complementary = |a: &Inst, b: &Inst| {
            a.pass == crate::render::Pass::Dither
                && b.pass == crate::render::Pass::Dither
                && (a.value - b.value).abs() == ONE
        };
        let f: Vec<_> = fights(&insts, &table, &seen)
            .into_iter()
            .filter(|x| !complementary(&insts[x.a], &insts[x.b]))
            .collect();
        let report: Vec<String> = f
            .iter()
            .take(6)
            .map(|x| crate::flicker::describe(&insts, x))
            .collect();
        assert!(
            f.is_empty(),
            "{}: {} pairs fight:\n{}",
            s.id,
            f.len(),
            report.join("\n")
        );
    }
}

#[test]
fn the_views_stand_the_player_where_they_say() {
    use crate::models::props_numbers as n;
    let mut w = world();
    // the boundary's line on the north lane: 250 m from the centre
    act(&mut w, "line", "north").unwrap();
    let d = w.player.pos.sub(centre(&w.land.layout)).len_xz();
    assert!((d - story::BOUNDARY).abs() <= fx::int(2), "{} m", d / ONE);
    // on the watchtower's platform, and still there a tick on
    act(&mut w, "top", "north_west_tower").unwrap();
    crate::places::arrive(&mut w);
    w.step(&Input::default());
    let (_, foot) = the(&w, "north_west_tower");
    assert_eq!(w.player.pos.y, foot.y + mm(n::WATCH_TOP));
    // before the well, looking at it
    act(&mut w, "look", "north_east_well 8").unwrap();
    let (_, well) = the(&w, "north_east_well");
    let gap = w.player.pos.sub(well).len_xz();
    assert!((gap - fx::int(8)).abs() <= ONE, "{} m", gap / ONE);
    assert!(act(&mut w, "look", "castle").unwrap_err().contains("\"castle\""));
    assert!(act(&mut w, "line", "south").unwrap_err().contains("\"south\""));
}
