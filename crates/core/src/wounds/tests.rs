//! The wounds' tests (M7, P4). Expected values are worked by hand from
//! `data/wounds.json` and `health.rs`'s damage rule: (speed − 1.5 m/s) × 2.5
//! × the part's share, rounded down; an arm's share is 60%, a leg's 70%.

use super::*;
use crate::blade::Part;
use crate::input::Input;
use crate::world::World;

/// A test duel with one foe of `kind`, held still.
fn duel(kind: &str) -> World {
    let mut w = World::new(3);
    player::act(&mut w, "pilot", "still").unwrap();
    player::act(&mut w, "spawn", kind).unwrap();
    w
}

fn idle(w: &mut World, n: u32) {
    for _ in 0..n {
        w.step(&Input::default());
    }
}

fn hit(on: u8, seg: usize, speed: Fx) -> Hit {
    Hit {
        by: 0,
        on,
        part: match seg {
            HEAD => Part::Head,
            TORSO => Part::Torso,
            2..=5 => Part::Arm(if seg <= 3 { 0 } else { 1 }),
            _ => Part::Leg(if seg <= 7 { 0 } else { 1 }),
        },
        seg: seg as u8,
        speed,
        at: V3::ZERO,
        vel: v3(ONE / 10, 0, 0),
    }
}

#[test]
fn a_cut_on_a_forearm_sets_its_damage_and_flashes_only_that_part() {
    let mut w = duel("orc");
    // 10 m/s on an arm: (10 − 1.5) × 2.5 × 0.6 = 12.75, down to 12
    w.act("wounds.cut", "forearm_r,10").unwrap();
    let l = w.foes[0].fighter.health.limbs;
    assert_eq!(l.dmg, [0, 0, 0, 0, 0, 12, 0, 0, 0, 0]);
    assert_eq!(l.flash, [0, 0, 0, 0, 0, 15, 0, 0, 0, 0]);
    assert_eq!(
        w.foes[0].fighter.health.hp,
        140 - 12,
        "the orc's 140, less the blow"
    );
    assert_eq!(l.lost, 0, "12 is under the forearm's 22");
}

/// The struck part's colour against the same figure's unstruck, drawn.
fn drawn(w: &World) -> Vec<Shape> {
    let look = crate::look::tests::look();
    let mut out = Shapes::default();
    let f = &w.foes[0].fighter;
    figure(w, 1, &f.pose, Dress::default(), &look, &mut out);
    out.list
}

#[test]
fn the_flash_is_red_on_the_part_weaker_on_the_body_and_exactly_nothing_after_its_ticks() {
    let mut w = duel("orc");
    w.ui.settings.blood = false;
    let before = drawn(&w);
    w.act("wounds.cut", "forearm_r,10").unwrap();
    let now = drawn(&w);
    assert_eq!(now.len(), before.len());
    let labels = labels(Kind::Orc, Dress::default());
    let solids: Vec<usize> = (0..now.len()).filter(|&k| now[k].pass == Pass::Solid).collect();
    // the struck part reddens by 80%, the rest of the body by 22%: on the
    // forearm the red channel rises toward 236 and the green falls toward 40
    let (mut part, mut body) = (0, 0);
    for (n, &k) in solids.iter().enumerate() {
        let (a, b) = (before[k].rgb, now[k].rgb);
        if a == b {
            continue;
        }
        if labels[n] == 5 {
            part += 1;
            let want = mix(a, rgb([236, 40, 46]), ONE * 80 / 100);
            assert_eq!(b, want, "part {n}");
        } else {
            body += 1;
            let want = mix(a, rgb([236, 40, 46]), ONE * 22 / 100);
            assert_eq!(b, want, "part {n}");
        }
    }
    assert!(
        part >= 2 && body > part,
        "{part} on the forearm, {body} elsewhere"
    );
    // 15 ticks of flash: still red after 14, the figure exactly as before at 15
    idle(&mut w, 14);
    assert_eq!(w.foes[0].fighter.health.limbs.flash[5], 1);
    assert_ne!(drawn(&w), before);
    idle(&mut w, 1);
    assert_eq!(w.foes[0].fighter.health.limbs.flash, [0; PARTS]);
    let f = &w.foes[0].fighter;
    let mut plain = Shapes::default();
    crate::models::figure_dressed(
        Kind::Orc,
        &f.pose,
        Dress::default(),
        &crate::look::tests::look(),
        &mut plain,
    );
    assert_eq!(drawn(&w), plain.list, "no trace of the flash");
}

