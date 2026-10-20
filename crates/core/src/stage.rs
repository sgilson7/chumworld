//! M7: the Equipment screen's figure (R9.82). Ocarina of Time's equipment
//! screen shows Link himself, dressed in what he wears; this draws the
//! player's own model, lit, dressed in the speedster pieces worn and holding
//! the sword in the sword hand, turning slowly, as a second small frame the
//! page draws on a canvas inside the screen. The world behind holds still.
//!
//! The figure stands at its rest pose (`models::rest_pose`) at the world's
//! origin, which is also the frame's origin; the camera stands in front of
//! it, a little above the chest, and looks at its middle. The numbers are in
//! `data/menus.json` (`figure`).

use std::sync::OnceLock;

use serde::Deserialize;

use crate::fx::{self, Fx};
use crate::game::{Game, Screen};
use crate::geom::{v3, V3};
use crate::items::Item;
use crate::models::{self, Dress};
use crate::pose::Held;
use crate::pose::Kind;
use crate::render::{Camera, Frame, Scene, Shapes};

#[derive(Debug, Deserialize)]
struct Spec {
    /// the stage's background, 0 to 255
    clear: [i32; 3],
    /// one whole turn takes this long
    turn_seconds: i32,
    /// the turn it starts from, degrees from facing the camera
    start_deg: i32,
    /// the camera's vertical field of view, degrees
    fov_deg: i32,
    /// where the camera stands and what it looks at, mm from the feet
    eye_mm: [i32; 3],
    look_mm: [i32; 3],
}

#[derive(Debug, Deserialize)]
struct MenusData {
    figure: Spec,
}

fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| {
        let f: File =
            serde_json::from_str(include_str!("../../../data/menus.json")).expect("data/menus.json");
        f.figure
    })
}

fn mm(v: [i32; 3]) -> V3 {
    v3(
        fx::ratio(v[0], 1000),
        fx::ratio(v[1], 1000),
        fx::ratio(v[2], 1000),
    )
}

/// Whether the figure shows: the ring open at Equipment.
pub fn shown(g: &Game) -> bool {
    g.screen == Screen::Menu && g.menu.face() == crate::ui::EQUIPMENT
}

/// The figure's heading after the ring has been open `ticks`: facing the
/// camera (half a turn from facing away, -z), turned by the start, then a
/// whole turn every `turn_seconds`.
pub fn heading(ticks: u32) -> i32 {
    let s = spec();
    let per_turn = (s.turn_seconds as i64 * crate::ticks::TICKS_PER_SECOND as i64).max(1);
    let spun = (ticks as i64 % per_turn) * fx::TURN as i64 / per_turn;
    fx::wrap(fx::HALF_TURN + fx::deg(s.start_deg) + spun as i32)
}

/// The held item the sword hand's sword is drawn as.
fn sword(i: Option<Item>) -> Held {
    match i {
        Some(Item::Sword) => Held::Sword,
        Some(Item::ShortSword) => Held::ShortSword,
        _ => Held::Nothing,
    }
}

/// The figure's shapes, in a fixed order: the player's model at its rest
/// pose, dressed in what is worn, the sword in hand at its guard.
pub fn shapes(g: &Game, out: &mut Shapes) {
    let w = &g.world;
    let pose = models::rest_pose(Kind::Player, V3::ZERO, heading(g.menu.open_ticks));
    let dress = Dress {
        worn: crate::items::speedster_mask(w),
        variant: 0,
    };
    models::figure_dressed(Kind::Player, &pose, dress, &g.look, out);
    let held = sword(w.inventory.weapon);
    if held != Held::Nothing {
        // the rest pose's line, at this sword's own length
        let along = pose.tip.sub(pose.grip).norm();
        let tip = pose.grip.add(along.scale(models::grip_to_tip(held)));
        models::held(held, pose.grip, tip, &g.look, out);
    }
}

