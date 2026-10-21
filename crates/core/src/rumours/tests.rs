//! T17's tests: every rumour names its thing's one route and where it is,
//! every item with a route of its own has a rumour, a rumour is offered only
//! while its condition holds and never in the opening, hearing one opens its
//! line and marks its place, the item in hand moves the line on and closes
//! it, a person's talk goes through what they know and ends on a closing
//! line, the people stand clear on flattened ground, and the same seed and
//! talk give the same world.

use super::*;
use crate::input::{Input, USE};
use crate::items::catalogue;
use crate::story::Phase;

/// A bare world (no opening, so rumours are told) with the roadside's
/// stops on the ground.
fn world() -> World {
    let mut w = World::new(1);
    crate::roadside::begin(&mut w);
    w
}

fn ids(rs: &[&RumourSpec]) -> Vec<String> {
    rs.iter().map(|r| r.id.clone()).collect()
}

fn give(w: &mut World, i: Item) {
    crate::inventory::give(w, i, 1);
    step(w);
}

fn rumour(id: &str) -> &'static RumourSpec {
    spec().rumours.iter().find(|r| r.id == id).unwrap()
}

/// The player stood `metres` in front of person `id`, facing them.
fn before(w: &mut World, id: &str, metres: i32) {
    act(w, "look", &format!("{id} {metres}")).unwrap();
    crate::places::arrive(w);
}

fn press_e(w: &mut World) {
    w.step(&Input {
        keys: USE,
        ..Input::default()
    });
    w.step(&Input::default());
}

/// What was said, line by line, and the dialogue put away.
fn said(w: &mut World) -> Vec<String> {
    w.ui.dialogue.take().map(|d| d.lines).unwrap_or_default()
}

#[test]
fn every_rumour_names_its_thing_s_one_route_and_the_place_it_is_found() {
    // worked by hand from data/items.json's routes, data/story.json's
    // pickups (the jacket at the scarecrow, the trousers at the hollow tree,
    // the boots at the shrine), camp.rs (the lookout's gloves, the leader's
    // sword and key, the chest's boomerang), story.json's trainer at the
    // hut, and travel.json (the traveller at the crossroads; the nearest
    // waystone, the village's, on the north lane)
    let want: [(&str, &str, Option<&str>); 12] = [
        ("hood", "merchant", None),
        ("jacket", "scarecrow", Some("scarecrow")),
        ("gloves", "lookout", Some("camp")),
        ("trousers", "hollow_tree", Some("hollow_tree")),
        ("boots", "river_shrine", Some("shrine")),
        ("sword", "leader_duel", Some("camp")),
        ("chest_key", "leader_drop", Some("camp")),
        ("boomerang", "camp_chest", Some("camp")),
        ("imbue", "trainer", Some("hut")),
        ("flute", "trainer_gift", Some("hut")),
        ("traveller", "ride", Some("crossroads")),
        ("waystone", "wake", Some("waystone_village")),
    ];
    assert_eq!(
        spec().rumours.len(),
        want.len(),
        "a rumour the table does not cover"
    );
    for (of, route, place) in want {
        let r = spec()
            .rumours
            .iter()
            .find(|r| r.of == of)
            .unwrap_or_else(|| panic!("no rumour of {of}"));
        assert_eq!(r.route, route, "{of}");
        assert_eq!(r.reveals.as_ref().map(|m| m.place.as_str()), place, "{of}");
        // the claim is the thing's own route, as the data that rules it says
        match Item::from_id(of) {
            Some(i) => {
                assert_eq!(
                    i.def().routes,
                    vec![r.route.clone()],
                    "{of}'s route in items.json"
                );
                assert!(
                    r.until.contains(&format!("has:{of}")),
                    "{of} is moot once in hand"
                );
            }
            None if of == "imbue" => {
                assert_eq!(catalogue().imbue.routes, vec![r.route.clone()]);
                assert!(r.until.contains(&"knows:imbue".to_string()));
            }
            None if of == "traveller" => {
                assert_eq!(Some(crate::travel::spec().traveller.place.as_str()), place);
            }
            None => {
                assert_eq!(of, "waystone");
                let p = place.unwrap();
                assert!(crate::travel::spec().points.iter().any(|q| q.place == p));
            }
        }
        // a thing that lies in the world lies at the place the rumour marks
        if let Some(i) = Item::from_id(of) {
            for p in &crate::story::spec().pickups {
                if p.item.as_deref() == Some(i.id()) {
                    assert_eq!(Some(p.place.as_str()), place, "{of} lies at {}", p.place);
                }
            }
        }
        // the mark's words exist
        if let Some(m) = &r.reveals {
            let s = crate::page::tests::strings();
            assert!(s.0.contains_key(&m.label), "{}", m.label);
            assert!(crate::places::at(&world(), &m.place).is_some(), "{}", m.place);
        }
    }
}

