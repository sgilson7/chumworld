//! `story` (M5 Places and story, task S): the opening, and what the rest of
//! the story shares.
//!
//! # The opening (the brief: "you being awoken to your village being raided
//! by orcs, and if you are killed by one, you wake up back in bed")
//!
//! `opening(seed)` is the game's start: the player lies in bed in their
//! house (`solid::Home`), the house drawn cut away so the room shows, while
//! six orcs walk their rounds of the village (`data/story.json`) and the
//! villagers flee. The player gets up after `WAKE_TICKS`, or at once on a
//! movement key. An orc that sees the player (`SIGHT`) runs at them, draws
//! within `DRAW_AT` and duels by its own behaviour tree (M1's pilot); one that
//! falls behind past `SHEATHE_AT` puts its blade away and runs again. An orc
//! runs four ticks in five, a little slower than the player's run, so a
//! player who runs gets away and one who stands to fight is soon outnumbered.
//! A fallen orc is followed by another (`COMES_AFTER`) from the village's
//! south edge (the journal's CW-Q5: "an orc can be beaten but another always
//! comes").
//!
//! Down to an orc, the player falls; the view fades (`veil`); the world is
//! the opening again, from the same seed (CW-Q5: the raid replays, so the
//! way out can be learned), and over the fading-in bed the player says
//! "Huh. What a funny dream." (from the third dream, a second line). Crossing
//! `BOUNDARY` (250 m from the village centre, PLAN.md) ends the opening: the
//! orcs turn back, a line says so, the quest moves on, and from then on the
//! player is free. The orcs keep the village (the journal: "the orcs hold
//! it and it is dangerous").
//!
//! # How the story moves foes (CW-3)
//!
//! Every orc, bandit and the leader is M1's foe with its own pilot. When it
//! is not duelling, the story's `steer` gives it an input record of its own:
//! the keys to walk its round or run toward the player, and X to draw or put
//! the blade away. The body still moves only through that record, by the
//! same feet as the player's.
//!
//! # What the rest of M5 shares from here
//!
//! The pickups (`Gift`, `Pickup`: what lies in the world for E to take), the
//! one use key (`USE`, E) and its prompt, the line over the game
//! (`banner`), `keys_toward` and `sheath_keys` for steering a foe, and the
//! quest's line (`task`, M4's `ui::task` door).
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{facing, right_of, v3, yaw_of, Affine, V3};
use crate::input::{self, Input};
use crate::items::Item;
use crate::look::{rgb, Look};
use crate::models::{self, Prop};
use crate::page::Strings;
use crate::pilot::{self, Control};
use crate::player::{Body, Fighter, Sheath};
use crate::pose::{Held, Kind, Pose};
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::rng::Rng;
use crate::solid;
use crate::world::{HudItem, World};

/// The opening ends this far from the village centre (PLAN.md).
pub const BOUNDARY: Fx = fx::int(250);
/// An orc sees the player this far off.
pub const SIGHT: Fx = fx::int(22);
/// A running orc draws its blade this near, and puts it away to run again
/// once the player is this far.
pub const DRAW_AT: Fx = fx::int(6);
pub const SHEATHE_AT: Fx = fx::int(14);
/// A chasing orc holds Shift this many ticks of every `RUN_OF`: a little
/// slower than the player's run.
pub const RUN_TICKS: u64 = 4;
pub const RUN_OF: u64 = 5;
/// An orc pauses at each point of its round for 1 to 3 s.
pub const PAUSE: (i32, i32) = (60, 180);
/// At a point of a round, within this.
pub const ARRIVED: Fx = ONE * 12 / 10;
/// After an orc falls, another comes this many ticks later (6 s), up to
/// `MOST_ORCS` in one opening.
pub const COMES_AFTER: u32 = 360;
pub const MOST_ORCS: usize = 12;
/// The player lies this long before getting up on their own (1.5 s).
pub const WAKE_TICKS: u32 = 90;
/// Getting up takes as long as falling (M1's `health::FALL_TICKS`).
pub const RISE_TICKS: u32 = crate::health::FALL_TICKS as u32;
/// Lying in bed, the body is this far over the floor: on the quilt.
pub const BED_TOP: Fx = ONE * 62 / 100;
/// The heels lie this far past the bed's middle, toward its foot; getting up,
/// the body slides on to `STAND` (clear of the bed by a body's radius).
pub const HEELS: Fx = ONE * 115 / 100;
pub const STAND: Fx = ONE * 142 / 100;
/// Down: the fall shows this long before the view fades; the fade is dark
/// this long before the world is the opening again; it lifts this long
/// after; and the dream line comes once it has lifted.
pub const FALL_SHOW: u32 = 60;
pub const FADE_TICKS: u32 = 80;
pub const DARK_TICKS: u32 = 30;
pub const LINE_AFTER: u32 = 80;
/// From this dream on, the bed line is the second one.
pub const AGAIN_FROM: u32 = 3;
/// A line over the game shows this long (5 s).
pub const BANNER_TICKS: u32 = 300;
/// E reaches this far.
pub const REACH: Fx = ONE * 22 / 10;
/// A walking villager's pace while fleeing: Shift held this many ticks of
/// every `RUN_OF`.
pub const FLEE_RUN_TICKS: u64 = 3;
/// A fleeing villager is gone once past this far from the village centre.
pub const GONE_AT: Fx = fx::int(170);
/// The player's house is cut away (its roof and the top of its walls left
/// off) while the player is within this of its footprint.
pub const CUT_NEAR: Fx = ONE / 2;
/// Its walls stand this high over the floor when cut away.
pub const CUT_WALL: Fx = ONE * 95 / 100;

// ---- data/story.json ------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OrcSpec {
    pub route: Vec<[i32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RaidSpec {
    pub orcs: Vec<OrcSpec>,
    pub comes_from: Vec<[i32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VillagerSpec {
    pub at: [i32; 2],
    pub look: u8,
    pub flee: Vec<[i32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PickupSpec {
    #[serde(default)]
    pub item: Option<String>,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub coins: Option<u32>,
    pub place: String,
    pub at_cm: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MerchantSpec {
    pub west_x: i32,
    pub east_x: i32,
    pub start_x: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CampSpec {
    pub lookout: [i32; 2],
    pub bandits: Vec<[i32; 2]>,
    pub leader: [i32; 2],
    pub bandit_coins: u32,
    pub leader_coins: u32,
    pub bandit_knives: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TrainerSpec {
    pub at: [i32; 2],
}

/// `data/story.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub raid: RaidSpec,
    pub villagers: Vec<VillagerSpec>,
    pub pickups: Vec<PickupSpec>,
    pub merchant: MerchantSpec,
    pub camp: CampSpec,
    pub trainer: TrainerSpec,
}

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let s: Spec =
            serde_json::from_str(json).map_err(|e| format!("The story file could not be read: {e}"))?;
        for p in &s.pickups {
            match (&p.item, p.coins) {
                (Some(id), None) if Item::from_id(id).is_some() => {}
                (None, Some(_)) => {}
                _ => {
                    return Err(format!(
                        "A pickup in the story file is neither an item nor coins: {p:?}."
                    ))
                }
            }
        }
        if s.raid.orcs.iter().any(|o| o.route.is_empty()) {
            return Err("An orc in the story file has no round to walk.".into());
        }
        Ok(s)
    }
}

/// The story file the game ships with, read once.
pub fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| Spec::parse(include_str!("../../../data/story.json")).expect("data/story.json"))
}

// ---- state ------------------------------------------------------------------

/// Where the opening stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// no opening: a bare world (`World::new`, tests, the test duels)
    Off,
    /// lying in bed, ticks so far
    Bed(u32),
    /// getting up, ticks so far
    Rising(u32),
    /// up, in the raid
    Raid,
    /// down to an orc, ticks since: the fall, the fade, the dream
    Dream(u32),
    /// past the boundary: the opening is over
    Free,
    /// down after the opening, ticks since: the fade, then coming to
    Fallen(u32),
}

/// What an orc of the raid is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// walking its round
    Round,
    /// after the player
    Chase,
    /// walking back to its round
    Back,
}

/// One orc of the raid: which foe it is, its round, and where it is in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raider {
    pub foe: usize,
    pub route: usize,
    pub next: usize,
    pub mode: Mode,
    /// ticks left to wait at a point
    pub wait: u32,
    /// the share carried between two key mixes (`keys_toward`)
    pub acc: Fx,
    /// ticks since it fell, once down
    pub down: u32,
    /// another has come in its place
    pub followed: bool,
}

/// A villager fleeing the raid: a body on its own feet, drawn by the
/// story (villagers are no fighters: nothing duels them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Villager {
    pub body: Body,
    pub gait: crate::motion::Gait,
    pub look: u8,
    pub route: Vec<V3>,
    pub next: usize,
    pub gone: bool,
}

/// What a pickup gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gift {
    Item(Item, u32),
    Coins(u32),
}

/// Something lying in the world for E to take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pickup {
    pub gift: Gift,
    pub at: V3,
    pub yaw: i32,
    pub taken: bool,
}

/// What E would do here, nearest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    Pickup(usize),
    /// a thrown rock or knife lying where it stopped (`throwables::resting`)
    Thrown(usize),
    Merchant,
    Trainer,
    Chest,
}