#[test]
fn a_part_comes_off_at_its_take_off_and_not_one_unit_before() {
    // 2.5 m/s on an arm: (2.5 − 1.5) × 2.5 × 0.6 = 1.5, down to 1
    let slow = ONE * 5 / 2;
    let mut w = duel("orc");
    w.foes[0].fighter.health.limbs.dmg[5] = 20;
    stage(&mut w, hit(1, 5, slow));
    assert_eq!(w.foes[0].fighter.health.limbs.dmg[5], 21);
    assert_eq!(w.foes[0].fighter.health.limbs.lost, 0, "21 of the orc's 22: on");
    assert!(w.wounds.pieces.is_empty());
    stage(&mut w, hit(1, 5, slow));
    let l = w.foes[0].fighter.health.limbs;
    assert_eq!(l.lost, bit(5), "22 of 22: the right forearm is off, and only it");
    assert_eq!(w.wounds.pieces.len(), 1);
    assert!(l.loose, "the sword hand gone, the cleaver drops");
    // the orc's health: 140, less 1 and 1, less 8 for the part
    assert_eq!(w.foes[0].fighter.health.hp, 130);
    // the player's take-off is 160% of a foe's: 22 × 1.6 = 35.2, down to 35
    assert_eq!(spec().take_off(5, Kind::Player), 35);
}

#[test]
fn one_very_fast_cut_takes_a_limb_off_at_once() {
    let mut w = duel("bandit");
    // 12.5 m/s: (12.5 − 1.5) × 2.5 × 0.7 = 19.25 on a shin, under its 28,
    // but as fast as `one_cut_cms` (1250)
    stage(&mut w, hit(1, 7, ONE * 25 / 2));
    let l = w.foes[0].fighter.health.limbs;
    assert_eq!(l.dmg[7], 19);
    assert_eq!(l.lost, bit(7));
    // a cut a unit slower leaves it on
    let mut w = duel("bandit");
    stage(&mut w, hit(1, 7, ONE * 25 / 2 - 1));
    assert_eq!(w.foes[0].fighter.health.limbs.lost, 0);
}

#[test]
fn a_blow_on_a_stump_counts_on_the_part_above() {
    assert_eq!(landed_on(5, bit(5)), 4);
    assert_eq!(landed_on(5, bit(4) | bit(5)), TORSO);
    assert_eq!(landed_on(9, bit(9)), 8);
    assert_eq!(landed_on(HEAD, bit(HEAD)), TORSO);
    assert_eq!(landed_on(3, 0), 3);
}

#[test]
fn a_part_taken_off_falls_and_comes_to_rest_exactly_within_its_bound() {
    let mut w = duel("orc");
    w.act("wounds.take_off", "forearm_r").unwrap();
    let start = w.wounds.pieces[0].p;
    let mut rested = None;
    for t in 1..=spec().pieces.rest_by as u32 + 5 {
        w.step(&Input::default());
        if w.wounds.pieces[0].still {
            rested = Some(t);
            break;
        }
    }
    let t = rested.expect("still within rest_by");
    assert!(
        t < spec().pieces.rest_by as u32,
        "came to rest by itself, at tick {t}"
    );
    let pc = w.wounds.pieces[0].clone();
    assert_ne!(pc.p, start, "it fell");
    for p in pc.p {
        let g = crate::solid::surface(&w, p.x, p.z);
        assert!(p.y >= g && p.y < g + ONE / 5, "on the ground: {} over {}", p.y, g);
    }
    idle(&mut w, 120);
    assert_eq!(w.wounds.pieces[0].p, pc.p, "and stays exactly where it lies");
    assert_eq!(w.wounds.pieces[0].was, pc.p);
}

