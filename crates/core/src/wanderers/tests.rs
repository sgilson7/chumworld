use super::*;
use crate::fx::int;
use crate::input::{DRAW, FORWARD};
use crate::player::{Body, Fighter};
use crate::pose::Held;

/// A world past the opening, its encounters placed, the player at `p`.
fn free_world(seed: u64) -> World {
    let mut w = World::new(seed);
    // as the opening sets it (`story::begin`): the save names this seed
    w.story.seed = seed;
    w.story.phase = Phase::Free;
    w.story.crossed = Some(crate::world::START);
    ensure(&mut w);
    w
}

fn put(w: &mut World, p: V3) {
    let mut p = p;
    p.y = w.ground(p.x, p.z);
    w.player = Body::new(p, 0);
    w.me = Fighter::new(Kind::Player, Held::ShortSword, 0, p, 0, false);
}

fn idle(w: &mut World, n: u32) {
    for _ in 0..n {
        w.step(&Input::default());
    }
}

fn first(w: &World, name: &str) -> Encounter {
    let k = spec().kinds.iter().position(|k| k.name == name).unwrap();
    w.wanderers.list.iter().find(|e| e.kind == k).cloned().unwrap()
}

/// A point `d` metres from `e`'s anchor, on the side away from the others.
fn off(w: &World, e: &Encounter, d: i32) -> V3 {
    let others: Vec<V3> = w
        .wanderers
        .list
        .iter()
        .filter(|o| o.id != e.id)
        .map(|o| o.anchor)
        .collect();
    (0..16)
        .map(|k| e.anchor.add(facing(fx::deg(k * 360 / 16), 0).scale(int(d))))
        .max_by_key(|p| others.iter().map(|o| o.sub(*p).len_xz()).min().unwrap_or(0))
        .unwrap()
}

fn count(w: &World, name: &str) -> usize {
    let k = spec().kinds.iter().position(|k| k.name == name).unwrap();
    w.wanderers.list.iter().filter(|e| e.kind == k).count()
}

#[test]
fn the_wanderers_file_reads_and_refuses_a_weapon_or_armour_as_what_is_left() {
    let s = spec();
    // (M7 forest: the kinds met in the clearings have no count either; they
    // are one a clearing, not drawn for the ways' slots)
    assert_eq!(
        s.kinds
            .iter()
            .filter(|k| k.count.is_none() && !in_glade(k))
            .count(),
        1
    );
    let mut v: serde_json::Value =
        serde_json::from_str(include_str!("../../../../data/wanderers.json")).unwrap();
    let fire = s.kinds.iter().position(|k| k.name == "orc_fire").unwrap();
    for bad in ["sword", "gloves", "chest_key"] {
        v["kinds"][fire]["cleared"][1]["item"] = bad.into();
        let e = Spec::parse(&v.to_string()).unwrap_err();
        assert!(e.contains("not a consumable"), "{bad}: {e}");
    }
    v["kinds"][fire]["cleared"][1]["item"] = "bandage".into();
    v["kinds"][fire]["foes"][0] = "wolf".into();
    assert!(Spec::parse(&v.to_string()).unwrap_err().contains("\"wolf\""));
}

/// CW-4: the same seed places the same encounters; another seed others.
#[test]
fn placement_is_the_same_for_a_seed_and_differs_between_seeds() {
    let a = free_world(7);
    let b = free_world(7);
    assert_eq!(a.wanderers.list, b.wanderers.list);
    assert!(!a.wanderers.list.is_empty());
    let c = free_world(8);
    assert_ne!(a.wanderers.list, c.wanderers.list);
    // what scene draws is what the world holds
    assert_eq!(place(&a, 7), a.wanderers.list);
    // the mix the file asks for, whatever the seed
    for seed in 1..=6 {
        let w = free_world(seed);
        let held = count(&w, "orc_fire") + count(&w, "lean_to");
        assert!((2..=4).contains(&held), "seed {seed}: {held} held sites");
        assert!((1..=2).contains(&count(&w, "ambush")), "seed {seed}: ambushes");
        assert!((1..=2).contains(&count(&w, "patrol")), "seed {seed}: patrols");
        assert!(count(&w, "orc_pair") >= 4, "seed {seed}: orc pairs");
    }
}

