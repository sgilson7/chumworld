//! The world and the order of one tick.
//!
//! `World` holds everything that changes as the game runs; `step` advances it
//! one tick (1/60 s) from the input records of that tick, and nothing else
//! changes it (CW-4: the same seed and the same records give the same world,
//! tick for tick, and `hash` says so in one number).
//!
//! # The tick, in order
//!
//! 1. the player's aim turns by the record's mouse movement (`camera::Aim`);
//! 2. *M1* pilots read the world and return each enemy's `Input` (CW-3);
//! 3. bodies move on their feet (*M0* `player`; *M1* `motion` takes over);
//! 4. *M1* arms, then blades, then health;
//! 5. *M1* throwables, *M5* the boomerang;
//! 6. *M6* orbs and the imbue;
//! 7. *M5* story, places, the merchant, the camp, the trainer;
//! 8. *M2* water.
//!
//! # Extension contract (M1–M7)
//!
//! A module `m` (one of the stubs below) joins the world in up to six
//! places, each marked `// M<n>:` in this file:
//!
//! - **State.** One field on `World`, `pub m: m::State`, built in
//!   `World::new`. Keep it serialisable values (no float, no hash map, D2).
//! - **Step.** `pub fn step(w: &mut World, input: &Input)` in `m`, called at
//!   its place in `World::step`. A module that moves a body does it through
//!   an `Input` record (CW-3).
//! - **Draw.** `pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut
//!   Shapes)` for what moves, called from `World::draw`: push shapes in world
//!   coordinates, placed `alpha` of the way between the last two ticks
//!   (`prev.lerp(now, alpha)`), in a fixed order (CW-18).
//! - **Scene.** `pub fn scene(w: &World, look: &Look, meshes: &mut
//!   MeshTable, out: &mut Vec<Group>)` for what never moves, called from
//!   `World::scene`: one `Group` per place, with a centre and a radius that
//!   holds it. Many triangles that never move (ground, terrain, a house's
//!   walls) go in one mesh the module builds (`MeshTable::add`, a colour per
//!   vertex) and are drawn as one shape: a software renderer draws a few
//!   frames a second when they are thousands of instances.
//! - **Screen.** `pub fn hud(w: &World, s: &Strings, out: &mut Vec<HudItem>)`
//!   for lines over the game, called from `World::hud`; M4's `ui.rs` adds
//!   panels in `api::ui`. Text comes from `data/strings.json` by key.
//! - **Act.** `pub fn act(w: &mut World, name: &str, value: &str) ->
//!   Result<(), String>` for settings and flags named `m.<name>`, routed by
//!   `World::act`. The error is the message the page shows.

use crate::camera::{self, Aim};
use crate::fx::{self, Fx};
use crate::geom::{v3, V3};
use crate::input::{Input, Mouse};
use crate::look::Look;
use crate::page::Strings;
use crate::player::Body;
use crate::prim::MeshTable;
use crate::render::{Group, Shapes};
use crate::rng::Rng;

/// Where the game opens: the village centre on the journal's map, (2.0 km,
/// 1.2 km) with north up, which is world (2000, 0, 2800) (`geom` conventions).
pub const START: V3 = v3(fx::int(2000), 0, fx::int(2800));
/// The heading the game opens with: north-north-west, toward the meadow.
pub const START_YAW: i32 = fx::deg(20);

/// A line over the game, by id; `shown` false keeps it in the page but out of
/// sight, so the page can fade it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct HudItem {
    pub id: String,
    pub text: String,
    pub shown: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    /// ticks since the world began
    pub tick: u64,
    /// chance for this session, from the page's seed (`rng.rs`)
    pub rng: Rng,
    /// where the player looks
    pub aim: Aim,
    pub player: Body,
    // M4: what the player carries and wears; the map's fog, the toast, dialogue, the shop, settings
    pub inventory: crate::inventory::State,
    pub ui: crate::ui::State,
    // M3: the model lineup for review (`models.lineup`), off in play
    pub lineup: crate::models::Lineup,
    // M1: the player's arms, health and footing (its feet are `player`),
    // the foes, their pilots' book and memory, the blades' contacts, what
    // is thrown, the speedster setting, and the physics view
    pub me: crate::player::Fighter,
    pub foes: Vec<crate::pilot::Foe>,
    pub pilots: crate::pilot::State,
    pub blades: crate::blade::State,
    pub throwables: crate::throwables::State,
    pub motion: crate::motion::State,
    pub sticks: bool,
    // M2: the land (heights, layout, river), shared by every copy of the
    // world; the water's waves; a place to put the player when play starts
    pub land: std::rc::Rc<crate::terrain::Land>,
    pub water: crate::water::State,
    pub arrival: Option<crate::places::Arrival>,
    // M5: pub story: story::State, pub merchant: merchant::State, …
    // M6: the orbs in flight and the casts under way; the imbue
    pub orbs: crate::orbs::State,
    pub imbue: crate::imbue::State,
}