#[test]
fn the_dropped_weapon_settles_and_lies_still() {
    let mut w = duel("orc");
    w.act("wounds.take_off", "forearm_r").unwrap();
    idle(&mut w, spec().weapon_rest_by as u32 + 2);
    let f = &w.foes[0].fighter;
    assert!(f.health.limbs.loose && f.health.limbs.settled);
    let lying = (f.arm.pts[arm::POMMEL].p, f.arm.pts[arm::TIP].p);
    let g = crate::solid::surface(&w, lying.1.x, lying.1.z);
    assert!(lying.1.y < g + ONE / 5, "the cleaver's tip is on the ground");
    // the orc walks off; the cleaver stays
    for _ in 0..60 {
        w.step(&Input::default());
        w.foes[0].body.pos.x += 0;
    }
    let f = &w.foes[0].fighter;
    assert_eq!((f.arm.pts[arm::POMMEL].p, f.arm.pts[arm::TIP].p), lying);
}

/// CW-3 for a wounded body: an orc on one leg, run by its tree; then the
/// same with the tree taken out and its records played back. It moves the
/// same, and never faster than a hop: 45% of a walk.
#[test]
fn a_one_legged_orc_hops_at_its_share_of_a_walk_through_its_input_record_alone() {
    let setup = || {
        let mut w = World::new(3);
        player::act(&mut w, "spawn", "orc").unwrap();
        player::act(&mut w, "gap", "800").unwrap();
        w.act("wounds.take_off", "shin_l").unwrap();
        w
    };
    let mut a = setup();
    let mut kept = Vec::new();
    let mut fastest = 0;
    let mut moved = 0;
    for t in 0..400 {
        let held = a.feel.stop > 0;
        let before = a.foes[0].body.pos;
        a.step(&Input::default());
        if !held {
            kept.push(a.foes[0].fighter.last);
        }
        let step = a.foes[0].body.pos.sub(before).len_xz();
        if t > 110 {
            fastest = fastest.max(step);
            moved += step;
        }
        if t < 24 {
            assert_eq!(step, 0, "toppling, it does not walk");
        }
    }
    // a walk: WALK m/s over 60 ticks; a hop 45% of it, and a unit of rounding
    let hop = crate::motion::WALK / 60 * 45 / 100;
    assert!(fastest <= hop + 1, "{fastest} a tick, a hop is {hop}");
    assert!(moved > ONE, "it came on: {moved}");
    assert!(a.foes[0].fighter.health.limbs.hopping());
    let mut b = setup();
    b.foes[0].brain.control = crate::pilot::Control::Script(kept);
    for _ in 0..400 {
        b.step(&Input::default());
    }
    assert_eq!(a.foes[0].body, b.foes[0].body);
    assert_eq!(a.foes[0].fighter, b.foes[0].fighter);
}

#[test]
fn with_both_legs_gone_a_body_lies_on_its_front_and_crawls_slower_than_a_hop() {
    let mut w = World::new(3);
    player::act(&mut w, "spawn", "orc").unwrap();
    player::act(&mut w, "gap", "800").unwrap();
    w.act("wounds.take_off", "thigh_l").unwrap();
    w.act("wounds.take_off", "thigh_r").unwrap();
    let l = w.foes[0].fighter.health.limbs;
    assert!(l.prone && l.loose, "on its front, its weapon dropped");
    idle(&mut w, 30);
    assert_eq!(w.foes[0].fighter.health.fall(), -ONE, "lying on its front");
    let crawl = crate::motion::WALK / 60 * 22 / 100;
    let mut fastest = 0;
    let start = w.foes[0].body.pos;
    for _ in 0..300 {
        let before = w.foes[0].body.pos;
        w.step(&Input::default());
        fastest = fastest.max(w.foes[0].body.pos.sub(before).len_xz());
    }
    assert!(fastest <= crawl + 1, "{fastest}, a crawl is {crawl}");
    assert!(
        w.foes[0].body.pos.sub(w.player.pos).len_xz() < start.sub(w.player.pos).len_xz(),
        "an orc keeps coming"
    );
}

