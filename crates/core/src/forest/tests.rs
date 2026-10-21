//! The forest's tests (P13, T19): the flag over the camp and its smoke,
//! the clearings, the trees and the floor.

use super::*;
use crate::fx::int;
use crate::input::{Input, FORWARD};
use crate::scatter::{self, Thing};
use crate::terrain::Land;

fn world() -> World {
    let mut w = crate::story::opening(7);
    crate::story::step_aside(&mut w);
    w
}

fn land() -> std::rc::Rc<Land> {
    crate::terrain::tests::land()
}

/// Where the trail to the camp leaves the road: the trail's end, an eye
/// 1.7 m over the ground.
fn trail_end_eye(w: &World) -> V3 {
    let p = crate::places::at(w, "trail_end").expect("trail_end");
    v3(p.x, w.ground(p.x, p.z) + ONE * 17 / 10, p.z)
}

/// The scatter's things within `r` of `c`.
fn things_round(l: &Land, c: V3, r: Fx) -> Vec<scatter::Item> {
    let side = l.spec.scatter.chunk_m * ONE;
    let mut out = Vec::new();
    for j in (c.z - r).div_euclid(side)..=(c.z + r).div_euclid(side) {
        for i in (c.x - r).div_euclid(side)..=(c.x + r).div_euclid(side) {
            out.extend(
                scatter::items(l, i, j)
                    .into_iter()
                    .filter(|t| t.at.sub(c).len_xz() <= r),
            );
        }
    }
    out
}

/// A tree's top as drawn whole: the highest of its parts.
fn top_of(l: &Land, it: &scatter::Item) -> Fx {
    scatter::tree_parts(&l.spec.scatter, it)
        .iter()
        .map(|(_, a, _)| a.t.y + a.c1.len() / 2)
        .max()
        .unwrap_or(it.at.y)
}

// ---- the flag and the smoke -------------------------------------------------------

/// The flag's top stands 10 m and more over every tree within 80 m of it:
/// the tallest tree there (a birch of 175%) is about 13 m.
#[test]
fn the_flag_s_top_stands_over_the_canopy_round_it() {
    let w = world();
    let top = flag_top(&w).expect("the camp has a flagpole");
    let round = things_round(&w.land, top, int(80));
    let trees: Vec<&scatter::Item> = round.iter().filter(|t| t.thing.tree()).collect();
    assert!(trees.len() > 20, "{} trees round the camp", trees.len());
    let tallest = trees.iter().map(|t| top_of(&w.land, t)).max().unwrap();
    assert!(
        tallest < int(20) + w.ground(top.x, top.z),
        "a tree {} m",
        tallest / ONE
    );
    assert!(
        top.y - tallest >= int(10),
        "the flag {} m over the trees",
        (top.y - tallest) / ONE
    );
}

/// From the trail's end, 960 m off, nothing stands between the eye and the
/// flag's top, nor its cloth's middle: no tree as drawn at its distance
/// (whole, a cone, the far crowns) and no rise of the ground. The check
/// can fail: the same line to a point 2 m over the camp's ground meets
/// the forest.
#[test]
fn the_flag_is_seen_from_the_trail_s_end() {
    let w = world();
    let top = flag_top(&w).unwrap();
    let eye = trail_end_eye(&w);
    assert!(
        top.sub(eye).len_xz() > int(900),
        "{} m",
        top.sub(eye).len_xz() / ONE
    );
    assert_eq!(sight(&w.land, eye, top), None);
    let cloth = top.sub(v3(0, int(2), 0));
    assert_eq!(sight(&w.land, eye, cloth), None);
    let low = v3(top.x, w.ground(top.x, top.z) + int(2), top.z);
    assert!(
        sight(&w.land, eye, low).is_some(),
        "the camp itself is hidden by the forest"
    );
}

