//! `trainer` (M5 Places and story, task S): the trainer in the hut on the
//! hill west of the river, and the practice that teaches the imbue (T2).
//!
//! The person: "you don't start with deflect, you have to learn it from a
//! trainer, who makes you practice it with them". The practice, step by
//! step:
//!
//! M7 (task P1): the practice is staged at a distance and taught in steps.
//! After the talk the trainer points to a mark on the ground `mark_m` (8 m)
//! in front of them (`mark`), drawn as a bright ring while the practice
//! lasts; the trainer begins once the player stands on it with the sword
//! drawn, looking toward them, so the orbs are seen coming. The first volley is one orb
//! (`Pattern::PracticeOne`), the next three (`PracticeFew`), then the
//! practice pattern itself (`Practice`); a step is passed once an orb of it
//! is turned, each with its line, and repeated otherwise.
//!
//! 1. E by the trainer: three lines (who they are, what the imbue does,
//!    "draw your sword"). From here the trainer lends the imbue: the blade
//!    is imbued while the practice lasts (`lending`, which
//!    is M6's `imbue::lend`, topped up each tick), though Q is not yet the
//!    player's.
//! 2. The player draws (X). The trainer casts the practice pattern
//!    (`orbs::cast(…, Pattern::Practice)`, M6's door).
//! 3. Each orb the player's blade turns counts (`orbs::deflected`). When a
//!    volley is spent short of `TO_PASS` turned, the trainer says so and,
//!    after `BETWEEN_TICKS`, casts again; the count carries over.
//! 4. At `TO_PASS` turned in all, the practice is passed: `imbue::learn` is
//!    called, once, the trainer says the imbue is the player's now, and a
//!    line over the game says Q imbues the sword.
//!
//! The trainer is a fighter (so M6's orbs fly from them) on the player's
//! side: no one holds on them, and they hold on no one.

use crate::fx::{self, Fx, ONE};
use crate::geom::{yaw_of, V3};
use crate::input::Input;
use crate::orbs::{self, Pattern};
use crate::pilot::Control;
use crate::player::{self, Sheath};
use crate::pose::Kind;
use crate::story;
use crate::world::World;

/// Orbs turned to pass the practice.
pub const TO_PASS: u32 = 5;
/// The trainer stands once the player is this near the hut.
pub const SPAWN_AT: Fx = fx::int(260);
/// E talks to the trainer from this near.
pub const TALK_FROM: Fx = ONE * 25 / 10;
/// A volley is over when no orb of it flies, or after this long at most
/// (15 s).
pub const VOLLEY_MOST: u32 = 900;
/// A volley is never over before this (the orbs leave the trainer's hand).
pub const VOLLEY_LEAST: u32 = 60;
/// The pause before the trainer casts again (2 s).
pub const BETWEEN_TICKS: u32 = 120;
/// Walk this far from the trainer and the practice waits for you to come
/// back and talk again.
pub const AWAY: Fx = fx::int(30);

/// Where the practice stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// not yet talked to
    Waiting,
    /// talked to: the imbue is lent; waiting for the sword to be drawn
    Ready,
    /// a volley in the air, ticks since the cast
    Volley(u32),
    /// a volley spent short of the count, ticks since
    Between(u32),
    /// passed: the imbue is the player's
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub foe: Option<usize>,
    pub stage: Stage,
    /// orbs turned in volleys already spent
    pub turned: u32,
    pub volleys: u32,
    /// how many times `imbue::learn` has been called (once)
    pub taught: u32,
    /// test-only: orbs counted as turned without M6's orbs (`trainer.credit`)
    pub credited: u32,
    /// M7: the practice's step: one orb, a few, the volley
    pub step: u8,
}

impl Default for State {
    fn default() -> State {
        State {
            foe: None,
            stage: Stage::Waiting,
            turned: 0,
            volleys: 0,
            taught: 0,
            credited: 0,
            step: 0,
        }
    }
}

/// The trainer's fighter id, if they stand.
pub fn id(w: &World) -> Option<usize> {
    w.trainer.foe.map(|k| k + 1).filter(|&id| id < player::count(w))
}

/// A test duel forgets the trainer.
pub fn forget(w: &mut World) {
    w.trainer.foe = None;
}

/// Whether the trainer lends the imbue now: from the first talk until the
/// practice is passed.
pub fn lending(w: &World) -> bool {
    matches!(
        w.trainer.stage,
        Stage::Ready | Stage::Volley(_) | Stage::Between(_)
    )
}

