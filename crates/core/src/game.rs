//! The game as the page sees it: the screens (title, play, paused), the
//! clock that turns display frames into ticks, the frame the page draws, and
//! the one door for settings, screens and flags (`act`).
//!
//! Each display frame the page calls `advance` with its timestamp, the held
//! action bits and the mouse and wheel movement since the last frame, then
//! reads the frame (`view`, `instances`, `draws`, and `static_values` when
//! `static_version` changes) and, when `ui_version` changes, the view model
//! (`api::ui`). The page does no arithmetic: the core has already placed
//! everything between the last two ticks (`alpha`).

use crate::camera::{self, Aim};
use crate::fx::{self, Fx, ONE};
use crate::glossary::Entry;
use crate::input::{Input, Mouse, Pending};
use crate::look::Look;
use crate::page::Strings;
use crate::render::{self, Camera, Frame, Group, Scene, Shapes};
use crate::ticks::Clock;
use crate::world::World;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Title,
    Play,
    Paused,
}

impl Screen {
    pub fn name(self) -> &'static str {
        match self {
            Screen::Title => "title",
            Screen::Play => "play",
            Screen::Paused => "paused",
        }
    }
}

/// The most pixels the drawing buffer may hold (2560 × 1600); a larger
/// screen draws at a lower resolution, scaled up by the browser.
pub const MAX_PIXELS: i64 = 2560 * 1600;
/// On a software renderer (no graphics card) the page draws at this share of
/// the screen's pixel density, percent.
pub const SOFTWARE_SCALE_PCT: i32 = 50;
/// How long the pause hint shows once play starts, ticks (6 s).
pub const HINT_TICKS: u64 = 360;
/// The title screen's camera drifts round the start this fast, angle units
/// a tick (2° a second).
pub const TITLE_DRIFT: i32 = fx::TURN / 360 * 2 / 60;
/// And looks down this far, from this far back.
pub const TITLE_PITCH: i32 = -fx::deg(9);
pub const TITLE_BACK: Fx = fx::int(8);
/// Play eases the camera in from the title's view to the shoulder over this
/// many ticks (0.75 s).
pub const ZOOM_TICKS: u64 = 45;

#[derive(Debug, Clone)]
pub struct Game {
    pub strings: Strings,
    pub glossary: Vec<Entry>,
    pub look: Look,
    pub clock: Clock,
    pub pending: Pending,
    pub world: World,
    pub screen: Screen,
    /// the drawing buffer's size and the CSS size's aspect (Q16)
    pub pixels: (i32, i32),
    pub aspect: Fx,
    css: (i32, i32),
    dpr_pct: i32,
    pub software: bool,
    groups: Vec<Group>,
    pub scene: Scene,
    pub frame: Frame,
    /// share of a tick the drawing is past the last tick
    pub alpha: Fx,
    /// ticks of play since play first started, for the pause hint
    pub play_ticks: u64,
    /// ticks the title screen has shown, for its drifting camera
    pub title_ticks: u64,
    /// ticks since play began from the title, for the camera's ease in
    pub zoom_ticks: u64,
    pub ui_version: u32,
}

impl Game {
    /// A game from the strings, the glossary, the look and the page's seed.
    pub fn new(strings_json: &str, glossary_md: &str, look_json: &str, seed: u32) -> Result<Game, String> {
        let strings = crate::page::parse_strings(strings_json).map_err(|e| e.message())?;
        let glossary = crate::glossary::parse(glossary_md).map_err(|e| e.message())?;
        let look = Look::parse(look_json)?;
        let world = World::new(seed as u64);
        let groups = world.scene(&look);
        let mut g = Game {
            strings,
            glossary,
            look,
            clock: Clock::default(),
            pending: Pending::default(),
            world,
            screen: Screen::Title,
            pixels: (1, 1),
            aspect: ONE,
            css: (1, 1),
            dpr_pct: 100,
            software: false,
            groups,
            scene: Scene::default(),
            frame: Frame::default(),
            alpha: 0,
            play_ticks: 0,
            title_ticks: 0,
            zoom_ticks: ZOOM_TICKS,
            ui_version: 1,
        };
        g.draw();
        Ok(g)
    }