#[test]
fn every_item_with_a_route_of_its_own_has_a_rumour() {
    let mut want: Vec<(String, String)> = Item::ALL
        .iter()
        .map(|i| i.def())
        .filter(|d| d.routes.len() == 1 && d.routes[0] != "start")
        .map(|d| (d.id.clone(), d.routes[0].clone()))
        .collect();
    want.push(("imbue".into(), catalogue().imbue.routes[0].clone()));
    // by hand: the sword, the boomerang, the chest key, the five speedster's
    // pieces and the flute, and the imbue
    assert_eq!(want.len(), 10);
    for (of, route) in want {
        assert!(
            spec().rumours.iter().any(|r| r.of == of && r.route == route),
            "no rumour says {of} comes by {route}"
        );
    }
    // and every rumour has its words
    let s = crate::page::tests::strings();
    for r in &spec().rumours {
        assert!(s.0.contains_key(&format!("rumour_{}", r.id)), "{}", r.id);
    }
}

#[test]
fn a_rumour_is_offered_only_while_its_condition_holds() {
    let mut w = world();
    // by data/rumours.json: the villager on the north lane knows the hood
    // and the traveller
    assert_eq!(ids(&offered(&w, "north_villager")), ["hood", "traveller"]);
    give(&mut w, Item::Hood);
    assert_eq!(
        ids(&offered(&w, "north_villager")),
        ["traveller"],
        "the hood is in hand"
    );
    for p in &mut w.travel.points {
        p.reached = true;
    }
    assert!(
        offered(&w, "north_villager").is_empty(),
        "the traveller has nowhere left to go"
    );
    // the chest's key is moot with the key or with the boomerang
    assert_eq!(ids(&offered(&w, "fisher")), ["boots", "chest_key", "imbue"]);
    give(&mut w, Item::Boomerang);
    assert_eq!(ids(&offered(&w, "fisher")), ["boots", "imbue"]);
    // a line done makes its rumours moot: the trainer's, once the imbue is
    // learned and the flute in hand
    hear(&mut w, rumour("flute"));
    crate::imbue::learn(&mut w);
    give(&mut w, Item::Flute);
    assert!(w.rumours.line("trainer").unwrap().done);
    assert_eq!(ids(&offered(&w, "fisher")), ["boots"]);
    // the waystones' line done (each woken, though none counted as been
    // near): the traveller's rumour is moot with it
    let mut v = world();
    hear(&mut v, rumour("waystone"));
    for p in &mut v.travel.points {
        p.woken = true;
    }
    step(&mut v);
    assert!(v.travel.points.iter().all(|p| !p.reached));
    assert_eq!(ids(&offered(&v, "north_villager")), ["hood"]);
    // a rumour heard is not offered again, by anyone
    hear(&mut w, rumour("boots"));
    assert!(offered(&w, "fisher").is_empty());
    assert!(offered(&w, "trainer").iter().all(|r| r.id != "boots"));
}

#[test]
fn no_rumour_is_told_during_the_opening() {
    let mut w = crate::story::opening(3);
    crate::places::act(&mut w, "go", "village").unwrap();
    assert!(w.story.opening());
    for t in &spec().tellers {
        assert!(offered(&w, &t.id).is_empty(), "{} tells in the opening", t.id);
    }
    // a roadside villager says its own lines and nothing more
    talk_roadside(
        &mut w,
        "north_villager",
        &["roadside_north_1", "roadside_north_2"],
    );
    assert_eq!(said(&mut w), ["roadside_north_1", "roadside_north_2"]);
    assert!(w.rumours.heard.is_empty() && w.rumours.met.is_empty() && w.rumours.lines.is_empty());
    // the opening over, the same talk tells
    w.story.phase = Phase::Free;
    talk_roadside(
        &mut w,
        "north_villager",
        &["roadside_north_1", "roadside_north_2"],
    );
    assert_eq!(
        said(&mut w),
        ["roadside_north_1", "roadside_north_2", "rumour_hood"]
    );
}