/// The cloth hangs from the pole's top in panels, each shorter than the
/// one before; it turns with the wind and ripples with the ticks; and held
/// still for a view (`story.still`) it draws the same frame after frame.
#[test]
fn the_flag_waves_with_the_wind_and_holds_still_when_the_story_does() {
    let calm = cloth(0, 0);
    let gust = cloth(ONE, 0);
    let n = spec().flag.panels as usize;
    assert_eq!(calm.parts.len(), n);
    assert_ne!(calm, gust, "the wind turns it");
    assert_ne!(cloth(0, fx::deg(90)), calm, "the ripple runs along it");
    for k in 1..n {
        assert!(
            calm.parts[k].at.c1.len() < calm.parts[k - 1].at.c1.len(),
            "panel {k}"
        );
    }
    // the first panel's top edge is the pole's top, less the knob
    let p0 = calm.parts[0].at;
    assert!((p0.t.y + p0.c1.len() / 2 - (models::FLAGPOLE_TOP - ONE / 4)).abs() < 64);
    let mut w = world();
    let draw_now = |w: &World| {
        let mut out = Shapes::default();
        draw(w, ONE / 2, &crate::look::tests::look(), &mut out);
        out.list
    };
    let a = draw_now(&w);
    for _ in 0..20 {
        w.step(&Input::default());
    }
    let b = draw_now(&w);
    assert_eq!(a.len(), b.len(), "as many shapes every frame");
    assert_ne!(a, b, "the flag and the smoke move");
    w.story.still = true;
    let held = draw_now(&w);
    for _ in 0..20 {
        w.step(&Input::default());
    }
    assert_eq!(draw_now(&w), held);
}

/// The smoke: a puff is born unseen at the fire (it fades in), is at its
/// strongest low in the column, thins with height and is gone at the top,
/// and widens as it rises; the shapes are glows, which write no depth.
#[test]
fn the_camp_s_smoke_fades_in_thins_with_height_and_widens() {
    let sp = &spec().smoke;
    let foot = V3::ZERO;
    let life = sp.life_ticks as u64;
    let at = |t: u64| crate::smoke::puff(sp, foot, 0, t, 0);
    let (born, w0, s0) = at(0);
    assert_eq!(s0, 0, "born unseen");
    let (low, w1, s1) = at(life * 25 / 100);
    let (high, w2, s2) = at(life * 80 / 100);
    let (_, _, s3) = at(life - 1);
    assert!(s1 > s2 && s2 > s3, "{s1} {s2} {s3}");
    assert!(s3 < ONE / 100);
    assert!(born.y < low.y && low.y < high.y);
    assert!(w0 < w1 && w1 < w2);
    // 34% at its strongest, a quarter of the way up: 34% × (1 − 1/8)
    assert_eq!(s1, fx::mul(ONE * 34 / 100, ONE - ONE / 8));
    let w = world();
    let mut out = Shapes::default();
    crate::smoke::column(sp, &w, foot, 0, &mut out);
    assert_eq!(out.list.len(), sp.puffs as usize);
    assert!(out.list.iter().all(|s| s.pass == crate::render::Pass::Glow));
}

// ---- the clearings ----------------------------------------------------------------

/// The clearings come from the land's seed: the same land built again has
/// the same ones, another seed others; four to six of them, every second
/// one holding foes; each with a stump and a log, and no tree in it.
#[test]
fn the_clearings_are_placed_from_the_seed_and_the_same_every_time() {
    let l = land();
    let again = Land::shipped();
    assert_eq!(l.glades, again.glades);
    let n = l.glades.len();
    assert!((4..=6).contains(&n), "{n} clearings");
    assert_eq!(l.glades.iter().filter(|g| g.foes).count(), n.div_ceil(2));
    let mut spec2 = l.spec.clone();
    spec2.seed ^= 0x5eed;
    let other = Land::new(spec2, l.layout.clone(), l.waters.clone());
    assert_ne!(other.glades, l.glades, "another seed, other clearings");
    for g in &l.glades {
        let inside = things_round(&l, g.centre, g.r);
        assert!(
            inside.iter().all(|t| !t.thing.tree()),
            "a tree in the clearing at {:?}",
            g.centre
        );
        let kinds: Vec<Thing> = inside.iter().map(|t| t.thing).collect();
        assert!(
            kinds.contains(&Thing::Stump) && kinds.contains(&Thing::Log),
            "{kinds:?}"
        );
        assert!(l.cleared(g.centre.x, g.centre.z, &l.scope_all()));
    }
}

