//! The models' tests: that a figure sits on its pose's joints and a held
//! item on its grip and tip, that every model draws in the file's colours,
//! that no two parts of any model share a plane and every mark stands off
//! its surface (CW-18), that a prop bakes to one mesh reused, and that the
//! lineup comes and goes.

use super::figure::model as figure_model;
use super::held::{model as held_model, pickup_model};
use super::props::{bake, donkey_model, model as prop_model};
use super::*;
use crate::flicker::{self, Inst, Seen};
use crate::fx::int;
use crate::pose::{build, LEFT, RIGHT};

fn look() -> Look {
    crate::look::tests::look()
}

/// A unit mesh's two ends along its up axis, placed.
fn ends(p: &Part) -> (V3, V3) {
    let h = ONE / 2;
    (p.at.apply(v3(0, -h, 0)), p.at.apply(v3(0, h, 0)))
}

fn near(a: V3, b: V3, tol: Fx) -> bool {
    a.dist(b) <= tol
}

/// Every figure the lineup shows, and the player wearing every piece.
fn every_figure() -> Vec<(Kind, Dress, Model)> {
    let mut out = Vec::new();
    let mut list = lineup::cast();
    list.push((
        Kind::Player,
        Dress {
            worn: SPEEDSTER_ALL,
            variant: 0,
        },
    ));
    for (kind, dress) in list {
        let pose = rest_pose(kind, v3(int(10), 0, int(20)), fx::deg(30));
        let mut m = figure_model(kind, &pose, dress);
        let held = held_model(kind_held(kind), pose.grip, pose.tip);
        m.parts.extend(held.parts);
        out.push((kind, dress, m));
    }
    out
}

fn every_other_model() -> Vec<(String, Model)> {
    let mut out: Vec<(String, Model)> = Prop::ALL
        .iter()
        .map(|p| (format!("{p:?}"), prop_model(*p)))
        .collect();
    for p in Pickup::ALL {
        out.push((format!("{p:?}"), pickup_model(p)));
    }
    out.push(("donkey".into(), donkey_model(fx::QUARTER_TURN / 2)));
    for item in [
        Held::ShortSword,
        Held::Sword,
        Held::Cleaver,
        Held::Boomerang,
        Held::Rock,
        Held::Knife,
    ] {
        out.push((
            format!("{item:?}"),
            held_model(
                item,
                v3(0, ONE, 0),
                v3(0, ONE, 0).add(v3(grip_to_tip(item), 0, 0)),
            ),
        ));
    }
    out
}

#[test]
fn every_model_draws_in_the_file_s_colours() {
    for (kind, dress, m) in every_figure() {
        for (i, p) in m.parts.iter().enumerate() {
            assert_ne!(
                p.rgb, MISSING,
                "{kind:?} {dress:?}: part {i} names a colour data/models.json lacks"
            );
        }
    }
    for (name, m) in every_other_model() {
        for (i, p) in m.parts.iter().enumerate() {
            assert_ne!(
                p.rgb, MISSING,
                "{name}: part {i} names a colour data/models.json lacks"
            );
        }
    }
    // the file's widths: 9 mm on a figure, 16 mm on a prop
    assert_eq!(table().ink_figure, 9 * ONE / 1000);
    assert_eq!(table().ink_prop, 16 * ONE / 1000);
}