#[test]
fn hearing_a_rumour_starts_its_line_and_marks_its_place() {
    let mut w = world();
    assert_eq!(
        tell(&mut w, "north_east_villager").as_deref(),
        Some("rumour_gloves")
    );
    assert_eq!(
        w.rumours.lines,
        [Line {
            id: "speedster".into(),
            latest: "gloves".into(),
            held: Vec::new(),
            done: false,
        }]
    );
    assert_eq!(w.rumours.marks, ["camp"]);
    assert_eq!(
        w.story.banner.as_ref().map(|b| b.key.as_str()),
        Some("rumour_marked")
    );
    // the next of the line's rumours becomes its latest; the camp is marked once
    assert_eq!(
        tell(&mut w, "north_east_villager").as_deref(),
        Some("rumour_trousers")
    );
    assert_eq!(w.rumours.lines.len(), 1);
    assert_eq!(w.rumours.lines[0].latest, "trousers");
    assert_eq!(w.rumours.marks, ["camp", "hollow_tree"]);
    // the hood marks nothing (the merchant's cart moves): the line takes it,
    // and a line over the game says where it went
    let mut v = world();
    hear(&mut v, rumour("hood"));
    assert!(v.rumours.marks.is_empty());
    assert_eq!(
        v.story.banner.as_ref().map(|b| b.key.as_str()),
        Some("rumour_noted")
    );
    // on the map: a question mark at the camp while it is unwalked
    let m = marks(&w);
    assert_eq!(m.len(), 2);
    assert!(m[0].unseen && m[0].label == "place_camp");
    let camp = crate::places::at(&w, "camp").unwrap();
    assert_eq!((m[0].at.x, m[0].at.z), (camp.x, camp.z));
    w.ui.fog.reveal(camp.x, camp.z);
    assert!(!marks(&w)[0].unseen, "walked, it is no longer a question");
}

#[test]
fn picking_up_the_item_advances_and_closes_the_line() {
    let mut w = world();
    hear(&mut w, rumour("sword"));
    assert_eq!(
        w.rumours.line("sword").map(|l| (l.held.len(), l.done)),
        Some((0, false))
    );
    give(&mut w, Item::Sword);
    assert_eq!(
        w.rumours.line("sword").map(|l| (l.held.len(), l.done)),
        Some((1, true))
    );
    assert_eq!(
        w.story.banner.as_ref().map(|b| b.key.as_str()),
        Some("line_sword_done")
    );
    // the speedster's armour, a piece at a time; a piece sold still counts
    hear(&mut w, rumour("hood"));
    give(&mut w, Item::Hood);
    assert_eq!(w.rumours.line("speedster").unwrap().held, ["has:hood"]);
    give(&mut w, Item::Jacket);
    crate::inventory::take(&mut w, Item::Hood, 1);
    step(&mut w);
    assert_eq!(
        w.rumours.line("speedster").unwrap().held,
        ["has:hood", "has:jacket"],
        "the hood was sold"
    );
    for i in [Item::Gloves, Item::Trousers] {
        give(&mut w, i);
    }
    assert!(!w.rumours.line("speedster").unwrap().done, "four of five");
    give(&mut w, Item::Boots);
    assert!(w.rumours.line("speedster").unwrap().done);
    // the chest: the key moves it on, the boomerang closes it (the key goes
    // into the lock)
    hear(&mut w, rumour("chest_key"));
    give(&mut w, Item::ChestKey);
    assert_eq!(
        w.rumours.line("chest").map(|l| (l.held.len(), l.done)),
        Some((1, false))
    );
    crate::inventory::take(&mut w, Item::ChestKey, 1);
    give(&mut w, Item::Boomerang);
    assert_eq!(
        w.rumours.line("chest").map(|l| (l.held.len(), l.done)),
        Some((2, true))
    );
    // a line that was never heard of does not open by itself
    crate::imbue::learn(&mut w);
    step(&mut w);
    assert!(w.rumours.line("trainer").is_none());
}

