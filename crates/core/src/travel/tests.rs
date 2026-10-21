//! The travel module's tests (M7 travel, T11). Expected values are worked
//! by hand from `data/travel.json` and `data/places.json`, never read back
//! from the function under test.

use super::*;
use crate::game::tests::{frames, game};
use crate::input::{ITEM_1, USE};

/// Points by their order in `data/travel.json`.
const VILLAGE: usize = 0;
const BRIDGE: usize = 1;
const HILL: usize = 2;
const SHRINE: usize = 3;
const CAMP: usize = 4;

/// The travel song as the file gives it: up, right, left, up, right, a.
const SONG: [&str; 6] = ["up", "right", "left", "up", "right", "a"];

/// After the opening, at the village centre, with the flute and its song.
fn free_world() -> World {
    let mut w = crate::story::opening(3);
    crate::story::step_aside(&mut w);
    w.inventory.add(Item::Flute, 1);
    w.travel.given = true;
    w
}

/// A game in play after the opening, the flute given, at the village centre.
fn playing() -> Game {
    let mut g = game();
    g.act("story.skip", "").unwrap();
    g.act("travel.give", "").unwrap();
    g.act("screen.show", "play").unwrap();
    frames(&mut g, 2, 0, 0);
    g
}

fn play_notes(g: &mut Game, notes: &[&str]) {
    for n in notes {
        g.act("travel.key", n).unwrap();
    }
}

fn json(g: &Game) -> serde_json::Value {
    serde_json::to_value(crate::api::ui(g)).unwrap()
}

// ---- the song ---------------------------------------------------------------

/// The song in the file is six notes, the staff gives each of the five
/// notes a line, and a file with a note the flute has not is refused.
#[test]
fn the_travel_file_gives_a_six_note_song_and_refuses_an_unknown_note() {
    let sp = spec();
    let ids: Vec<&str> = sp.song.iter().map(|n| n.id()).collect();
    assert_eq!(ids, SONG);
    // the staff, bottom to top: a, down, right, left, up (Ocarina of Time's)
    assert_eq!((Note::A.row(), Note::Down.row(), Note::Up.row()), (0, 1, 4));
    let mut v: serde_json::Value =
        serde_json::from_str(include_str!("../../../../data/travel.json")).unwrap();
    v["song"][2] = "sideways".into();
    assert_eq!(
        Spec::parse(&v.to_string()).unwrap_err(),
        "The travel file names a note \"sideways\" the flute has not."
    );
    let mut v: serde_json::Value =
        serde_json::from_str(include_str!("../../../../data/travel.json")).unwrap();
    v["staff"] = serde_json::json!(["a", "down", "right", "left"]);
    assert!(Spec::parse(&v.to_string())
        .unwrap_err()
        .contains("each of the five notes"));
}

/// The exact song opens the list of waystones once it has held on the staff
/// for `right_ticks` (50); a phrase with one wrong note, the sixth, fades
/// over `wrong_ticks` (45) and clears, and the list never shows.
#[test]
fn playing_the_exact_song_opens_the_list_and_a_wrong_note_does_not() {
    let mut g = playing();
    g.act("travel.wake", "hill").unwrap();
    g.act("travel.play", "").unwrap();
    assert_eq!(g.screen, Screen::Song);
    let held = g.world.tick;
    // five right notes: not yet judged
    play_notes(&mut g, &SONG[..5]);
    let s = g.world.travel.song.clone().unwrap();
    assert_eq!((s.played.len(), s.judged), (5, None));
    // a wrong sixth: judged wrong, and it fades and clears
    g.act("travel.key", "up").unwrap();
    assert_eq!(json(&g)["song"]["state"], "wrong");
    frames(&mut g, 44, 0, 0);
    assert_eq!(
        g.world.travel.song.as_ref().unwrap().played.len(),
        6,
        "still fading at 44"
    );
    frames(&mut g, 1, 0, 0);
    let s = g.world.travel.song.clone().unwrap();
    assert_eq!((s.phase, s.played.len(), s.judged), (Phase::Notes, 0, None));
    // a note while a phrase fades is not played: the staff is clear after
    // the fade, and the song from the start is right
    play_notes(&mut g, &SONG);
    assert_eq!(json(&g)["song"]["state"], "right");
    frames(&mut g, 49, 0, 0);
    assert_eq!(
        g.world.travel.song.as_ref().unwrap().phase,
        Phase::Notes,
        "held at 49"
    );
    frames(&mut g, 1, 0, 0);
    assert_eq!(
        g.world.travel.song.as_ref().unwrap().phase,
        Phase::Pick { ride: false }
    );
    assert_eq!(json(&g)["song"]["phase"], "pick");
    assert_eq!(g.world.tick, held, "the world held still while the flute played");
}

