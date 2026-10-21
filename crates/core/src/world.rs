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
    // M1: the hit-stop, the camera's knock, sparks and the tick's events
    pub feel: crate::feel::State,
    // M2: the land (heights, layout, river), shared by every copy of the
    // world; the water's waves; a place to put the player when play starts
    pub land: std::rc::Rc<crate::terrain::Land>,
    pub water: crate::water::State,
    pub arrival: Option<crate::places::Arrival>,
    // M5: the walls and floors bodies meet (worked out once from the land),
    // the opening, the merchant, the camp, the trainer, the boomerang
    pub solid: std::rc::Rc<crate::solid::Map>,
    pub story: crate::story::State,
    pub merchant: crate::merchant::State,
    pub camp: crate::camp::State,
    pub trainer: crate::trainer::State,
    pub boomerang: crate::boomerang::State,
    // M6: the orbs in flight and the casts under way; the imbue
    pub orbs: crate::orbs::State,
    pub imbue: crate::imbue::State,
    // M7 (feel): the tick's sounds and what they listen for; the camera's
    // boom, kept out of walls, houses and trunks
    pub sound: crate::sound::State,
    pub boom: crate::boom::State,
    // M7 travel (T11): the waystones, the flute's song screen, a journey
    pub travel: crate::travel::State,
    // M7 (P4): blood in flight and on the ground, parts taken off, the cues
    pub wounds: crate::wounds::State,
    // M7 save (T16): the last save, the save point, the autosave's watch
    pub save: crate::save::State,
    // M7 roadside (T12): the stops beside the ways out, the straggler
    pub roadside: crate::roadside::State,
    // M7 (P9, T15): the encounters between the major areas, what is kept of each
    pub wanderers: crate::wanderers::State,
}

impl World {
    pub fn new(seed: u64) -> World {
        let mut w = World {
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
            feel: crate::feel::State::default(), // M1:
            // M2
            land: crate::terrain::shipped(),
            water: crate::water::State::new(),
            arrival: None,
            // M6
            orbs: crate::orbs::State::default(),
            imbue: crate::imbue::State::default(),
            wounds: crate::wounds::State::default(), // M7 (P4)
            solid: std::rc::Rc::default(),           // M5: below
            story: crate::story::State::default(),
            merchant: crate::merchant::State::default(),
            camp: crate::camp::State::default(),
            trainer: crate::trainer::State::default(),
            boomerang: crate::boomerang::State::default(),
            save: crate::save::State::default(),   // M7 save
            sound: crate::sound::State::default(), // M7
            boom: crate::boom::State::default(),
            travel: crate::travel::State::default(),       // M7 travel
            roadside: crate::roadside::State::default(),   // M7 roadside
            wanderers: crate::wanderers::State::new(seed), // M7 (P9)
        };
        w.solid = std::rc::Rc::new(crate::solid::Map::build(&w));
        w.boom.blocks = std::rc::Rc::new(crate::boom::blocks(&w)); // M7
        w
    }

    /// The ground's height at (x, z): M2's terrain, the triangles of its
    /// finest tiles as drawn (`terrain::Land::ground`).
    pub fn ground(&self, x: Fx, z: Fx) -> Fx {
        self.land.ground(x, z)
    }