/// The figure's limbs run exactly between the pose's joints: on the rest
/// pose, and on a pose with the right arm thrown up over the head, as a
/// swing would put it.
#[test]
fn limbs_run_between_the_pose_s_joints() {
    let mut pose = rest_pose(Kind::Player, V3::ZERO, 0);
    pose.elbow[RIGHT] = pose.shoulder[RIGHT].add(v3(0, mm(310), 0));
    pose.hand[RIGHT] = pose.elbow[RIGHT].add(v3(-mm(100), mm(270), -mm(40)));
    for p in [rest_pose(Kind::Player, V3::ZERO, 0), pose] {
        let m = figure_model(Kind::Player, &p, Dress::default());
        let has = |a: V3, b: V3| {
            m.parts.iter().filter(|q| q.mesh == Mesh::Cyl).any(|q| {
                let (x, y) = ends(q);
                near(x, a, 4) && near(y, b, 4)
            })
        };
        for s in [LEFT, RIGHT] {
            assert!(has(p.shoulder[s], p.elbow[s]), "upper arm {s}");
            assert!(has(p.elbow[s], p.hand[s]), "forearm {s}");
            assert!(has(p.hip[s], p.knee[s]), "thigh {s}");
            // the shin to the ankle, 7 cm over the foot
            assert!(has(p.knee[s], p.foot[s].add(v3(0, mm(70), 0))), "shin {s}");
        }
        // the hand balls sit on the hands
        for s in [LEFT, RIGHT] {
            assert!(
                m.parts
                    .iter()
                    .any(|q| q.mesh == Mesh::Facet && q.at.t == p.hand[s]),
                "hand {s}"
            );
        }
    }
}

/// The head sits at `pose.head` and faces `pose.facing`: turned a quarter
/// left (facing west, −x), the eyes are west of the head's middle.
#[test]
fn the_head_sits_at_the_pose_s_head_and_faces_its_way() {
    for (yaw, west) in [(fx::QUARTER_TURN, true), (-fx::QUARTER_TURN, false)] {
        let pose = rest_pose(Kind::Bandit, v3(int(5), 0, int(5)), yaw);
        let m = figure_model(Kind::Bandit, &pose, Dress::default());
        let head = m
            .parts
            .iter()
            .find(|p| p.mesh == Mesh::Facet && p.at.t == pose.head);
        assert!(head.is_some(), "a ball at the pose's head");
        // the eyes: the decals laid on the head
        let k = m
            .parts
            .iter()
            .position(|p| p.mesh == Mesh::Facet && p.at.t == pose.head)
            .unwrap();
        let eyes: Vec<&Part> = m
            .decals
            .iter()
            .filter(|(_, b)| *b == k)
            .map(|(d, _)| &m.parts[*d])
            .collect();
        assert!(eyes.len() >= 2);
        for e in eyes {
            // 18 cm out from the middle of the head, to the west or east
            let dx = e.at.t.x - pose.head.x;
            assert!(if west { dx < -int(1) / 10 } else { dx > int(1) / 10 }, "{dx}");
        }
    }
}

/// `rest_pose` keeps the build's lengths, so M1's arms and the drawn arms
/// agree: upper arm 31 cm, forearm 29 cm, thigh 37 cm, shin 35 cm on the
/// player, and the head's middle 1.70 − 0.03 − 0.21 = 1.46 m up.
#[test]
fn the_rest_pose_keeps_the_build_s_lengths() {
    let p = rest_pose(Kind::Player, V3::ZERO, fx::deg(70));
    let close = |a: V3, b: V3, cm: i32| (a.dist(b) - cm * ONE / 100).abs() <= 8;
    for s in [LEFT, RIGHT] {
        assert!(close(p.shoulder[s], p.elbow[s], 31), "upper arm {s}");
        assert!(close(p.elbow[s], p.hand[s], 29), "forearm {s}");
        assert!(close(p.hip[s], p.knee[s], 37), "thigh {s}");
        assert!(close(p.knee[s], p.foot[s], 35), "shin {s}");
        assert_eq!(p.foot[s].y, 0, "feet on the ground");
    }
    assert!((p.head.y - ONE * 146 / 100).abs() <= 4, "{}", p.head.y);
    // the short sword: the tip 80.6 cm from the grip
    assert!((p.tip.dist(p.grip) - mm(806)).abs() <= 4);
    // an orc's build is its own: its forearm 29 × 200/170 = 34.1 cm
    let o = rest_pose(Kind::Orc, V3::ZERO, 0);
    assert!((o.elbow[RIGHT].dist(o.hand[RIGHT]) - build(Kind::Orc).forearm).abs() <= 8);
}