#[test]
fn a_person_s_talk_goes_through_what_they_know_and_ends_on_a_closing_line() {
    let mut w = world();
    // by data/rumours.json's order: the woodcutter knows the hood, the
    // gloves, the trousers and the sword
    before(&mut w, "woodcutter", 1);
    let mut heard = Vec::new();
    for _ in 0..6 {
        press_e(&mut w);
        heard.push(said(&mut w));
    }
    assert_eq!(
        heard,
        [
            vec!["woodcutter_hello", "rumour_hood"],
            vec!["rumour_gloves"],
            vec!["rumour_trousers"],
            vec!["rumour_sword"],
            vec!["rumour_woodcutter_done"],
            vec!["rumour_woodcutter_done"],
        ]
    );
    assert_eq!(w.rumours.met, ["woodcutter"]);
    // what one person told, another skips: the villager to the north-east
    // knows the gloves, the trousers and the boomerang
    let stop = w
        .roadside
        .stops
        .iter()
        .position(|(s, _)| s.id == "north_east_villager")
        .unwrap();
    crate::roadside::use_stop(&mut w, stop);
    assert_eq!(
        said(&mut w),
        [
            "roadside_north_east_1",
            "roadside_north_east_2",
            "rumour_boomerang"
        ]
    );
    crate::roadside::use_stop(&mut w, stop);
    assert_eq!(said(&mut w), ["rumour_villager_done"], "its own lines once");
    // the trainer, after the practice, then its own closing line
    w.trainer.taught = 1;
    w.trainer.stage = crate::trainer::Stage::Done;
    let mut t = Vec::new();
    for _ in 0..3 {
        crate::trainer::talk(&mut w);
        t.extend(said(&mut w));
    }
    assert_eq!(t, ["rumour_boots", "rumour_waystone", "trainer_done"]);
    // the merchant adds a rumour to its greeting while it has one
    crate::merchant::talk(&mut w);
    assert_eq!(said(&mut w), ["merchant_hello", "rumour_jacket"]);
}

#[test]
fn the_people_stand_clear_on_flattened_ground() {
    let w = world();
    let l = &w.land.layout;
    let ps = people(&w);
    assert!((2..=4).contains(&ps.len()));
    for p in &ps {
        let id = &p.teller.id;
        let place = crate::places::at(&w, &format!("{PLACE}{id}")).unwrap_or_else(|| panic!("{id}"));
        assert_eq!((place.x, place.z), (p.at.x, p.at.z));
        // the ground under them is the woodpile's flattened ground: the
        // same height as under the pile
        let logs = crate::places::metres([
            p.teller.at.unwrap()[0] + p.teller.logs[0],
            p.teller.at.unwrap()[1] + p.teller.logs[1],
        ]);
        assert_eq!(
            w.land.height(p.at.x, p.at.z),
            w.land.height(logs.x, logs.z),
            "{id} is off the flat"
        );
        // the ground is level round them: within 2 cm a metre each way
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let h = w.land.height(p.at.x + dx * ONE, p.at.z + dz * ONE);
            assert!(
                (h - w.land.height(p.at.x, p.at.z)).abs() < ONE / 50,
                "{id} stands on a slope"
            );
        }
        let room = fx::int(2);
        // off every road's surface, with a metre to spare
        for r in &l.roads {
            for k in 0..r.line.len() - 1 {
                let (a, b) = (
                    crate::places::metres(r.line[k]),
                    crate::places::metres(r.line[k + 1]),
                );
                let n = (b.sub(a).len_xz() / ONE).max(1);
                for q in 0..=n {
                    let at = a.lerp(b, q * ONE / n);
                    assert!(
                        at.sub(p.at).len_xz() > r.half_cm * ONE / 100 + room + ONE,
                        "{id} stands on a {}",
                        r.kind
                    );
                }
            }
        }
        assert!(!crate::terrain::is_water(&w, p.at.x, p.at.z), "{id} is in water");
        // out of every waystone's gap
        for pt in &crate::travel::spec().points {
            let at = crate::places::at(&w, &pt.place).unwrap();
            assert!(
                at.sub(p.at).len_xz() > fx::int(spec().keep_clear_m),
                "{id} by {}",
                pt.place
            );
        }
        // clear of every footprint but its own woodpile
        for q in l.places.iter().filter(|q| q.kind != format!("{PLACE}{id}")) {
            for s in &q.props {
                let Some(f) = l.footprints_m.get(&s.prop) else {
                    continue;
                };
                let at = crate::places::metres([q.centre[0] + s.at[0], q.centre[1] + s.at[1]]);
                assert!(
                    at.sub(p.at).len_xz() > fx::length(f[0] * ONE / 2, f[1] * ONE / 2) + room,
                    "{id} stands on {}'s {}",
                    q.kind,
                    s.prop
                );
            }
        }
    }
}