/// A line over the game, by string key, and how long it has left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Banner {
    pub key: String,
    pub ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub phase: Phase,
    /// the seed the opening is built from, so a dream rebuilds it
    pub seed: u64,
    /// how many times the player has dreamt
    pub dreams: u32,
    /// this opening began from a dream: the line comes as the fade lifts
    pub dreamt: bool,
    /// the view fades to dark while set (the page eases it)
    pub veil: bool,
    pub raid: Vec<Raider>,
    pub villagers: Vec<Villager>,
    pub pickups: Vec<Pickup>,
    pub banner: Option<Banner>,
    /// how far over what it stands on the player's body is (lying in bed)
    pub lift: Fx,
    /// where the player crossed the boundary, to come to after a fall
    pub crossed: Option<V3>,
    /// last tick's keys, so E acts once a press
    pub last_keys: u32,
    /// test-only: everything the story moves holds still (`story.still`)
    pub still: bool,
    /// chance for the story's own choices, from the seed
    pub rng: Rng,
}

impl Default for State {
    fn default() -> State {
        State {
            phase: Phase::Off,
            seed: 0,
            dreams: 0,
            dreamt: false,
            veil: false,
            raid: Vec::new(),
            villagers: Vec::new(),
            pickups: Vec::new(),
            banner: None,
            lift: 0,
            crossed: None,
            last_keys: 0,
            still: false,
            rng: Rng::new(0),
        }
    }
}

impl State {
    /// The opening is under way: in bed, getting up, in the raid, dreaming.
    pub fn opening(&self) -> bool {
        matches!(
            self.phase,
            Phase::Bed(_) | Phase::Rising(_) | Phase::Raid | Phase::Dream(_)
        )
    }
}

// ---- places -----------------------------------------------------------------

/// The village centre (PLAN.md: (2.0 km, 1.2 km) on the map), on the ground.
pub fn centre(w: &World) -> V3 {
    crate::places::at(w, "village").unwrap_or(crate::world::START)
}

/// A point `at` metres east and south of a place's centre, on the ground.
pub fn near_place(w: &World, place: &str, at: [i32; 2]) -> V3 {
    let c = crate::places::at(w, place).unwrap_or(crate::world::START);
    let mut p = c.add(v3(at[0] * ONE, 0, at[1] * ONE));
    p.y = solid::surface(w, p.x, p.z);
    p
}

/// A point in the bed's own frame: `right` metres to its right (seen lying
/// in it) and `foot` metres toward its foot.
pub fn bed_point(w: &World, right: Fx, foot: Fx) -> Option<V3> {
    let h = w.solid.home?;
    let f = facing(h.bed.yaw, 0);
    let r = right_of(h.bed.yaw).neg();
    let mut p = h.bed.at.add(f.scale(foot)).add(r.scale(right));
    p.y = solid::surface(w, p.x, p.z);
    Some(p)
}

fn cm(v: i32) -> Fx {
    v * ONE / 100
}

fn pickup_at(w: &World, p: &PickupSpec) -> V3 {
    if p.place == "bed" {
        bed_point(w, cm(p.at_cm[0]), cm(p.at_cm[1])).unwrap_or(crate::world::START)
    } else {
        let c = crate::places::at(w, &p.place).unwrap_or(crate::world::START);
        let mut q = c.add(v3(cm(p.at_cm[0]), 0, cm(p.at_cm[1])));
        q.y = solid::surface(w, q.x, q.z);
        q
    }
}

// ---- the opening's start ----------------------------------------------------

/// The game's start from `seed`: a world with the opening begun.
pub fn opening(seed: u64) -> World {
    let mut w = World::new(seed);
    begin(&mut w, seed);
    w
}

/// Begins the opening on a fresh world: in bed, the raid, the villagers, the
/// pickups, the merchant on the road.
pub fn begin(w: &mut World, seed: u64) {
    let sp = spec();
    let st = &mut w.story;
    st.seed = seed;
    st.rng = Rng::new(seed ^ 0x005e_ed0f_5702);
    st.phase = Phase::Bed(0);
    lie_down(w);
    arrive(w);
    // the raid
    for (k, o) in sp.raid.orcs.iter().enumerate() {
        let at = near_place(w, "village", o.route[0]);
        let next = near_place(w, "village", o.route[1 % o.route.len()]);
        add_orc(w, at, yaw_of(next.sub(at)), k);
    }
    // the villagers
    for v in &sp.villagers {
        let at = near_place(w, "village", v.at);
        let route: Vec<V3> = v.flee.iter().map(|p| near_place(w, "village", *p)).collect();
        let yaw = yaw_of(route[0].sub(at));
        w.story.villagers.push(Villager {
            body: Body::new(at, yaw),
            gait: crate::motion::Gait::default(),
            look: v.look,
            route,
            next: 0,
            gone: false,
        });
    }
    // what lies in the world
    for p in &sp.pickups {
        let gift = match (&p.item, p.coins) {
            (Some(id), _) => Gift::Item(Item::from_id(id).expect("checked"), p.count.unwrap_or(1)),
            (None, Some(n)) => Gift::Coins(n),
            _ => continue,
        };
        let at = pickup_at(w, p);
        let yaw = fx::deg((w.story.pickups.len() as i32 * 67) % 360);
        w.story.pickups.push(Pickup {
            gift,
            at,
            yaw,
            taken: false,
        });
    }
    crate::merchant::begin(w);
}

/// The player lying in bed: their heels past its foot, on the quilt.
fn lie_down(w: &mut World) {
    let Some(h) = w.solid.home else {
        return;
    };
    let Some(mut heels) = bed_point(w, 0, HEELS) else {
        return;
    };
    heels.y += BED_TOP;
    let yaw = h.bed.yaw;
    w.player = Body::new(heels, yaw);
    let held = crate::items::held(w);
    w.me = Fighter::new(Kind::Player, held, 0, heels, yaw, false);
    w.me.health.down_ticks = crate::health::FALL_TICKS;
    w.story.lift = BED_TOP;
    w.aim.yaw = fx::wrap(yaw + fx::HALF_TURN);
}

/// A foe of `kind` with its blade put away: the story's foes walk sheathed
/// until there is someone to fight.
pub fn sheathed_foe(w: &mut World, kind: Kind, look: Kind, at: V3, yaw: i32) -> usize {
    let salt = w.rng.next_u64();
    let mut foe = pilot::foe(&w.pilots.book, kind, at, yaw, salt);
    let f = &foe.fighter;
    let mut g = Fighter::new(look, f.held, f.team, at, yaw, false);
    g.health = f.health;
    g.servo_pct = f.servo_pct;
    g.pouch = f.pouch;
    // a foe's mouse always moves its hand: it looks where its pilot holds
    // on (`Fighter::new` sets this for a sheathed player, whose mouse looks)
    g.mouse_looks = false;
    foe.fighter = g;
    foe.brain.control = Control::Tree;
    w.foes.push(foe);
    w.foes.len() - 1
}

fn add_orc(w: &mut World, at: V3, yaw: i32, route: usize) {
    let foe = sheathed_foe(w, Kind::Orc, Kind::Orc, at, yaw);
    let len = spec().raid.orcs[route].route.len();
    let wait = w.story.rng.between(PAUSE.0, PAUSE.1) as u32;
    w.story.raid.push(Raider {
        foe,
        route,
        next: 1 % len,
        mode: Mode::Round,
        wait,
        acc: 0,
        down: 0,
        followed: false,
    });
}

/// In bed, the view looks across the bed from its side, a little toward its
/// foot, down into the room and out over the meadow (the way out): the title's view (`Game::title_aim`), so
/// play opens on it without a jump, and the view after a dream.
pub fn title_view(w: &World) -> Option<(i32, i32)> {
    if !(matches!(w.story.phase, Phase::Bed(_)) && in_bed(w)) {
        return None;
    }
    let h = w.solid.home?;
    Some((fx::wrap(h.bed.yaw + fx::QUARTER_TURN + deg(8)), -deg(34)))
}

/// After a dream, the view is the title's.
pub fn arrive(w: &mut World) {
    if let Some((yaw, pitch)) = title_view(w) {
        w.aim.yaw = yaw;
        w.aim.pitch = pitch;
    }
}

/// Whether the player is in the bed (lying, or getting up), where the
/// opening put them.
pub fn in_bed(w: &World) -> bool {
    matches!(w.story.phase, Phase::Bed(_) | Phase::Rising(_))
        && bed_point(w, 0, HEELS).is_some_and(|p| p.sub(w.player.pos).len_xz() < ONE * 4 / 10)
}

/// The story's foes and walkers hold still, and the opening is over: for a
/// test duel or a view staged from the address. A duel act calls it
/// first (`World::act`), so a test duel never meets the raid.
pub fn step_aside(w: &mut World) {
    if w.story.opening() {
        let raid: Vec<usize> = w.story.raid.iter().map(|r| r.foe).collect();
        // the raid's foes are the first ones made; nothing else stands yet
        if raid.iter().enumerate().all(|(k, f)| *f == k) && w.foes.len() == raid.len() {
            w.foes.clear();
        }
        w.story.raid.clear();
        w.story.villagers.clear();
        if in_bed(w) {
            let held = crate::items::held(w);
            let at = crate::world::START;
            w.player = Body::new(at, crate::world::START_YAW);
            w.me = Fighter::new(Kind::Player, held, 0, at, crate::world::START_YAW, false);
            // and the view as the game had it before the opening
            let pct = w.aim.pct;
            w.aim = crate::camera::Aim::new(crate::world::START_YAW);
            w.aim.pct = pct;
        }
        w.story.lift = 0;
        w.story.phase = Phase::Free;
    }
    w.story.raid.clear();
    crate::camp::forget(w);
    crate::trainer::forget(w);
}