    /// The CSS size and the device pixel ratio (percent) in; the drawing
    /// buffer's size out.
    pub fn resize(&mut self, css_w: i32, css_h: i32, dpr_pct: i32) -> [i32; 2] {
        self.css = (css_w.max(1), css_h.max(1));
        self.dpr_pct = dpr_pct.clamp(25, 400);
        let scale = if self.software {
            self.dpr_pct * SOFTWARE_SCALE_PCT / 100
        } else {
            self.dpr_pct
        };
        let mut w = self.css.0 as i64 * scale as i64 / 100;
        let mut h = self.css.1 as i64 * scale as i64 / 100;
        if w * h > MAX_PIXELS {
            let k = fx::isqrt_u64((MAX_PIXELS * 1_000_000 / (w * h)) as u64) as i64;
            w = w * k / 1000;
            h = h * k / 1000;
        }
        self.pixels = (w.max(1) as i32, h.max(1) as i32);
        self.aspect = fx::ratio(self.css.0, self.css.1);
        self.draw();
        [self.pixels.0, self.pixels.1]
    }

    /// One display frame: the page's clock in microseconds, the held action
    /// bits, and the mouse and wheel movement in hundredths of a CSS pixel.
    pub fn advance(&mut self, now_us: i64, bits: u32, dx: i32, dy: i32, wheel: i32) {
        let ticks = self.clock.frame(now_us, bits);
        match self.screen {
            Screen::Play => {
                self.pending.add(dx, dy, wheel);
                let hinted = self.hint_shown();
                for (k, b) in ticks.iter().enumerate() {
                    let m = if k == 0 {
                        self.pending.take()
                    } else {
                        Mouse::default()
                    };
                    self.world.step(&Input::from_page(*b, m));
                    self.play_ticks += 1;
                    self.zoom_ticks = (self.zoom_ticks + 1).min(ZOOM_TICKS);
                }
                if hinted != self.hint_shown() {
                    self.ui_version += 1;
                }
                self.alpha = self.clock.alpha();
            }
            Screen::Title => {
                self.pending.clear();
                self.title_ticks += ticks.len() as u64;
                self.alpha = self.clock.alpha();
            }
            Screen::Paused => {
                // the world holds still; time passing while paused is
                // forgotten, so resuming runs no catch-up ticks
                self.pending.clear();
                self.clock.rest();
            }
        }
        self.draw();
    }

    pub fn hint_shown(&self) -> bool {
        self.screen == Screen::Play && self.play_ticks < HINT_TICKS
    }

    /// The title screen's slow turn round the start.
    fn title_aim(&self) -> Aim {
        let drift =
            (self.title_ticks as i64 * TITLE_DRIFT as i64 + fx::mul(self.alpha, TITLE_DRIFT) as i64) as i32;
        Aim {
            yaw: fx::wrap(crate::world::START_YAW.wrapping_add(drift)),
            pitch: TITLE_PITCH,
            rest: (0, 0),
        }
    }

    /// How far back the camera sits: the title's wide view, easing in to
    /// the shoulder (`camera::BACK`) once play begins.
    fn back(&self) -> Fx {
        if self.screen == Screen::Title {
            return TITLE_BACK;
        }
        let t = ((self.zoom_ticks as i64 * ONE as i64 + self.alpha as i64) / ZOOM_TICKS as i64)
            .min(ONE as i64) as Fx;
        let smooth = fx::mul(fx::mul(t, t), 3 * ONE - 2 * t);
        fx::lerp(TITLE_BACK, camera::BACK, smooth)
    }

    pub fn camera(&self) -> Camera {
        let ground = |x, z| self.world.ground(x, z);
        let feet = self.world.player.feet(self.alpha);
        match self.screen {
            Screen::Title => camera::view_from(feet, self.title_aim(), self.back(), ground),
            _ => camera::view_from(
                feet,
                self.world.aim.turned(self.pending.peek()),
                self.back(),
                ground,
            ),
        }
    }

    /// Builds the frame the page draws next.
    fn draw(&mut self) {
        let cam = self.camera();
        let origin = render::origin_for(cam.eye);
        if origin != self.scene.origin || self.scene.version == 0 {
            self.scene = Scene::build(&self.groups, origin, self.scene.version + 1);
        }
        let mut moving = Shapes::default();
        moving.sky(cam.eye);
        self.world.draw(self.alpha, &self.look, &mut moving);
        self.frame = Frame::build(&self.scene, &moving, &cam, self.aspect);
    }