/// Every point an encounter has (its anchor, its round, the lines between,
/// its homes and props): by hand from `data/places.json`, 700 m and more
/// from the village centre (2000, 2800), so outside its 250 m boundary and
/// the roadside band; 40 m and more from each waystone, the trainer by the
/// hut (330 + 5, 1750 + 7), the traveller at the crossroads (2000, 2250)
/// and the merchant's beat (the road between x 470 and 2040); 80 m and more
/// from every roadside stop.
#[test]
fn nothing_is_placed_in_the_village_on_a_waystone_or_near_the_trainer_the_merchant_or_the_traveller() {
    let pt = |x: i32, z: i32| v3(int(x), 0, int(z));
    let waystones = [
        pt(2000, 2540),
        pt(652, 2236),
        pt(324, 1764),
        pt(735, 997),
        pt(3157, 1140),
    ];
    let people = [pt(335, 1757), pt(2000, 2250)];
    for seed in 1..=6 {
        let w = free_world(seed);
        let beat: Vec<V3> = crate::places::road(&w)
            .points
            .into_iter()
            .filter(|p| p.x >= int(470) && p.x <= int(2040))
            .collect();
        // the roadside part's stops (T12), the straggler's among them, as the core lays them
        let stops: Vec<V3> = crate::roadside::stops(&w.land.layout)
            .into_iter()
            .map(|s| s.at)
            .collect();
        assert!(!stops.is_empty());
        for e in &w.wanderers.list {
            let mut pts = vec![e.anchor];
            for i in 0..e.route.len() {
                let (a, b) = (e.route[i], e.route[(i + 1) % e.route.len()]);
                pts.extend((0..=10).map(|k| a.lerp(b, k * ONE / 10)));
            }
            pts.extend(e.homes.iter().map(|h| h.0));
            pts.extend(e.props.iter().map(|p| p.1));
            for p in pts {
                let d = |q: V3| v3(p.x - q.x, 0, p.z - q.z).len_xz();
                assert!(
                    d(pt(2000, 2800)) >= int(700),
                    "seed {seed}, {e:?}: in the village's band"
                );
                for q in waystones.iter().chain(&people) {
                    assert!(d(*q) >= int(40), "seed {seed}, {} by {q:?}", e.id);
                }
                for q in &stops {
                    assert!(
                        d(*q) >= int(80),
                        "seed {seed}, {} by the roadside stop at {q:?}",
                        e.id
                    );
                }
                assert!(
                    beat.iter().all(|q| d(*q) >= int(40)),
                    "seed {seed}, {} on the merchant's beat",
                    e.id
                );
                assert!(!w.land.is_water(p.x, p.z), "seed {seed}, {} in the water", e.id);
            }
        }
    }
}

/// The spacing, measured along each way's clear run (`runs`, its points 4
/// m apart): an encounter is met at its slot, if the nearest of its points
/// (anchor, round, homes) lies within 120 m of the way; the run's two ends count too
/// (a major place's edge, or the village's band, where the roadside part's
/// stops are). Between one thing met and the next is 500 m or less.
#[test]
fn along_each_way_something_is_met_every_five_hundred_metres_or_less() {
    let mut longest = 0;
    let mut gaps = 0;
    for seed in 1..=6 {
        let w = free_world(seed);
        for run in runs(&w).iter().filter(|r| !r.is_empty()) {
            let mut at = vec![0, run.len()];
            for e in w.wanderers.list.iter().filter(|e| e.way == run.way) {
                let pts = std::iter::once(e.anchor)
                    .chain(e.route.iter().copied())
                    .chain(e.homes.iter().map(|h| h.0));
                let d = pts.map(|p| run.nearest(p).1).min().unwrap();
                if d <= int(120) {
                    at.push(e.along);
                }
            }
            at.sort();
            for g in at.windows(2) {
                longest = longest.max(g[1] - g[0]);
                gaps += 1;
                assert!(
                    g[1] - g[0] <= int(500),
                    "seed {seed}, way {}: {} m with nothing",
                    run.way,
                    (g[1] - g[0]) / ONE
                );
            }
        }
    }
    assert!(gaps > 40, "{gaps} gaps measured");
    assert!(longest > int(250), "the longest gap {} m", longest / ONE);
}