// ---- the tick -----------------------------------------------------------------

/// Step 0 of the tick: lying, getting up or down, the player's keys and
/// buttons do nothing (the mouse still looks). A movement key in bed gets
/// the player up.
pub fn filter(w: &mut World, input: &Input) -> Input {
    let held = match w.story.phase {
        Phase::Bed(_) | Phase::Rising(_) => in_bed(w),
        Phase::Dream(_) | Phase::Fallen(_) => true,
        _ => false,
    };
    if !held {
        return *input;
    }
    if let Phase::Bed(_) = w.story.phase {
        let moves = input::FORWARD | input::BACK | input::LEFT | input::RIGHT;
        if input.keys & moves != 0 && !w.story.dreamt_line_pending() && !w.story.still {
            w.story.phase = Phase::Rising(0);
        }
    }
    Input {
        keys: 0,
        buttons: 0,
        ..*input
    }
}

impl State {
    /// A dream's line is still to come: the player waits in bed for it.
    fn dreamt_line_pending(&self) -> bool {
        matches!(self.phase, Phase::Bed(t) if self.dreamt && t <= DARK_TICKS + LINE_AFTER)
    }
}

/// Whether this tick begins with the world rebuilt as the opening (a dream
/// has gone dark): `World::step` calls `wake` instead of a tick.
pub fn waking(w: &World) -> bool {
    matches!(w.story.phase, Phase::Dream(t) if t >= FALL_SHOW + FADE_TICKS)
}

/// The dream ends: the world is the opening again from the same seed, the
/// reader's settings kept, the fade still dark, the line to come.
pub fn wake(w: &mut World) {
    let dreams = w.story.dreams + 1;
    let settings = w.ui.settings;
    let pct = w.aim.pct;
    let mut fresh = opening(w.story.seed);
    fresh.ui.settings = settings;
    fresh.aim.pct = pct;
    fresh.story.dreams = dreams;
    fresh.story.dreamt = true;
    fresh.story.veil = true;
    *w = fresh;
    arrive(w);
}

/// Step 2b, after the pilots: each story foe that is not duelling gets its
/// record from the story (CW-3: still only a record).
pub fn steer(w: &mut World, mut inputs: Vec<Input>) -> Vec<Input> {
    steer_raid(w, &mut inputs);
    crate::camp::steer(w, &mut inputs);
    crate::trainer::steer(w, &mut inputs);
    if w.story.still {
        for i in inputs.iter_mut() {
            *i = Input::default();
        }
    }
    inputs
}

fn steer_raid(w: &mut World, inputs: &mut [Input]) {
    let sp = spec();
    let me = w.player.pos;
    let free = !matches!(w.story.phase, Phase::Raid | Phase::Rising(_) | Phase::Bed(_));
    let player_down = w.player_down();
    let home = w.solid.home;
    let hidden = |p: V3| home.is_some_and(|h| h.wall.holds(p.x, p.z, 0));
    let near_centre = centre(w).sub(me).len_xz() < BOUNDARY;
    for k in 0..w.story.raid.len() {
        let r = w.story.raid[k].clone();
        let Some(foe) = w.foes.get(r.foe) else {
            continue;
        };
        if foe.brain.control != Control::Tree || foe.fighter.health.down() {
            continue;
        }
        let f = &foe.fighter;
        let at = foe.body.pos;
        let gap = me.sub(at).len_xz();
        let sees = !free
            && !player_down
            && !in_bed(w)
            && near_centre
            && (gap < SIGHT || r.mode == Mode::Chase)
            && !(hidden(me) && !hidden(at));
        let mut r = r;
        r.mode = match (r.mode, sees) {
            (_, true) => Mode::Chase,
            (Mode::Chase, false) => Mode::Back,
            (m, false) => m,
        };
        let route = &sp.raid.orcs[r.route].route;
        let input = match r.mode {
            Mode::Chase => {
                let drawn = f.drawn();
                let far = if drawn { gap > SHEATHE_AT } else { gap > DRAW_AT };
                if far {
                    if f.sheath == Sheath::Drawn {
                        Input {
                            keys: sheath_keys(f, false),
                            ..Input::default()
                        }
                    } else if f.sheath == Sheath::Sheathed {
                        let run = (w.tick + r.foe as u64) % RUN_OF < RUN_TICKS;
                        let keys = keys_toward(f.look_yaw, me.sub(at), &mut r.acc);
                        Input {
                            keys: keys | if run { input::RUN } else { 0 },
                            ..Input::default()
                        }
                    } else {
                        Input::default()
                    }
                } else if f.sheath == Sheath::Sheathed {
                    Input {
                        keys: sheath_keys(f, true) | keys_toward(f.look_yaw, me.sub(at), &mut r.acc),
                        ..Input::default()
                    }
                } else {
                    // drawn and near: its own tree fights
                    inputs[r.foe]
                }
            }
            Mode::Round | Mode::Back => {
                if f.drawn() {
                    Input {
                        keys: sheath_keys(f, false),
                        ..Input::default()
                    }
                } else {
                    let goal = near_place(w, "village", route[r.next]);
                    if goal.sub(at).len_xz() < ARRIVED {
                        r.mode = Mode::Round;
                        if r.wait > 0 {
                            r.wait -= 1;
                        } else {
                            r.next = (r.next + 1) % route.len();
                            r.wait = w.story.rng.between(PAUSE.0, PAUSE.1) as u32;
                        }
                        Input::default()
                    } else {
                        Input {
                            keys: keys_toward(f.look_yaw, goal.sub(at), &mut r.acc),
                            ..Input::default()
                        }
                    }
                }
            }
        };
        inputs[r.foe] = input;
        w.story.raid[k] = r;
    }
}

/// The movement keys that walk a body looking along `look` toward `way`:
/// the nearest two of the eight key mixes, taken in turn so that their mean
/// heads along `way` (`acc` carries the share between ticks).
pub fn keys_toward(look: i32, way: V3, acc: &mut Fx) -> u32 {
    if way.len_xz() < ONE / 20 {
        return 0;
    }
    // the eight mixes by heading, counter-clockwise from straight ahead (a
    // heading grows turning left)
    const MIX: [u32; 8] = [
        input::FORWARD,
        input::FORWARD | input::LEFT,
        input::LEFT,
        input::BACK | input::LEFT,
        input::BACK,
        input::BACK | input::RIGHT,
        input::RIGHT,
        input::FORWARD | input::RIGHT,
    ];
    let eighth = fx::TURN / 8;
    let rel = (yaw_of(way) - look).rem_euclid(fx::TURN);
    let k = (rel / eighth) as usize;
    let share = (rel % eighth) * ONE / eighth;
    *acc += share;
    if *acc >= ONE {
        *acc -= ONE;
        MIX[(k + 1) % 8]
    } else {
        MIX[k]
    }
}

/// X, if pressing it now takes a fighter's blade toward `drawn`: a press
/// only from a still state, and never two ticks running.
pub fn sheath_keys(f: &Fighter, drawn: bool) -> u32 {
    let want = match f.sheath {
        Sheath::Sheathed => drawn,
        Sheath::Drawn => !drawn,
        _ => false,
    };
    if want && !f.last.key(input::DRAW) {
        input::DRAW
    } else {
        0
    }
}

/// Step 7: the opening's phases, the raid's reinforcements, the villagers,
/// E and the pickups, the line over the game; then the merchant, the camp
/// and the trainer.
pub fn step(w: &mut World, raw: &Input) {
    let used = raw.key(input::USE) && w.story.last_keys & input::USE == 0;
    w.story.last_keys = raw.keys;
    if let Some(b) = &mut w.story.banner {
        b.ticks = b.ticks.saturating_sub(1);
        if b.ticks == 0 {
            w.story.banner = None;
        }
    }
    phases(w);
    reinforce(w);
    flee(w);
    if used && !w.player_down() && !in_bed(w) {
        if let Some((u, _)) = nearest_use(w) {
            act_on(w, u);
        }
    }
    crate::merchant::step(w);
    crate::camp::step(w);
    crate::trainer::step(w);
}

fn phases(w: &mut World) {
    let down = w.player_down();
    let st = &mut w.story;
    match st.phase {
        Phase::Off | Phase::Free => {}
        Phase::Bed(t) => {
            // a dream's fade lifts, then the line; on its own, up after a while
            if st.dreamt && t == DARK_TICKS {
                st.veil = false;
            }
            if st.dreamt && t == DARK_TICKS + LINE_AFTER {
                let line = if st.dreams >= AGAIN_FROM {
                    "dream_line_again"
                } else {
                    "dream_line"
                };
                crate::ui::say(w, "you", &[line]);
            }
            let st = &mut w.story;
            let wait = if st.dreamt {
                DARK_TICKS + LINE_AFTER + WAKE_TICKS
            } else {
                WAKE_TICKS
            };
            st.phase = if st.still {
                // held for a picture: the player lies on
                Phase::Bed(t)
            } else if t + 1 >= wait {
                Phase::Rising(0)
            } else {
                Phase::Bed(t + 1)
            };
            if !in_bed(w) {
                // moved out of bed (`places.go`): the raid starts at once
                w.story.phase = Phase::Raid;
                w.story.lift = 0;
            }
        }
        Phase::Rising(t) => {
            if !in_bed(w) {
                w.story.phase = Phase::Raid;
                w.story.lift = 0;
            } else {
                let left = RISE_TICKS.saturating_sub(t + 1);
                w.me.health.down_ticks = left as u16;
                let fall = w.me.health.fall();
                w.story.lift = fx::mul(BED_TOP, fall);
                // sliding off the bed's foot as the body comes up
                let along = HEELS + fx::mul(STAND - HEELS, ONE - fall);
                if let Some(p) = bed_point(w, 0, along) {
                    w.player.pos.x = p.x;
                    w.player.pos.z = p.z;
                }
                if left == 0 {
                    // standing at the bed's foot, the view easing round behind
                    w.story.phase = Phase::Raid;
                    w.story.lift = 0;
                    banner(w, "story_wake");
                } else {
                    w.story.phase = Phase::Rising(t + 1);
                }
            }
        }
        Phase::Raid => {
            if down {
                st.phase = Phase::Dream(0);
            } else {
                boundary(w);
            }
        }
        Phase::Dream(t) => {
            if t == FALL_SHOW {
                st.veil = true;
            }
            st.phase = Phase::Dream(t + 1);
        }
        Phase::Fallen(t) => {
            if t == FALL_SHOW {
                st.veil = true;
            }
            if t >= FALL_SHOW + FADE_TICKS {
                come_to(w);
            } else {
                st.phase = Phase::Fallen(t + 1);
            }
        }
    }
    if w.story.phase == Phase::Free && w.player_down() {
        w.story.phase = Phase::Fallen(0);
    }
}