/// Every clearing keeps its distances (`data/places.json`): 220 m from the
/// camp, 70 m past a trail's edge and its own radius, 140 m from every
/// place (the waystones, the hollow tree), 240 m from each other. And the
/// rule refuses a spot on the trail, at the camp, at its waystone and
/// beside a clearing kept, but takes one deep in the forest.
#[test]
fn the_clearings_keep_clear_of_the_camp_the_trails_and_the_waystones() {
    let l = land();
    let g = l.layout.forest.glades.clone().unwrap();
    let all = l.scope_all();
    let camp = crate::places::metres([3213, 987]);
    for (k, c) in l.glades.iter().enumerate() {
        let p = c.centre;
        assert!(camp.sub(p).len_xz() >= int(220));
        let (rd, rh) = l.road_distance(p.x, p.z, &all);
        assert!(rd - rh >= int(70) + c.r, "{} m from a trail", (rd - rh) / ONE);
        for place in &l.layout.places {
            let at = crate::places::metres(place.centre);
            assert!(at.sub(p).len_xz() >= int(140), "{} too near", place.kind);
        }
        for o in &l.glades[k + 1..] {
            assert!(o.centre.sub(p).len_xz() >= int(240));
        }
    }
    let r = int(15);
    // deep in the forest, west of the camp, away from the trail
    let good = v3(int(2700), 0, int(900));
    assert!(l.glade_ok(&g, good, r, &[]), "a deep spot");
    // each refused by one rule alone: on the trail; 150 m west of the
    // camp's centre; 130 m north of the hollow tree
    let on_trail = v3(int(3080), 0, int(1300));
    let by_camp = camp.sub(v3(int(150), 0, 0));
    let by_place = v3(int(3600), 0, int(1270));
    for (bad, what) in [
        (on_trail, "the trail"),
        (by_camp, "the camp"),
        (by_place, "the hollow tree"),
    ] {
        assert!(!l.glade_ok(&g, bad, r, &[]), "{what}");
    }
    let kept = [crate::terrain::Glade {
        centre: good.add(v3(int(100), 0, 0)),
        r,
        foes: false,
        pick: 0,
        props: Vec::new(),
    }];
    assert!(!l.glade_ok(&g, good, r, &kept), "beside a clearing kept");
}

// ---- the trees and the floor ------------------------------------------------------

/// The forest holds every kind of tree and, on its floor, logs, stumps,
/// bushes, ferns and mushrooms, all the same when built again; a land of
/// another seed has its floor elsewhere.
#[test]
fn the_forest_holds_every_kind_of_tree_and_its_floor_is_the_seed_s() {
    let l = land();
    let mut kinds = std::collections::BTreeSet::new();
    let mut count = 0;
    for (i, j) in [(22, 6), (23, 7), (24, 8), (25, 7), (26, 9), (22, 9), (27, 6)] {
        let a = scatter::items(&l, i, j);
        assert_eq!(a, scatter::items(&l, i, j));
        for t in &a {
            kinds.insert(format!("{:?}", t.thing));
        }
        count += a.len();
    }
    for k in ["Pine", "Broadleaf", "Birch", "Snag", "Log", "Stump"] {
        assert!(kinds.contains(k), "no {k} in {count} things: {kinds:?}");
    }
    let bushes = scatter::floor(&l, scatter::FLOOR, 24, 8);
    assert!(bushes.len() > 40, "{} bushes", bushes.len());
    let at = |v: &[(V3, crate::rng::Rng)]| v.iter().map(|(p, _)| *p).collect::<Vec<_>>();
    assert_eq!(at(&bushes), at(&scatter::floor(&l, scatter::FLOOR, 24, 8)));
    let ferns = (0..6)
        .map(|k| scatter::floor(&l, scatter::FERN, 170 + k, 50).len())
        .sum::<usize>();
    assert!(ferns > 20, "{ferns} ferns");
    let sp = &l.spec.scatter;
    assert!(scatter::build(&l, scatter::FERN, 170, 50, &[], [0; 3]).is_some());
    assert!(scatter::build(&l, scatter::FLOOR, 24, 8, &[], [0; 3]).is_some());
    let mut spec2 = l.spec.clone();
    spec2.seed ^= 0x5eed;
    let other = Land::new(spec2, l.layout.clone(), l.waters.clone());
    assert_ne!(at(&scatter::floor(&other, scatter::FLOOR, 24, 8)), at(&bushes));
    // none on the plains
    assert!(scatter::floor(&l, scatter::FLOOR, 12, 20).is_empty());
    let _ = sp;
}

