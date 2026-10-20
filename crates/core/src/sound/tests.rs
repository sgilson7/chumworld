use super::*;
use crate::blade::{Clash, Hit};
use crate::input::{Input, DRAW, FORWARD};

/// A bare world whose first tick has been heard, at tick 100.
fn heard_world() -> World {
    let mut w = World::new(3);
    w.sound.started = true;
    w.tick = 100;
    w
}

fn count_of(w: &World, name: &str) -> usize {
    let v = id(name).expect(name) as u16;
    w.sound.cues.iter().filter(|c| c.voice == v).count()
}

/// Every name the code starts a voice by is a row of `data/sound.json`, and
/// every row is started somewhere (or, for a loop, heard by `loop_sources`).
#[test]
fn every_sound_the_core_names_has_a_row() {
    let code = [include_str!("../sound.rs"), include_str!("../game.rs")].concat();
    let used = [
        "step_grass",
        "step_road",
        "step_wood",
        "step_floor",
        "draw",
        "sheathe",
        "swish",
        "clash",
        "hit",
        "fall",
        "imbue_on",
        "imbue_pulse",
        "imbue_off",
        "whoosh",
        "ping",
        "throw",
        "thunk",
        "bray",
        "shout",
        "tick",
        "blip",
        "chime",
        "dream",
        "free",
        "river",
        "wind",
        "fire",
        "cart",
        "whirr",
    ];
    for n in used {
        assert!(id(n).is_some(), "{n} has no row");
        assert!(
            code.contains(&format!("\"{n}\"")),
            "{n} is never named in the code"
        );
    }
    for name in spec().voices.keys() {
        assert!(used.contains(&name.as_str()), "{name} is a row no code starts");
    }
    assert_eq!(count(), used.len());
}

/// One clash, one clash cue. A second clash a tick later is inside the
/// clash's 4-tick gap and is not heard again; one 4 ticks after the first is.
#[test]
fn a_clash_is_heard_exactly_once_and_not_again_within_its_gap() {
    let mut w = heard_world();
    let clash = Clash {
        a: 0,
        b: 1,
        at: v3(fx::int(2000), ONE, fx::int(2830)),
        speed: fx::int(6),
    };
    w.blades.clashes = vec![clash];
    begin(&mut w);
    step(&mut w);
    assert_eq!(count_of(&w, "clash"), 1);
    for (tick, want) in [(101, 0), (103, 0), (104, 1)] {
        w.tick = tick;
        begin(&mut w);
        step(&mut w);
        assert_eq!(count_of(&w, "clash"), want, "tick {tick}");
    }
    // blades that only brush (under 1 m/s) make no sound
    w.tick = 200;
    w.blades.clashes = vec![Clash {
        speed: ONE - 1,
        ..clash
    }];
    begin(&mut w);
    step(&mut w);
    assert_eq!(count_of(&w, "clash"), 0);
}

/// A blow that takes health is one hit cue; one that takes none is silent.
#[test]
fn a_blow_that_lands_is_heard_once() {
    let mut w = heard_world();
    let hit = Hit {
        by: 1,
        on: 0,
        part: crate::blade::Part::Torso,
        speed: fx::int(9),
        at: v3(fx::int(2000), ONE, fx::int(2800)),
    };
    assert!(crate::health::damage(hit.speed, hit.part) > 0);
    w.blades.hits = vec![hit];
    begin(&mut w);
    step(&mut w);
    assert_eq!(count_of(&w, "hit"), 1);
    w.tick += 10;
    w.blades.hits = vec![Hit { speed: 0, ..hit }];
    begin(&mut w);
    step(&mut w);
    assert_eq!(count_of(&w, "hit"), 0);
}

/// X once: one draw as the blade starts to slide out, and nothing else of
/// the sword's until X again: then one sheathe as it starts to slide home.
#[test]
fn drawing_and_sheathing_are_each_heard_once() {
    let mut w = World::new(5);
    let (mut draws, mut sheathes) = (0, 0);
    for t in 0..240 {
        let keys = if t == 2 || t == 120 { DRAW } else { 0 };
        w.step(&Input {
            keys,
            ..Input::default()
        });
        draws += count_of(&w, "draw");
        sheathes += count_of(&w, "sheathe");
    }
    assert_eq!((draws, sheathes), (1, 1));
}

/// A foot comes down as its phase passes a quarter turn (the first foot)
/// or three quarters (the second), and only then.
#[test]
fn a_foot_lands_as_its_stride_passes_a_quarter_turn() {
    let q = fx::QUARTER_TURN;
    assert!(lands(q - 10, q + 10));
    assert!(lands(q - 10, q), "landing on the quarter itself");
    assert!(!lands(q, q + 10), "already down");
    assert!(!lands(q + 10, q + 20));
    assert!(lands(-q - 5, -q + 5), "the other foot");
    assert!(
        lands(fx::HALF_TURN + q - 3, fx::HALF_TURN + q + 3),
        "the other foot past the wrap"
    );
    assert!(!lands(0, 100));
}