#[test]
fn blood_never_passes_its_caps() {
    let mut w = duel("orc");
    let (most, splats) = (spec().drops.most, spec().splats.most);
    for t in 0..600 {
        if t % 3 == 0 {
            w.act("wounds.cut", "torso,20").unwrap();
            w.foes[0].fighter.health.hp = 140;
        }
        w.step(&Input::default());
        assert!(w.wounds.drops.len() <= most, "{} drops", w.wounds.drops.len());
        assert!(
            w.wounds.splats.len() <= splats,
            "{} splats",
            w.wounds.splats.len()
        );
    }
    assert!(
        w.wounds.drops.len() > most / 2,
        "it fills: {}",
        w.wounds.drops.len()
    );
    assert!(!w.wounds.splats.is_empty());
}

#[test]
fn overlapping_marks_never_lie_at_one_height() {
    let mut w = duel("orc");
    for t in 0..400 {
        if t % 4 == 0 {
            w.act("wounds.cut", "torso,18").unwrap();
            w.foes[0].fighter.health.hp = 140;
        }
        w.step(&Input::default());
    }
    let s = &w.wounds.splats;
    assert!(s.len() > 4, "{}", s.len());
    for i in 0..s.len() {
        for j in i + 1..s.len() {
            let apart = s[i].at.sub(s[j].at).len_xz();
            if apart < s[i].goal.max(s[i].r) + s[j].goal.max(s[j].r) {
                assert_ne!(s[i].layer, s[j].layer, "marks {i} and {j} overlap at one height");
            }
        }
    }
}

#[test]
fn with_blood_off_nothing_of_blood_is_made_or_drawn() {
    let mut w = duel("orc");
    w.ui.settings.blood = false;
    w.act("wounds.cut", "torso,20").unwrap();
    w.act("wounds.take_off", "forearm_r").unwrap();
    w.foes[0].fighter.health.hp = 0;
    idle(&mut w, 200);
    assert!(w.wounds.drops.is_empty() && w.wounds.splats.is_empty());
    let look = crate::look::tests::look();
    let mut out = Shapes::default();
    w.draw(0, &look, &mut out);
    let c = &spec().colours;
    let blood = [rgb(c.blood), rgb(c.stump)];
    assert!(
        out.list.iter().all(|s| !blood.contains(&s.rgb)),
        "no drop and no red stump drawn"
    );
    assert!(out
        .list
        .iter()
        .all(|s| s.mesh != Mesh::Cyl.into() || s.pass != Pass::Dither));
    // and on again, a blow sheds it
    w.ui.settings.blood = true;
    w.foes[0].fighter.health.hp = 140;
    w.foes[0].fighter.health.down_ticks = 0;
    w.act("wounds.cut", "torso,20").unwrap();
    assert!(!w.wounds.drops.is_empty());
}

#[test]
fn a_recorded_duel_with_a_part_taken_off_hashes_the_same_every_run() {
    let run = || {
        let mut w = World::new(9);
        player::act(&mut w, "spawn", "orc").unwrap();
        let mut hashes = Vec::new();
        for k in 0..360 {
            if k == 40 {
                w.act("wounds.cut", "forearm_r,11").unwrap();
            }
            if k == 90 {
                w.act("wounds.take_off", "shin_r").unwrap();
            }
            w.step(&Input {
                keys: if k == 2 { crate::input::DRAW } else { 0 },
                dx: if k % 30 < 15 { 300 } else { -300 },
                buttons: if k % 60 > 40 { crate::input::SWING } else { 0 },
                ..Input::default()
            });
            if k % 60 == 59 {
                hashes.push(w.hash());
            }
        }
        (hashes, w.wounds.pieces.len(), w.wounds.splats.len())
    };
    let a = run();
    // the cut at 11 m/s takes 14 of the forearm's 22; the shin comes off
    assert!(a.1 == 1 && a.2 > 0, "pieces {}, splats {}", a.1, a.2);
    assert_eq!(a, run());
}