/// The opening ends once the player stands `BOUNDARY` or more from the
/// village centre. Walked across (the last tick was inside), a line says
/// so; put there (`places.go`), the opening ends without one.
pub fn boundary(w: &mut World) {
    let c = centre(w);
    let now = w.player.pos.sub(c).len_xz();
    if now < BOUNDARY {
        return;
    }
    let before = w.player.prev_pos.sub(c).len_xz();
    w.story.phase = Phase::Free;
    w.story.crossed = Some(w.player.pos);
    for r in &mut w.story.raid {
        if r.mode == Mode::Chase {
            r.mode = Mode::Back;
        }
    }
    if before < BOUNDARY {
        banner(w, "story_free");
    }
}

/// After the opening, down: the player comes to where they left the
/// village, whole, their things kept; the world goes on.
fn come_to(w: &mut World) {
    let at = w.story.crossed.unwrap_or_else(|| centre(w));
    let mut at = at;
    at.y = solid::surface(w, at.x, at.z);
    let held = crate::items::held(w);
    let yaw = yaw_of(at.sub(centre(w)));
    w.player = Body::new(at, yaw);
    w.me = Fighter::new(Kind::Player, held, 0, at, yaw, false);
    w.aim.yaw = yaw;
    w.story.veil = false;
    w.story.phase = Phase::Free;
    banner(w, "story_came_to");
}

/// A fallen orc is followed by another, from the south edge farthest from
/// the player, while the opening lasts and up to `MOST_ORCS`.
fn reinforce(w: &mut World) {
    if !matches!(w.story.phase, Phase::Raid | Phase::Bed(_) | Phase::Rising(_)) {
        return;
    }
    let sp = spec();
    for k in 0..w.story.raid.len() {
        let r = &w.story.raid[k];
        let down = w.foes.get(r.foe).is_some_and(|f| f.fighter.health.down());
        if !down || r.followed {
            continue;
        }
        let full = w.story.raid.len() >= MOST_ORCS;
        let r = &mut w.story.raid[k];
        r.down += 1;
        if r.down < COMES_AFTER || full {
            continue;
        }
        r.followed = true;
        let route = r.route;
        let me = w.player.pos;
        let entry = sp
            .raid
            .comes_from
            .iter()
            .map(|p| near_place(w, "village", *p))
            .max_by_key(|p| p.sub(me).len_xz())
            .unwrap_or(crate::world::START);
        add_orc(w, entry, yaw_of(me.sub(entry)), route);
        if let Some(n) = w.story.raid.last_mut() {
            n.mode = Mode::Back;
        }
    }
}

/// The villagers run their ways out once the player is up, and are gone
/// past `GONE_AT`.
fn flee(w: &mut World) {
    if w.story.still || matches!(w.story.phase, Phase::Bed(_) | Phase::Rising(_)) {
        return;
    }
    let c = centre(w);
    let walls = w.solid.clone();
    for k in 0..w.story.villagers.len() {
        if w.story.villagers[k].gone {
            continue;
        }
        let v = &w.story.villagers[k];
        let goal = v.route[v.next.min(v.route.len() - 1)];
        let way = goal.sub(v.body.pos);
        let yaw = yaw_of(way);
        let run = (w.tick + k as u64) % RUN_OF < FLEE_RUN_TICKS;
        let inp = Input {
            keys: input::FORWARD | if run { input::RUN } else { 0 },
            ..Input::default()
        };
        let pos = v.body.pos;
        let ground = solid::surface(w, pos.x, pos.z);
        let v = &mut w.story.villagers[k];
        let mut f = Fighter::new(Kind::Villager, Held::Nothing, 0, pos, v.body.facing, false);
        f.gait = v.gait;
        f.last = inp;
        crate::motion::step_body(&mut v.body, &mut f, &inp, yaw, 0, None, ground);
        let (mut p, mut vel) = (v.body.pos, v.body.vel);
        let close = solid::near(&walls.walls, p.x, p.z, ONE * 2);
        solid::keep_out(&close, &mut p, &mut vel, solid::RADIUS);
        v.body.pos = p;
        v.body.vel = vel;
        v.gait = f.gait;
        if way.len_xz() < ONE * 2 && v.next + 1 < v.route.len() {
            v.next += 1;
        }
        if p.sub(c).len_xz() > GONE_AT {
            v.gone = true;
        }
    }
}

// ---- E, the pickups, the line ---------------------------------------------

/// Shows a line over the game for `BANNER_TICKS`.
pub fn banner(w: &mut World, key: &str) {
    w.story.banner = Some(Banner {
        key: key.into(),
        ticks: BANNER_TICKS,
    });
}

/// Something new lies in the world (a drop): E takes it.
pub fn drop(w: &mut World, gift: Gift, at: V3) {
    let mut at = at;
    at.y = solid::surface(w, at.x, at.z);
    let yaw = fx::deg((w.story.pickups.len() as i32 * 67) % 360);
    w.story.pickups.push(Pickup {
        gift,
        at,
        yaw,
        taken: false,
    });
}

/// What E would do, the nearest within `REACH` of the player's feet (a
/// person or the chest a little farther, at their edge), and how far.
pub fn nearest_use(w: &World) -> Option<(Use, Fx)> {
    let me = w.player.pos;
    let mut best: Option<(Use, Fx)> = None;
    let mut offer = |u: Use, d: Fx| {
        if d <= REACH && best.is_none_or(|(_, b)| d < b) {
            best = Some((u, d));
        }
    };
    for (k, p) in w.story.pickups.iter().enumerate() {
        if !p.taken {
            offer(Use::Pickup(k), p.at.sub(me).len_xz());
        }
    }
    for (k, r) in w.throwables.resting.iter().enumerate() {
        offer(Use::Thrown(k), r.at.sub(me).len_xz());
    }
    if let Some(d) = crate::merchant::reach(w) {
        offer(Use::Merchant, d);
    }
    if let Some(d) = crate::trainer::reach(w) {
        offer(Use::Trainer, d);
    }
    if let Some(d) = crate::camp::chest_reach(w) {
        offer(Use::Chest, d);
    }
    best
}

fn act_on(w: &mut World, u: Use) {
    match u {
        Use::Pickup(k) => {
            let p = w.story.pickups[k];
            match p.gift {
                Gift::Item(i, n) => {
                    if crate::inventory::give(w, i, n) == 0 {
                        banner(w, "story_full");
                        return;
                    }
                }
                Gift::Coins(n) => {
                    w.inventory.coins += n;
                    banner(w, "story_coins");
                }
            }
            w.story.pickups[k].taken = true;
        }
        Use::Thrown(k) => {
            let r = w.throwables.resting[k];
            let item = if r.item == Held::Knife {
                Item::Knife
            } else {
                Item::Rock
            };
            if crate::inventory::give(w, item, 1) > 0 {
                w.throwables.resting.remove(k);
            } else {
                banner(w, "story_full");
            }
        }
        Use::Merchant => crate::merchant::talk(w),
        Use::Trainer => crate::trainer::talk(w),
        Use::Chest => crate::camp::open_chest(w),
    }
}

/// The quest screen's line (M4's `ui::task` door): what to do next.
pub fn task(w: &World) -> &'static str {
    if w.story.opening() {
        return "quest_opening";
    }
    if !crate::imbue::learned(w) {
        return "quest_trainer";
    }
    if !w.inventory.has(Item::Boomerang) {
        return "quest_camp";
    }
    if crate::items::speedster_worn(w) < 5
        && [
            Item::Hood,
            Item::Jacket,
            Item::Gloves,
            Item::Trousers,
            Item::Boots,
        ]
        .iter()
        .any(|i| !w.inventory.has(*i))
    {
        return "quest_speedster";
    }
    "quest_done"
}

/// The lines over the game: the story's line, the prompt for E, the fade.
pub fn hud(w: &World, s: &Strings, out: &mut Vec<HudItem>) {
    let line = w.story.banner.as_ref();
    out.push(HudItem {
        id: "story".into(),
        text: line.map(|b| s.get(&b.key)).unwrap_or_default(),
        shown: line.is_some(),
    });
    let prompt = prompt(w, s);
    out.push(HudItem {
        id: "prompt".into(),
        shown: prompt.is_some(),
        text: prompt.unwrap_or_default(),
    });
    out.push(HudItem {
        id: "veil".into(),
        text: String::new(),
        shown: w.story.veil,
    });
}