/// Each note sounds as it is played (an event for the sound table), and the
/// staff shows it at its line: the first note, up, on the top line (22% from
/// the top: 80% − 4 × 15%) and at the first of six places (10% + 80% ×
/// 1/12 = 16.66%).
#[test]
fn each_note_sounds_and_shows_on_the_staff_at_its_line() {
    let mut g = playing();
    g.act("travel.play", "").unwrap();
    let seq = g.world.travel.sound.seq;
    g.act("travel.key", "up").unwrap();
    assert_eq!(
        g.world.travel.sound,
        Sound {
            seq: seq + 1,
            id: "note_up"
        }
    );
    g.act("travel.key", "a").unwrap();
    assert_eq!(g.world.travel.sound.id, "note_a");
    let v = json(&g);
    let played = &v["song"]["played"];
    assert_eq!(played[0]["id"], "up");
    assert_eq!(played[0]["top"], "20.00%");
    assert_eq!(played[0]["left"], "16.66%");
    // a, the bottom line (80%), second of six (10% + 80% × 3/12 = 30%)
    assert_eq!(played[1]["top"], "80.00%");
    assert_eq!(played[1]["left"], "30.00%");
    assert_eq!(v["keymap"], "travel");
}

/// Esc puts the flute away: back to play, nothing travelled.
#[test]
fn escape_puts_the_flute_away() {
    let mut g = playing();
    g.act("travel.play", "").unwrap();
    play_notes(&mut g, &SONG[..3]);
    let at = g.world.player.pos;
    g.act("key.escape", "").unwrap();
    assert_eq!(g.screen, Screen::Play);
    assert_eq!(g.world.travel.song, None);
    assert_eq!(g.world.travel.sound.id, "flute_away");
    frames(&mut g, 120, 0, 0);
    assert_eq!(g.world.player.pos, at);
}

/// The flute on a quick key: pressing the key in play opens the song
/// screen; it is never held ready in the free hand (F would throw it).
#[test]
fn the_flute_s_quick_key_plays_it_and_never_readies_it() {
    let mut g = playing();
    g.world.inventory.assign(Item::Flute, 0).unwrap();
    frames(&mut g, 1, ITEM_1, 0);
    assert_eq!(g.screen, Screen::Song);
    assert_eq!(g.world.inventory.ready, None);
}

/// Enter on the flute in Select Item (its cell, 4) plays it.
#[test]
fn enter_on_the_flute_in_select_item_plays_it() {
    let mut g = playing();
    g.act("menu.open", "").unwrap();
    g.act("menu.choose", "4").unwrap();
    assert_eq!(g.screen, Screen::Song);
}

// ---- the list ----------------------------------------------------------------

/// The flute lists the woken waystones only, the traveller the ones not
/// yet been near: with the bridge and the camp woken and the shrine
/// reached, the flute lists 1 and 4 and the traveller offers 0 and 2.
#[test]
fn only_woken_waystones_are_listed_and_only_unreached_ones_offered() {
    let mut w = free_world();
    assert_eq!(listed(&w, false), Vec::<usize>::new());
    wake(&mut w, BRIDGE);
    wake(&mut w, CAMP);
    w.travel.points[SHRINE].reached = true;
    assert_eq!(listed(&w, false), vec![BRIDGE, CAMP]);
    assert_eq!(listed(&w, true), vec![VILLAGE, HILL]);
}

/// The song screen's list, after the song: the woken waystones by name,
/// at their places on the map (the bridge's waystone at (652, 2236) m:
/// 652 / 4000 = 16.30%, 2236 / 4000 = 55.90%).
#[test]
fn the_list_names_each_woken_waystone_at_its_place_on_the_map() {
    let mut g = playing();
    g.act("travel.wake", "bridge").unwrap();
    g.act("travel.play", "").unwrap();
    g.act("travel.song", "").unwrap();
    let v = json(&g);
    let rows = v["song"]["pick"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["label"], "The bridge");
    assert_eq!(
        (rows[0]["left"].as_str(), rows[0]["top"].as_str()),
        (Some("16.30%"), Some("55.90%"))
    );
    assert!(rows[0].get("price").is_none(), "the flute's journeys are free");
}