#[test]
fn the_dream_wakes_the_player_with_every_part() {
    let mut w = crate::story::opening(3);
    idle(&mut w, crate::story::WAKE_TICKS + crate::story::RISE_TICKS + 30);
    take_off(&mut w, 0, THIGH[0], v3(ONE / 10, 0, 0), fx::int(12));
    take_off(&mut w, 0, FOREARM[1], v3(ONE / 10, 0, 0), fx::int(12));
    assert_ne!(w.me.health.limbs.lost, 0);
    take_off(&mut w, 0, HEAD, v3(ONE / 10, 0, 0), fx::int(12));
    assert!(w.player_down(), "a head off is down");
    let mut n = 0;
    while !crate::story::waking(&w) {
        w.step(&Input::default());
        n += 1;
        assert!(n < 2000, "the dream comes");
    }
    w.step(&Input::default());
    assert_eq!(w.story.dreams, 1);
    assert_eq!(w.me.health.limbs, Limbs::default(), "whole");
    assert!(w.wounds.pieces.is_empty() && w.wounds.splats.is_empty());
}

#[test]
fn the_leader_takes_its_sword_up_in_the_other_hand_and_a_bandit_runs() {
    let mut w = World::new(5);
    player::act(&mut w, "spawn", "leader").unwrap();
    w.act("wounds.take_off", "forearm_r").unwrap();
    assert!(w.foes[0].fighter.health.limbs.loose);
    let mut switched = None;
    for t in 0..900 {
        w.step(&Input::default());
        let l = w.foes[0].fighter.health.limbs;
        if l.switched && !l.loose && l.take_up == 0 && w.foes[0].fighter.arm.hold[1] {
            switched = Some(t);
            break;
        }
    }
    assert!(switched.is_some(), "the leader holds its sword in its left hand");
    // and swings it again
    let mut swung = false;
    for _ in 0..400 {
        w.step(&Input::default());
        swung |= w.foes[0].fighter.last.button(crate::input::SWING);
    }
    assert!(swung, "one-handed, it fights on");

    let mut w = World::new(5);
    player::act(&mut w, "spawn", "bandit").unwrap();
    let start = w.foes[0].body.pos.sub(w.player.pos).len_xz();
    w.act("wounds.take_off", "forearm_r").unwrap();
    let mut ran = false;
    for _ in 0..180 {
        w.step(&Input::default());
        ran |= w.foes[0].fighter.last.key(crate::input::RUN);
    }
    let now = w.foes[0].body.pos.sub(w.player.pos).len_xz();
    assert!(ran, "the bandit runs");
    assert!(now > start + fx::int(3), "and gets away: {start} to {now}");
    assert!(
        !w.foes[0].fighter.health.limbs.switched,
        "it does not take its sword up"
    );
}