/// What is underfoot: the house's floor, the bridge's planks, a road, grass.
#[test]
fn a_step_sounds_of_what_it_lands_on() {
    let w = World::new(1);
    let h = w.solid.home.unwrap();
    assert_eq!(underfoot(&w, h.wall.x, h.wall.z), "step_floor");
    let b = crate::places::prop_spots(&w, crate::models::Prop::Bridge)[0];
    assert_eq!(underfoot(&w, b.at.x, b.at.z), "step_wood");
    let road = crate::places::road(&w);
    let p = road.points[road.points.len() / 2];
    assert_eq!(underfoot(&w, p.x, p.z), "step_road");
    // the meadow 60 m west of the player's house
    assert_eq!(underfoot(&w, h.wall.x - fx::int(60), h.wall.z), "step_grass");
}

/// Walking, the player's feet are heard, every one on what it lands on.
#[test]
fn walking_the_feet_are_heard_on_what_they_land_on() {
    let mut w = World::new(1);
    let b = crate::places::prop_spots(&w, crate::models::Prop::Bridge)[0];
    let mut at = b.at;
    at.y = crate::solid::surface(&w, at.x, at.z);
    w.player = crate::player::Body::new(at, b.yaw);
    w.me = crate::player::Fighter::new(
        crate::pose::Kind::Player,
        crate::pose::Held::ShortSword,
        0,
        at,
        b.yaw,
        false,
    );
    w.aim.yaw = b.yaw;
    let wood = id("step_wood").unwrap() as u16;
    let mut steps = Vec::new();
    for _ in 0..90 {
        w.step(&Input {
            keys: FORWARD,
            ..Input::default()
        });
        steps.extend(
            w.sound
                .cues
                .iter()
                .filter(|c| c.voice < count() as u16)
                .map(|c| c.voice),
        );
    }
    let on_wood = steps.iter().filter(|v| **v == wood).count();
    assert!(on_wood >= 2, "{steps:?}");
}

fn camera_at(eye: V3, yaw: i32) -> Camera {
    Camera {
        eye,
        fwd: crate::geom::facing(yaw, 0),
        fov: crate::camera::FOV,
    }
}

/// By hand: the clash's row plays at 88%, its bus at 100% and the volume
/// at 100%: 65 536 × 88 / 100 = 57 671. Heard from 8 m (twice its 4 m
/// near), half that, 28 835, a little under for the rounding of the
/// distance; straight to the right of the camera, panned 70% right
/// (45 875), the full pan from twice the near distance on.
#[test]
fn a_sound_is_heard_by_its_distance_and_side() {
    let v = id("clash").unwrap();
    let cam = camera_at(V3::ZERO, 0); // facing −z: right is +x
    let (g, pan) = heard(v, Some(v3(fx::int(8), 0, 0)), &cam, 100);
    assert!((g - 28_835).abs() <= 2, "{g}");
    assert!((pan - 45_875).abs() <= 2, "{pan}");
    // to the left, panned left; behind, in the middle
    let (_, left) = heard(v, Some(v3(-fx::int(8), 0, 0)), &cam, 100);
    assert!((left + 45_875).abs() <= 2, "{left}");
    let (_, mid) = heard(v, Some(v3(0, 0, fx::int(8))), &cam, 100);
    assert!(mid.abs() <= 2, "{mid}");
    // past its 70 m reach, nothing
    assert_eq!(heard(v, Some(v3(fx::int(71), 0, 0)), &cam, 100).0, 0);
    // the reader's volume: half at 50%, nothing at 0
    let (half, _) = heard(v, Some(v3(fx::int(8), 0, 0)), &cam, 50);
    assert!((half - 14_417).abs() <= 2, "{half}");
    assert_eq!(heard(v, None, &cam, 0).0, 0);
    // a menu's tick is heard everywhere at its level: 45% on a 55% bus,
    // 65 536 × 45 / 100 × 55 / 100 = 16 220
    assert_eq!(heard(id("tick").unwrap(), None, &cam, 100), (16_220, 0));
}