/// The eight ways measured by hand from `data/places.json` (straight
/// lengths between the places' centres; a road a little longer): the open
/// ways from the hut to the shrine 854 m, the trail's end to the hollow
/// tree 878 m, the scarecrow to the bridge 1 305 m, the shrine to the camp
/// 2 475 m; each run is that less its two keep distances (80 m at a place,
/// 400 at the camp, 100 at a waystone), give or take the slant.
#[test]
fn the_open_ways_are_as_long_as_the_map_says_less_their_ends() {
    let w = World::new(1);
    let r = runs(&w);
    let len = |k: usize| r[k].len() / ONE;
    // hut to shrine: 854 less about 100 (the hill's waystone) and 100 (the
    // shrine's waystone)
    assert!((600..=700).contains(&len(4)), "{}", len(4));
    // trail's end to the hollow tree: 878 less 80 and 80
    assert!((690..=730).contains(&len(5)), "{}", len(5));
    // scarecrow to bridge: 1 305 less 80 and about 100 (the bridge's waystone)
    assert!((1100..=1150).contains(&len(6)), "{}", len(6));
    // shrine to camp: 2 475 less about 100 and 400
    assert!((1950..=2000).contains(&len(7)), "{}", len(7));
}

#[test]
fn nothing_stands_before_the_opening_is_over() {
    let mut w = World::new(3);
    ensure(&mut w);
    let e = first(&w, "orc_pair");
    let p = off(&w, &e, 20);
    put(&mut w, p);
    idle(&mut w, 5);
    assert!(
        w.wanderers.standing.is_empty() && w.foes.is_empty(),
        "no opening yet"
    );
    w.story.phase = Phase::Free;
    w.story.crossed = Some(crate::world::START);
    idle(&mut w, 1);
    assert_eq!(w.wanderers.standing.len(), 1);
    assert_eq!(w.foes.len(), 2, "an orc pair");
}

#[test]
fn only_so_many_foes_stand_near_the_player() {
    let mut w = free_world(3);
    let e = first(&w, "orc_pair");
    let p = off(&w, &e, 30);
    put(&mut w, p);
    // five hostile foes already stand round the player: two more would be seven
    for k in 0..5 {
        let at = w.player.pos.add(v3(int(8 + 2 * k), 0, int(8)));
        story::sheathed_foe(&mut w, Kind::Bandit, Kind::Bandit, at, 0);
    }
    w.story.still = true;
    idle(&mut w, 3);
    assert!(w.wanderers.standing.is_empty(), "7 is over the cap of 6");
    assert_eq!(w.foes.len(), 5);
    // with one of them down, the pair makes six
    w.foes[0].fighter.health.hp = 0;
    idle(&mut w, 1);
    assert_eq!(w.wanderers.standing.len(), 1);
    assert_eq!(near_count(&w), 6);
}

/// CW-11: a parked encounter is out of `World::foes`; one with a later foe
/// after it is held still, and its places are used again when it stands.
#[test]
fn a_parked_encounter_leaves_the_world_and_stands_again_when_the_player_comes_back() {
    let mut w = free_world(5);
    let e = first(&w, "orc_pair");
    let near = off(&w, &e, 40);
    put(&mut w, near);
    idle(&mut w, 2);
    assert_eq!(w.foes.len(), 2);
    // the village centre: every encounter is 700 m and more from it
    let away = crate::world::START;
    put(&mut w, away);
    idle(&mut w, 2);
    assert!(w.wanderers.standing.is_empty());
    assert_eq!(w.foes.len(), 0, "parked foes cost nothing");
    assert!(!w.wanderers.records[e.id as usize].beaten);
    put(&mut w, near);
    idle(&mut w, 2);
    assert_eq!(w.foes.len(), 2, "it stands again");
    // a foe made after them (as the camp's are) keeps their places held
    story::sheathed_foe(&mut w, Kind::Bandit, Kind::Bandit, away, 0);
    put(&mut w, away);
    idle(&mut w, 2);
    assert_eq!(w.foes.len(), 3);
    assert_eq!(w.wanderers.free.len(), 2);
    assert!(w.foes[..2].iter().all(|f| f.brain.control == Control::Still));
    put(&mut w, near);
    idle(&mut w, 2);
    assert_eq!(w.foes.len(), 3, "the held places are used again");
    assert!(w.wanderers.free.is_empty());
}