/// "E · Talk to the merchant": what E would do here.
pub fn prompt(w: &World, s: &Strings) -> Option<String> {
    if w.player_down() || in_bed(w) || w.story.veil {
        return None;
    }
    let (u, _) = nearest_use(w)?;
    let does = match u {
        Use::Pickup(k) => {
            let name = match w.story.pickups[k].gift {
                Gift::Item(i, _) => s.get(&format!("item_{}", i.id())),
                Gift::Coins(_) => s.get("coins"),
            };
            crate::api::fill(&s.get("does_pick_up"), &[("item", name)])
        }
        Use::Thrown(k) => {
            let i = if w.throwables.resting[k].item == Held::Knife {
                Item::Knife
            } else {
                Item::Rock
            };
            crate::api::fill(
                &s.get("does_pick_up"),
                &[("item", s.get(&format!("item_{}", i.id())))],
            )
        }
        Use::Merchant => s.get(crate::merchant::prompt_key(w)),
        Use::Trainer => s.get(crate::trainer::prompt_key(w)),
        Use::Chest => s.get("does_open_chest"),
    };
    Some(crate::api::fill(
        &s.get("prompt"),
        &[("key", s.get("key_e")), ("does", does)],
    ))
}

/// One value for what the lines over the game show, so the page reads the
/// screen again when it changes (`Game::advance`).
pub fn stamp(w: &World) -> (Option<String>, Option<Use>, bool) {
    (
        w.story.banner.as_ref().map(|b| b.key.clone()),
        if in_bed(w) || w.player_down() {
            None
        } else {
            nearest_use(w).map(|(u, _)| u)
        },
        w.story.veil,
    )
}

// ---- drawing ------------------------------------------------------------------

/// What the story draws that moves or changes: the player's house (cut
/// away while the player is in or by it) and its bed, the villagers, the
/// pickups; then the merchant, the camp, the trainer's things.
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    draw_home(w, look, out);
    for v in &w.story.villagers {
        if !v.gone {
            draw_villager(w, v, alpha, look, out);
        }
    }
    for p in &w.story.pickups {
        if !p.taken {
            draw_pickup(p, look, out);
        }
    }
    crate::merchant::draw(w, alpha, look, out);
    crate::camp::draw(w, alpha, look, out);
}

/// Whether the player's house shows cut away: the player is in it or at
/// its walls.
pub fn cut_away(w: &World) -> bool {
    let Some(h) = w.solid.home else {
        return false;
    };
    let p = w.player.pos;
    h.wall.holds(p.x, p.z, CUT_NEAR + solid::RADIUS)
}

fn draw_home(w: &World, look: &Look, out: &mut Shapes) {
    let Some(h) = w.solid.home else {
        return;
    };
    let place = Affine::translate(h.foot).then(&Affine::rot_y(h.wall.yaw));
    let house = models::prop_model(Prop::House);
    let floor = h.foot.y + solid::FLOOR;
    if !cut_away(w) {
        house.placed(&place).emit(look.ink(), out);
    } else {
        // the footing and what stands low on it; the walls cut low, with a
        // rail along their tops; the roof left off
        let low = models::Model {
            parts: house
                .parts
                .iter()
                .filter(|p| p.at.t.y + p.at.c1.y.abs() / 2 <= solid::FLOOR + ONE / 100)
                .copied()
                .collect(),
            decals: Vec::new(),
        };
        low.placed(&place).emit(look.ink(), out);
        let t = models::table();
        let (plaster, timber) = (t.c("plaster"), t.c("timber"));
        let ink = look.ink();
        let wall_y = solid::FLOOR + CUT_WALL / 2;
        for k in solid::house_walls(&solid::Wall {
            x: 0,
            z: 0,
            yaw: 0,
            ..h.wall
        }) {
            let size = v3(2 * k.hx, CUT_WALL, 2 * k.hz);
            let at = place
                .then(&Affine::translate(v3(k.x, wall_y, k.z)))
                .then(&Affine::scale(size));
            out.inked(Mesh::Cube, at, plaster, ink);
            let rail = place
                .then(&Affine::translate(v3(
                    k.x,
                    solid::FLOOR + CUT_WALL + ONE * 4 / 100,
                    k.z,
                )))
                .then(&Affine::scale(v3(
                    2 * k.hx + ONE * 6 / 100,
                    ONE * 8 / 100,
                    2 * k.hz + ONE * 6 / 100,
                )));
            out.inked(Mesh::Cube, rail, timber, ink);
        }
        // floorboards a little over the footing
        let boards = place
            .then(&Affine::translate(v3(0, solid::FLOOR + ONE * 15 / 1000, 0)))
            .then(&Affine::scale(v3(
                2 * h.wall.hx - solid::WALL_THICK * 2,
                ONE * 3 / 100,
                2 * h.wall.hz - solid::WALL_THICK * 2,
            )));
        out.solid(Mesh::Cube, boards, t.c("plank_1"), 0);
    }
    // the bed, on the floor
    let mut bed = h.bed.at;
    bed.y = floor;
    models::prop_shapes(Prop::Bed, bed, h.bed.yaw, look, out);
}

fn draw_villager(w: &World, v: &Villager, alpha: Fx, look: &Look, out: &mut Shapes) {
    let feet = v.body.feet(alpha);
    let yaw = fx::lerp_angle(v.body.prev_facing, v.body.facing, alpha);
    let pose = walking_pose(Kind::Villager, feet, yaw, &v.body, &v.gait, w);
    let dress = models::Dress {
        worn: 0,
        variant: v.look,
    };
    models::figure_dressed(Kind::Villager, &pose, dress, look, out);
}

/// A figure standing as `rest_pose` has it, its legs stepping by `gait`.
pub fn walking_pose(
    kind: Kind,
    feet: V3,
    yaw: i32,
    b: &Body,
    gait: &crate::motion::Gait,
    _w: &World,
) -> Pose {
    let mut p = models::rest_pose(kind, feet, yaw);
    let build = crate::pose::build(kind);
    let mut body = *b;
    body.pos = feet;
    body.facing = yaw;
    let foot = crate::motion::feet(&body, gait, &build, feet);
    let fwd = facing(yaw, 0);
    for (s, f) in foot.iter().enumerate() {
        p.foot[s] = *f;
        p.knee[s] = crate::motion::knee(p.hip[s], *f, build.thigh, build.shin, fwd);
    }
    p
}

fn draw_pickup(p: &Pickup, look: &Look, out: &mut Shapes) {
    use crate::models::Pickup as P;
    let ink = look.ink();
    let lay = |out: &mut Shapes, held: Held, len: Fx| {
        let d = facing(p.yaw, 0);
        let grip = p.at.add(v3(0, ONE * 4 / 100, 0));
        models::held(held, grip, grip.add(d.scale(len)), look, out);
    };
    match p.gift {
        Gift::Item(i, n) => match i {
            Item::Hood => models::pickup(P::Hood, p.at, p.yaw, look, out),
            Item::Jacket => models::pickup(P::Jacket, p.at, p.yaw, look, out),
            Item::Gloves => models::pickup(P::Gloves, p.at, p.yaw, look, out),
            Item::Trousers => models::pickup(P::Trousers, p.at, p.yaw, look, out),
            Item::Boots => models::pickup(P::Boots, p.at, p.yaw, look, out),
            Item::ChestKey => models::pickup(P::ChestKey, p.at, p.yaw, look, out),
            Item::Rock => {
                for k in 0..n.min(3) as i32 {
                    let d = right_of(p.yaw + k * deg(120)).scale(ONE * 12 / 100);
                    let at = p.at.add(d).add(v3(0, crate::throwables::ROCK_R, 0));
                    models::held(
                        Held::Rock,
                        at,
                        at.add(v3(crate::throwables::ROCK_R * 2, 0, 0)),
                        look,
                        out,
                    );
                }
            }
            Item::Knife => lay(out, Held::Knife, models::grip_to_tip(Held::Knife)),
            Item::Sword => lay(out, Held::Sword, models::grip_to_tip(Held::Sword)),
            Item::ShortSword => lay(out, Held::ShortSword, models::grip_to_tip(Held::ShortSword)),
            Item::Boomerang => lay(out, Held::Boomerang, models::grip_to_tip(Held::Boomerang)),
            Item::Bandage => {
                let at = Affine::translate(p.at.add(v3(0, ONE * 6 / 100, 0)))
                    .then(&Affine::rot_y(p.yaw))
                    .then(&Affine::rot_z(fx::QUARTER_TURN))
                    .then(&Affine::scale(v3(ONE * 12 / 100, ONE * 16 / 100, ONE * 12 / 100)));
                out.inked(Mesh::Cyl, at, models::table().c("pillow"), ink);
            }
        },
        Gift::Coins(_) => {
            let gold = models::table().c("brass");
            for k in 0..3 {
                let at = Affine::translate(p.at.add(v3(
                    (k - 1) * ONE * 3 / 100,
                    ONE / 100 + k * ONE * 2 / 100,
                    0,
                )))
                .then(&Affine::scale(v3(ONE * 9 / 100, ONE * 2 / 100, ONE * 9 / 100)));
                out.inked(Mesh::Cyl, at, gold, ink);
            }
            out.glow(
                Mesh::Dot,
                Affine::translate(p.at.add(v3(0, ONE / 10, 0))).then(&Affine::scale(v3(
                    ONE * 4 / 10,
                    ONE / 4,
                    ONE * 4 / 10,
                ))),
                gold,
                ONE / 6,
            );
        }
    }
    let _ = rgb;
}