/// A held item runs from grip to tip with the hilt at the grip: the
/// sword's handle is centred on the grip and its point ends at the tip.
#[test]
fn a_held_item_runs_from_its_grip_to_its_tip() {
    assert_eq!(grip_to_tip(Held::ShortSword), mm(80 + 26 + 700));
    assert_eq!(grip_to_tip(Held::Sword), mm(110 + 28 + 1000));
    let grip = v3(int(3), int(1), int(4));
    for dir in [
        v3(ONE, 0, 0),
        v3(0, 0, -ONE),
        v3(ONE * 6 / 10, ONE * 8 / 10, 0),
        v3(0, ONE, 0),
    ] {
        let tip = grip.add(dir.scale(grip_to_tip(Held::Sword)));
        let m = held_model(Held::Sword, grip, tip);
        let hilt = &m.parts[0];
        assert_eq!(hilt.mesh, Mesh::Cyl);
        assert!(near(hilt.at.t, grip, 2), "the hilt's middle is the grip");
        let point = m.parts.iter().find(|p| p.mesh == Mesh::Wedge).unwrap();
        assert!(
            near(ends(point).1, tip, 6),
            "the point ends at the tip: {:?} {:?}",
            ends(point).1,
            tip
        );
    }
    assert!(held_model(Held::Nothing, grip, grip.add(v3(ONE, 0, 0)))
        .parts
        .is_empty());
}

/// The lift a mark needs, worked by hand: the near layer's depth step at
/// its 128 m far plane is 159 Q16 units (2.43 mm, `render.rs`), the far
/// layer's at 1 km 34 units (0.53 mm), so four steps of the worse: 636
/// units, 9.7 mm. `DECAL_LIFT` (1 cm, 655 units) clears it.
#[test]
fn the_lift_a_mark_needs_is_four_depth_steps_at_the_worst_distance() {
    assert_eq!(decal_lift_needed(FIGURE_FAR), 4 * 159);
    assert!(DECAL_LIFT >= decal_lift_needed(FIGURE_FAR));
    // to 3 km: the far layer's step there is 9 × 34.6 = 311 units, four of
    // them 1 245 (19 mm), under `PROP_GAP` (25 mm, 1 638)
    let at_3km = decal_lift_needed(PROP_FAR);
    assert!((at_3km - 4 * 311).abs() <= 4, "{at_3km}");
    assert!(PROP_GAP >= at_3km);
}