#[test]
fn a_beaten_encounter_stays_beaten_and_a_roaming_one_returns_after_a_long_while() {
    let mut w = free_world(2);
    let site = w
        .wanderers
        .list
        .iter()
        .find(|e| e.moves == Moves::Hold)
        .cloned()
        .unwrap();
    let near = off(&w, &site, 25);
    put(&mut w, near);
    idle(&mut w, 2);
    assert_eq!(w.foes.len(), 2);
    for f in &mut w.foes {
        f.fighter.health.hp = 0;
    }
    idle(&mut w, 2);
    let r = w.wanderers.records[site.id as usize];
    assert!(r.beaten && r.rewarded && r.fallen == 0b11);
    let k = &spec().kinds[site.kind];
    let gifts: Vec<Gift> = w.story.pickups.iter().map(|p| p.gift).collect();
    for c in &k.cleared {
        assert!(gifts.contains(&c.gift().unwrap()), "{c:?} lies by the fire");
    }
    assert_eq!(w.story.banner.as_ref().unwrap().key, "wanderer_cleared");
    let away = crate::world::START;
    put(&mut w, away);
    idle(&mut w, 2);
    w.tick += 3_600_000;
    idle(&mut w, 2);
    put(&mut w, near);
    idle(&mut w, 2);
    assert!(w.wanderers.standing.is_empty(), "a held site stays beaten");
    assert!(w.foes.iter().all(|f| f.fighter.health.down()) || w.foes.is_empty());
    // a roaming pair beaten comes back after 20 minutes, while the player is away
    let pair = first(&w, "orc_pair");
    let near = off(&w, &pair, 30);
    put(&mut w, near);
    idle(&mut w, 2);
    let st = w
        .wanderers
        .standing
        .iter()
        .find(|s| s.enc == pair.id as usize)
        .unwrap()
        .clone();
    for mb in &st.members {
        w.foes[mb.foe].fighter.health.hp = 0;
    }
    idle(&mut w, 2);
    assert!(w.wanderers.records[pair.id as usize].beaten);
    let away = crate::world::START;
    put(&mut w, away);
    w.tick += 1200 * 60 - 10;
    idle(&mut w, 2);
    assert!(w.wanderers.records[pair.id as usize].beaten, "not yet");
    idle(&mut w, 10);
    assert!(
        !w.wanderers.records[pair.id as usize].beaten,
        "back after 20 minutes"
    );
}

#[test]
fn an_ambush_stands_hidden_until_the_player_passes_then_springs() {
    let mut w = free_world(4);
    let e = first(&w, "ambush");
    let p = off(&w, &e, 40);
    put(&mut w, p);
    idle(&mut w, 2);
    let st = w
        .wanderers
        .standing
        .iter()
        .find(|s| s.enc == e.id as usize)
        .unwrap()
        .clone();
    let homes: Vec<V3> = st.members.iter().map(|m| w.foes[m.foe].body.pos).collect();
    idle(&mut w, 120);
    for (m, h) in st.members.iter().zip(&homes) {
        let f = &w.foes[m.foe];
        assert!(f.body.pos.sub(*h).len_xz() < ONE / 4, "hidden, still");
        assert!(!f.fighter.drawn());
    }
    assert!(!w.wanderers.standing[0].alarm);
    // passing within 14 m: they spring, with a shout
    put(&mut w, e.homes[0].0.add(v3(int(10), 0, 0)));
    idle(&mut w, 1);
    assert!(w.wanderers.standing[0].alarm);
    assert_eq!(w.story.banner.as_ref().unwrap().key, "wanderer_ambush");
    idle(&mut w, 180);
    assert!(
        st.members.iter().any(|m| w.foes[m.foe].fighter.drawn()),
        "blades out"
    );
}

