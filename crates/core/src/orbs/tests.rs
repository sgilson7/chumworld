use super::*;
use crate::fx::int;
use crate::input::{Input, DRAW, LEFT, RIGHT};

const R: i32 = 4;

fn near(a: V3, b: V3, units: i32) -> bool {
    (a.x - b.x).abs() <= units && (a.y - b.y).abs() <= units && (a.z - b.z).abs() <= units
}

/// A spot in open air 30 m east of the start, 5 m over the ground: far
/// from every body, so an orb there meets only what a test puts there.
fn sky(w: &World) -> V3 {
    let x = crate::world::START.x + int(30);
    let z = crate::world::START.z;
    v3(x, w.ground(x, z) + int(5), z)
}

/// An orb of `kind` at `pos` flying along `dir` at `speed` metres a tick.
fn orb(kind: &str, caster: u8, pos: V3, dir: V3, speed: Fx, id: u32) -> Orb {
    Orb {
        id,
        kind: spec().kind(kind).unwrap(),
        caster,
        pos,
        prev: pos,
        dir,
        speed,
        curl: 0,
        hold: 0,
        held: 0,
        launch: Launch::Along,
        age: 0,
        from: pos,
        turned: None,
    }
}

/// An upright edge 80 cm long centred on `at`, moving `travel` over the
/// tick, centred on `at` halfway through it.
fn upright_edge(at: V3, travel: V3) -> Swept {
    let h = v3(0, ONE * 4 / 10, 0);
    let (s, e) = (at.sub(travel.scale(ONE / 2)), at.add(travel.scale(ONE / 2)));
    Swept {
        a0: s.sub(h),
        b0: s.add(h),
        a1: e.sub(h),
        b1: e.add(h),
    }
}

// ---- CW-10 ----------------------------------------------------------------