// ---- acts ---------------------------------------------------------------------

/// `story.<name>`: `still` (`on` or `off`, test-only) holds every foe the
/// story steers, the villagers and the merchant still, for a picture or a
/// still-frame check; `skip` ends the opening at once and stands the player
/// at the village centre, as `step_aside` does for a test duel; `down`
/// (test-only) puts the player down, as an orc's blow would.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    match name {
        "still" => {
            w.story.still = match value.trim() {
                "on" | "1" | "true" | "" => true,
                "off" | "0" | "false" => false,
                other => return Err(format!("Holding the story still is on or off, not \"{other}\".")),
            };
            Ok(())
        }
        "skip" => {
            step_aside(w);
            Ok(())
        }
        // test-only: the player falls, as to an orc's blow, for a view of
        // the dream
        "down" => {
            w.me.health.hp = 0;
            Ok(())
        }
        _ => Err(format!("Unknown action \"story.{name}\".")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{FORWARD, RUN};

    fn idle(w: &mut World, n: u32) {
        for _ in 0..n {
            w.step(&Input::default());
        }
    }

    /// The world after a dream, with what only a dream changes put back:
    /// its count, the fade, and the line still to come.
    fn as_if_fresh(w: &World) -> World {
        let mut v = w.clone();
        v.story.dreams = 0;
        v.story.dreamt = false;
        v.story.veil = false;
        v
    }

    #[test]
    fn the_game_opens_in_bed_with_six_orcs_in_the_village_and_none_in_a_wall() {
        let w = opening(3);
        assert_eq!(w.story.phase, Phase::Bed(0));
        assert!(in_bed(&w));
        assert_eq!(w.me.health.fall(), ONE, "lying down");
        assert!(!w.me.health.down(), "but not hurt");
        let h = w.solid.home.unwrap();
        assert!(h.wall.holds(w.player.pos.x, w.player.pos.z, 0), "in the house");
        assert_eq!(w.story.raid.len(), 6);
        assert_eq!(w.foes.len(), 6);
        let c = centre(&w);
        for r in &w.story.raid {
            let f = &w.foes[r.foe];
            assert_eq!(f.fighter.kind, Kind::Orc);
            assert!(!f.fighter.drawn(), "blades away while they walk their rounds");
            let p = f.body.pos;
            assert!(p.sub(c).len_xz() < fx::int(120), "in the village");
            assert!(
                !w.solid
                    .walls
                    .iter()
                    .any(|k| k.push(p.x, p.z, solid::RADIUS).is_some()),
                "an orc stands in a wall at {p:?}"
            );
        }
        for v in &w.story.villagers {
            let p = v.body.pos;
            assert!(!w
                .solid
                .walls
                .iter()
                .any(|k| k.push(p.x, p.z, solid::RADIUS).is_some()));
        }
        assert_eq!(ui_task(&w), "quest_opening");
    }

    fn ui_task(w: &World) -> &'static str {
        crate::ui::task(w)
    }

    #[test]
    fn the_player_gets_up_after_a_while_or_at_once_on_a_movement_key() {
        let mut w = opening(3);
        // 1.5 s lying: 89 ticks in bed, then up
        idle(&mut w, 89);
        assert!(matches!(w.story.phase, Phase::Bed(_)));
        idle(&mut w, 1 + RISE_TICKS);
        assert_eq!(w.story.phase, Phase::Raid);
        // the tick after: on the floor, nothing under the body but it
        idle(&mut w, 1);
        assert_eq!(w.me.health.fall(), 0, "standing");
        assert_eq!(w.story.lift, 0);
        // standing clear of the bed, on the floor
        let h = w.solid.home.unwrap();
        assert_eq!(w.player.pos.y, h.foot.y + solid::FLOOR);
        let mut w = opening(3);
        w.step(&Input {
            keys: FORWARD,
            ..Input::default()
        });
        assert!(
            matches!(w.story.phase, Phase::Rising(_)),
            "a key gets the player up"
        );
        // and in bed the keys move nothing
        assert_eq!(w.player.vel, V3::ZERO);
    }

    #[test]
    fn the_opening_ends_at_250_metres_from_the_village_centre_and_not_at_249() {
        for (east, ends) in [(249, false), (250, true)] {
            let mut w = opening(3);
            idle(&mut w, WAKE_TICKS + RISE_TICKS);
            let c = centre(&w);
            // 249.99 m: as near 250 as a unit allows, inside
            let x = if ends {
                fx::int(east)
            } else {
                fx::int(east + 1) - 1
            };
            let mut p = c.add(v3(x, 0, 0));
            p.y = w.ground(p.x, p.z);
            w.player = Body::new(p, 0);
            w.me = Fighter::new(Kind::Player, Held::ShortSword, 0, p, 0, false);
            w.step(&Input::default());
            assert_eq!(w.story.phase == Phase::Free, ends, "at {east} m");
            assert_eq!(ui_task(&w) != "quest_opening", ends);
        }
    }

    #[test]
    fn walking_across_the_boundary_says_so_and_turns_the_raid_back() {
        let mut w = opening(3);
        idle(&mut w, WAKE_TICKS + RISE_TICKS);
        let c = centre(&w);
        let mut p = c.add(v3(-fx::int(248), 0, 0));
        p.y = w.ground(p.x, p.z);
        // facing west, the aim west: W walks out across it
        w.player = Body::new(p, deg(90));
        w.aim.yaw = deg(90);
        w.story.raid[0].mode = Mode::Chase;
        for _ in 0..90 {
            w.step(&Input {
                keys: FORWARD | RUN,
                ..Input::default()
            });
        }
        assert_eq!(w.story.phase, Phase::Free);
        assert_eq!(
            w.story.banner.as_ref().map(|b| b.key.as_str()),
            Some("story_free")
        );
        assert_ne!(w.story.raid[0].mode, Mode::Chase);
        let x = w.story.crossed.unwrap().sub(c).len_xz();
        assert!((BOUNDARY..BOUNDARY + ONE / 5).contains(&x), "{x}");
    }

    #[test]
    fn an_orc_that_sees_the_player_runs_at_them_draws_and_fights() {
        let mut w = opening(3);
        idle(&mut w, WAKE_TICKS + RISE_TICKS);
        // the player out on the green, 20 m from the first orc
        let o = w.foes[w.story.raid[0].foe].body.pos;
        let mut p = o.add(v3(fx::int(20), 0, 0));
        p.y = w.ground(p.x, p.z);
        w.player = Body::new(p, 0);
        w.me = Fighter::new(Kind::Player, Held::ShortSword, 0, p, 0, false);
        w.me.health = crate::health::Health::new(100_000);
        let mut drew = false;
        let mut nearest = fx::int(99);
        for _ in 0..600 {
            w.step(&Input::default());
            let f = &w.foes[w.story.raid[0].foe];
            drew |= f.fighter.sheath == Sheath::Drawn;
            nearest = nearest.min(f.body.pos.sub(w.player.pos).len_xz());
        }
        assert_eq!(w.story.raid[0].mode, Mode::Chase);
        assert!(drew, "it drew its blade");
        assert!(nearest < fx::int(3), "and came within a blade's reach: {nearest}");
        assert!(w.me.health.hp < 100_000, "and struck");
    }

    #[test]
    fn a_player_who_runs_from_a_chasing_orc_gets_away() {
        // an orc 8 m behind a player who runs straight out west: the gap
        // grows, as an orc runs four ticks in five
        let mut w = opening(3);
        idle(&mut w, WAKE_TICKS + RISE_TICKS);
        let c = centre(&w);
        let mut p = c.add(v3(-fx::int(60), 0, fx::int(20)));
        p.y = w.ground(p.x, p.z);
        w.player = Body::new(p, deg(90));
        w.aim.yaw = deg(90);
        let k = w.story.raid[0].foe;
        let mut o = p.add(v3(fx::int(8), 0, 0));
        o.y = w.ground(o.x, o.z);
        w.foes[k].body = Body::new(o, deg(90));
        w.story.raid[0].mode = Mode::Chase;
        for _ in 0..600 {
            w.step(&Input {
                keys: FORWARD | RUN,
                ..Input::default()
            });
        }
        let gap = w.foes[k].body.pos.sub(w.player.pos).len_xz();
        // 10 s at 7 m/s against an orc's 4 ticks in 5 at 7 and one at 3,
        // 6.2 m/s: some 8 m gained on the 8
        assert!(gap > fx::int(12), "{gap}");
        assert!(!w.me.health.down());
    }

    /// The dream loop: down to the raid, the view fades, and the world is
    /// the opening again exactly (its hash equal to a fresh opening's, with
    /// only the dream's own count, fade and coming line put back), and the
    /// line is said once the fade lifts.
    #[test]
    fn down_to_an_orc_the_player_wakes_in_bed_and_the_world_is_the_opening_again() {
        let fresh = opening(3);
        let mut w = opening(3);
        // a reader's setting survives the dream
        w.ui.settings.hand_pct = 150;
        idle(&mut w, WAKE_TICKS + RISE_TICKS + 30);
        // pick something up and walk about, so there is something to undo
        w.inventory.coins = 40;
        w.me.health.hp = 0;
        w.step(&Input::default());
        assert_eq!(w.story.phase, Phase::Dream(0));
        idle(&mut w, FALL_SHOW + 1);
        assert!(w.story.veil, "the view fades");
        while !waking(&w) {
            w.step(&Input::default());
        }
        w.step(&Input::default());
        assert_eq!(w.story.dreams, 1);
        assert!(w.story.dreamt && w.story.veil);
        assert_eq!(w.ui.settings.hand_pct, 150, "the reader's settings kept");
        let mut back = as_if_fresh(&w);
        back.ui.settings.hand_pct = 100;
        if back.hash() != fresh.hash() {
            let (a, b) = (format!("{back:?}"), format!("{fresh:?}"));
            let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(0);
            panic!(
                "the world is not the opening again: {}",
                &a[at.saturating_sub(300)..(at + 200).min(a.len())]
            );
        }
        assert!(in_bed(&w));
        // the fade lifts, then the line, from the player
        idle(&mut w, DARK_TICKS + 1);
        assert!(!w.story.veil);
        idle(&mut w, LINE_AFTER);
        let d = w.ui.dialogue.clone().expect("the dream line");
        assert_eq!((d.who.as_str(), d.lines[0].as_str()), ("you", "dream_line"));
    }

    #[test]
    fn from_the_third_dream_the_line_is_the_second_one() {
        let mut w = opening(5);
        for n in 1..=3 {
            idle(&mut w, 3);
            w.story.phase = Phase::Raid;
            w.me.health.hp = 0;
            while !waking(&w) {
                w.step(&Input::default());
            }
            w.step(&Input::default());
            idle(&mut w, DARK_TICKS + LINE_AFTER + 1);
            let line = w.ui.dialogue.take().unwrap().lines[0].clone();
            let want = if n >= AGAIN_FROM {
                "dream_line_again"
            } else {
                "dream_line"
            };
            assert_eq!(line, want, "dream {n}");
        }
    }

    /// The raid itself kills a player who stands in the green: the dream
    /// loop through the orcs' own blades, not a set health.
    #[test]
    fn standing_still_in_the_raid_an_orc_kills_the_player_and_they_wake_in_bed() {
        let mut w = opening(3);
        idle(&mut w, WAKE_TICKS + RISE_TICKS);
        let c = centre(&w);
        let mut p = c.add(v3(-fx::int(5), 0, fx::int(25)));
        p.y = w.ground(p.x, p.z);
        w.player = Body::new(p, 0);
        w.me = Fighter::new(Kind::Player, Held::ShortSword, 0, p, 0, false);
        let mut t = 0;
        while !matches!(w.story.phase, Phase::Dream(_)) {
            w.step(&Input::default());
            t += 1;
            assert!(t < 60 * 90, "an orc came and struck within 90 s");
        }
        while !waking(&w) {
            w.step(&Input::default());
        }
        w.step(&Input::default());
        assert!(in_bed(&w));
        assert_eq!(w.story.dreams, 1);
    }

    #[test]
    fn a_fallen_orc_is_followed_by_another_from_the_south_edge() {
        let mut w = opening(3);
        idle(&mut w, WAKE_TICKS + RISE_TICKS);
        let k = w.story.raid[2].foe;
        w.foes[k].fighter.health.hp = 0;
        idle(&mut w, COMES_AFTER - 1);
        assert_eq!(w.story.raid.len(), 6);
        idle(&mut w, 2);
        assert_eq!(w.story.raid.len(), 7);
        let new = w.foes.last().unwrap().body.pos;
        assert!(new.z > centre(&w).z + fx::int(100), "from the south: {new:?}");
        idle(&mut w, COMES_AFTER * 2);
        assert_eq!(w.story.raid.len(), 7, "one for one");
    }

    /// CW-4: a recorded opening (the wake, a walk out of the house, a
    /// turn, a run into the village and back) hashes the same every run;
    /// one hundredth of a pixel's change shows.
    #[test]
    fn a_recorded_opening_hashes_the_same_every_run() {
        let record: Vec<Input> = (0..900)
            .map(|k| Input {
                keys: match k {
                    0..=40 => 0,
                    41..=300 => FORWARD,
                    301..=600 => FORWARD | RUN,
                    _ => crate::input::BACK,
                },
                dx: if k % 120 < 20 { 700 } else { 0 },
                ..Input::default()
            })
            .collect();
        let run = |rec: &[Input]| {
            let mut w = opening(11);
            let mut marks = Vec::new();
            for (k, i) in rec.iter().enumerate() {
                w.step(i);
                if k % 60 == 59 {
                    marks.push(w.hash());
                }
            }
            marks
        };
        let a = run(&record);
        assert_eq!(a, run(&record));
        let mut other = record.clone();
        other[450].dx += 1;
        let b = run(&other);
        assert_eq!(b[..7], a[..7]);
        assert_ne!(b[14], a[14]);
    }

    #[test]
    fn e_picks_up_what_lies_in_reach_and_the_prompt_names_it() {
        let mut w = opening(3);
        idle(&mut w, WAKE_TICKS + RISE_TICKS);
        // the bandage lies by the bed, a metre from where the player stands
        let k = w
            .story
            .pickups
            .iter()
            .position(|p| p.gift == Gift::Item(Item::Bandage, 1))
            .unwrap();
        let d = w.story.pickups[k].at.sub(w.player.pos).len_xz();
        assert!(d < REACH, "{d}");
        assert_eq!(nearest_use(&w).map(|(u, _)| u), Some(Use::Pickup(k)));
        let s = crate::page::parse_strings(include_str!("../../../data/strings.json")).unwrap();
        assert_eq!(prompt(&w, &s).unwrap(), "E · Pick up the Bandage");
        w.step(&Input {
            keys: crate::input::USE,
            ..Input::default()
        });
        assert!(w.story.pickups[k].taken);
        assert_eq!(w.inventory.count(Item::Bandage), 1);
        // holding E does not take the next thing too
        w.step(&Input {
            keys: crate::input::USE,
            ..Input::default()
        });
        assert_eq!(w.inventory.count(Item::Bandage), 1);
    }

    #[test]
    fn the_quest_moves_on_with_the_story() {
        let mut w = opening(3);
        assert_eq!(task(&w), "quest_opening");
        w.story.phase = Phase::Free;
        assert_eq!(task(&w), "quest_trainer");
        crate::imbue::learn(&mut w);
        assert_eq!(task(&w), "quest_camp");
        crate::inventory::give(&mut w, Item::Boomerang, 1);
        assert_eq!(task(&w), "quest_speedster");
        for i in [
            Item::Hood,
            Item::Jacket,
            Item::Gloves,
            Item::Trousers,
            Item::Boots,
        ] {
            crate::inventory::give(&mut w, i, 1);
        }
        assert_eq!(task(&w), "quest_done");
    }

    #[test]
    fn the_story_file_refuses_a_pickup_that_is_no_item() {
        let mut v: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/story.json")).unwrap();
        v["pickups"][0]["item"] = "castle".into();
        assert!(Spec::parse(&v.to_string())
            .unwrap_err()
            .starts_with("A pickup in the story file"));
        let mut v: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/story.json")).unwrap();
        v["raid"]["orcs"][0]["route"] = serde_json::json!([]);
        assert_eq!(
            Spec::parse(&v.to_string()).unwrap_err(),
            "An orc in the story file has no round to walk."
        );
    }

    #[test]
    fn keys_toward_alternate_between_the_two_nearest_mixes_so_their_mean_heads_true() {
        // 22.5° left of straight ahead: forward and forward-left in turn,
        // about half each (atan2 is good to a few units)
        let mut acc = 0;
        let way = facing(fx::TURN / 16, 0);
        let keys: Vec<u32> = (0..8).map(|_| keys_toward(0, way, &mut acc)).collect();
        let fl = input::FORWARD | input::LEFT;
        let n = keys.iter().filter(|k| **k == fl).count();
        assert!((3..=5).contains(&n), "{keys:?}");
        assert_eq!(keys.iter().filter(|k| **k == input::FORWARD).count(), 8 - n);
        // 10° left: mostly forward
        let mut acc = 0;
        let keys: Vec<u32> = (0..9)
            .map(|_| keys_toward(0, facing(deg(10), 0), &mut acc))
            .collect();
        assert_eq!(
            keys.iter().filter(|k| **k == fl).count(),
            2,
            "10/45 of 9: {keys:?}"
        );
        // just past straight behind: back
        let mut acc = 0;
        assert_eq!(keys_toward(deg(30), facing(deg(212), 0), &mut acc), input::BACK);
    }
}