/// The travel song is refused while a wanderer's blade is out near
/// (`travel::refusal`, a foe drawn within 32 m), as it is for any foe.
#[test]
fn the_flute_is_refused_while_a_wanderer_s_blade_is_out_near() {
    let mut w = free_world(6);
    let e = first(&w, "orc_pair");
    let p = off(&w, &e, 40);
    put(&mut w, p);
    idle(&mut w, 2);
    assert_eq!(crate::travel::refusal(&w), None);
    let orc = w.foes[0].body.pos;
    w.me.health = crate::health::Health::new(100_000);
    put(&mut w, orc.add(v3(int(9), 0, 0)));
    w.me.health = crate::health::Health::new(100_000);
    let mut refused = false;
    for _ in 0..200 {
        w.step(&Input::default());
        refused |= crate::travel::refusal(&w) == Some("travel_refuse_duel");
        if w.me.health.down() {
            w.me.health = crate::health::Health::new(100_000);
        }
    }
    assert!(refused, "the orcs drew within 32 m");
}

#[test]
fn a_foe_does_not_fight_by_the_merchant_s_cart_the_trainer_or_the_village() {
    let mut w = free_world(1);
    let p = v3(int(1000), 0, int(1500));
    assert!(fights_here(&w, p));
    w.merchant.path = vec![p, p.add(v3(int(10), 0, 0))];
    w.merchant.at = p.add(v3(int(30), 0, 0));
    assert!(!fights_here(&w, p), "the cart 30 m off");
    w.merchant.at = p.add(v3(int(60), 0, 0));
    assert!(fights_here(&w, p));
    assert!(
        !fights_here(&w, v3(int(2000), 0, int(2560))),
        "250 m from the village centre"
    );
}

/// The records a recorded walk past an orc pair gives its orcs (CW-3):
/// played back with the trees and the steering taken out, the same bodies.
#[test]
fn a_wanderer_moves_only_through_its_input_record() {
    let mut a = free_world(3);
    let e = first(&a, "orc_pair");
    let p = off(&a, &e, 60);
    put(&mut a, p);
    idle(&mut a, 2);
    assert_eq!(a.foes.len(), 2);
    // 40 m from the first orc, facing it
    let orc = a.foes[0].body.pos;
    let start = orc.add(e.anchor.sub(orc).norm().scale(int(40)));
    put(&mut a, start);
    a.aim.yaw = yaw_of(orc.sub(start));
    a.me.health = crate::health::Health::new(100_000);
    let mut b = a.clone();
    let walk: Vec<Input> = (0..500)
        .map(|k| Input {
            keys: if k == 3 {
                DRAW
            } else if k < 300 {
                FORWARD
            } else {
                0
            },
            dx: if k % 40 < 20 { 300 } else { -300 },
            buttons: if k % 50 > 40 { input::SWING } else { 0 },
            ..Input::default()
        })
        .collect();
    let mut kept = vec![Vec::new(), Vec::new()];
    let before: Vec<u32> = a.foes.iter().map(|f| f.brain.ticks).collect();
    let mut saw = false;
    for i in &walk {
        let held = a.feel.stop > 0;
        a.step(i);
        saw |= a.wanderers.standing.first().is_some_and(|s| s.alarm);
        if !held {
            for (k, list) in kept.iter_mut().enumerate() {
                list.push(a.foes[k].fighter.last);
            }
        }
    }
    assert!(saw, "they saw the player");
    assert!(kept[0].iter().any(|i| i.keys != 0), "an orc pressed keys");
    for (k, list) in kept.into_iter().enumerate() {
        let mut l = vec![Input::default(); before[k] as usize];
        l.extend(list);
        b.foes[k].brain.control = Control::Script(l);
    }
    for i in &walk {
        b.step(i);
    }
    assert_eq!(a.player, b.player);
    for k in 0..2 {
        assert_eq!(a.foes[k].body, b.foes[k].body, "orc {k}");
        assert_eq!(a.foes[k].fighter, b.foes[k].fighter, "orc {k}");
    }
}