/// The stage's camera.
pub fn camera() -> Camera {
    let s = spec();
    let eye = mm(s.eye_mm);
    Camera {
        eye,
        fwd: mm(s.look_mm).sub(eye).norm(),
        fov: fx::deg(s.fov_deg),
    }
}

/// The frame the page draws on the figure's canvas: empty unless the
/// figure shows.
pub fn frame(g: &Game, aspect: Fx) -> Frame {
    if !shown(g) {
        return Frame::default();
    }
    let mut out = Shapes::default();
    shapes(g, &mut out);
    Frame::build(&Scene::default(), &out, &camera(), aspect)
}

/// The stage's background, as the page's clear colour (0 to 1 a channel).
pub fn clear_color() -> Vec<i32> {
    spec().clear.iter().map(|c| fx::ratio(*c, 255)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::game;

    /// 16 s a turn at 60 ticks a second is 960 ticks: a quarter turn at
    /// 240, whole at 960; it starts 25° from facing the camera.
    #[test]
    fn the_figure_turns_once_every_sixteen_seconds_from_a_quarter_view() {
        let start = fx::wrap(fx::HALF_TURN + fx::deg(-25));
        assert_eq!(heading(0), start);
        assert_eq!(heading(240), fx::wrap(start + fx::TURN / 4));
        assert_eq!(heading(960), start);
        assert_eq!(heading(480), fx::wrap(start + fx::HALF_TURN));
    }

    /// Drawn only with the ring open at Equipment; there it draws the
    /// player's model with the sword in hand, and the speedster's hood when
    /// worn adds its shapes.
    #[test]
    fn the_equipment_screen_draws_the_dressed_player_and_no_other_screen_does() {
        let mut g = game();
        g.act("story.skip", "").unwrap();
        g.act("screen.show", "play").unwrap();
        assert!(frame(&g, fx::ONE).draws.is_empty(), "in play");
        crate::ui::open(&mut g, crate::ui::SELECT);
        assert!(frame(&g, fx::ONE).draws.is_empty(), "at Select Item");
        crate::ui::open(&mut g, crate::ui::EQUIPMENT);
        let bare = frame(&g, fx::ONE);
        assert!(!bare.draws.is_empty());
        let mut plain = Shapes::default();
        shapes(&g, &mut plain);
        let mut figure_only = Shapes::default();
        let pose = models::rest_pose(Kind::Player, V3::ZERO, heading(0));
        models::figure_dressed(Kind::Player, &pose, Dress::default(), &g.look, &mut figure_only);
        assert!(
            plain.len() > figure_only.len(),
            "the short sword is drawn in hand"
        );
        g.world.inventory.add(Item::Hood, 1);
        g.world.inventory.worn[crate::items::Slot::Head as usize] = Some(Item::Hood);
        let mut hooded = Shapes::default();
        shapes(&g, &mut hooded);
        assert_ne!(hooded, plain, "the hood worn is drawn");
        let mut dressed = Shapes::default();
        let hood = Dress {
            worn: models::HOOD,
            variant: 0,
        };
        models::figure_dressed(Kind::Player, &pose, hood, &g.look, &mut dressed);
        assert_eq!(
            hooded.list[..dressed.len()],
            dressed.list[..],
            "as the model dresses it"
        );
    }

    /// The camera sees the whole figure: the feet and the top of the head
    /// both land inside the canvas (a 3:4 canvas, the page's).
    #[test]
    fn the_stage_s_camera_frames_the_whole_figure() {
        let cam = camera();
        let aspect = fx::ratio(3, 4);
        let pose = models::rest_pose(Kind::Player, V3::ZERO, heading(0));
        let top = pose.head.add(v3(0, fx::ratio(16, 100), 0));
        for p in [V3::ZERO, top] {
            let (x, y) = cam.screen(aspect, p).expect("in front");
            assert!(x > fx::ONE / 10 && x < fx::ONE * 9 / 10, "{x}");
            assert!(y > fx::ONE / 20 && y < fx::ONE * 19 / 20, "{y}");
        }
    }
}