#[test]
fn the_player_without_a_sword_hand_takes_the_sword_up_in_the_other_and_is_weaker() {
    let mut w = duel("orc");
    w.step(&Input {
        keys: crate::input::DRAW,
        ..Default::default()
    });
    idle(&mut w, 40);
    w.act("wounds.take_off_me", "forearm_r").unwrap();
    assert!(w.me.health.limbs.loose);
    idle(&mut w, spec().weapon_rest_by as u32 + 2);
    // stand over the sword
    let grip = w.me.arm.grip_point();
    w.player.pos = v3(grip.x, w.player.pos.y, grip.z);
    w.player.prev_pos = w.player.pos;
    idle(&mut w, 2);
    assert!(stamp(&w), "the line offers it");
    w.step(&Input {
        keys: crate::input::DRAW,
        ..Default::default()
    });
    idle(&mut w, (spec().take_up_reach + spec().take_up_lift) as u32 + 2);
    let l = w.me.health.limbs;
    assert!(l.switched && !l.loose, "in the left hand");
    assert!(w.me.arm.in_hand && w.me.arm.hold[1]);
    let (_, _, g) = player::goals(&w, 0, &Input::default());
    // the player's left hand at 75% of the right's
    let whole = crate::player::Fighter::new(Kind::Player, pose::Held::ShortSword, 0, V3::ZERO, 0, true);
    assert_eq!(g.strength[1], whole.servo_pct * 75 / 100);
    // drawn in the left hand: the hand on the hilt is the body's left
    assert!(w.me.pose.hand[LEFT].dist(w.me.pose.grip) < ONE / 10);
}

#[test]
fn every_kind_s_parts_are_labelled_on_any_pose() {
    for kind in models::KINDS {
        let labels = labels(kind, Dress::default());
        let p = models::rest_pose(kind, v3(ONE, 0, ONE), fx::deg(30));
        assert_eq!(
            models::figure_model(kind, &p, Dress::default()).parts.len(),
            labels.len()
        );
        for (s, name) in NAMES.iter().enumerate() {
            assert!(labels.contains(&(s as u8)), "{kind:?} has parts on {name}");
        }
    }
}

#[test]
fn a_struck_part_flinches_as_drawn_and_eases_back() {
    // 70 mm along the blow at the blow, 9 ticks; a knee also drops half of it
    let mut l = Limbs {
        flinch: 9,
        flinch_seg: 7,
        flinch_way: v3(ONE, 0, 0),
        ..Limbs::default()
    };
    let p = models::rest_pose(Kind::Player, V3::ZERO, 0);
    let k = mm(70);
    let q = flinched(&l, &p);
    assert_eq!(q.knee[0], p.knee[0].add(v3(k * 3 / 4, -k / 2, 0)));
    assert_eq!(q.head, p.head, "only the struck part");
    l.flinch = 3;
    l.flinch_seg = HEAD as u8;
    assert_eq!(flinched(&l, &p).head, p.head.add(v3(k * 3 / 9, 0, 0)));
    l.flinch = 0;
    assert_eq!(flinched(&l, &p), p);
}

#[test]
fn a_blade_s_contact_names_the_capsule_it_struck() {
    // a still orc, the player's sword swept across and walked into its
    // guard: every blow names a capsule of the part it landed on
    let mut w = duel("orc");
    w.step(&Input {
        keys: crate::input::DRAW,
        ..Default::default()
    });
    idle(&mut w, 40);
    let mut seen = Vec::new();
    for k in 0..200 {
        w.step(&Input {
            keys: crate::input::FORWARD,
            dx: if k % 40 < 20 { -15_000 } else { 15_000 },
            buttons: crate::input::SWING,
            ..Default::default()
        });
        seen.extend(w.blades.hits.iter().copied());
    }
    assert!(!seen.is_empty(), "the sweeps landed");
    for h in &seen {
        let ok = match h.part {
            Part::Head => h.seg == 0,
            Part::Torso => h.seg == 1,
            Part::Arm(s) => h.seg as usize == 2 + 2 * s || h.seg as usize == 3 + 2 * s,
            Part::Leg(s) => h.seg as usize == 6 + 2 * s || h.seg as usize == 7 + 2 * s,
        };
        assert!(ok, "{h:?}");
    }
    for h in seen.iter().filter(|h| crate::health::damage(h.speed, h.part) > 0) {
        let l = player::fighter(&w, h.on as usize).health.limbs;
        assert!(l.dmg[h.seg as usize] > 0, "the struck part took it: {h:?}");
    }
}