/// Every voice is made to its row's peak, and none past full scale.
#[test]
fn every_voice_is_made_to_its_peak_and_never_clips() {
    for (v, (name, row)) in spec().voices.iter().enumerate() {
        let s = samples(v);
        assert_eq!(s.len(), synth::samples(row.ms, spec().rate), "{name}");
        let peak = s.iter().map(|x| x.abs()).max().unwrap();
        let want = ONE * row.peak_pct / 100;
        assert!((peak - want).abs() <= 1, "{name}: {peak} for {want}");
        assert!(peak < ONE, "{name} clips");
        if row.looped {
            // round the loop, the last sample steps to the first no more
            // than one sample steps to the next anywhere inside it
            let gap = (s[s.len() - 1] - s[0]).abs();
            let most = s.windows(2).map(|p| (p[1] - p[0]).abs()).max().unwrap();
            assert!(gap <= most, "{name} jumps {gap} at its seam, {most} inside");
        } else {
            assert_eq!(*s.last().unwrap(), 0, "{name} ends at nothing");
            assert_eq!(s[0], 0, "{name} starts at nothing");
        }
    }
}

/// A dialogue blips every third letter as it appears.
#[test]
fn a_dialogue_blips_every_third_letter() {
    assert!(blips(0, 1));
    assert!(!blips(1, 2));
    assert!(blips(2, 3));
    assert!(!blips(3, 3), "nothing new");
    assert!(blips(4, 9), "passing a third");
}

/// Paused, every loop is silent and nothing starts; playing again, the
/// wind is back.
#[test]
fn pausing_silences_every_loop_and_starts_nothing() {
    let mut g = crate::game::tests::game();
    g.act("story.skip", "").unwrap();
    g.act("screen.show", "play").unwrap();
    crate::game::tests::frames(&mut g, 30, FORWARD, 0);
    let wind = id("wind").unwrap() as i32 * ONE;
    let level = |g: &crate::game::Game| {
        g.loops
            .chunks(CUE_VALUES)
            .find(|c| c[0] == wind)
            .map(|c| c[1])
            .unwrap()
    };
    assert!(level(&g) > 0, "the wind blows in play");
    g.act("key.escape", "").unwrap();
    crate::game::tests::frames(&mut g, 2, FORWARD, 0);
    assert!(g.loops.chunks(CUE_VALUES).all(|c| c[1] == 0), "{:?}", g.loops);
    assert!(g.sounds.is_empty(), "{:?}", g.sounds);
    g.act("screen.show", "play").unwrap();
    crate::game::tests::frames(&mut g, 2, 0, 0);
    assert!(level(&g) > 0);
}

/// The menus tick: a press heard once, wherever the camera is.
#[test]
fn a_press_on_the_menus_ticks_once() {
    let mut g = crate::game::tests::game();
    g.act("screen.show", "play").unwrap();
    crate::game::tests::frames(&mut g, 1, 0, 0);
    g.act("menu.open", "").ok();
    crate::game::tests::frames(&mut g, 1, 0, 0);
    let tick = id("tick").unwrap() as i32 * ONE;
    let ticks = g.sounds.chunks(CUE_VALUES).filter(|c| c[0] == tick).count();
    assert_eq!(ticks, 1, "{:?}", g.sounds);
    crate::game::tests::frames(&mut g, 1, 0, 0);
    assert!(g.sounds.chunks(CUE_VALUES).all(|c| c[0] != tick), "heard once");
}

/// The dream's fade is heard once, as it falls.
#[test]
fn the_dream_s_fade_is_heard_once() {
    let mut w = crate::story::opening(4);
    for _ in 0..3 {
        w.step(&Input::default());
    }
    w.story.phase = crate::story::Phase::Raid;
    w.me.health.hp = 0;
    let mut dreams = 0;
    for _ in 0..(crate::story::FALL_SHOW + crate::story::FADE_TICKS + 4) {
        w.step(&Input::default());
        dreams += count_of(&w, "dream");
    }
    assert_eq!(dreams, 1);
}

/// The draw, quicker (M7): X, and the blade starts to slide out 11 ticks
/// later, heard then, and is out 18 ticks after the press (0.30 s, from
/// M1's 28).
#[test]
fn the_draw_is_heard_as_the_blade_slides_and_is_done_in_eighteen_ticks() {
    let mut w = World::new(5);
    let mut heard_at = None;
    let mut drawn_at = None;
    for t in 1..=40 {
        let keys = if t == 1 { DRAW } else { 0 };
        w.step(&Input {
            keys,
            ..Input::default()
        });
        if heard_at.is_none() && count_of(&w, "draw") == 1 {
            heard_at = Some(t);
        }
        if drawn_at.is_none() && w.me.sheath == crate::player::Sheath::Drawn {
            drawn_at = Some(t);
        }
    }
    assert_eq!(heard_at, Some(12), "11 ticks after the tick X was pressed");
    assert_eq!(drawn_at, Some(19), "18 ticks after it");
}