/// With nothing woken the list says how to wake one.
#[test]
fn with_no_waystone_woken_the_list_says_how_to_wake_one() {
    let mut g = playing();
    g.act("travel.play", "").unwrap();
    g.act("travel.song", "").unwrap();
    let v = json(&g);
    assert_eq!(v["song"]["pick"]["rows"].as_array().unwrap().len(), 0);
    assert_eq!(
        v["song"]["pick"]["empty"],
        "No waystone is awake yet. Walk up to one and press E to wake it."
    );
}

// ---- refusals ---------------------------------------------------------------

/// No travel in the opening: in bed, during the raid, the flute plays no
/// song, and the line says to get past the village's edge.
#[test]
fn travel_is_refused_during_the_opening_with_its_line() {
    let mut w = crate::story::opening(3);
    w.inventory.add(Item::Flute, 1);
    assert_eq!(refusal(&w), Some("travel_refuse_opening"));
    request(&mut w);
    assert_eq!(w.travel.song, None);
    assert_eq!(
        w.story.banner.as_ref().map(|b| b.key.as_str()),
        Some("travel_refuse_opening")
    );
    let s = crate::page::parse_strings(include_str!("../../../../data/strings.json")).unwrap();
    assert_eq!(
        s.get("travel_refuse_opening"),
        "The flute cannot carry you out of the raid. Run past the village's edge first."
    );
}

/// No travel with a foe on you: a test duel's orc, its blade out, a few
/// metres off; past the 32 m, or with its blade away, and holding on no
/// one, travel may go.
#[test]
fn travel_is_refused_in_a_duel_and_allowed_once_clear() {
    let mut w = free_world();
    w.act("duel.spawn", "orc").unwrap();
    let id = w.foes.len();
    {
        let f = &mut w.foes[id - 1];
        f.fighter.sheath = crate::player::Sheath::Drawn;
    }
    assert!(crate::player::fighter(&w, id).drawn());
    assert_eq!(refusal(&w), Some("travel_refuse_duel"));
    request(&mut w);
    assert_eq!(w.travel.song, None);
    assert_eq!(
        w.story.banner.as_ref().map(|b| b.key.as_str()),
        Some("travel_refuse_duel")
    );
    // not holding on the player: 31 m off with its blade out, still
    // refused; 33 m off, blade out, clear (past the 32 m); 5 m off with its
    // blade away, clear
    let me = w.player.pos;
    w.foes[id - 1].fighter.engaged = None;
    w.foes[id - 1].body.pos = me.add(v3(fx::int(31), 0, 0));
    assert_eq!(refusal(&w), Some("travel_refuse_duel"));
    w.foes[id - 1].body.pos = me.add(v3(fx::int(33), 0, 0));
    assert_eq!(refusal(&w), None);
    w.foes[id - 1].body.pos = me.add(v3(fx::int(5), 0, 0));
    w.foes[id - 1].fighter.sheath = crate::player::Sheath::Sheathed;
    assert_eq!(refusal(&w), None);
    // a foe holding on the player is a duel however far
    w.foes[id - 1].fighter.engaged = Some(0);
    assert_eq!(refusal(&w), Some("travel_refuse_duel"));
}

/// No travel mid-dialogue: the flute asked for while someone talks says to
/// finish talking first.
#[test]
fn travel_is_refused_mid_dialogue_with_its_line() {
    let mut g = playing();
    crate::ui::say(&mut g.world, "trainer", &["trainer_done"]);
    frames(&mut g, 1, 0, 0);
    assert_eq!(g.screen, Screen::Dialogue);
    assert_eq!(refusal(&g.world), Some("travel_refuse_talk"));
    g.act("travel.play", "").unwrap();
    assert_eq!(g.screen, Screen::Dialogue);
    assert_eq!(g.world.travel.song, None);
    assert_eq!(
        g.world.story.banner.as_ref().map(|b| b.key.as_str()),
        Some("travel_refuse_talk")
    );
    assert_eq!(
        g.strings.get("travel_refuse_talk"),
        "Finish talking first, then play."
    );
}