/// CW-4: a recorded walk out past the wanderers hashes the same every run,
/// checked every second; one record changed by a hundredth of a pixel shows.
#[test]
fn a_recorded_walk_past_the_wanderers_hashes_the_same_every_run() {
    let record: Vec<Input> = (0..900)
        .map(|k| Input {
            keys: match k {
                3 => DRAW,
                _ if k % 200 < 150 => FORWARD | input::RUN,
                _ => crate::input::LEFT,
            },
            dx: if k % 90 < 45 { 200 } else { -260 },
            buttons: if k % 70 > 60 { input::SWING } else { 0 },
            ..Input::default()
        })
        .collect();
    let run = |rec: &[Input]| {
        let mut w = free_world(9);
        let e = first(&w, "patrol");
        let p = off(&w, &e, 120);
        put(&mut w, p);
        w.aim.yaw = yaw_of(e.anchor.sub(p));
        w.me.health = crate::health::Health::new(100_000);
        let mut marks = Vec::new();
        for (k, i) in rec.iter().enumerate() {
            w.step(i);
            if k % 60 == 59 {
                marks.push(w.hash());
            }
        }
        (marks, w)
    };
    let (a, wa) = run(&record);
    let (b, _) = run(&record);
    assert_eq!(a, b);
    assert!(!wa.wanderers.standing.is_empty(), "the walk met them");
    let mut other = record.clone();
    other[450].dx += 1;
    let (c, _) = run(&other);
    assert_eq!(c[..7], a[..7]);
    assert_ne!(c[8], a[8]);
}

#[test]
fn a_held_site_is_marked_on_the_map_once_seen() {
    let mut w = free_world(2);
    assert!(marks(&w).is_empty());
    let site = w
        .wanderers
        .list
        .iter()
        .find(|e| e.moves == Moves::Hold)
        .cloned()
        .unwrap();
    let p = off(&w, &site, 80);
    put(&mut w, p);
    w.story.still = true;
    idle(&mut w, 1);
    assert!(marks(&w).is_empty(), "80 m off: not yet");
    let p = off(&w, &site, 50);
    put(&mut w, p);
    idle(&mut w, 1);
    let m = marks(&w);
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].1, site.anchor);
    let strings = crate::page::parse_strings(include_str!("../../../../data/strings.json")).unwrap();
    for k in spec().kinds.iter().filter_map(|k| k.mark.as_deref()) {
        assert!(strings.0.contains_key(&format!("place_{k}")), "place_{k}");
    }
    for k in ["wanderer_ambush", "wanderer_lookout", "wanderer_cleared"] {
        assert!(strings.0.contains_key(k), "{k}");
    }
}

/// CW-18: the held sites' props, with the terrain and scatter the detail
/// levels draw round each, seen from beside each site: no two surfaces in
/// one plane (the scan of `flicker.rs`).
#[test]
fn the_held_sites_props_draw_no_two_surfaces_in_one_plane() {
    use crate::flicker::{fights, instances, Inst, Seen};
    let mut g = crate::game::tests::game();
    let sites: Vec<Encounter> = place(&g.world, g.world.wanderers.seed)
        .into_iter()
        .filter(|e| !e.props.is_empty())
        .collect();
    assert!(sites.len() >= 2);
    let mut groups = Vec::new();
    scene(&g.world, &g.look, &mut g.meshes.clone(), &mut groups);
    assert_eq!(groups.len(), sites.len());
    assert!(
        groups.iter().all(|x| x.name.starts_with(crate::places::FADED)),
        "each site is drawn by distance, faded"
    );
    for e in &sites {
        let mut eye = e.anchor.add(v3(int(9), 0, int(9)));
        eye.y = g.world.ground(eye.x, eye.z) + int(2);
        let mut insts = instances(&g.scene);
        let mut shapes = crate::render::Shapes::default();
        let (world, look) = (g.world.clone(), g.look.clone());
        g.detail.frame(&world, &look, eye, &mut shapes);
        let mut props = 0;
        for s in shapes.list {
            if s.at.t.sub(e.anchor).len_xz() < int(12) {
                props += 1;
            }
            insts.push(Inst {
                mesh: s.mesh,
                at: s.at,
                rgb: s.rgb,
                value: s.value,
                pass: s.pass,
            });
        }
        assert!(props > 0, "the site's props are drawn");
        let table = g.detail.scan_table(&g.meshes);
        let seen = Seen {
            bottom: -int(100),
            eyes: vec![eye, e.anchor.add(v3(-int(6), int(2), int(5)))],
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
            "site {}: {} pairs fight:\n{}",
            e.id,
            f.len(),
            report.join("\n")
        );
    }
}