/// Orbs turned so far in the practice: spent volleys and this one.
pub fn turned(w: &World) -> u32 {
    let now = match (w.trainer.stage, id(w)) {
        (Stage::Volley(_), Some(id)) => orbs::deflected(w, id),
        _ => 0,
    };
    w.trainer.turned + now + w.trainer.credited
}

fn spawn(w: &mut World) {
    let at = story::near_place(w, "hut", story::spec().trainer.at);
    let hut = crate::places::at(w, "hut").unwrap_or(at);
    let yaw = yaw_of(at.sub(hut));
    let k = story::sheathed_foe(w, Kind::Bandit, Kind::Trainer, at, yaw);
    let foe = &mut w.foes[k];
    foe.fighter.team = 0;
    foe.fighter.held = crate::pose::Held::Sword;
    foe.fighter.health = crate::health::Health::new(100_000);
    foe.brain.control = Control::Still;
    w.trainer.foe = Some(k);
}

/// Step 2b: the trainer stands (held still by their pilot).
pub fn steer(w: &mut World, inputs: &mut [Input]) {
    if let Some(k) = w.trainer.foe {
        if let Some(i) = inputs.get_mut(k) {
            *i = Input::default();
        }
    }
}

/// Step 7: the trainer stands when the player comes near; the practice.
pub fn step(w: &mut World) {
    if w.trainer.foe.is_none() {
        if w.story.phase != story::Phase::Off {
            if let Some(hut) = crate::places::at(w, "hut") {
                if hut.sub(w.player.pos).len_xz() < SPAWN_AT {
                    spawn(w);
                }
            }
        }
        return;
    }
    let Some(id) = id(w) else {
        return;
    };
    practice(w, id);
    // M6's door: the lent imbue holds from tick to tick while the practice
    // lasts (it wears off a tick at a time in `imbue::step`)
    if lending(w) {
        crate::imbue::lend(w, LEND_TICKS);
    }
}

/// The lent imbue is topped up to this each tick of the practice.
pub const LEND_TICKS: u32 = 2;

fn practice(w: &mut World, id: usize) {
    let gap = player::body(w, id).pos.sub(w.player.pos).len_xz();
    let stage = w.trainer.stage;
    if matches!(stage, Stage::Ready | Stage::Volley(_) | Stage::Between(_)) && gap > AWAY {
        w.trainer.turned = turned(w) - w.trainer.credited;
        w.trainer.stage = Stage::Waiting;
        crate::imbue::lend(w, 0);
        return;
    }
    match stage {
        Stage::Waiting | Stage::Done => {}
        Stage::Ready => {
            if w.me.sheath == Sheath::Drawn && ready_on_mark(w) {
                if w.trainer.step == 0 && w.trainer.volleys == 0 {
                    crate::ui::say(w, "trainer", &["trainer_one"]);
                }
                cast(w, id);
            }
        }
        Stage::Volley(t) => {
            if turned(w) >= TO_PASS {
                pass(w);
            } else if (t >= VOLLEY_LEAST && !orbs::flying(w, id)) || t >= VOLLEY_MOST {
                let got = orbs::deflected(w, id);
                w.trainer.turned += got;
                w.trainer.stage = Stage::Between(0);
                // M7: a step passed on one orb turned, the next step's line
                let line = match (got > 0, w.trainer.step) {
                    (true, 0) => "trainer_few",
                    (true, 1) => "trainer_volley",
                    _ => "trainer_again",
                };
                if got > 0 {
                    w.trainer.step = (w.trainer.step + 1).min(STEPS - 1);
                }
                crate::ui::say(w, "trainer", &[line]);
            } else {
                w.trainer.stage = Stage::Volley(t + 1);
            }
        }
        Stage::Between(t) => {
            if turned(w) >= TO_PASS {
                pass(w);
            } else if t + 1 >= BETWEEN_TICKS {
                cast(w, id);
            } else {
                w.trainer.stage = Stage::Between(t + 1);
            }
        }
    }
}

/// M7: the practice's steps: one orb, then three, then the practice pattern.
pub const STEPS: u8 = 3;

fn cast(w: &mut World, id: usize) {
    w.trainer.volleys += 1;
    w.trainer.stage = Stage::Volley(0);
    let pattern = match w.trainer.step {
        0 => Pattern::PracticeOne,
        1 => Pattern::PracticeFew,
        _ => Pattern::Practice,
    };
    orbs::cast(w, id, pattern);
}