/// From Select Item, a refusal shows under the flute.
#[test]
fn a_refusal_in_select_item_shows_under_the_flute() {
    let mut g = playing();
    g.act("duel.spawn", "orc").unwrap();
    let n = g.world.foes.len();
    g.world.foes[n - 1].fighter.sheath = crate::player::Sheath::Drawn;
    g.act("menu.open", "").unwrap();
    g.act("menu.choose", "4").unwrap();
    assert_eq!(g.screen, Screen::Menu);
    assert_eq!(
        g.menu.note.as_deref(),
        Some("A foe is on you. Get clear of the fight, then play.")
    );
}

// ---- arrival -----------------------------------------------------------------

/// Arriving: 4 m in front of the waystone (400 cm, 262 144 units), facing
/// the way it faces, on what is underfoot there, on dry ground, outside
/// every wall. The village's waystone at (2000, 2540) m faces north (yaw 0,
/// the front −z): the player stands at (2000, 2536).
#[test]
fn arrival_is_in_front_of_the_waystone_on_dry_ground_outside_any_wall() {
    let w = free_world();
    let (p, yaw) = arrival(&w, VILLAGE).unwrap();
    assert_eq!((p.x, p.z, yaw), (2000 * ONE, 2536 * ONE, 0));
    for k in 0..5 {
        let (p, yaw) = arrival(&w, k).unwrap();
        let (s, sy) = stone(&w, k).unwrap();
        assert_eq!(yaw, sy, "facing the way the waystone faces");
        let d = p.sub(s).len_xz();
        assert!((d - 262_144).abs() <= 2, "{k}: {d}");
        assert_eq!(
            p.y,
            crate::solid::surface(&w, p.x, p.z),
            "{k}: on what is underfoot"
        );
        assert!(!crate::terrain::is_water(&w, p.x, p.z), "{k}: arrives in water");
        for wall in &w.solid.walls {
            assert!(
                wall.push(p.x, p.z, crate::solid::RADIUS).is_none(),
                "{k}: arrives inside a wall at {p:?}"
            );
        }
    }
}

/// A journey: the view fades out over 80 ticks, the player is put at the
/// arrival on the 80th, the fade holds 20 more, then lifts; the keys do
/// nothing meanwhile.
#[test]
fn a_journey_fades_out_moves_the_player_and_fades_back() {
    let mut w = free_world();
    wake(&mut w, HILL);
    w.travel.warp = Some(Warp {
        to: HILL,
        ticks: 0,
        ride: false,
    });
    let start = w.player.pos;
    let (to, yaw) = arrival(&w, HILL).unwrap();
    let run = Input {
        keys: crate::input::FORWARD | crate::input::RUN,
        ..Input::default()
    };
    w.step(&run);
    assert!(w.story.veil, "the fade begins");
    for _ in 0..79 {
        w.step(&run);
    }
    assert_eq!(w.player.pos.sub(start).len_xz(), 0, "held still while it darkens");
    w.step(&run);
    assert_eq!((w.player.pos, w.player.facing, w.aim.yaw), (to, yaw, yaw));
    for _ in 0..20 {
        w.step(&run);
    }
    assert!(!w.story.veil && w.travel.warp.is_none(), "the fade lifts");
}

/// Choosing a waystone from the list travels there.
#[test]
fn choosing_a_waystone_travels_there() {
    let mut g = playing();
    g.act("travel.wake", "shrine").unwrap();
    g.act("travel.play", "").unwrap();
    play_notes(&mut g, &SONG);
    frames(&mut g, 50, 0, 0);
    g.act("travel.key", "choose").unwrap();
    assert_eq!(g.screen, Screen::Play);
    frames(&mut g, 110, 0, 0);
    let (to, _) = arrival(&g.world, SHRINE).unwrap();
    assert_eq!((g.world.player.pos.x, g.world.player.pos.z), (to.x, to.z));
}

// ---- waking --------------------------------------------------------------------