    fn show(&mut self, s: Screen) {
        if self.screen != s {
            if self.screen == Screen::Title && s == Screen::Play {
                // play opens looking where the title's camera had drifted
                // to, and eases in from its distance
                self.world.aim = self.title_aim();
                self.zoom_ticks = 0;
            }
            self.screen = s;
            self.pending.clear();
            self.ui_version += 1;
            self.draw();
        }
    }

    /// The one door for screens, settings and flags. The error is the
    /// message the page shows.
    pub fn act(&mut self, name: &str, value: &str) -> Result<(), String> {
        match name {
            "screen.show" => match value {
                "title" => self.show(Screen::Title),
                "play" => self.show(Screen::Play),
                "paused" => self.show(Screen::Paused),
                other => return Err(format!("There is no screen \"{other}\".")),
            },
            // Esc, a lost pointer lock and a hidden tab pause play
            "key.escape" | "pointer.lost" | "page.hidden" => {
                if self.screen == Screen::Play {
                    self.show(Screen::Paused);
                }
            }
            "display.software" => {
                self.software = value == "true";
                let (w, h) = self.css;
                self.resize(w, h, self.dpr_pct);
            }
            _ => self.world.act(name, value)?,
        }
        Ok(())
    }

    /// The numbers glossary placeholders name, as text.
    pub fn glossary_values(&self) -> Vec<(&'static str, String)> {
        vec![
            ("walk.speed", fx::decimal(crate::player::WALK, 1)),
            ("run.speed", fx::decimal(crate::player::RUN, 1)),
        ]
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::input::FORWARD;
    use crate::ticks::TICK_US;

    pub fn game() -> Game {
        let mut g = Game::new(
            include_str!("../../../data/strings.json"),
            include_str!("../../../GLOSSARY.md"),
            include_str!("../../../data/look.json"),
            42,
        )
        .expect("the game starts from the repository's data");
        g.resize(1920, 1080, 100);
        g
    }

    /// The time of the last frame.
    fn now(g: &Game) -> i64 {
        g.clock.last().unwrap_or(0)
    }

    /// `n` frames a tick apart, with these bits held and this movement.
    pub fn frames(g: &mut Game, n: i64, bits: u32, dx: i32) {
        for _ in 0..n {
            g.advance(now(g) + TICK_US, bits, dx, 0, 0);
        }
    }

    #[test]
    fn the_game_opens_on_the_title_and_play_goes_through_the_door() {
        let mut g = game();
        assert_eq!(g.screen, Screen::Title);
        g.act("screen.show", "play").unwrap();
        assert_eq!(g.screen, Screen::Play);
        assert_eq!(
            g.act("screen.show", "nowhere"),
            Err("There is no screen \"nowhere\".".into())
        );
        assert_eq!(
            g.act("screen.jump", "play"),
            Err("Unknown action \"screen.jump\".".into())
        );
    }

    #[test]
    fn escape_a_lost_pointer_and_a_hidden_tab_pause_play_and_only_play() {
        for name in ["key.escape", "pointer.lost", "page.hidden"] {
            let mut g = game();
            g.act(name, "").unwrap();
            assert_eq!(g.screen, Screen::Title, "{name} on the title changes nothing");
            g.act("screen.show", "play").unwrap();
            g.act(name, "").unwrap();
            assert_eq!(g.screen, Screen::Paused, "{name}");
        }
    }

    #[test]
    fn a_paused_world_holds_still_and_resuming_runs_no_catch_up() {
        let mut g = game();
        g.act("screen.show", "play").unwrap();
        frames(&mut g, 10, FORWARD, 0);
        let tick = g.world.tick;
        g.act("key.escape", "").unwrap();
        let held = g.world.clone();
        // ten seconds paused with W held
        frames(&mut g, 600, FORWARD, 500);
        assert_eq!(g.world, held);
        g.act("screen.show", "play").unwrap();
        g.advance(now(&g) + TICK_US, FORWARD, 0, 0, 0);
        assert_eq!(g.world.tick, tick + 1, "one frame of play runs one tick");
    }

    #[test]
    fn the_same_world_gives_the_same_frame_byte_for_byte() {
        let run = || {
            let mut g = game();
            g.act("screen.show", "play").unwrap();
            frames(&mut g, 90, FORWARD, 120);
            g.frame.clone()
        };
        let (a, b) = (run(), run());
        assert_eq!(a, b);
        assert!(a.instances.len() > 16 && !a.draws.is_empty());
    }

    #[test]
    fn two_still_ticks_give_identical_frames_and_static_arrays() {
        let mut g = game();
        g.act("screen.show", "play").unwrap();
        frames(&mut g, 60, FORWARD, 0);
        // let the body come to rest, then compare two ticks with nothing held
        frames(&mut g, 60, 0, 0);
        let (f1, s1) = (g.frame.clone(), g.scene.clone());
        frames(&mut g, 1, 0, 0);
        assert_eq!(g.frame, f1, "a still view's instance array changed between ticks");
        assert_eq!(g.scene, s1);
        // between ticks too: half a tick on, the still world draws the same
        g.advance(now(&g) + TICK_US / 2, 0, 0, 0, 0);
        assert_eq!(g.frame, f1);
    }

    #[test]
    fn the_body_is_drawn_between_ticks_by_the_clock_s_alpha() {
        let mut g = game();
        g.act("screen.show", "play").unwrap();
        frames(&mut g, 30, FORWARD, 0);
        let (prev, here) = (g.world.player.prev_pos, g.world.player.pos);
        assert_ne!(prev, here);
        g.advance(now(&g) + TICK_US / 2, FORWARD, 0, 0, 0);
        assert_eq!(g.world.player.pos, here, "half a tick runs no tick");
        let half = g.alpha;
        assert!((half - ONE / 2).abs() < ONE / 100, "{half}");
        assert_eq!(g.world.player.feet(half), prev.lerp(here, half));
    }

    #[test]
    fn walking_far_moves_the_origin_and_rebuilds_the_static_array() {
        let mut g = game();
        let v = g.scene.version;
        g.act("screen.show", "play").unwrap();
        // 5 m/s for 60 s is 300 m: past at least one 256 m cell edge
        frames(&mut g, 3600, FORWARD | crate::input::RUN, 0);
        assert!(g.scene.version > v);
        assert_eq!(g.scene.origin, render::origin_for(g.camera().eye));
    }

    #[test]
    fn a_software_renderer_draws_half_as_many_pixels_each_way() {
        let mut g = game();
        // 3840 × 2160 is 8 294 400 pixels; capped to 4 096 000 by
        // √(4 096 000 / 8 294 400) = 0.702 each way: 2695 × 1516
        assert_eq!(g.resize(1920, 1080, 200), [2695, 1516]);
        g.act("display.software", "true").unwrap();
        assert_eq!(g.pixels, (1920, 1080));
        g.act("display.software", "false").unwrap();
        assert_eq!(g.resize(1280, 720, 100), [1280, 720]);
    }

    #[test]
    fn the_pause_hint_shows_for_the_first_six_seconds_of_play() {
        let mut g = game();
        g.act("screen.show", "play").unwrap();
        assert!(g.hint_shown());
        let v = g.ui_version;
        frames(&mut g, 359, 0, 0);
        assert!(g.hint_shown());
        frames(&mut g, 2, 0, 0);
        assert!(!g.hint_shown());
        assert!(g.ui_version > v, "the page is told to read the screen again");
    }

    #[test]
    fn play_opens_looking_where_the_title_camera_had_turned() {
        let mut g = game();
        frames(&mut g, 120, 0, 0);
        let title = g.camera();
        g.act("screen.show", "play").unwrap();
        assert_eq!(g.camera(), title, "no jump from the title's view to play's");
        // the ease ends at the shoulder: 2.6 m behind the pivot
        frames(&mut g, ZOOM_TICKS as i64, 0, 0);
        let c = g.camera();
        assert_eq!(
            c,
            crate::camera::view(g.world.player.feet(g.alpha), g.world.aim, |_, _| 0)
        );
        // 120 ticks at 6 units a tick
        assert_eq!(g.world.aim.yaw, crate::world::START_YAW + 720);
    }
}
