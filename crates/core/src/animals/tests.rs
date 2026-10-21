//! The animals' tests (P13, T19): placed from the seed, capped near the
//! player and parked far off, running from the player and settling again,
//! and the same run tick for tick.

use super::*;
use crate::input::{Input, FORWARD};
use crate::player::{Body, Fighter};
use crate::pose::{Held, Kind};
use crate::story::Phase;

fn free_world(seed: u64) -> World {
    let mut w = World::new(seed);
    w.story.seed = seed;
    w.story.phase = Phase::Free;
    w.story.crossed = Some(crate::world::START);
    ensure(&mut w);
    w
}

fn put(w: &mut World, p: V3) {
    let p = v3(p.x, w.ground(p.x, p.z), p.z);
    w.player = Body::new(p, 0);
    w.me = Fighter::new(Kind::Player, Held::ShortSword, 0, p, 0, false);
}

fn idle(w: &mut World, n: u32) {
    for _ in 0..n {
        w.step(&Input::default());
    }
}

/// The herds come from the land's seed: placed again, the same; every kind
/// is met; one rests in every clearing without foes; none is in water, on
/// a road or within 140 m of the camp.
#[test]
fn the_herds_are_placed_from_the_land_s_seed_the_same_every_time() {
    let w = free_world(1);
    let a = place(&w.land);
    assert_eq!(a, w.animals.herds);
    assert_eq!(a, place(&crate::terrain::Land::shipped()));
    let sp = spec();
    for k in 0..sp.kinds.len() {
        assert!(a.iter().any(|h| h.kind == k), "no {}", sp.kinds[k].name);
    }
    let quiet: Vec<_> = w.land.glades.iter().filter(|g| !g.foes).collect();
    assert!(!quiet.is_empty());
    for g in quiet {
        assert!(
            a.iter().any(|h| h.glade && h.home.sub(g.centre).len_xz() < ONE),
            "a clearing with no herd"
        );
    }
    let camp = crate::camp::centre(&w);
    let all = w.land.scope_all();
    for h in &a {
        assert!(!w.land.is_water(h.home.x, h.home.z));
        let (rd, rh) = w.land.road_distance(h.home.x, h.home.z, &all);
        assert!(rd > rh, "a herd on a road");
        assert!(h.home.sub(camp).len_xz() >= fx::int(140));
    }
    // and nothing stands in a bare world
    let mut bare = World::new(1);
    put(&mut bare, a[0].home);
    idle(&mut bare, 3);
    assert_eq!(standing(&bare), 0);
}

/// Walking through the forest past every herd, no more than `most_near`
/// animals ever stand, and far off, every herd is parked.
#[test]
fn only_so_many_animals_stand_near_the_player_and_far_ones_are_parked() {
    let mut w = free_world(2);
    let homes: Vec<V3> = w.animals.herds.iter().map(|h| h.home).collect();
    let mut most = 0;
    let mut stood = 0;
    for h in &homes {
        put(&mut w, *h);
        idle(&mut w, 2);
        most = most.max(standing(&w));
        stood += usize::from(standing(&w) > 0);
    }
    assert!(most <= spec().most_near, "{most} standing");
    assert!(
        stood * 2 >= homes.len(),
        "herds stood at {stood} of {}",
        homes.len()
    );
    // the cap lowered to one herd's worth: at a crowded spot, no more stand
    put(&mut w, crate::world::START);
    idle(&mut w, 2);
    w.animals.most = 5;
    let mut capped = 0;
    for h in &homes {
        put(&mut w, *h);
        idle(&mut w, 2);
        assert!(standing(&w) <= 5, "{} over the cap", standing(&w));
        capped = capped.max(standing(&w));
    }
    assert!(capped > 0);
    // with the cap as shipped, some spot had more than five near
    assert!(
        most > 5,
        "the walk met {most} at most, so the lowered cap is tested"
    );
    put(&mut w, crate::world::START);
    idle(&mut w, 2);
    assert_eq!(standing(&w), 0, "parked");
}

/// An animal the player comes near runs straight away, faster than it
/// walks, until it is `calm_m` off; then it walks again.
#[test]
fn an_animal_runs_from_the_player_and_settles_again() {
    let mut w = free_world(3);
    let hi = w
        .animals
        .herds
        .iter()
        .position(|h| !h.glade && spec().kinds[h.kind].name == "deer")
        .unwrap();
    let home = w.animals.herds[hi].home;
    put(&mut w, home.add(v3(fx::int(150), 0, 0)));
    idle(&mut w, 2);
    assert!(!w.animals.herds[hi].animals.is_empty(), "the herd stands");
    let a = w.animals.herds[hi].animals[0];
    assert_ne!(a.mode, Mode::Flee);
    let k = &spec().kinds[w.animals.herds[hi].kind];
    // the player steps up 6 m off it
    let me = a.pos.add(v3(fx::int(6), 0, 0));
    put(&mut w, me);
    let start = w.animals.herds[hi].animals[0].pos.sub(me).len_xz();
    idle(&mut w, 1);
    assert_eq!(w.animals.herds[hi].animals[0].mode, Mode::Flee);
    idle(&mut w, 120);
    let b = w.animals.herds[hi].animals[0];
    let gone = b.pos.sub(me).len_xz();
    // two seconds at 9.5 m/s, less its turn and what stood in the way
    assert!(
        gone > start + fx::int(8),
        "{} m from {} m",
        gone / ONE,
        start / ONE
    );
    for _ in 0..1200 {
        idle(&mut w, 1);
        if w.animals.herds[hi].animals[0].mode != Mode::Flee {
            break;
        }
    }
    let c = w.animals.herds[hi].animals[0];
    assert_ne!(c.mode, Mode::Flee, "it settles");
    assert!(c.pos.sub(me).len_xz() > fx::int(k.calm_m));
}

/// CW-4: a recorded walk among the animals hashes the same every run, and
/// the animals are in the hash (a herd stood and moved).
#[test]
fn a_recorded_walk_among_the_animals_hashes_the_same_every_run() {
    let run = || {
        let mut w = free_world(5);
        let h = w.animals.herds.iter().find(|h| !h.glade).unwrap().home;
        put(&mut w, h.add(v3(fx::int(30), 0, 0)));
        w.aim.yaw = fx::deg(90);
        let mut marks = Vec::new();
        for k in 0..600 {
            w.step(&Input {
                keys: if k % 200 < 120 { FORWARD } else { 0 },
                dx: if k % 90 < 45 { 120 } else { -150 },
                ..Input::default()
            });
            if k % 60 == 59 {
                marks.push(w.hash());
            }
        }
        (marks, w)
    };
    let (a, wa) = run();
    let (b, _) = run();
    assert_eq!(a, b);
    assert!(standing(&wa) > 0);
    let moved = wa
        .animals
        .herds
        .iter()
        .flat_map(|h| &h.animals)
        .any(|x| x.pos != x.prev || x.mode == Mode::Flee);
    assert!(moved, "the animals moved in the walk");
}