/// E beside a dormant waystone wakes it: it glows, and its mark appears on
/// the map (seven marks before, eight after, the eighth the hill's).
#[test]
fn e_wakes_a_waystone_and_its_mark_appears_on_the_map() {
    let mut g = playing();
    let (s, _) = stone(&g.world, HILL).unwrap();
    // 1.4 m in front of it, facing it
    g.act("places.go", &format!("{},{}", fx::floor(s.x) + 1, fx::floor(s.z)))
        .unwrap();
    frames(&mut g, 2, 0, 0);
    // the seven places and (M7 roadside, T12) the roadside's marks after them
    // (M7 save, T16: and the save points' posts)
    let roadside = crate::roadside::marks(&g.world).len() + crate::save::marks(&g.world).len();
    assert_eq!(crate::places::markers(&g.world).len(), 7 + roadside);
    assert!(matches!(
        crate::story::nearest_use(&g.world),
        Some((crate::story::Use::Waystone(HILL), _))
    ));
    let before = g.world.travel.sound.seq;
    frames(&mut g, 1, USE, 0);
    frames(&mut g, 1, 0, 0);
    assert!(g.world.travel.points[HILL].woken);
    assert_eq!(
        g.world.travel.sound,
        Sound {
            seq: before + 1,
            id: "waystone_wake"
        }
    );
    let m = crate::places::markers(&g.world);
    assert_eq!(m.len(), 8 + roadside);
    assert_eq!(m[7].0, "waystone_hill");
    // E there now does nothing more to it
    assert!(crate::travel::wake_reach(&g.world).is_none());
}

/// A dormant waystone farther than E reaches is not offered: 3 m off is
/// past 0.5 m of footprint and 1.2 m of reach.
#[test]
fn a_waystone_out_of_reach_is_not_offered() {
    let mut w = free_world();
    let (s, _) = stone(&w, HILL).unwrap();
    let at = s.add(v3(fx::int(3), 0, 0));
    w.player = crate::player::Body::new(at, 0);
    assert_eq!(wake_reach(&w), None);
    w.player = crate::player::Body::new(s.add(v3(ONE * 17 / 10, 0, 0)), 0);
    assert_eq!(wake_reach(&w).map(|(k, _)| k), Some(HILL));
}

/// Coming within 20 m of a waystone counts as having been there.
#[test]
fn coming_within_twenty_metres_counts_as_having_been_there() {
    let mut w = free_world();
    let (s, _) = stone(&w, BRIDGE).unwrap();
    w.player = crate::player::Body::new(s.add(v3(fx::int(21), 0, 0)), 0);
    w.step(&Input::default());
    assert!(!w.travel.points[BRIDGE].reached, "21 m");
    w.player = crate::player::Body::new(s.add(v3(fx::int(19), 0, 0)), 0);
    w.step(&Input::default());
    assert!(w.travel.points[BRIDGE].reached, "19 m");
    assert!(!w.travel.points[BRIDGE].woken, "but not woken");
}

// ---- the gift, the dialogue, the quest -------------------------------------------

/// The trainer gives the flute once the practice is passed, and the talk
/// teaches the song: its second line shows the six notes.
#[test]
fn the_trainer_gives_the_flute_and_teaches_the_song_with_its_notes() {
    let mut g = playing();
    g.world.inventory.remove(Item::Flute, 1);
    g.world.travel.given = false;
    g.world.trainer.taught = 1;
    crate::imbue::learn(&mut g.world);
    frames(&mut g, 1, 0, 0);
    assert!(g.world.inventory.has(Item::Flute));
    assert_eq!(g.screen, Screen::Dialogue);
    let lines = g.world.ui.dialogue.as_ref().unwrap().lines.clone();
    assert_eq!(
        lines,
        vec!["trainer_flute_1", "trainer_flute_2", "trainer_flute_3"]
    );
    assert!(
        json(&g)["dialogue"]["song"].is_null(),
        "the first line shows no notes"
    );
    // to the second line (the first shown whole, then next)
    g.act("dialogue.next", "").unwrap();
    g.act("dialogue.next", "").unwrap();
    let notes = json(&g)["dialogue"]["song"]["notes"].clone();
    let ids: Vec<&str> = notes
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, SONG);
    assert_eq!(crate::ui::task(&g.world), "quest_flute");
}