/// Read from the file itself, not through `Spec::parse`: every kind's shape
/// differs from every other's, and there are six kinds, so six shapes.
#[test]
fn no_two_kinds_of_orb_share_a_shape() {
    let v: serde_json::Value = serde_json::from_str(include_str!("../../../../data/orbs.json")).unwrap();
    let shapes: Vec<&str> = v["kinds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["shape"].as_str().unwrap())
        .collect();
    assert_eq!(shapes.len(), 6);
    for (i, a) in shapes.iter().enumerate() {
        for b in &shapes[..i] {
            assert_ne!(a, b, "two kinds are drawn as a {a}");
        }
    }
    // and each shape is drawn with a mesh of its own
    let meshes: Vec<Mesh> = spec().kinds.iter().map(|k| k.shape.mesh()).collect();
    for (i, a) in meshes.iter().enumerate() {
        assert!(!meshes[..i].contains(a), "{a:?} drawn for two kinds");
    }
}

#[test]
fn the_orbs_file_is_refused_when_two_kinds_share_a_shape() {
    let mut v: serde_json::Value = serde_json::from_str(include_str!("../../../../data/orbs.json")).unwrap();
    v["kinds"][1]["shape"] = "sphere".into();
    assert_eq!(
        Spec::parse(&v.to_string()),
        Err("The orbs file gives \"sphere\" and \"dart\" one shape; each kind needs its own.".into())
    );
    let mut v: serde_json::Value = serde_json::from_str(include_str!("../../../../data/orbs.json")).unwrap();
    v["patterns"]["leader"][0]["kind"] = "comet".into();
    assert!(Spec::parse(&v.to_string()).unwrap_err().contains("\"comet\""));
}

// ---- turning, by hand ------------------------------------------------------

/// An edge swept along +x meets an orb flying +z square: d · t = 0, so
/// 2 (d · t) t − d = −d, straight back along −z. The blade pointing up or
/// pointing at the caster makes no difference.
#[test]
fn an_orb_met_square_by_a_sweep_goes_straight_back() {
    for axis in [v3(0, ONE, 0), v3(0, 0, -ONE), v3(0, 46_341, -46_341)] {
        let d = turned_way(v3(0, 0, ONE), axis, v3(ONE / 10, 0, 0), v3(0, 0, -ONE / 10));
        assert_eq!(d, v3(0, 0, -ONE), "edge along {axis:?}");
    }
}

/// An edge swept along t = (1, 0, 1)/√2, an orb flying d = (0, 0, 1):
/// d · t = 1/√2, so 2 (d · t) t − d = (1, 0, 1) − (0, 0, 1) = (1, 0, 0):
/// turned a quarter turn, off along the swing.
#[test]
fn an_orb_met_by_a_slanting_sweep_turns_by_twice_the_slant() {
    let d = turned_way(v3(0, 0, ONE), v3(0, ONE, 0), v3(ONE / 10, 0, ONE / 10), V3::ZERO);
    assert!(near(d, v3(ONE, 0, 0), R), "{d:?}");
    // swept down and across, t = (1, −1, 0)/√2, d = (0, 0, 1): d · t = 0,
    // straight back again
    let d = turned_way(v3(0, 0, ONE), v3(0, ONE, 0), v3(ONE / 10, -ONE / 10, 0), V3::ZERO);
    assert!(near(d, v3(0, 0, -ONE), R), "{d:?}");
}

/// A still edge turns an orb off its flat, facing the orb: from the edge to
/// the orb (0.1, 0.3, 0.2) less its part along the edge (the y) is
/// (1, 0, 2)/√5. An orb flying +z: d · n = 2/√5, d − (4/5)(1, 0, 2) =
/// (−0.8, 0, −0.6).
#[test]
fn a_still_edge_turns_an_orb_off_its_flat() {
    let off = v3(ONE / 10, ONE * 3 / 10, ONE * 2 / 10);
    let d = turned_way(v3(0, 0, ONE), v3(0, ONE, 0), v3(ONE / 1000, 0, 0), off);
    assert!(near(d, v3(-52_429, 0, -39_322), R * 2), "{d:?}");
}

// ---- turning in the world --------------------------------------------------

/// The staged case of the first hand test, flown through `advance`: the
/// orb 30 cm before an upright edge sweeping +x, flying +z at 0.2 m a tick,
/// comes back along −z at 140% of its speed (18 349 units a tick), counted
/// once for its caster; the edge sweeping again the next tick, and the
/// next, does not turn it again.
#[test]
fn an_imbued_edge_turns_an_orb_back_once_and_counts_it() {
    let mut w = World::new(5);
    let at = sky(&w);
    let edge = upright_edge(at, v3(ONE / 10, 0, 0));
    w.orbs.flying.push(orb(
        "sphere",
        0,
        at.sub(v3(0, 0, ONE * 3 / 10)),
        v3(0, 0, ONE),
        ONE / 5,
        0,
    ));
    advance(&mut w, Some(edge));
    let o = w.orbs.flying[0];
    assert_eq!(o.turned, Some(1));
    assert_eq!(o.dir, v3(0, 0, -ONE));
    assert_eq!(o.speed, 18_349);
    assert_eq!(deflected(&w, 0), 1);
    assert_eq!(w.orbs.sparks.len(), 1, "a ping where it turned");
    assert!(w.orbs.sparks[0].turn);
    // the edge swept back through it the other way, twice more
    let back = upright_edge(o.pos, v3(-ONE / 10, 0, 0));
    for _ in 0..2 {
        advance(&mut w, Some(back));
    }
    let o = w.orbs.flying[0];
    assert_eq!(o.dir, v3(0, 0, -ONE), "turned once only");
    assert_eq!(deflected(&w, 0), 1);
}

/// Three orbs side by side, 60 cm apart: the edge meets the middle one
/// alone. It turns; its neighbours fly on as they were.
#[test]
fn turning_one_orb_leaves_its_neighbours_alone() {
    let mut w = World::new(5);
    let at = sky(&w);
    let edge = upright_edge(at, v3(ONE / 10, 0, 0));
    for (k, dx) in [-60, 0, 60].into_iter().enumerate() {
        let p = at.add(v3(dx * ONE / 100, 0, -ONE * 3 / 10));
        w.orbs
            .flying
            .push(orb("sphere", 0, p, v3(0, 0, ONE), ONE / 5, k as u32));
    }
    advance(&mut w, Some(edge));
    let f = &w.orbs.flying;
    assert_eq!(f.len(), 3);
    assert_eq!(f[1].turned, Some(1));
    for side in [0, 2] {
        assert_eq!(f[side].turned, None);
        assert_eq!(f[side].dir, v3(0, 0, ONE));
        assert_eq!(f[side].speed, ONE / 5);
        assert_eq!(f[side].pos, f[side].prev.add(v3(0, 0, ONE / 5)));
    }
    assert_eq!(deflected(&w, 0), 1);
}

/// The ward, a cube, is never turned: the same edge passes it by.
#[test]
fn a_ward_is_never_turned() {
    let mut w = World::new(5);
    let at = sky(&w);
    let edge = upright_edge(at, v3(ONE / 10, 0, 0));
    w.orbs.flying.push(orb(
        "ward",
        0,
        at.sub(v3(0, 0, ONE * 3 / 10)),
        v3(0, 0, ONE),
        ONE / 5,
        0,
    ));
    advance(&mut w, Some(edge));
    assert_eq!(w.orbs.flying[0].turned, None);
    assert_eq!(w.orbs.flying[0].dir, v3(0, 0, ONE));
    assert_eq!(deflected(&w, 0), 0);
}

/// The player's own edge, through `step`: an orb flown across the drawn
/// blade near its tip passes through while the blade is not imbued, and
/// is turned while it is.
#[test]
fn the_blade_turns_orbs_only_while_imbued() {
    let turned = |imbued: bool| {
        let mut w = World::new(5);
        w.step(&Input {
            keys: DRAW,
            ..Input::default()
        });
        for _ in 0..60 {
            w.step(&Input::default());
        }
        assert!(w.me.arm.in_hand);
        if imbued {
            crate::imbue::lend(&mut w, 100);
        }
        assert_eq!(imbued_edge(&w).is_some(), imbued);
        let e = edge(&w.me.arm);
        let dir = e.b1.sub(e.a1).norm();
        let at = e.a1.lerp(e.b1, ONE * 85 / 100);
        let across = dir.cross(V3::UP).norm();
        w.orbs.flying.push(orb(
            "sphere",
            0,
            at.sub(across.scale(ONE * 3 / 10)),
            across,
            ONE / 5,
            0,
        ));
        crate::imbue::step(&mut w, &Input::default());
        step(&mut w);
        w.orbs.flying.first().and_then(|o| o.turned)
    };
    assert_eq!(turned(false), None, "not imbued: it passes through");
    assert_eq!(turned(true), Some(1), "imbued: turned");
}

/// A turned orb flown at a still leader's chest lands on it for the
/// sphere's 6 off its health. Not turned, the same orb passes it by.
#[test]
fn a_turned_orb_hits_its_caster() {
    let hit = |turned: bool| {
        let mut w = World::new(5);
        w.act("duel.pilot", "still").unwrap();
        w.act("duel.spawn", "leader").unwrap();
        for _ in 0..30 {
            w.step(&Input::default());
        }
        let chest = w.foes[0].fighter.pose.chest;
        let from = chest.add(v3(0, 0, int(2)));
        let mut o = orb("sphere", 1, from, chest.sub(from).norm(), ONE / 5, 0);
        if turned {
            o.turned = Some(0);
        }
        w.orbs.flying.push(o);
        let before = (w.foes[0].fighter.health.hp, w.me.health.hp);
        for _ in 0..30 {
            advance(&mut w, None);
        }
        (before.0 - w.foes[0].fighter.health.hp, before.1 - w.me.health.hp)
    };
    assert_eq!(hit(true), (6, 0));
    assert_eq!(hit(false), (0, 0));
}

/// Not turned, an orb lands on the player for its kind's damage: a gem,
/// 10.
#[test]
fn an_orb_hits_the_player_for_its_kind() {
    let mut w = World::new(5);
    for _ in 0..30 {
        w.step(&Input::default());
    }
    let chest = w.me.pose.chest;
    let from = chest.add(v3(0, 0, -int(2)));
    w.orbs
        .flying
        .push(orb("gem", 1, from, chest.sub(from).norm(), ONE / 5, 0));
    for _ in 0..30 {
        advance(&mut w, None);
    }
    assert_eq!(w.me.health.hp, 100 - 10);
    assert!(w.orbs.flying.is_empty(), "spent on the body");
}

/// Turning a dart jars the arms for `knock_ticks` (45); a gem shoves the
/// player along its flight at 2.6 m/s, 2 839 units a tick along +z; a
/// turned star landing weakens both arms for 150.
#[test]
fn darts_knock_gems_shove_and_stars_stun() {
    let mut w = World::new(5);
    let at = sky(&w);
    let edge = upright_edge(at, v3(ONE / 10, 0, 0));
    w.orbs.flying.push(orb(
        "dart",
        0,
        at.sub(v3(0, 0, ONE * 3 / 10)),
        v3(0, 0, ONE),
        ONE / 5,
        0,
    ));
    advance(&mut w, Some(edge));
    assert_eq!(w.me.health.weak, [45, 45]);

    let mut w = World::new(5);
    let at = sky(&w);
    let edge = upright_edge(at, v3(ONE / 10, 0, 0));
    w.orbs.flying.push(orb(
        "gem",
        0,
        at.sub(v3(0, 0, ONE * 3 / 10)),
        v3(0, 0, ONE),
        ONE / 5,
        0,
    ));
    let before = w.player.vel;
    advance(&mut w, Some(edge));
    assert_eq!(w.player.vel.sub(before), v3(0, 0, 2_839));

    let mut w = World::new(5);
    w.act("duel.pilot", "still").unwrap();
    w.act("duel.spawn", "leader").unwrap();
    for _ in 0..30 {
        w.step(&Input::default());
    }
    let chest = w.foes[0].fighter.pose.chest;
    let from = chest.add(v3(0, 0, int(2)));
    let mut o = orb("star", 1, from, chest.sub(from).norm(), ONE / 5, 0);
    o.turned = Some(0);
    w.orbs.flying.push(o);
    for _ in 0..30 {
        advance(&mut w, None);
    }
    assert_eq!(w.foes[0].fighter.health.weak, [150, 150]);
}

/// Turned to within 45° of its caster, a turned orb flies straight at the
/// caster's chest: here an orb flying away from the leader, met by a swing
/// across its line, turned straight back along (−1, −1, −6)/√38, 4.6°
/// off the chest.
#[test]
fn a_turned_orb_near_its_caster_s_line_flies_at_it() {
    let mut w = World::new(5);
    w.act("duel.pilot", "still").unwrap();
    w.act("duel.spawn", "leader").unwrap();
    for _ in 0..30 {
        w.step(&Input::default());
    }
    let chest = w.foes[0].fighter.pose.chest;
    // an orb 6 m off the leader, flying away from it, met square by an
    // edge sweeping across: mirrored straight back, then on the chest
    let start = chest.add(v3(int(1), int(1), int(6)));
    let away = start.sub(chest).norm();
    let across = away.cross(V3::UP).norm();
    let at = start.add(away.scale(ONE * 3 / 10));
    let h = V3::UP.scale(ONE * 4 / 10);
    let half = across.scale(ONE / 20);
    let e = Swept {
        a0: at.sub(half).sub(h),
        b0: at.sub(half).add(h),
        a1: at.add(half).sub(h),
        b1: at.add(half).add(h),
    };
    w.orbs.flying.push(orb("sphere", 1, start, away, ONE / 5, 0));
    advance(&mut w, Some(e));
    let o = w.orbs.flying[0];
    assert!(o.turned.is_some());
    assert_eq!(o.dir, chest.sub(o.pos).norm());
    assert_eq!(deflected(&w, 1), 1);
}

// ---- throwing ---------------------------------------------------------------

/// A fan of five 30° wide: the middle orb on the line to the target, the
/// outer two 15° off it either side (cos 15° = 0.9659, 63 303 units).
#[test]
fn a_fan_spreads_its_orbs_evenly_across_the_line() {
    let v = &spec().patterns.leader[0];
    assert_eq!((v.form, v.count, v.spread_deg), (Form::Fan, 5, 30));
    let origin = v3(0, int(1), 0);
    let to = v3(0, int(1), -int(9));
    let mut next = 0;
    let orbs = fire(v, origin, to, 1, 1, &mut next);
    assert_eq!(orbs.len(), 5);
    assert_eq!(next, 5);
    // the middle repeat of three has no sweep: the middle orb is on the line
    assert_eq!(orbs[2].dir, v3(0, 0, -ONE));
    for k in [0, 4] {
        assert!(
            (orbs[k].dir.dot(v3(0, 0, -ONE)) - 63_303).abs() <= 8,
            "{:?}",
            orbs[k].dir
        );
    }
    // a heading turns left as it grows: the first orb, turned −15°, is on
    // the caster's right (+x, facing −z)
    assert!(orbs[0].dir.x > 0 && orbs[4].dir.x < 0, "right to left");
    // 650 cm/s is 10.83 cm a tick: 7 099 units
    assert_eq!(orbs[0].speed, 7_099);
}

/// A ring of five darts 1.2 m round the origin, in the plane facing the
/// target, hanging 50 ticks before they close on it.
#[test]
fn a_ring_hangs_round_the_caster_facing_the_target() {
    let v = &spec().patterns.leader[1];
    assert_eq!((v.form, v.count, v.radius_cm, v.hold), (Form::Ring, 5, 120, 50));
    let origin = v3(0, int(1), 0);
    let to = v3(0, int(1), -int(9));
    let orbs = fire(v, origin, to, 0, 1, &mut 0);
    assert_eq!(orbs.len(), 5);
    for o in &orbs {
        let off = o.pos.sub(origin);
        assert!((off.len() - ONE * 12 / 10).abs() <= 8, "{off:?}");
        assert_eq!(off.z, 0, "in the plane facing the target");
        assert_eq!(o.hold, 50);
    }
}

/// A wave of nine with a hole two wide: seven orbs, the hole at the third
/// and fourth places (left), then the fourth and fifth, then the sixth and
/// seventh (n/4, (n − g)/2, n − n/4 − g).
#[test]
fn a_wave_leaves_a_hole_that_moves_across() {
    let v = &spec().patterns.leader[2];
    assert_eq!((v.form, v.count, v.gap, v.spacing_cm), (Form::Wave, 9, 2, 55));
    let origin = v3(0, int(1), 0);
    let to = v3(0, int(1), -int(9));
    // the place of each orb across the line, in spacings from the left end
    let places = |r: u32| -> Vec<i32> {
        fire(v, origin, to, r, 1, &mut 0)
            .iter()
            .map(|o| (o.pos.x + ONE * 55 / 100 * 4 + 8) / (ONE * 55 / 100))
            .collect()
    };
    assert_eq!(places(0), vec![0, 1, 4, 5, 6, 7, 8]);
    assert_eq!(places(1), vec![0, 1, 2, 5, 6, 7, 8]);
    assert_eq!(places(2), vec![0, 1, 2, 3, 4, 7, 8]);
}

// ---- the doors ------------------------------------------------------------

/// `flying` holds from the cast until the last orb is spent, `deflected`
/// starts again at each cast.
#[test]
fn a_cast_is_flying_from_the_cast_until_its_last_orb_is_spent() {
    let mut w = World::new(5);
    w.act("duel.pilot", "still").unwrap();
    w.act("duel.spawn", "leader").unwrap();
    w.orbs.turned.push((1, 4));
    cast(&mut w, 1, Pattern::Practice);
    assert_eq!(deflected(&w, 1), 0);
    assert!(flying(&w, 1), "before its first volley");
    assert!(!flying(&w, 0));
    let mut ticks = 0;
    while flying(&w, 1) {
        w.step(&Input::default());
        ticks += 1;
        assert!(ticks < 3000, "it ends");
    }
    // the last volley is thrown on the practice's tick 840, and hangs 60
    assert!(ticks > 900, "{ticks}");
    assert!(w.orbs.casts.is_empty() && w.orbs.flying.is_empty());
}

/// The practice does no harm: a player standing still through all of it
/// keeps every point of health.
#[test]
fn the_practice_round_does_no_harm() {
    let mut w = World::new(5);
    w.act("magic.practice", "").unwrap();
    let mut landed = 0;
    while flying(&w, 1) {
        w.step(&Input::default());
        // a spark struck this tick has burnt a tick by the step's end
        landed += w.orbs.sparks.iter().filter(|s| !s.turn && s.age == 1).count();
    }
    assert!(landed >= 5, "orbs landed on the player: {landed}");
    assert_eq!(w.me.health.hp, 100);
}

/// The leader's pattern is a threat to a player who stands still and much
/// less to one who keeps stepping side to side.
#[test]
fn the_leader_s_pattern_hurts_a_still_player_and_spares_a_moving_one() {
    let taken = |moving: bool| {
        let mut w = World::new(5);
        w.me.health = crate::health::Health::new(1_000);
        w.act("magic.leader", "").unwrap();
        let mut k = 0;
        while flying(&w, 1) {
            let keys = if !moving {
                0
            } else if (k / 50) % 2 == 0 {
                LEFT
            } else {
                RIGHT
            };
            w.step(&Input {
                keys,
                ..Input::default()
            });
            k += 1;
        }
        1_000 - w.me.health.hp
    };
    let (still, moving) = (taken(false), taken(true));
    assert!(still >= 60, "standing still takes {still}");
    assert!(moving * 2 <= still, "moving takes {moving}, standing {still}");
}

// ---- determinism, budget, flicker ------------------------------------------

/// CW-4: a recorded volley, the player swinging through it imbued, hashes
/// the same every run; one record changed changes it.
#[test]
fn a_recorded_volley_hashes_the_same_every_run() {
    let record: Vec<Input> = (0..1000)
        .map(|k| Input {
            keys: if k == 2 { DRAW } else { 0 },
            dx: if k % 40 < 20 { 1_800 } else { -1_800 },
            buttons: if k % 40 > 30 { crate::input::SWING } else { 0 },
            ..Input::default()
        })
        .collect();
    let run = |rec: &[Input]| {
        let mut w = World::new(11);
        w.act("magic.leader", "").unwrap();
        crate::imbue::lend(&mut w, 600);
        let mut hashes = Vec::new();
        for (k, i) in rec.iter().enumerate() {
            w.step(i);
            if k % 100 == 0 {
                hashes.push(w.hash());
            }
        }
        (hashes, w.orbs.clone())
    };
    let (a, orbs) = run(&record);
    assert_eq!(run(&record).0, a);
    assert!(!orbs.flying.is_empty() || !orbs.sparks.is_empty() || orbs.next > 0);
    let mut other = record.clone();
    other[300].dx += 1;
    assert_ne!(run(&other).0.last(), a.last());
}

/// A full leader volley in flight draws each kind as one run of solids and
/// one of outlines, every halo as one run: the page's draws grow with the
/// kinds, not the orbs. Three shapes an orb, or one halo once it has
/// passed the player.
#[test]
fn every_orb_of_a_kind_is_one_draw() {
    let mut w = World::new(5);
    w.me.health = crate::health::Health::new(1_000);
    w.act("magic.leader", "").unwrap();
    let mut most = (0, Shapes::default());
    for _ in 0..1100 {
        w.step(&Input::default());
        if w.orbs.flying.len() > most.0 {
            let mut s = Shapes::default();
            w.orbs.sparks.clear();
            draw(&w, 0, &crate::look::tests::look(), &mut s);
            most = (w.orbs.flying.len(), s);
        }
    }
    let (n, shapes) = most;
    assert!(n >= 20, "the most in flight at once: {n}");
    let mut runs: Vec<(crate::render::Pass, crate::prim::MeshId)> =
        shapes.list.iter().map(|s| (s.pass, s.mesh)).collect();
    runs.sort();
    runs.dedup();
    let kinds = runs
        .iter()
        .filter(|(p, _)| *p == crate::render::Pass::Solid)
        .count();
    assert_eq!(runs.len(), kinds * 2 + 1, "{runs:?}");
    assert!(kinds <= spec().kinds.len());
    assert!(
        shapes.len() <= n * 3 && shapes.len() >= n,
        "{} shapes for {n} orbs",
        shapes.len()
    );
}

/// CW-18: a frozen volley holds still. Its orbs do not move, and it draws
/// the same shapes at any share of a tick, tick after tick.
#[test]
fn a_frozen_volley_draws_the_same_shapes_every_frame() {
    let mut w = World::new(5);
    w.act("magic.leader", "").unwrap();
    w.act("magic.freeze", "100").unwrap();
    for _ in 0..140 {
        w.step(&Input::default());
    }
    assert!(w.orbs.frozen, "frozen at the cast's tick 100");
    assert!(!w.orbs.flying.is_empty());
    let look = crate::look::tests::look();
    let shot = |w: &World, alpha: Fx| {
        let mut s = Shapes::default();
        draw(w, alpha, &look, &mut s);
        s
    };
    let first = shot(&w, 0);
    let held = w.orbs.clone();
    for k in 0..5 {
        w.step(&Input::default());
        assert_eq!(w.orbs, held);
        assert_eq!(shot(&w, ONE * k / 5), first);
    }
    w.act("magic.freeze", "off").unwrap();
    w.step(&Input::default());
    assert_ne!(w.orbs.flying[0].pos, held.flying[0].pos, "and flies on");
}

#[test]
fn magic_acts_refuse_what_they_do_not_know() {
    let mut w = World::new(5);
    assert_eq!(
        w.act("magic.fireball", ""),
        Err("Unknown action \"magic.fireball\".".into())
    );
    assert_eq!(
        w.act("magic.freeze", "soon"),
        Err("Freeze takes on, off or a whole number of ticks, not \"soon\".".into())
    );
    w.act("magic.leader", "").unwrap();
    assert_eq!(w.foes.len(), 1, "a leader to cast it");
    assert_eq!(w.foes[0].body.pos.dist(w.player.pos) / ONE, 9);
    w.act("magic.clear", "").unwrap();
    assert!(!flying(&w, 1));
}