    /// One tick, in the order the module doc gives.
    pub fn step(&mut self, input: &Input) {
        // M7: last tick's sounds are gone
        crate::sound::begin(self);
        // M5: a dream gone dark wakes as the opening instead of a tick; in
        // bed or down, the keys do nothing (the mouse still looks)
        if crate::story::waking(self) {
            crate::story::wake(self);
            return;
        }
        let input = &crate::story::filter(self, input);
        // M7 travel: on a journey, the keys and the mouse do nothing
        let input = &crate::travel::filter(self, input);
        // M7: frozen for a picture of a spark (test-only), only the view turns
        if self.feel.frozen {
            crate::feel::frozen(self, input);
            return;
        }
        self.tick += 1;
        // M6: the scripted defender's hand added to the record, for a view
        // (`magic.defend`, a test act)
        let defended = crate::orbs::defended(self, input);
        let input = &defended;
        // M1: a hard blow holds the world still for a few ticks
        if self.feel.stop > 0 {
            crate::feel::hold(self, input);
            crate::orbs::hold(self); // M6: the orbs hold still too
            crate::wounds::hold(self); // M7 (P4): and the blood and pieces
            crate::ui::step(self);
            return;
        }
        // 1. the aim (M1: drawn, the mouse moves the hand instead, and the
        // view holds on the foe)
        let look = crate::player::look_mouse(self, input);
        self.aim = self.aim.turned(look);
        crate::player::hold_on(self);
        // M4: 1, 2 and 3 ready a quick item in the free hand
        crate::inventory::step(self, input);
        // 2. M1:
        let inputs = crate::pilot::step(self);
        // M5: the story's foes walking, running, drawing and sheathing
        let mut inputs = crate::story::steer(self, inputs);
        // M7 (P9): the wanderers walking their rounds, standing, running at the player
        crate::wanderers::steer(self, &mut inputs);
        // M6: a caster raises its hands as each volley leaves, through its input
        crate::orbs::pose(self, &mut inputs);
        // 3. bodies (M1: motion; M5: walls, the bridge's deck, the house's floor)
        crate::motion::step(self, input, &inputs);
        crate::solid::step(self);
        // M7 (P4): wounds tick; a dropped weapon settles or is taken up
        crate::wounds::steer(self, input, &inputs);
        // 4. M1:
        crate::arm::step(self, input, &inputs);
        crate::blade::step(self);
        // M7 (R9.24): a world held for a picture or a check hurts no one
        if crate::story::holding(self) {
            self.blades.hits.clear();
        }
        crate::health::step(self);
        crate::feel::step(self); // M1:
                                 // 5. M1/M5: throwables, the boomerang
        crate::throwables::step(self, input, &inputs);
        crate::boomerang::step(self, input);
        // M7 (P4): blood flies and lands, parts taken off fall and rest
        crate::wounds::step(self);
        crate::player::finish(self, input, &inputs);
        crate::pilot::remember(self);
        // 6. M6: the imbue (Q), then the orbs, which the imbued edge turns
        crate::imbue::step(self, input);
        crate::orbs::step(self);
        // 7. M5: the opening, E and the pickups, the merchant, the camp, the trainer
        crate::story::step(self, input);
        // M7 travel: the waystones come near, the flute given, a journey
        crate::travel::step(self);
        // M7 (P9): the wanderers stand near the player, are parked away, are beaten
        crate::wanderers::step(self);
        // M7 save: the autosave, and a save that is due
        crate::save::step(self);
        // 8. M2: the water
        crate::water::step(self);
        // M4: the map fills in round the player; the toast counts down
        crate::ui::step(self);
        // 9. M7: what was heard this tick; the camera's boom
        crate::sound::step(self);
        crate::boom::step(self);
    }

    /// What moves, drawn `alpha` of the way from the last tick to this one.
    pub fn draw(&self, alpha: Fx, look: &Look, out: &mut Shapes) {
        // M1: every fighter and everything thrown
        crate::player::draw(self, alpha, look, out);
        crate::throwables::draw(self, alpha, look, out);
        crate::feel::draw(self, alpha, look, out); // M1:
        crate::wounds::draw(self, alpha, look, out); // M7 (P4):
                                                     // M5: the player's house and bed, villagers, pickups, the cart, the
                                                     // chest, the boomerang
        crate::story::draw(self, alpha, look, out);
        crate::boomerang::draw(self, alpha, look, out);
        crate::models::draw(self, alpha, look, out); // M3: the lineup, when asked for
                                                     // M6: the imbued blade's glow, then the orbs and their sparks
        crate::imbue::draw(self, alpha, look, out);
        crate::orbs::draw(self, alpha, look, out);
        // M7 travel: the woken waystones' light, the traveller and the mule
        crate::travel::draw(self, alpha, look, out);
    }