/// The quest screen shows the song learned with its notes, and the quest
/// line moves on to the camp once a waystone is woken.
#[test]
fn the_quest_screen_shows_the_song_and_the_line_moves_on_once_one_is_woken() {
    let mut g = playing();
    crate::imbue::learn(&mut g.world);
    assert_eq!(crate::ui::task(&g.world), "quest_flute");
    g.act("menu.open", "").unwrap();
    g.act("menu.turn", "1").unwrap();
    g.act("menu.turn", "1").unwrap();
    let v = json(&g);
    let q = &v["menu"]["faces"][2]["quest"];
    assert_eq!(q["song"]["name"], "Travel song");
    assert_eq!(q["song"]["notes"].as_array().unwrap().len(), 6);
    g.act("menu.close", "").unwrap();
    g.act("travel.wake", "hill").unwrap();
    assert_eq!(crate::ui::task(&g.world), "quest_camp");
}

// ---- the traveller -------------------------------------------------------------

/// Prices by hand: the traveller stands at the crossroads (2000, 2250)
/// and (−9, 9) from it, (1991, 2259) m. Each price is 10 coins and one a
/// whole 80 m, rounded down:
/// - the village's waystone (2000, 2540): √(9² + 281²) = 281.1 m, 13;
/// - the bridge's (652, 2236): √(1339² + 23²) = 1 339.2 m, 26;
/// - the hill's (324, 1764): √(1667² + 495²) = 1 738.9 m, 31;
/// - the shrine's (735, 997): √(1256² + 1262²) = 1 780.5 m, 32;
/// - the camp's (3157, 1140): √(1166² + 1119²) = 1 616.1 m, 30.
///
/// All between 10 and 40, and all affordable with the 84 coins the village
/// and the camp leave (12, the lookout's 12, 3 × 12 and the leader's 24).
#[test]
fn each_ride_is_priced_by_its_distance() {
    let w = free_world();
    let prices: Vec<u32> = (0..5).map(|k| price(&w, k)).collect();
    assert_eq!(prices, vec![13, 26, 31, 32, 30]);
    assert_eq!(
        (price_for(0), price_for(fx::int(80)), price_for(fx::int(9000))),
        (10, 11, 40)
    );
    let sp = crate::story::spec();
    let coins: u32 = sp.pickups.iter().filter_map(|p| p.coins).sum::<u32>()
        + sp.camp.bandit_coins * (1 + sp.camp.bandits.len() as u32)
        + sp.camp.leader_coins;
    assert_eq!(coins, 84);
    assert!(prices.iter().all(|p| *p <= coins));
}

/// Talking to the traveller, then a ride: refused for want of a coin, then
/// paid exactly, a line, the journey, and the waystone woken on arrival;
/// the traveller no longer offers it.
#[test]
fn the_traveller_charges_exactly_refuses_short_and_the_ride_wakes_the_waystone() {
    let mut g = playing();
    let ((t, _), _) = traveller(&g.world).unwrap();
    g.act("places.go", &format!("{},{}", fx::floor(t.x) - 2, fx::floor(t.z)))
        .unwrap();
    frames(&mut g, 2, 0, 0);
    assert!(matches!(
        crate::story::nearest_use(&g.world),
        Some((crate::story::Use::Traveller, _))
    ));
    g.world.inventory.coins = 29;
    frames(&mut g, 1, USE, 0);
    assert_eq!(g.screen, Screen::Dialogue);
    assert_eq!(
        g.world.ui.dialogue.as_ref().unwrap().lines,
        // M7 rumours (T17): then the first rumour the traveller knows
        vec!["traveller_hello", "rumour_trousers"]
    );
    g.act("dialogue.next", "").unwrap();
    g.act("dialogue.next", "").unwrap();
    g.act("dialogue.next", "").unwrap();
    g.act("dialogue.next", "").unwrap();
    frames(&mut g, 1, 0, 0);
    assert_eq!(g.screen, Screen::Song);
    // every waystone is offered: none has been near
    assert_eq!(listed(&g.world, true), vec![0, 1, 2, 3, 4]);
    let v = json(&g);
    assert_eq!(v["song"]["pick"]["rows"][4]["price"], "30 coins");
    assert_eq!(v["song"]["pick"]["rows"][4]["enabled"], false);
    // the camp's waystone, 30 coins, with 29: refused, a coin short
    g.act("travel.choose", "4").unwrap();
    assert_eq!(g.world.inventory.coins, 29);
    assert_eq!(json(&g)["song"]["pick"]["note"], "That ride needs 1 more coin.");
    assert_eq!(g.screen, Screen::Song);
    // with 75: paid exactly 30
    g.world.inventory.coins = 75;
    g.act("travel.choose", "4").unwrap();
    assert_eq!(g.world.inventory.coins, 45);
    assert_eq!(g.world.travel.sound.id, "ride_pay");
    assert_eq!(g.screen, Screen::Dialogue);
    assert_eq!(
        g.world.ui.dialogue.as_ref().unwrap().lines,
        vec!["traveller_ride"]
    );
    g.act("dialogue.next", "").unwrap();
    g.act("dialogue.next", "").unwrap();
    frames(&mut g, 110, 0, 0);
    let (to, _) = arrival(&g.world, CAMP).unwrap();
    assert_eq!((g.world.player.pos.x, g.world.player.pos.z), (to.x, to.z));
    assert!(g.world.travel.points[CAMP].woken, "the ride wakes the waystone");
    assert_eq!(
        g.world.story.banner.as_ref().map(|b| b.key.as_str()),
        Some("traveller_arrived")
    );
    // been there: never offered again, and the flute lists it
    assert_eq!(listed(&g.world, true), vec![0, 1, 2, 3]);
    assert_eq!(listed(&g.world, false), vec![CAMP]);
}