/// A dream makes the world again from the same seed: the placing is kept,
/// not worked out a second time, and every record is new.
#[test]
fn a_dream_keeps_the_placing_and_starts_the_records_again() {
    let mut w = story::opening(7);
    ensure(&mut w);
    let list = w.wanderers.list.clone();
    w.wanderers.records[0].beaten = true;
    story::wake(&mut w);
    assert!(w.wanderers.placed, "placed already");
    assert_eq!(w.wanderers.list, list);
    assert!(!w.wanderers.records[0].beaten);
    assert_eq!(w.wanderers.records.len(), list.len());
}

/// T16: a beaten encounter stays beaten across a save and a load (the
/// save's text, read back), and a held site seen keeps its mark; what it
/// left is not laid again.
#[test]
fn a_beaten_encounter_stays_beaten_across_a_save_and_a_load() {
    let mut w = free_world(2);
    let site = w
        .wanderers
        .list
        .iter()
        .find(|e| e.moves == Moves::Hold)
        .cloned()
        .unwrap();
    let p = off(&w, &site, 25);
    put(&mut w, p);
    idle(&mut w, 2);
    for f in &mut w.foes {
        f.fighter.health.hp = 0;
    }
    idle(&mut w, 2);
    assert!(w.wanderers.records[site.id as usize].beaten);
    let laid = w.story.pickups.len();
    let text = crate::save::text(&crate::save::progress(&w));
    assert!(text.contains(&foe_key(site.id, 0)), "the save names it");
    let back = crate::save::read(&text).expect("the save reads back");
    let mut l = crate::save::world(&back);
    assert_eq!(l.wanderers.seed, 2);
    put(&mut l, p);
    idle(&mut l, 3);
    let r = l.wanderers.records[site.id as usize];
    assert!(r.beaten && r.rewarded, "still beaten: {r:?}");
    assert!(r.seen, "its mark kept");
    assert!(
        !l.wanderers.standing.iter().any(|s| s.enc == site.id as usize),
        "it does not stand"
    );
    let again = l
        .story
        .pickups
        .iter()
        .filter(|p| !p.taken && p.at.sub(site.anchor).len_xz() < int(3))
        .count();
    assert_eq!(again, 0, "its reward is not laid again ({laid} before)");
    // and the keys are the save's to know
    assert!(crate::save::known_foe(&foe_key(site.id, 1)));
    assert!(crate::save::known_taken(&crate::save::drop_key(
        &site_key(site.id),
        0
    )));
    assert!(!crate::save::known_foe("wanderer.x.1") && !crate::save::known_foe("wanderer.3.9"));
}

// ---- M7 forest (P13, T19): the clearings' foes ----------------------------------

fn glade_sites(w: &World) -> Vec<Encounter> {
    w.wanderers
        .list
        .iter()
        .filter(|e| e.way == GLADE_WAY)
        .cloned()
        .collect()
}

/// Each clearing that holds foes has one encounter of a glade kind, one
/// or two foes standing round its centre, after the ways' (their ids
/// follow); for every seed no other encounter comes within `glade_m` of a
/// clearing's edge, and the keep distances refuse a clearing's centre.
#[test]
fn each_clearing_with_foes_holds_one_or_two_and_no_other_wanderer_comes_near() {
    for seed in [1, 2, 3, 4] {
        let w = free_world(seed);
        let sites = glade_sites(&w);
        let with_foes: Vec<_> = w.land.glades.iter().filter(|g| g.foes).collect();
        assert_eq!(sites.len(), with_foes.len());
        assert!(sites.len() >= 2);
        let first_glade = w.wanderers.list.iter().position(|e| e.way == GLADE_WAY).unwrap();
        assert!(w.wanderers.list[first_glade..].iter().all(|e| e.way == GLADE_WAY));
        for (e, g) in sites.iter().zip(&with_foes) {
            let k = &spec().kinds[e.kind];
            assert!(in_glade(k) && (1..=2).contains(&k.foes.len()), "{}", k.name);
            assert!(e.anchor.sub(g.centre).len_xz() < ONE);
            assert!(e.homes.iter().all(|(p, _)| p.sub(g.centre).len_xz() < g.r));
        }
        let z = zones(&w);
        let keep = m(spec().keep.glade_m);
        for g in &w.land.glades {
            assert!(
                !zone_ok(&z, g.centre),
                "a clearing's centre is no place for a wanderer"
            );
            for e in w.wanderers.list.iter().filter(|e| e.way != GLADE_WAY) {
                let points = e
                    .route
                    .iter()
                    .chain(e.homes.iter().map(|(p, _)| p))
                    .chain([&e.anchor]);
                for p in points {
                    assert!(
                        p.sub(g.centre).len_xz() >= g.r + keep,
                        "seed {seed}: a {} by a clearing",
                        spec().kinds[e.kind].name
                    );
                }
            }
        }
    }
}