impl World {
    pub fn new(seed: u64) -> World {
        World {
            tick: 0,
            rng: Rng::new(seed),
            aim: Aim::new(START_YAW),
            player: Body::new(START, START_YAW),
            // M4: the start's gear (CW-7), an unwalked map
            inventory: crate::inventory::State::default(),
            ui: crate::ui::State::default(),
            lineup: crate::models::Lineup::default(), // M3:
            // M1:
            me: crate::player::Fighter::new(
                crate::pose::Kind::Player,
                crate::pose::Held::ShortSword,
                0,
                START,
                START_YAW,
                false,
            ),
            foes: Vec::new(),
            pilots: crate::pilot::State::new(),
            blades: crate::blade::State::default(),
            throwables: crate::throwables::State::default(),
            motion: crate::motion::State::default(),
            sticks: false,
            // M2
            land: crate::terrain::shipped(),
            water: crate::water::State::new(),
            arrival: None,
            // M6
            orbs: crate::orbs::State::default(),
            imbue: crate::imbue::State::default(),
        }
    }

    /// The ground's height at (x, z): M2's terrain, the triangles of its
    /// finest tiles as drawn (`terrain::Land::ground`).
    pub fn ground(&self, x: Fx, z: Fx) -> Fx {
        self.land.ground(x, z)
    }

    /// One tick, in the order the module doc gives.
    pub fn step(&mut self, input: &Input) {
        self.tick += 1;
        // 1. the aim (M1: drawn, the mouse moves the hand instead, and the
        // view holds on the foe)
        let look = crate::player::look_mouse(self, input);
        self.aim = self.aim.turned(look);
        crate::player::hold_on(self);
        // M4: 1, 2 and 3 ready a quick item in the free hand
        crate::inventory::step(self, input);
        // 2. M1:
        let inputs = crate::pilot::step(self);
        // 3. bodies (M1: motion)
        crate::motion::step(self, input, &inputs);
        // 4. M1:
        crate::arm::step(self, input, &inputs);
        crate::blade::step(self);
        crate::health::step(self);
        // 5. M1/M5: throwables, crate::boomerang::step(self, input);
        crate::throwables::step(self, input, &inputs);
        crate::player::finish(self, input, &inputs);
        crate::pilot::remember(self);
        // 6. M6: the imbue (Q), then the orbs, which the imbued edge turns
        crate::imbue::step(self, input);
        crate::orbs::step(self);
        // 7. M5: crate::story::step(self); crate::merchant::step(self); …
        // 8. M2: the water
        crate::water::step(self);
        // M4: the map fills in round the player; the toast counts down
        crate::ui::step(self);
    }

    /// What moves, drawn `alpha` of the way from the last tick to this one.
    pub fn draw(&self, alpha: Fx, look: &Look, out: &mut Shapes) {
        // M1: every fighter and everything thrown
        crate::player::draw(self, alpha, look, out);
        crate::throwables::draw(self, alpha, look, out);
        crate::models::draw(self, alpha, look, out); // M3: the lineup, when asked for
                                                     // M6: the imbued blade's glow, then the orbs and their sparks
        crate::imbue::draw(self, alpha, look, out);
        crate::orbs::draw(self, alpha, look, out);
    }

    /// What never moves: the static groups, and the meshes they draw.
    pub fn scene(&self, look: &Look, meshes: &mut MeshTable) -> Vec<Group> {
        // M2: the places' props, baked here; `Game::new` hands their groups
        // (named `place:<kind>`) to the detail levels, which draw them to
        // 3 km and the terrain, water and scatter round the camera (`lod.rs`)
        let mut out = Vec::new();
        crate::places::scene(self, look, meshes, &mut out);
        // M5: …
        out
    }