/// M7: the mark the player stands on for the practice, in front of the
/// trainer, on the ground.
pub fn mark(w: &World) -> Option<V3> {
    let id = id(w)?;
    let t = player::body(w, id);
    let sp = &story::spec().trainer;
    let mut p = t
        .pos
        .add(crate::geom::facing(t.facing, 0).scale(fx::int(sp.mark_m)));
    p.y = crate::solid::surface(w, p.x, p.z);
    Some(p)
}

/// M7: whether the player stands on the mark looking toward the trainer
/// (within `FACING` either way), so the orbs are seen coming.
pub fn ready_on_mark(w: &World) -> bool {
    let Some(t) = id(w).map(|k| player::body(w, k).pos) else {
        return false;
    };
    let off = fx::wrap(yaw_of(t.sub(w.player.pos)).wrapping_sub(w.aim.yaw));
    on_mark(w) && off.abs() <= FACING
}

/// M7: the view must be turned toward the trainer this nearly.
pub const FACING: i32 = fx::deg(50);

/// M7: whether the player stands on the mark.
pub fn on_mark(w: &World) -> bool {
    let r = story::spec().trainer.mark_r_cm * ONE / 100;
    mark(w).is_some_and(|m| m.sub(w.player.pos).len_xz() <= r)
}

/// M7: the mark, a bright ring on the ground with a glow, while the
/// practice waits for the player or runs.
pub fn draw(w: &World, look: &crate::look::Look, out: &mut crate::render::Shapes) {
    if !lending(w) {
        return;
    }
    let Some(m) = mark(w) else {
        return;
    };
    let sp = &story::spec().trainer;
    let r = sp.mark_r_cm * ONE / 100;
    let rgb = [
        sp.mark[0] * ONE / 255,
        sp.mark[1] * ONE / 255,
        sp.mark[2] * ONE / 255,
    ];
    let lift = ONE * 3 / 100;
    let ring = crate::geom::Affine::translate(m.add(crate::geom::v3(0, lift, 0))).then(
        &crate::geom::Affine::scale(crate::geom::v3(2 * r, ONE * 4 / 100, 2 * r)),
    );
    out.solid(crate::prim::Mesh::Tube, ring, rgb, ONE);
    out.outline(crate::prim::Mesh::Tube, &ring, look.ink(), ONE);
    out.glow(
        crate::prim::Mesh::Dot,
        crate::geom::Affine::translate(m.add(crate::geom::v3(0, ONE / 10, 0))).then(
            &crate::geom::Affine::scale(crate::geom::v3(3 * r, ONE / 2, 3 * r)),
        ),
        rgb,
        ONE / 4,
    );
}

fn pass(w: &mut World) {
    w.trainer.stage = Stage::Done;
    crate::imbue::lend(w, 0);
    if w.trainer.taught == 0 {
        crate::imbue::learn(w);
        w.trainer.taught += 1;
        crate::ui::say(w, "trainer", &["trainer_pass_1", "trainer_pass_2"]);
        story::banner(w, "story_learned");
    }
}

/// How far the player is from talking to the trainer (not while a volley
/// flies).
pub fn reach(w: &World) -> Option<Fx> {
    let id = id(w)?;
    if matches!(w.trainer.stage, Stage::Volley(_)) {
        return None;
    }
    let d = player::body(w, id).pos.sub(w.player.pos).len_xz();
    (d <= TALK_FROM).then(|| (d - TALK_FROM + story::REACH).max(0))
}

pub fn prompt_key(_w: &World) -> &'static str {
    "does_talk_trainer"
}

/// E by the trainer.
pub fn talk(w: &mut World) {
    match w.trainer.stage {
        Stage::Waiting if w.trainer.taught == 0 => {
            crate::ui::say(
                w,
                "trainer",
                &["trainer_1", "trainer_2", "trainer_3", "trainer_mark"],
            );
            w.trainer.stage = Stage::Ready;
        }
        Stage::Ready if !on_mark(w) => crate::ui::say(w, "trainer", &["trainer_mark"]),
        Stage::Ready => crate::ui::say(w, "trainer", &["trainer_draw"]),
        Stage::Between(_) => crate::ui::say(w, "trainer", &["trainer_again"]),
        _ => crate::ui::say(w, "trainer", &["trainer_done"]),
    }
}