/// Every route in `data/items.json` taken in one play, the way a reader
/// takes it: the wake, the escape on foot, the trainer, the camp, the
/// chest, the merchant stopped with a thrown rock, the speedster's pieces.
/// Movement, looking, E, X and F are the reader's inputs through
/// `Game::advance`; the shop and dialogue through their acts, as the page
/// sends them. Two things are not by inputs, and say so: the long walks
/// between places are `places.go` (a few kilometres at a run is minutes of
/// ticks); and the camp's foes are put down rather than duelled (M1's duel
/// is tested on its own). The practice passes on M6's orbs, turned by the
/// held guard with the lent imbue.
#[cfg(test)]
mod playthrough {
    use super::*;
    use crate::game::tests::{frames, game};
    use crate::game::{Game, Screen};
    use crate::input::{DRAW, FORWARD, RUN, THROW, USE};

    /// Hundredths of a pixel of mouse movement that turn the aim by `d`
    /// (12° a 100 px at the default look speed, `camera.rs`).
    fn dx_for(d: i32) -> i32 {
        (-(d as i64) * 10_000 / (fx::TURN as i64 * 12 / 360)).clamp(-40_000, 40_000) as i32
    }

    /// Turns to face `to` and walks (or runs) there, through the inputs.
    fn walk_to(g: &mut Game, to: V3, run: bool) {
        walk_near(g, to, run, ONE * 6 / 10);
    }