/// How far a point is outside the ellipsoid part `base` along `-dir`, to
/// the tenth of a millimetre: where its implicit function changes sign.
fn depth_into(base: &Part, p: V3, dir: V3) -> Fx {
    let inside = |q: V3| {
        let rel = q.sub(base.at.t);
        let mut sum: i128 = 0;
        for c in [base.at.c0, base.at.c1, base.at.c2] {
            // the share of the column's half length (the mesh's radius ½)
            let l2 = c.x as i128 * c.x as i128 + c.y as i128 * c.y as i128 + c.z as i128 * c.z as i128;
            let dot = c.x as i128 * rel.x as i128 + c.y as i128 * rel.y as i128 + c.z as i128 * rel.z as i128;
            let u = dot * 2 * ONE as i128 / l2;
            sum += u * u;
        }
        sum <= (ONE as i128) * (ONE as i128)
    };
    let (mut lo, mut hi) = (0, mm(60));
    if inside(p) {
        return 0;
    }
    while hi - lo > 6 {
        let mid = (lo + hi) / 2;
        if inside(p.sub(dir.scale(mid))) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    lo
}

/// Every mark (eyes, catch-lights, blush, rivets, buttons, stripes) stands
/// off the surface under it by at least the lift the depth buffer needs
/// out to `FIGURE_FAR`, measured from the mark's outermost point (its dot's
/// pole) back along its normal to where the surface under it starts.
#[test]
fn every_mark_stands_off_its_surface_by_the_lift_the_depth_buffer_needs() {
    let need = decal_lift_needed(FIGURE_FAR);
    let mut checked = 0;
    let mut all: Vec<(String, Model)> = every_figure()
        .into_iter()
        .map(|(k, d, m)| (format!("{k:?} {d:?}"), m))
        .collect();
    all.extend(every_other_model());
    for (name, m) in all {
        for &(d, b) in &m.decals {
            let mark = &m.parts[d];
            let n = mark.at.c1.norm();
            let pole = mark.at.apply(v3(0, ONE / 2, 0));
            let lift = depth_into(&m.parts[b], pole, n);
            assert!(
                lift >= need - 6,
                "{name}: mark {d} on part {b} stands {} µm off, under {} µm",
                lift * 1_000_000 / ONE,
                need * 1_000_000 / ONE
            );
            checked += 1;
        }
    }
    assert!(checked > 80, "{checked} marks measured");
}

/// A model's parts as the flicker scan reads them.
fn insts(m: &Model) -> Vec<Inst> {
    m.parts
        .iter()
        .map(|p| Inst {
            mesh: p.mesh.into(),
            at: p.at,
            rgb: p.rgb,
            value: p.glow,
            pass: render::Pass::Solid,
        })
        .collect()
}

/// Cameras all round a model at `d`, at eye height and from above.
fn eyes_round(centre: V3, d: Fx) -> Vec<V3> {
    let mut out = Vec::new();
    for k in 0..8 {
        let a = fx::TURN * k / 8;
        for up in [ONE * 2, d / 2] {
            out.push(centre.add(v3(fx::mul(fx::cos(a), d), up, fx::mul(fx::sin(a), d))));
        }
    }
    out
}

fn scan(name: &str, m: &Model, centre: V3, far: Fx) -> Vec<String> {
    let i = insts(m);
    let table = MeshTable::default();
    let mut bad = Vec::new();
    // two parts in one plane, from anywhere; then planes too near for the
    // depth buffer from `far - 8 m` round the model, the farthest it is drawn
    let seen_far = Seen {
        bottom: centre.y,
        eyes: eyes_round(centre, far - int(8)),
        far,
    };
    for seen in [
        Seen {
            bottom: centre.y,
            ..Seen::near()
        },
        seen_far,
    ] {
        for f in flicker::fights(&i, &table, &seen) {
            bad.push(format!("{name}: {}", flicker::describe(&i, &f)));
        }
    }
    bad
}

/// No two parts of any figure share a plane facing one way (CW-18): the
/// flicker scan over every figure at rest holding its item, seen from
/// near and from 120 m, where the near layer's depth steps are coarsest.
#[test]
fn no_two_parts_of_any_figure_share_a_plane() {
    let mut bad = Vec::new();
    for (kind, dress, m) in every_figure() {
        let name = format!("{kind:?} {dress:?}");
        bad.extend(scan(&name, &m, v3(int(10), 0, int(20)), render::NEAR_LAYER_FAR));
        bad.extend(scan(&name, &m, v3(int(10), 0, int(20)), FIGURE_FAR));
    }
    assert!(bad.is_empty(), "{} fights:\n{}", bad.len(), bad.join("\n"));
}

/// The same for every prop, pickup, the donkey and every held item, the
/// props seen out to `PROP_FAR`.
#[test]
fn no_two_parts_of_any_prop_share_a_plane() {
    let mut bad = Vec::new();
    let props: Vec<String> = Prop::ALL.iter().map(|p| format!("{p:?}")).collect();
    for (name, m) in every_other_model() {
        // a prop is drawn to `PROP_FAR`; a pickup, the donkey and an item
        // with the figures, to `FIGURE_FAR`
        let far = if props.contains(&name) {
            PROP_FAR
        } else {
            FIGURE_FAR
        };
        bad.extend(scan(&name, &m, V3::ZERO, render::NEAR_LAYER_FAR));
        bad.extend(scan(&name, &m, V3::ZERO, far));
    }
    assert!(bad.is_empty(), "{} fights:\n{}", bad.len(), bad.join("\n"));
}

/// The scan over the static array the page receives: every prop placed
/// by `prop` in its own group, each one mesh, and no fights between them.
#[test]
fn the_flicker_scan_reads_every_prop_in_the_static_array() {
    let l = look();
    let mut meshes = MeshTable::default();
    let mut groups = Vec::new();
    let mut want = 0;
    for (i, p) in Prop::ALL.iter().enumerate() {
        let at = v3(int(30 * i as i32), 0, int(40));
        let mut g = Group::new("prop", at, prop_radius(*p));
        prop(*p, at, fx::deg(15 * i as i32), &l, &mut meshes, &mut g);
        groups.push(g);
        want += bake(&prop_model(*p)).body.len() / (3 * VERTEX_VALUES);
    }
    let scene = render::Scene::build(&groups, V3::ZERO, 1);
    let i = flicker::instances(&scene);
    let tris: usize = i
        .iter()
        .filter(|x| x.pass == render::Pass::Solid && x.value == 0 && !meshes.vertices(x.mesh).is_empty())
        .map(|x| meshes.vertices(x.mesh).len() / (3 * VERTEX_VALUES))
        .sum();
    assert!(
        tris >= want,
        "the scan reads {tris} of the props' {want} triangles"
    );
    let seen = Seen {
        bottom: 0,
        eyes: eyes_round(v3(int(200), 0, int(40)), int(60)),
        far: render::FAR,
    };
    let f = flicker::fights(&i, &meshes, &seen);
    assert!(
        f.is_empty(),
        "{:?}",
        f.iter().map(|x| flicker::describe(&i, x)).collect::<Vec<_>>()
    );
}

/// A prop is one mesh for its kind and one for its outline, added to the
/// table once and reused by every later placement.
#[test]
fn a_prop_bakes_to_one_mesh_reused_by_every_later_placement() {
    let l = look();
    let mut meshes = MeshTable::default();
    let mut g = Group::new("houses", V3::ZERO, int(100));
    prop(Prop::House, v3(int(10), 0, 0), 0, &l, &mut meshes, &mut g);
    assert_eq!(meshes.world.len(), 2, "the house's body and outline");
    // a house has nothing that glows: its body and its outline
    assert_eq!(g.shapes.len(), 2);
    prop(
        Prop::House,
        v3(int(30), 0, 0),
        fx::QUARTER_TURN,
        &l,
        &mut meshes,
        &mut g,
    );
    assert_eq!(meshes.world.len(), 2, "the second house reuses both");
    assert_eq!(g.shapes.list[0].mesh, g.shapes.list[2].mesh);
    prop(Prop::Campfire, V3::ZERO, 0, &l, &mut meshes, &mut g);
    assert_eq!(meshes.world.len(), 4);
    // the campfire's two flames are shapes of their own, unlit: the outer
    // with a halo, the core without
    assert_eq!(g.shapes.len(), 4 + 2 + 3);
}

/// A prop's base stands at `at` and turns by `yaw`: the house's footing is
/// on the ground (its lowest vertex at 0), and turned a quarter left at
/// (100, 5, 200), its front, 2.5 m ahead of its middle, is at x = 97.5.
#[test]
fn a_prop_s_base_sits_at_its_place_and_turns_by_its_yaw() {
    let b = bake(&prop_model(Prop::House));
    let lowest = b.body.chunks(VERTEX_VALUES).map(|v| v[1]).min().unwrap();
    assert_eq!(lowest, 0);
    let l = look();
    let mut meshes = MeshTable::default();
    let mut g = Group::new("house", V3::ZERO, int(10));
    let at = v3(int(100), int(5), int(200));
    prop(Prop::House, at, fx::QUARTER_TURN, &l, &mut meshes, &mut g);
    let place = g.shapes.list[0].at;
    let front = place.apply(v3(0, 0, -int(5) / 2));
    assert!(
        near(front, v3(int(100) - int(5) / 2, int(5), int(200)), 2),
        "{front:?}"
    );
    // its radius holds every part: the barn's corner is 4.35 m out on x,
    // 3.45 m on z, so over 5.5 m
    assert!(prop_radius(Prop::Barn) > int(55) / 10);
}

/// Each speedster piece worn shows on its own part of the body: gloves on
/// the hands, trousers on the knees, boots on the feet, the jacket on the
/// chest, the hood over the head; and none worn shows none.
#[test]
fn each_speedster_piece_shows_on_its_own_part_of_the_body() {
    let pose = rest_pose(Kind::Player, V3::ZERO, 0);
    let speed = table().c("speed_base");
    let colour_at = |worn: u8, at: V3| {
        let m = figure_model(Kind::Player, &pose, Dress { worn, variant: 0 });
        m.parts
            .iter()
            .find(|p| p.mesh == Mesh::Facet && p.at.t == at)
            .map(|p| p.rgb)
    };
    let hand = pose.hand[RIGHT];
    let knee = pose.knee[RIGHT];
    assert_eq!(colour_at(GLOVES, hand), Some(speed));
    assert_ne!(colour_at(0, hand), Some(speed));
    assert_ne!(
        colour_at(TROUSERS, hand),
        Some(speed),
        "trousers leave the hands bare"
    );
    assert_eq!(colour_at(TROUSERS, knee), Some(speed));
    assert_ne!(colour_at(BOOTS, knee), Some(speed));
    let count = |worn: u8| {
        figure_model(Kind::Player, &pose, Dress { worn, variant: 0 })
            .parts
            .iter()
            .filter(|p| p.rgb == speed)
            .count()
    };
    assert_eq!(count(0), 0);
    for bit in [HOOD, JACKET, GLOVES, TROUSERS, BOOTS] {
        assert!(count(bit) > 0, "piece {bit} shows");
    }
    assert_eq!(
        count(SPEEDSTER_ALL),
        [HOOD, JACKET, GLOVES, TROUSERS, BOOTS]
            .iter()
            .map(|b| count(*b))
            .sum::<usize>()
    );
}

/// The donkey's legs swing with its step: at phase 0 they hang straight
/// down; a quarter stride on, the front left hoof is 0.82 m × sin 22° =
/// 0.307 m back of its hip.
#[test]
fn the_donkey_s_legs_swing_with_its_step() {
    let hoof_z = |phase: i32| {
        let m = donkey_model(phase);
        // the front left leg's hoof: the first hoof pushed
        let legs: Vec<&Part> = m
            .parts
            .iter()
            .filter(|p| p.mesh == Mesh::Cyl && p.at.c1.len() > mm(700))
            .collect();
        let (top, foot) = ends(legs[0]);
        (top, foot)
    };
    let (top, foot) = hoof_z(0);
    assert!(
        (foot.z - top.z).abs() <= 16,
        "straight down at phase 0: {}",
        foot.z - top.z
    );
    let (top, foot) = hoof_z(fx::QUARTER_TURN);
    let back = foot.z - top.z;
    assert!((back.abs() - mm(307)).abs() <= mm(3), "{}", back * 1000 / ONE);
}

/// The lineup is off until asked for, shows every figure and prop, refuses
/// a view it does not have by name, and goes when turned off.
#[test]
fn the_lineup_comes_when_asked_for_and_goes_when_turned_off() {
    let l = look();
    let mut w = crate::world::World::new(1);
    let count = |w: &crate::world::World| {
        let mut s = Shapes::default();
        draw(w, 0, &l, &mut s);
        s.len()
    };
    assert_eq!(count(&w), 0, "off in play");
    w.act("models.lineup", "all").unwrap();
    let all = count(&w);
    let one: usize = every_figure()
        .iter()
        .take(lineup::cast().len())
        .map(|(_, _, m)| m.instances())
        .sum::<usize>();
    assert!(all > one, "{all} shapes for figures of {one}");
    assert_eq!(
        w.act("models.lineup", "everyone"),
        Err("The lineup has no view \"everyone\".".into())
    );
    assert_eq!(
        w.act("models.lineup", "all:a bit"),
        Err("The lineup's turn is whole degrees, such as 40, not \"a bit\".".into())
    );
    w.act("models.lineup", "orc:40").unwrap();
    assert_eq!(w.lineup.turn, fx::deg(40));
    w.act("models.lineup", "off").unwrap();
    assert_eq!(count(&w), 0);
    assert_eq!(
        w.act("models.parade", "all"),
        Err("Unknown action \"models.parade\".".into())
    );
}