/// The floor fades as the trees and the grass do, each triangle by its own
/// distance (no pop-in, R9.44): the bushes with the whole trees, the ferns
/// with the grass; and no part's band is another's in the shader's slot.
#[test]
fn the_forest_floor_fades_by_each_triangle_s_distance() {
    let sp = &land().spec.scatter;
    assert_eq!(
        scatter::band(sp, scatter::FLOOR),
        scatter::band(sp, scatter::WHOLE)
    );
    assert_eq!(
        scatter::band(sp, scatter::FERN),
        scatter::band(sp, scatter::GRASS)
    );
    let u = scatter::uniform_values(sp);
    for part in scatter::PARTS {
        let k = scatter::band_slot(part);
        let b = scatter::band(sp, part).map(fx::int);
        assert_eq!(&u[k * 4..k * 4 + 4], &b, "part {part}'s band in slot {k}");
    }
}

/// Every new kind of tree, the stumps and the logs stand in a body's way
/// (a player walking into one stops at its bark and slides round), stop
/// a thing in flight, and the trees stand in the camera's boom.
#[test]
fn the_new_trees_stand_in_the_way_of_bodies_and_things_in_flight() {
    let l = land();
    let mut found: Vec<scatter::Item> = Vec::new();
    for (i, j) in [(22, 6), (23, 7), (24, 8), (25, 7), (26, 9), (22, 9), (27, 6)] {
        for t in scatter::items(&l, i, j) {
            if t.thing != Thing::Pine && t.thing != Thing::Rock && !found.iter().any(|f| f.thing == t.thing) {
                // one standing alone, so nothing else is in the walk
                let near = things_round(&l, t.at, int(5)).len();
                if near == 1 {
                    found.push(t);
                }
            }
        }
    }
    let names: Vec<Thing> = found.iter().map(|t| t.thing).collect();
    for k in [Thing::Broadleaf, Thing::Birch, Thing::Snag, Thing::Stump] {
        assert!(names.contains(&k), "no lone {k:?}: {names:?}");
    }
    let mut w = world();
    w.story.phase = crate::story::Phase::Free;
    for t in &found {
        if t.thing == Thing::Log {
            continue;
        }
        let r = crate::obstacles::trunk_circles(t)[0].1;
        // a body walking east into it from 3 m west
        let start = t.at.sub(v3(int(3), 0, 0));
        let start = v3(start.x, w.ground(start.x, start.z), start.z);
        w.player = crate::player::Body::new(start, fx::deg(-90));
        w.me = crate::player::Fighter::new(
            crate::pose::Kind::Player,
            crate::pose::Held::ShortSword,
            0,
            start,
            fx::deg(-90),
            false,
        );
        w.aim.yaw = fx::deg(-90);
        let mut closest = int(10);
        for _ in 0..150 {
            w.step(&Input {
                keys: FORWARD,
                ..Input::default()
            });
            closest = closest.min(w.player.pos.sub(t.at).len_xz());
        }
        assert!(
            closest >= r + crate::solid::RADIUS - ONE / 50,
            "{:?}: {} cm from its middle",
            t.thing,
            closest * 100 / ONE
        );
        // a rock thrown through it a metre up (a stump's height, 35 cm)
        let up = if t.thing == Thing::Stump {
            ONE * 35 / 100
        } else {
            ONE
        };
        let a = t.at.add(v3(-int(3), up, 0));
        let b = t.at.add(v3(int(3), up, 0));
        assert!(
            w.obstacles.first(&w, a, b, ONE / 20).is_some(),
            "{:?} lets a rock through",
            t.thing
        );
        if t.thing.tree() {
            assert!(
                !crate::boom::tree(t).is_empty(),
                "{:?} is no solid to the camera",
                t.thing
            );
        }
    }
}