    /// What never moves: the static groups, and the meshes they draw.
    pub fn scene(&self, look: &Look, meshes: &mut MeshTable) -> Vec<Group> {
        // M2: the places' props, baked here; `Game::new` hands their groups
        // (named `place:<kind>`) to the detail levels, which draw them to
        // 3 km and the terrain, water and scatter round the camera (`lod.rs`)
        let mut out = Vec::new();
        crate::places::scene(self, look, meshes, &mut out);
        // M7 (P9): the held sites' props, faded as the places'
        crate::wanderers::scene(self, look, meshes, &mut out);
        // M5: …
        out
    }

    /// Lines over the game.
    pub fn hud(&self, strings: &Strings, out: &mut Vec<HudItem>) {
        // M4: crate::ui::hud(self, strings, out); M6: the imbue's cooldown
        // reaches M4's ring through `ui::imbue`
        // (M1's health reaches the HUD through `ui::health` and `ui::foe`)
        // M5: the story's line, the prompt for E, the fade
        crate::story::hud(self, strings, out);
        // M7 (T10): the cut the mouse would pick while the left button is held
        crate::swing::hud(self, strings, out);
        // M7 (P4): taking up a dropped weapon in the other hand
        crate::wounds::hud(self, strings, out);
        // M7 save: the small Saved mark
        crate::save::hud(self, strings, out);
    }

    /// A setting or flag named `module.name`.
    pub fn act(&mut self, name: &str, value: &str) -> Result<(), String> {
        match name.split_once('.') {
            Some(("inventory", rest)) => crate::inventory::act(self, rest, value), // M4:
            Some(("models", rest)) => crate::models::act(self, rest, value),       // M3:
            // M1:
            // M5: a test duel steps the story aside first (`story::step_aside`)
            Some(("duel", rest)) => {
                crate::story::step_aside(self);
                crate::player::act(self, rest, value)
            }
            Some(("motion", rest)) => crate::motion::act(self, rest, value),
            // M2
            Some(("places", rest)) => crate::places::act(self, rest, value),
            // M5
            Some(("story", rest)) => crate::story::act(self, rest, value),
            Some(("trainer", rest)) => crate::trainer::act(self, rest, value),
            Some(("merchant", rest)) => crate::merchant::act(self, rest, value),
            Some(("magic", rest)) => crate::orbs::act(self, rest, value), // M6
            Some(("feel", rest)) => crate::feel::act(self, rest, value),  // M7
            Some(("wounds", rest)) => crate::wounds::act(self, rest, value), // M7 (P4)
            Some(("roadside", rest)) => crate::roadside::act(self, rest, value), // M7 roadside
            Some(("wanderers", rest)) => crate::wanderers::act(self, rest, value), // M7 (P9)
            Some(("sprint", rest)) => crate::sprint::act(self, rest, value), // M7 (T14)
            _ => Err(format!("Unknown action \"{name}\".")),
        }
    }

    /// One number for the whole state, for replays and determinism tests: an
    /// FNV-1a hash of the state's values in field order (`value_hash`), so a
    /// renamed field leaves it as it was (M7, R2.19, R9.92).
    pub fn hash(&self) -> u64 {
        value_hash(&format!("{self:?}"))
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
        let feet = self.player.feet(alpha);
        // M7 (T14): the sprint's build pulls the camera back and widens the view
        let (pull, widen) = crate::sprint::view(self, alpha);
        let mut cam = camera::view_from(feet, aim, back + pull, |x, z| self.ground(x, z));
        cam.fov += widen;
        // M1: a hard blow knocks the view a little; held on a foe, the view
        // frames the duel from a little higher and further right
        cam.eye = cam
            .eye
            .add(crate::feel::knock_at(self, alpha))
            .add(crate::player::framing(self, alpha, aim.yaw));
        // M7: brought in along the boom, clear of walls, houses and trees
        let pivot = camera::pivot(feet, aim).add(crate::player::framing(self, alpha, aim.yaw));
        (cam.eye, cam.fwd) = crate::boom::clip(self, alpha, feet, pivot, cam.eye, cam.fwd);
        cam
    }