#[test]
fn the_quest_screen_lists_the_open_lines_then_those_done() {
    let mut g = crate::game::tests::game();
    let w = &mut g.world;
    hear(w, rumour("jacket"));
    hear(w, rumour("sword"));
    hear(w, rumour("imbue"));
    give(w, Item::Hood);
    give(w, Item::Sword);
    crate::ui::open(&mut g, crate::ui::QUEST);
    let u = serde_json::to_value(crate::api::ui(&g)).unwrap();
    let q = &u["menu"]["faces"][crate::ui::QUEST]["quest"];
    assert_eq!(q["lines_heading"], "Quest lines");
    let lines = q["lines"].as_array().unwrap();
    let row = |k: usize| {
        let l = &lines[k];
        (
            l["title"].as_str().unwrap().to_string(),
            l["progress"].as_str().unwrap().to_string(),
            l["done"].as_bool().unwrap(),
        )
    };
    assert_eq!(lines.len(), 3);
    assert_eq!(row(0), ("Speedster's armour".into(), "1 of 5".into(), false));
    assert_eq!(row(1), ("The trainer".into(), "0 of 2".into(), false));
    assert_eq!(row(2), ("The leader's sword".into(), "".into(), true));
    let s = crate::page::tests::strings();
    assert_eq!(lines[0]["text"], s.get("rumour_jacket"), "its latest rumour");
    assert_eq!(lines[2]["text"], s.get("line_sword_done"));
    assert_eq!(lines[2]["done_label"], "Done");
    // the map: a question mark at the scarecrow and at the hut
    crate::ui::open(&mut g, crate::ui::MAP_FACE);
    let u = serde_json::to_value(crate::api::ui(&g)).unwrap();
    let marks = u["menu"]["faces"][crate::ui::MAP_FACE]["map"]["marks"]
        .as_array()
        .unwrap()
        .clone();
    let q: Vec<_> = marks
        .iter()
        .filter(|m| m["icon"] == "data/icons/place_rumour.svg")
        .map(|m| m["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(q, ["rumour_scarecrow", "rumour_camp", "rumour_hut"]);
}

#[test]
fn the_same_seed_and_talk_give_the_same_world() {
    let run = |talk: bool| {
        let mut w = world();
        before(&mut w, "fisher", 1);
        for _ in 0..3 {
            if talk {
                press_e(&mut w);
            }
            w.ui.dialogue = None;
            for _ in 0..30 {
                w.step(&Input::default());
            }
        }
        w
    };
    let (a, b) = (run(true), run(true));
    assert_eq!(a, b);
    assert_eq!(a.hash(), b.hash());
    assert_eq!(a.rumours.heard, ["boots", "chest_key", "imbue"]);
    assert_ne!(
        run(false).hash(),
        a.hash(),
        "what was heard is in the world's one number"
    );
}

#[test]
fn the_rumours_file_refuses_what_it_cannot_mean() {
    let base: serde_json::Value =
        serde_json::from_str(include_str!("../../../../data/rumours.json")).unwrap();
    let with = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = base.clone();
        f(&mut v);
        Spec::parse(&v.to_string()).unwrap_err()
    };
    assert_eq!(
        with(&|v| v["rumours"][0]["line"] = "castle".into()),
        "The rumours file's rumour \"hood\" moves on a line there is none of: \"castle\"."
    );
    assert_eq!(
        with(&|v| v["rumours"][0]["until"][0] = "has:crown".into()),
        "The rumours file's rumour \"hood\" names a condition there is none of: \"has:crown\"."
    );
    assert!(with(&|v| v["rumours"][0]["told_by"][0] = "king".into()).contains("\"king\""));
}