/// `trainer.<name>`: `credit` (test-only) counts `n` orbs as turned, for
/// tests and views before M6's orbs can be turned.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    match name {
        "credit" => {
            let n: u32 = value
                .trim()
                .parse()
                .map_err(|_| format!("Orbs to count are a whole number, not \"{value}\"."))?;
            w.trainer.credited += n;
            Ok(())
        }
        // M7, test-only: the player stood on the practice's mark, looking
        // toward the trainer, for a view of the practice
        "mark" => {
            let m = mark(w).ok_or_else(|| "The trainer is not standing yet.".to_string())?;
            let t = player::body(w, id(w).unwrap_or(0)).pos;
            let yaw = yaw_of(t.sub(m));
            let held = w.me.held;
            w.player = crate::player::Body::new(m, yaw);
            w.me = crate::player::Fighter::new(Kind::Player, held, 0, m, yaw, false);
            w.aim.yaw = yaw;
            w.aim.pitch = -fx::deg(8);
            Ok(())
        }
        _ => Err(format!("Unknown action \"trainer.{name}\".")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input;

    /// The opening skipped, the player 2 m from the trainer.
    fn by_trainer() -> World {
        let mut w = story::opening(6);
        story::step_aside(&mut w);
        let hut = crate::places::at(&w, "hut").unwrap();
        let p = hut.add(crate::geom::v3(fx::int(9), 0, fx::int(5)));
        w.player = crate::player::Body::new(p, 0);
        w.me = crate::player::Fighter::new(Kind::Player, crate::pose::Held::ShortSword, 0, p, 0, false);
        w.step(&Input::default());
        let t = player::body(&w, id(&w).expect("the trainer stands")).pos;
        let mut p = t.add(crate::geom::v3(0, 0, fx::int(2)));
        p.y = w.ground(p.x, p.z);
        w.player = crate::player::Body::new(p, 0);
        w.me = crate::player::Fighter::new(Kind::Player, crate::pose::Held::ShortSword, 0, p, 0, false);
        w
    }

    /// M7: the player put on the practice's mark, facing the trainer.
    fn onto_mark(w: &mut World) {
        let m = mark(w).expect("the trainer stands");
        let t = player::body(w, id(w).unwrap()).pos;
        let yaw = yaw_of(t.sub(m));
        let held = w.me.held;
        w.player = crate::player::Body::new(m, yaw);
        w.me = crate::player::Fighter::new(Kind::Player, held, 0, m, yaw, false);
        w.aim.yaw = yaw;
    }

    fn press(w: &mut World, keys: u32) {
        w.step(&Input {
            keys,
            ..Input::default()
        });
        w.ui.dialogue = None;
        w.step(&Input::default());
    }

    #[test]
    fn the_imbue_is_not_known_at_the_start() {
        let w = story::opening(1);
        assert!(!crate::imbue::learned(&w));
        assert!(!lending(&w));
    }

    #[test]
    fn talking_to_the_trainer_starts_the_practice_with_the_imbue_lent() {
        let mut w = by_trainer();
        assert_eq!(story::nearest_use(&w).map(|(u, _)| u), Some(story::Use::Trainer));
        press(&mut w, input::USE);
        assert_eq!(w.trainer.stage, Stage::Ready);
        assert!(lending(&w));
        assert!(
            crate::imbue::on(&w),
            "the trainer imbues the blade for the practice"
        );
        assert!(!crate::imbue::learned(&w), "but Q is not the player's yet");
        press(&mut w, input::DRAW);
        for _ in 0..crate::player::DRAW_TICKS {
            w.step(&Input::default());
        }
        // M7: drawn by the trainer, the practice waits for the mark
        assert_eq!(w.trainer.stage, Stage::Ready);
        onto_mark(&mut w);
        press(&mut w, input::DRAW);
        for _ in 0..crate::player::DRAW_TICKS + 2 {
            w.step(&Input::default());
        }
        assert!(
            matches!(w.trainer.stage, Stage::Volley(_)),
            "{:?}",
            w.trainer.stage
        );
        assert_eq!(w.trainer.volleys, 1);
    }

    /// With M6's orbs: the guard held still with the lent imbue turns the
    /// practice's motes, and the practice passes on them alone.
    #[test]
    fn the_practice_passes_on_orbs_the_lent_imbue_turns() {
        let mut w = by_trainer();
        press(&mut w, input::USE);
        onto_mark(&mut w);
        press(&mut w, input::DRAW);
        let mut t = 0;
        while !crate::imbue::learned(&w) {
            w.step(&Input::default());
            w.ui.dialogue = None;
            t += 1;
            assert!(t < 60 * 120, "passed within two minutes: {} turned", turned(&w));
        }
        assert_eq!(w.trainer.credited, 0);
        assert!(w.trainer.turned + crate::orbs::deflected(&w, id(&w).unwrap()) >= TO_PASS);
        assert_eq!(w.trainer.taught, 1);
    }

    #[test]
    fn the_practice_cannot_pass_without_the_deflections_and_learn_is_called_once() {
        let mut w = by_trainer();
        press(&mut w, input::USE);
        onto_mark(&mut w);
        press(&mut w, input::DRAW);
        for _ in 0..crate::player::DRAW_TICKS {
            w.step(&Input::default());
        }
        assert!(matches!(w.trainer.stage, Stage::Volley(_)));
        // the sword put away again: no blade, so no orb is turned, and the
        // volleys come and go: never passed
        press(&mut w, input::DRAW);
        for _ in 0..3000 {
            w.step(&Input::default());
            w.ui.dialogue = None;
        }
        assert!(!crate::imbue::learned(&w));
        assert_eq!(turned(&w), 0, "nothing turned");
        assert_ne!(w.trainer.stage, Stage::Done);
        assert!(
            w.trainer.volleys >= 2,
            "the trainer casts again: {}",
            w.trainer.volleys
        );
        // one short of the count: still not passed
        w.trainer.credited = TO_PASS - 1;
        for _ in 0..30 {
            w.step(&Input::default());
            w.ui.dialogue = None;
        }
        assert!(!crate::imbue::learned(&w));
        // the last one: passed, taught once, and the lending over
        w.trainer.credited = TO_PASS;
        for _ in 0..(BETWEEN_TICKS + 30) {
            w.step(&Input::default());
            w.ui.dialogue = None;
        }
        assert!(crate::imbue::learned(&w));
        assert_eq!(w.trainer.stage, Stage::Done);
        assert_eq!(w.trainer.taught, 1);
        assert!(!lending(&w));
        // more turned orbs, more talk: never taught twice
        w.trainer.credited += 10;
        press(&mut w, input::USE);
        w.step(&Input::default());
        assert_eq!(w.trainer.taught, 1);
    }

    /// M7: the mark stands 8 m in front of the trainer, and the practice
    /// is staged there: talked to from 2 m, the player is not on it.
    #[test]
    fn the_practice_s_mark_is_eight_metres_in_front_of_the_trainer() {
        let w = by_trainer();
        let t = player::body(&w, id(&w).unwrap());
        let m = mark(&w).unwrap();
        let d = m.sub(t.pos).len_xz();
        assert!((d - fx::int(8)).abs() <= 8, "{d}");
        let ahead = crate::geom::facing(t.facing, 0).dot(m.sub(t.pos).norm());
        assert!(ahead > ONE * 99 / 100, "in front: {ahead}");
        assert!(!on_mark(&w));
    }

    /// M7: an ordinary player passes the practice within a minute (3,600
    /// ticks): E by the trainer, walk to the mark (turning the view toward
    /// it and holding W), turn to face the trainer, X, and then hold the
    /// guard, closing each dialogue as it opens (Space). The steps come in
    /// order: one orb, then three, then the pattern.
    #[test]
    fn an_ordinary_player_passes_the_practice_within_a_minute() {
        let mut w = by_trainer();
        press(&mut w, input::USE);
        let mut steps = vec![w.trainer.step];
        let mut t = 0u32;
        let mut drew = false;
        while !crate::imbue::learned(&w) {
            t += 1;
            assert!(
                t <= 3600,
                "not passed in a minute: {} turned, step {}",
                turned(&w),
                w.trainer.step
            );
            // a dialogue opened: Space closes it
            w.ui.dialogue = None;
            let m = mark(&w).unwrap();
            let me = w.player.pos;
            let mut keys = 0;
            if !on_mark(&w) || m.sub(me).len_xz() > ONE / 2 && !drew {
                w.aim.yaw = yaw_of(m.sub(me));
                keys = input::FORWARD;
            } else {
                let tr = player::body(&w, id(&w).unwrap()).pos;
                w.aim.yaw = yaw_of(tr.sub(me));
                if !drew && w.me.sheath == Sheath::Sheathed {
                    keys = input::DRAW;
                    drew = true;
                }
            }
            w.step(&Input {
                keys,
                ..Input::default()
            });
            if *steps.last().unwrap() != w.trainer.step {
                steps.push(w.trainer.step);
            }
        }
        assert_eq!(steps, vec![0, 1, 2], "one orb, then three, then the pattern");
        assert!(w.trainer.volleys >= 3);
    }
}