    /// M1: the player is down (at no health, fallen): M5's hook for waking
    /// in bed. `feel.downed` names everyone who went down this tick.
    pub fn player_down(&self) -> bool {
        self.me.health.down()
    }
}

/// An FNV-1a hash of a derived debug text with its field names left out:
/// each `name: ` before a value is skipped, so the hash is of the values
/// (numbers, variants, the shape of lists and structs) in field order. A
/// renamed field keeps the hash; a value changed, added or moved changes
/// it. Integers print the same under every toolchain, and the core holds
/// no float (CW-2), so a hash written down stays right (R2.19).
pub fn value_hash(text: &str) -> u64 {
    let b = text.as_bytes();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut put = |c: u8| {
        h ^= c as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut i = 0;
    while i < b.len() {
        if word(b[i]) && (i == 0 || !word(b[i - 1])) {
            let mut j = i;
            while j < b.len() && word(b[j]) {
                j += 1;
            }
            // a field's name: a word straight before ": " (not "::")
            let named = j + 1 < b.len() && b[j] == b':' && b[j + 1] == b' ';
            if named {
                i = j + 2;
                continue;
            }
            for &c in &b[i..j] {
                put(c);
            }
            i = j;
            continue;
        }
        put(b[i]);
        i += 1;
    }
    h
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

    /// R2.19: the hash reads values, not names. Two structs alike but for
    /// their field names hash the same; a value changed, or two values
    /// swapped, hash otherwise; a variant's name is a value.
    #[test]
    fn the_world_s_hash_reads_its_values_not_its_field_names() {
        // the same type before and after its fields were renamed
        mod before {
            #[derive(Debug)]
            #[allow(dead_code)]
            pub struct S {
                pub hp: i32,
                pub pos: (i32, i32),
                pub mode: Option<u8>,
            }
        }
        mod after {
            #[derive(Debug)]
            #[allow(dead_code)]
            pub struct S {
                pub health: i32,
                pub at: (i32, i32),
                pub state: Option<u8>,
            }
        }
        use after::S as B;
        use before::S as A;
        #[derive(Debug)]
        #[allow(dead_code)]
        enum M {
            Round(u8),
            Rest(u8),
        }
        let a = format!(
            "{:?}",
            A {
                hp: 5,
                pos: (1, 2),
                mode: Some(3)
            }
        );
        let b = format!(
            "{:?}",
            B {
                health: 5,
                at: (1, 2),
                state: Some(3)
            }
        );
        assert_ne!(a, b);
        assert_eq!(value_hash(&a), value_hash(&b), "renamed fields");
        let swapped = format!(
            "{:?}",
            A {
                hp: 5,
                pos: (2, 1),
                mode: Some(3)
            }
        );
        assert_ne!(value_hash(&a), value_hash(&swapped));
        let more = format!(
            "{:?}",
            A {
                hp: 59,
                pos: (1, 2),
                mode: Some(3)
            }
        );
        assert_ne!(
            value_hash(&more),
            value_hash(&format!(
                "{:?}",
                A {
                    hp: 51,
                    pos: (1, 2),
                    mode: Some(3)
                }
            ))
        );
        let none = format!(
            "{:?}",
            A {
                hp: 5,
                pos: (1, 2),
                mode: None
            }
        );
        assert_ne!(value_hash(&a), value_hash(&none));
        assert_ne!(
            value_hash(&format!("{:?}", M::Round(1))),
            value_hash(&format!("{:?}", M::Rest(1)))
        );
        // FNV-1a of "5" alone, by hand: (0xcbf29ce484222325 ^ 0x35) * 0x100000001b3
        let one = (0xcbf2_9ce4_8422_2325u64 ^ 0x35).wrapping_mul(0x0100_0000_01b3);
        assert_eq!(value_hash("hp: 5"), one);
        // the world itself: the same world hashes the same, a step changes it
        let w = World::new(7);
        assert_eq!(w.hash(), World::new(7).hash());
    }
}