/// When the player has been near every waystone, the traveller says so
/// and offers no list.
#[test]
fn the_traveller_offers_nothing_once_every_place_is_reached() {
    let mut w = free_world();
    for p in w.travel.points.iter_mut() {
        p.reached = true;
    }
    talk(&mut w);
    // M7 rumours (T17): then the first rumour the traveller knows
    assert_eq!(
        w.ui.dialogue.as_ref().unwrap().lines,
        vec!["traveller_nowhere", "rumour_trousers"]
    );
    assert_eq!(w.travel.song, None);
}

// ---- determinism, drawing ---------------------------------------------------------

/// CW-4: a recorded journey (the flute played, the song, a waystone
/// chosen, the journey run out) hashes the same every run, and another
/// destination hashes differently.
#[test]
fn a_recorded_journey_hashes_the_same_every_run() {
    let run = |to: &str| {
        let mut g = playing();
        g.act("travel.wake", "all").unwrap();
        frames(&mut g, 30, crate::input::FORWARD, 40);
        g.act("travel.play", "").unwrap();
        play_notes(&mut g, &SONG);
        frames(&mut g, 50, 0, 0);
        g.act("travel.choose", to).unwrap();
        frames(&mut g, 130, crate::input::FORWARD, 0);
        g.world.hash()
    };
    assert_eq!(run("2"), run("2"));
    assert_ne!(run("2"), run("3"));
}

/// CW-18: a woken waystone in view draws the same instance array tick
/// after tick: its light does not pulse or move.
#[test]
fn a_woken_waystone_in_view_draws_the_same_frame_tick_after_tick() {
    let mut g = game();
    g.act("story.skip", "").unwrap();
    g.act("story.still", "on").unwrap();
    g.act("travel.wake", "village").unwrap();
    // 8 m north of the waystone, looking back at it (yaw 180: south)
    g.act("places.go", "2000,2532,180").unwrap();
    g.act("screen.show", "play").unwrap();
    frames(&mut g, 120, 0, 0);
    let f1 = g.frame.clone();
    frames(&mut g, 1, 0, 0);
    assert_eq!(g.frame, f1);
    // and its light is drawn: the glow's marks, unlit, are in the array
    let glow = models::table().c("waystone_glow");
    let mut shapes = Shapes::default();
    draw(&g.world, 0, &g.look, &mut shapes);
    assert!(shapes.list.iter().filter(|s| s.rgb == glow).count() >= 9);
}

/// A dormant waystone draws no light.
#[test]
fn a_dormant_waystone_draws_no_light() {
    let w = free_world();
    let mut shapes = Shapes::default();
    let look = crate::look::Look::parse(include_str!("../../../../data/look.json")).unwrap();
    draw(&w, 0, &look, &mut shapes);
    let glow = models::table().c("waystone_glow");
    assert_eq!(shapes.list.iter().filter(|s| s.rgb == glow).count(), 0);
}