    fn walk_near(g: &mut Game, to: V3, run: bool, near: Fx) {
        for _ in 0..60 * 120 {
            let w = &g.world;
            let d = to.sub(w.player.pos);
            if d.len_xz() < near {
                frames(g, 20, 0, 0);
                return;
            }
            let turn = fx::wrap(yaw_of(d) - w.aim.yaw);
            let keys = if turn.abs() < deg(40) {
                FORWARD | if run { RUN } else { 0 }
            } else {
                0
            };
            frames(g, 1, keys, dx_for(turn));
            assert_eq!(g.screen, Screen::Play, "walking to {to:?}");
        }
        panic!("did not reach {to:?} from {:?}", g.world.player.pos);
    }

    /// E, and whatever dialogue it opens read to its end.
    fn press_e(g: &mut Game) {
        frames(g, 1, USE, 0);
        frames(g, 1, 0, 0);
        read_on(g);
    }

    fn read_on(g: &mut Game) {
        for _ in 0..20 {
            if g.screen != Screen::Dialogue {
                break;
            }
            g.act("dialogue.next", "").unwrap();
            frames(g, 1, 0, 0);
        }
    }

    fn go(g: &mut Game, x: i32, z: i32) {
        g.act("places.go", &format!("{x},{z}")).unwrap();
        frames(g, 2, 0, 0);
    }

    /// Walks to each pickup of `gift` lying within `near` and takes it.
    fn take_all(g: &mut Game, near: Fx) {
        loop {
            let me = g.world.player.pos;
            let next = g
                .world
                .story
                .pickups
                .iter()
                .filter(|p| !p.taken && p.at.sub(me).len_xz() < near)
                .min_by_key(|p| p.at.sub(me).len_xz())
                .copied();
            let Some(p) = next else {
                return;
            };
            // within reach of it, then E
            walk_near(g, p.at, false, REACH - ONE / 2);
            press_e(g);
            let taken = g
                .world
                .story
                .pickups
                .iter()
                .find(|q| q.at == p.at)
                .is_some_and(|q| q.taken);
            assert!(taken, "took {:?}", p.gift);
        }
    }

    fn home_point(w: &World, lx: Fx, lz: Fx) -> V3 {
        let h = w.solid.home.unwrap();
        let (dx, dz) = h.wall.world_dir(lx, lz);
        v3(h.wall.x + dx, h.foot.y, h.wall.z + dz)
    }

    #[test]
    fn every_item_s_one_route_can_be_taken_in_one_play() {
        let mut g = game();
        g.act("screen.show", "play").unwrap();
        // the wake, the bandage by the bed
        frames(&mut g, (WAKE_TICKS + RISE_TICKS + 2) as i64, 0, 0);
        assert_eq!(g.world.story.phase, Phase::Raid);
        take_all(&mut g, fx::int(3));
        assert!(g.world.inventory.has(Item::Bandage), "route: village");
        // out of the door and west across the meadow, at a run
        let h = g.world.solid.home.unwrap();
        let inside = home_point(&g.world, 0, -h.wall.hz + ONE);
        let out = home_point(&g.world, 0, -h.wall.hz - fx::int(3));
        walk_to(&mut g, inside, false);
        walk_to(&mut g, out, false);
        let far = home_point(&g.world, 0, -h.wall.hz - fx::int(280));
        walk_to(&mut g, far, true);
        assert_eq!(g.world.story.phase, Phase::Free, "got away on foot");
        assert_eq!(crate::ui::task(&g.world), "quest_trainer");
        // rocks by the crossroads (the "ground" route)
        let cross = crate::places::at(&g.world, "crossroads").unwrap();
        go(&mut g, fx::floor(cross.x) - 4, fx::floor(cross.z) - 4);
        take_all(&mut g, fx::int(20));
        assert!(g.world.inventory.count(Item::Rock) >= 3, "route: ground");
        // the trainer: talk, draw, the practice
        let hut = crate::places::at(&g.world, "hut").unwrap();
        go(&mut g, fx::floor(hut.x) + 12, fx::floor(hut.z) + 12);
        let t = crate::player::body(&g.world, crate::trainer::id(&g.world).unwrap()).pos;
        walk_to(&mut g, t.add(v3(0, 0, fx::int(2))), false);
        press_e(&mut g);
        assert_eq!(g.world.trainer.stage, crate::trainer::Stage::Ready);
        frames(&mut g, 1, DRAW, 0);
        frames(&mut g, 30, 0, 0);
        assert!(matches!(g.world.trainer.stage, crate::trainer::Stage::Volley(_)));
        // the guard held toward the trainer, the lent imbue turns the motes
        // (M6's orbs)
        for _ in 0..30 {
            let w = &g.world;
            let tp = crate::player::body(w, crate::trainer::id(w).unwrap()).pos;
            let turn = fx::wrap(yaw_of(tp.sub(w.player.pos)) - w.aim.yaw);
            frames(&mut g, 1, 0, dx_for(turn));
        }
        for _ in 0..60 * 120 {
            if crate::imbue::learned(&g.world) {
                break;
            }
            frames(&mut g, 1, 0, 0);
            read_on(&mut g);
        }
        read_on(&mut g);
        assert!(
            crate::imbue::learned(&g.world),
            "route: trainer: {} turned, stage {:?}, facing {} aim {} screen {:?} tick {} stop {}",
            crate::trainer::turned(&g.world),
            g.world.trainer.stage,
            g.world.player.facing,
            g.world.aim.yaw,
            g.screen,
            g.world.tick,
            g.world.feel.stop
        );
        let _ = g.world.feel.stop;
        frames(&mut g, 1, DRAW, 0);
        frames(&mut g, 30, 0, 0);
        // the camp: the lookout sees you; its people put down; their drops
        let c = crate::camp::centre(&g.world);
        go(&mut g, fx::floor(c.x) - 40, fx::floor(c.z) + 40);
        walk_to(&mut g, c.add(v3(-fx::int(22), 0, fx::int(26))), false);
        assert!(g.world.camp.alarm, "the lookout raised the alarm");
        for m in g.world.camp.members.clone() {
            g.world.foes[m.foe].fighter.health.hp = 0;
        }
        frames(&mut g, 60, 0, 0);
        take_all(&mut g, fx::int(40));
        for (i, route) in [
            (Item::Gloves, "lookout"),
            (Item::Sword, "leader_duel"),
            (Item::Knife, "bandits"),
            (Item::ChestKey, "leader_drop"),
        ] {
            assert!(g.world.inventory.has(i), "route: {route}");
        }
        // the chest, with the key
        let chest = crate::camp::chest(&g.world).unwrap();
        let d = chest.at.sub(g.world.player.pos).norm();
        walk_to(&mut g, chest.at.sub(d.scale(ONE * 14 / 10)), false);
        press_e(&mut g);
        assert!(
            g.world.camp.open && g.world.inventory.has(Item::Boomerang),
            "route: camp_chest"
        );
        // the merchant: beside the cart, a rock thrown at it, E, the shop
        let m = &g.world.merchant;
        let side = crate::geom::right_of(m.yaw);
        let at = m.at.add(side.scale(fx::int(6)));
        go(&mut g, fx::floor(at.x), fx::floor(at.z));
        for _ in 0..40 {
            let w = &g.world;
            let ahead = w.merchant.at.add(facing(w.merchant.yaw, 0).scale(ONE / 2));
            let turn = fx::wrap(yaw_of(ahead.sub(w.player.pos)) - w.aim.yaw);
            // level: the mouse up or down by the pitch there is
            let dy = (w.aim.pitch as i64 * 10_000 / (fx::TURN as i64 * 12 / 360)) as i32;
            let now = g.clock.last().unwrap_or(0) + crate::ticks::TICK_US;
            g.advance(now, 0, dx_for(turn), dy, 0);
        }
        frames(&mut g, 1, THROW, 0);
        frames(&mut g, 60, 0, 0);
        assert!(g.world.merchant.waiting(), "the rock stopped the cart");
        let cart = g.world.merchant.at;
        let d = cart.sub(g.world.player.pos).norm();
        walk_to(&mut g, cart.sub(d.scale(fx::int(2))), false);
        press_e(&mut g);
        assert_eq!(g.screen, Screen::Shop);
        g.act("shop.buy", "hood").unwrap();
        g.act("shop.close", "").unwrap();
        assert!(g.world.inventory.has(Item::Hood), "route: merchant");
        // the three pieces lying in the world
        for (place, item, route) in [
            ("scarecrow", Item::Jacket, "scarecrow"),
            ("hollow_tree", Item::Trousers, "hollow_tree"),
            ("shrine", Item::Boots, "river_shrine"),
        ] {
            let p = crate::places::at(&g.world, place).unwrap();
            go(&mut g, fx::floor(p.x) + 5, fx::floor(p.z) + 5);
            take_all(&mut g, fx::int(12));
            assert!(g.world.inventory.has(item), "route: {route}");
        }
        // every item the MVP has, held (the key went into the chest's lock)
        for i in Item::ALL {
            if i != Item::ChestKey {
                assert!(g.world.inventory.has(i), "{i:?} is held at the end");
            }
        }
        assert_eq!(crate::ui::task(&g.world), "quest_done");
    }
}