/// CW-18: no two parts of a broadleaf, a birch, a snag, a log or a stump
/// lie in one plane, seen from near and from round it out to where the
/// whole trees are drawn (`full_m`), at the sizes the forest draws them.
#[test]
fn no_two_parts_of_the_forest_s_new_things_share_a_plane() {
    use crate::flicker::{self, Inst, Seen};
    let sp = &land().spec.scatter;
    let mut bad = Vec::new();
    for thing in [
        Thing::Broadleaf,
        Thing::Birch,
        Thing::Snag,
        Thing::Log,
        Thing::Stump,
    ] {
        for size in [85, 130, 175] {
            let it = scatter::Item {
                thing,
                at: v3(int(10), 0, int(20)),
                size,
                yaw: fx::deg(30),
                tone: 1,
            };
            let insts: Vec<Inst> = scatter::tree_parts(sp, &it)
                .into_iter()
                .map(|(m, at, rgb)| Inst {
                    mesh: m.into(),
                    at,
                    rgb,
                    value: 0,
                    pass: crate::render::Pass::Solid,
                })
                .collect();
            assert!(insts.len() >= 2, "{thing:?}");
            let far = int(sp.full_m[1]);
            let eyes = (0..8)
                .flat_map(|k| {
                    let a = fx::TURN * k / 8;
                    [ONE * 2, far / 3].map(|up| {
                        it.at.add(v3(
                            fx::mul(fx::cos(a), far - int(8)),
                            up,
                            fx::mul(fx::sin(a), far - int(8)),
                        ))
                    })
                })
                .collect();
            let table = crate::prim::MeshTable::default();
            for seen in [
                Seen {
                    bottom: 0,
                    ..Seen::near()
                },
                Seen { bottom: 0, eyes, far },
            ] {
                for f in flicker::fights(&insts, &table, &seen) {
                    bad.push(format!("{thing:?} {size}: {}", flicker::describe(&insts, &f)));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} fights:\n{}", bad.len(), bad.join("\n"));
}

/// T18 with T19: a body sprinting at ten times the run (76 450 units a
/// tick, 1.17 m) straight at a pine, a broadleaf, a birch, a snag or a
/// stump, from eight starts 15 cm apart, never sinks into the bark: every
/// tick its middle stays `RADIUS` off the trunk, and the line it moved
/// along never crosses the trunk itself (the swept steps of `solid::step`
/// meet the trunk too).
#[test]
fn a_body_sprinting_at_ten_times_the_run_does_not_pass_through_a_tree() {
    let l = land();
    let mut found: Vec<scatter::Item> = Vec::new();
    for (i, j) in [(22, 6), (23, 7), (24, 8), (25, 7), (26, 9), (22, 9), (27, 6)] {
        for t in scatter::items(&l, i, j) {
            if t.thing != Thing::Rock
                && t.thing != Thing::Log
                && !found.iter().any(|f| f.thing == t.thing)
                && things_round(&l, t.at, int(9)).len() == 1
            {
                found.push(t);
            }
        }
    }
    assert!(
        found.len() >= 4,
        "{:?}",
        found.iter().map(|t| t.thing).collect::<Vec<_>>()
    );
    let base = {
        let mut w = world();
        w.story.phase = crate::story::Phase::Free;
        w
    };
    let seg = |a: V3, b: V3, c: V3| {
        let d = b.sub(a);
        let dd = (d.x as i64 * d.x as i64 + d.z as i64 * d.z as i64).max(1);
        let q = c.sub(a);
        let t = ((q.x as i64 * d.x as i64 + q.z as i64 * d.z as i64) * 1024 / dd).clamp(0, 1024);
        let p = v3(
            a.x + (d.x as i64 * t / 1024) as Fx,
            0,
            a.z + (d.z as i64 * t / 1024) as Fx,
        );
        p.sub(v3(c.x, 0, c.z)).len_xz()
    };
    for t in &found {
        let r = crate::obstacles::trunk_circles(t)[0].1;
        for phase in 0..8 {
            let mut w = base.clone();
            w.motion.forced_pieces = Some(5);
            let back = r + crate::solid::RADIUS + int(6) + phase * ONE * 15 / 100;
            let at = t.at.sub(v3(back, 0, 0));
            let yaw = fx::deg(-90);
            w.aim.yaw = yaw;
            w.player = crate::player::Body::new(v3(at.x, w.ground(at.x, at.z), at.z), yaw);
            w.player.vel = crate::geom::facing(yaw, 0).scale(76_450);
            w.player.prev_vel = w.player.vel;
            crate::sprint::act(&mut w, "set", "480").unwrap();
            for tick in 0..12 {
                let from = w.player.pos;
                w.step(&Input {
                    keys: FORWARD,
                    ..Input::default()
                });
                let to = w.player.pos;
                let off = to.sub(t.at).len_xz();
                assert!(
                    off >= r + crate::solid::RADIUS - ONE / 50,
                    "{:?}, start {phase}, tick {tick}: {} cm from its middle",
                    t.thing,
                    off * 100 / ONE
                );
                assert!(
                    seg(from, to, t.at) >= r,
                    "{:?}, start {phase}, tick {tick}: through the trunk",
                    t.thing
                );
            }
        }
    }
}