/// CW-3: a clearing's foes move only through their input records: the
/// same records played back as scripts move them the same.
#[test]
fn a_clearing_s_foes_move_only_through_their_input_records() {
    let mut a = free_world(3);
    let e = glade_sites(&a)[0].clone();
    let p = e.anchor.add(v3(0, 0, int(17)));
    put(&mut a, p);
    idle(&mut a, 2);
    let st = a
        .wanderers
        .standing
        .iter()
        .find(|s| s.enc == e.id as usize)
        .cloned()
        .expect("it stands");
    let ids: Vec<usize> = st.members.iter().map(|m| m.foe).collect();
    a.aim.yaw = yaw_of(e.anchor.sub(p));
    a.me.health = crate::health::Health::new(100_000);
    let mut b = a.clone();
    let walk: Vec<Input> = (0..400)
        .map(|k| Input {
            keys: if k == 3 {
                DRAW
            } else if k < 250 {
                FORWARD
            } else {
                0
            },
            dx: if k % 40 < 20 { 250 } else { -250 },
            ..Input::default()
        })
        .collect();
    let before: Vec<u32> = ids.iter().map(|&f| a.foes[f].brain.ticks).collect();
    let mut kept = vec![Vec::new(); ids.len()];
    for i in &walk {
        let held = a.feel.stop > 0;
        a.step(i);
        if !held {
            for (k, list) in kept.iter_mut().enumerate() {
                list.push(a.foes[ids[k]].fighter.last);
            }
        }
    }
    let alarm = a
        .wanderers
        .standing
        .iter()
        .find(|s| s.enc == e.id as usize)
        .map(|s| s.alarm);
    assert!(
        kept[0].iter().any(|i| i.keys != 0),
        "a foe pressed keys: {} m off, alarm {alarm:?}, {} m walked",
        a.player.pos.sub(e.anchor).len_xz() / ONE,
        a.player.pos.sub(p).len_xz() / ONE
    );
    for (k, list) in kept.into_iter().enumerate() {
        let mut l = vec![Input::default(); before[k] as usize];
        l.extend(list);
        b.foes[ids[k]].brain.control = Control::Script(l);
    }
    for i in &walk {
        b.step(i);
    }
    for &f in &ids {
        assert_eq!(a.foes[f].body, b.foes[f].body, "foe {f}");
    }
}

/// A clearing whose foes are beaten stays beaten across a save and a load:
/// its foes do not stand again, what they left is not laid again, and the
/// line said is the clearing's.
#[test]
fn a_beaten_clearing_stays_beaten_across_a_save_and_a_load() {
    let mut w = free_world(2);
    let site = glade_sites(&w)[0].clone();
    let p = site.anchor.add(v3(0, 0, int(25)));
    put(&mut w, p);
    idle(&mut w, 2);
    assert!(w.wanderers.standing.iter().any(|s| s.enc == site.id as usize));
    for f in &mut w.foes {
        f.fighter.health.hp = 0;
    }
    idle(&mut w, 2);
    assert!(w.wanderers.records[site.id as usize].beaten);
    let text = crate::save::text(&crate::save::progress(&w));
    assert!(text.contains(&foe_key(site.id, 0)), "the save names it");
    let back = crate::save::read(&text).expect("the save reads back");
    let mut l = crate::save::world(&back);
    put(&mut l, p);
    idle(&mut l, 3);
    let r = l.wanderers.records[site.id as usize];
    assert!(r.beaten && r.rewarded, "still beaten: {r:?}");
    assert!(
        !l.wanderers.standing.iter().any(|s| s.enc == site.id as usize),
        "it does not stand"
    );
    let again = l
        .story
        .pickups
        .iter()
        .filter(|q| !q.taken && q.at.sub(site.anchor).len_xz() < int(3))
        .count();
    assert_eq!(again, 0);
    assert_eq!(spec().kinds[site.kind].banner.as_deref(), Some("glade_cleared"));
}