    /// Lines over the game.
    pub fn hud(&self, _strings: &Strings, _out: &mut Vec<HudItem>) {
        // M4: crate::ui::hud(self, strings, out); M6: the imbue's cooldown
        // reaches M4's ring through `ui::imbue`
        // (M1's health reaches the HUD through `ui::health` and `ui::foe`)
    }

    /// A setting or flag named `module.name`.
    pub fn act(&mut self, name: &str, value: &str) -> Result<(), String> {
        match name.split_once('.') {
            Some(("inventory", rest)) => crate::inventory::act(self, rest, value), // M4:
            Some(("models", rest)) => crate::models::act(self, rest, value),       // M3:
            // M1:
            Some(("duel", rest)) => crate::player::act(self, rest, value),
            Some(("motion", rest)) => crate::motion::act(self, rest, value),
            // M2
            Some(("places", rest)) => crate::places::act(self, rest, value),
            Some(("magic", rest)) => crate::orbs::act(self, rest, value), // M6
            _ => Err(format!("Unknown action \"{name}\".")),
        }
    }

    /// One number for the whole state, for replays and determinism tests: an
    /// FNV-1a hash of the state's debug text, which names every field.
    pub fn hash(&self) -> u64 {
        let text = format!("{self:?}");
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in text.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h
    }

    /// The camera for this frame: the aim with `pending` mouse movement
    /// applied as the next tick will apply it, over the feet at `alpha`,
    /// `back` from the shoulder's pivot (`camera::BACK` in play).
    pub fn camera(&self, alpha: Fx, pending: Mouse, back: Fx) -> crate::render::Camera {
        // M1: drawn, the mouse moves the hand, not the view
        let aim = if self.me.mouse_looks {
            self.aim.turned(pending)
        } else {
            self.aim
        };
        camera::view_from(self.player.feet(alpha), aim, back, |x, z| self.ground(x, z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{FORWARD, RUN};

    fn run(inputs: &[Input]) -> World {
        let mut w = World::new(7);
        for i in inputs {
            w.step(i);
        }
        w
    }

    fn walk_and_turn() -> Vec<Input> {
        (0..300)
            .map(|k| Input {
                keys: if k % 90 < 60 { FORWARD } else { FORWARD | RUN },
                dx: if k % 50 < 10 { 400 } else { -35 },
                dy: if k % 70 < 5 { 120 } else { 0 },
                ..Input::default()
            })
            .collect()
    }

    #[test]
    fn the_same_inputs_give_the_same_world_tick_for_tick() {
        let a = run(&walk_and_turn());
        let b = run(&walk_and_turn());
        assert_eq!(a, b);
        assert_eq!(a.hash(), b.hash());
        let mut other = walk_and_turn();
        other[150].dx += 1;
        assert_ne!(run(&other).hash(), a.hash(), "one hundredth of a pixel shows");
    }

    #[test]
    fn the_mouse_turns_the_aim_before_the_body_moves_the_same_tick() {
        // one tick with W and a 100 px turn right: the body walks along the
        // new heading, 12° right of the start
        let mut w = World::new(1);
        w.step(&Input {
            keys: FORWARD,
            dx: 10_000,
            ..Input::default()
        });
        assert_eq!(w.aim.yaw, START_YAW - 2184);
        let moved = w.player.pos.sub(START);
        let heading = crate::geom::yaw_of(moved);
        assert!(
            fx::wrap(heading - w.aim.yaw).abs() <= fx::deg(1) / 2,
            "{heading} {}",
            w.aim.yaw
        );
    }

    #[test]
    fn the_camera_shows_pending_mouse_movement_and_the_tick_does_not_jump_it() {
        let w = World::new(1);
        let m = Mouse {
            dx: 2_500,
            dy: 0,
            wheel: 0,
        };
        let before = w.camera(0, m, camera::BACK);
        let mut after = w.clone();
        after.step(&Input {
            dx: m.dx,
            ..Input::default()
        });
        assert_eq!(before, after.camera(0, Mouse::default(), camera::BACK));
    }

    #[test]
    fn an_unknown_action_is_refused_by_name() {
        assert_eq!(
            World::new(1).act("arm.reach", "1"),
            Err("Unknown action \"arm.reach\".".into())
        );
    }
}
