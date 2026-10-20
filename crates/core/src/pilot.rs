//! `pilot` (M1 Duel, task D): each foe's behaviour tree, read from
//! `data/pilots.json`, after Vagrancy's `crates/pilot` (its `Tree`, `Rule`
//! and `Cond`) and the journal's bandit tree.
//!
//! **A pilot returns an `Input` and nothing else** (CW-3). `decide` takes
//! the world by shared reference and gives back the record for one tick,
//! the same keys, buttons, mouse movement and wheel the player's hands
//! make, with the pilot's own memory (`Brain`) for the next tick. The tick
//! then runs that record through the rules the player's runs through: the
//! same feet, the same arms, the same blocks. `a_pilot_moves_its_body_only_through_its_input_record`
//! replays a pilot's records with the pilot taken out and finds the same
//! duel.
//!
//! **What it sees.** A pilot reads the others as they were its reaction
//! time ago (`Memory`, `reaction_ticks`), and itself as it is now, as a
//! person feels their own arm. It checks its tree every `CHECK_TICKS`, top
//! to bottom, and the first rule whose conditions all hold picks the move;
//! the move then plays tick by tick until it ends. A rule marked
//! `interrupt` may break into a move under way, except a committed swing.
//!
//! **In a group** one foe duels, the one nearest the player, and the others
//! run the `ring` tree: they keep their distance, circle and throw.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::arm::{self, mouse_for};
use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{yaw_of, V3};
use crate::input::{self, Input};
use crate::player::{self, Body, Fighter};
use crate::pose::{Held, Kind};
use crate::rng::Rng;
use crate::world::World;

/// A tree is checked this often (0.1 s, the journal).
pub const CHECK_TICKS: u64 = 6;
/// The longest reaction any pilot may have, ticks: the memory's length.
pub const MAX_REACTION: usize = 30;

/// One kind of foe in `data/pilots.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindSpec {
    /// the weapon: `cleaver`, `sword` or `short_sword`
    pub held: String,
    pub health: i32,
    pub reaction_ticks: u32,
    /// arm strength, percent of the player's
    pub servo_pct: i32,
    /// how far the pilot moves its mouse a tick, pixels, and in a swing
    pub hand_px: i32,
    pub strike_px: i32,
    pub rocks: u8,
    pub knives: u8,
    /// the band it keeps to in a duel, centimetres outside its own reach
    /// (`reach`): nearer, it steps back; farther, it steps in
    pub keep_cm: [i32; 2],
    /// the tree it duels with
    pub tree: String,
}

/// One condition on what the pilot sees. Distances in centimetres, speeds
/// in centimetres a second, shares in percent.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cond {
    /// my health is below this share
    HurtBelow(i32),
    /// the one I fight stands farther than this, feet to feet
    GapAbove(i32),
    GapBelow(i32),
    /// the one I fight stands farther than my reach and this, or nearer
    BeyondReach(i32),
    WithinReach(i32),
    /// I have a throwable and my last throw is over
    HasThrowable,
    /// their blade's tip is within this of my head
    TipNearHead(i32),
    /// their tip comes at me faster than this, within reach
    SwingComing(i32),
    /// their tip is off the line between us by more than this: their guard
    /// is open
    Opening(i32),
    /// a seeded chance, drawn when the rule is checked
    Chance(u32),
    /// the one I fight is down
    FoeDown,
}

/// What a rule does: a move the pilot plays for some ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Move {
    Guard,
    CloseIn,
    Circle,
    RaiseGuard,
    Block,
    /// one of `Overhead`, `Sweep` and `Thrust`, chosen by chance
    Attack,
    Overhead,
    Sweep,
    Thrust,
    Feint,
    Flee,
    Throw,
    /// keep a throwing distance and circle (the others in a group)
    Ring,
    BackOff,
    /// M6's hook: the leader's pattern. Until then it guards.
    Cast,
}

impl Move {
    /// A committed swing: no interrupt breaks in.
    fn committed(self) -> bool {
        matches!(self, Move::Overhead | Move::Sweep | Move::Thrust)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    #[serde(default, rename = "if")]
    pub when: Vec<Cond>,
    #[serde(rename = "do")]
    pub act: Move,
    #[serde(default)]
    pub interrupt: bool,
}

/// Every kind and tree, as read from `data/pilots.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Book {
    pub kinds: BTreeMap<String, KindSpec>,
    pub trees: BTreeMap<String, Vec<Rule>>,
}

impl Book {
    pub fn parse(json: &str) -> Result<Book, String> {
        let mut v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if let Some(o) = v.as_object_mut() {
            o.retain(|k, _| !k.starts_with('_'));
        }
        let book: Book = serde_json::from_value(v).map_err(|e| e.to_string())?;
        for (name, k) in &book.kinds {
            if !book.trees.contains_key(&k.tree) {
                return Err(format!(
                    "the {name} names a tree \"{}\" that is not there",
                    k.tree
                ));
            }
            if k.reaction_ticks as usize > MAX_REACTION {
                return Err(format!("the {name} reacts slower than {MAX_REACTION} ticks"));
            }
        }
        if !book.trees.contains_key("ring") {
            return Err("there is no \"ring\" tree for a group's others".into());
        }
        Ok(book)
    }

    pub fn standard() -> Book {
        Book::parse(include_str!("../../../data/pilots.json")).expect("data/pilots.json")
    }
}

/// The kinds a test duel can spawn, by the name the act takes.
pub fn kind_named(name: &str) -> Option<Kind> {
    match name {
        "orc" => Some(Kind::Orc),
        "bandit" => Some(Kind::Bandit),
        "leader" => Some(Kind::Leader),
        _ => None,
    }
}

/// A foe kind's name, as the act, the book and the strings (`who_<kind>`)
/// name it.
pub fn kind_name(k: Kind) -> &'static str {
    key_of(k)
}

fn key_of(k: Kind) -> &'static str {
    match k {
        Kind::Orc => "orc",
        Kind::Leader => "leader",
        _ => "bandit",
    }
}

fn held_named(s: &str) -> Held {
    match s {
        "cleaver" => Held::Cleaver,
        "sword" => Held::Sword,
        _ => Held::ShortSword,
    }
}

/// Who drives a foe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    /// its tree
    Tree,
    /// nothing: a still target, for tests
    Still,
    /// these records in turn, for tests (CW-3)
    Script(Vec<Input>),
}

/// A pilot's own memory between ticks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Brain {
    pub kind: Kind,
    pub control: Control,
    /// the move under way and its tick
    pub current: Option<(Move, u32)>,
    pub rng: Rng,
    /// which way it circles: 1 right, −1 left
    pub dir: i32,
    /// ticks it has been driven
    pub ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Foe {
    pub body: Body,
    pub fighter: Fighter,
    pub brain: Brain,
}

/// A foe of `kind` at `at` facing `yaw`, sword drawn, its chance seeded
/// from `salt`.
pub fn foe(book: &Book, kind: Kind, at: V3, yaw: i32, salt: u64) -> Foe {
    let spec = &book.kinds[key_of(kind)];
    let mut f = Fighter::new(kind, held_named(&spec.held), 1, at, yaw, true);
    f.health = crate::health::Health::new(spec.health);
    f.servo_pct = spec.servo_pct;
    f.pouch = crate::throwables::Pouch {
        rocks: spec.rocks,
        knives: spec.knives,
    };
    Foe {
        body: Body::new(at, yaw),
        fighter: f,
        brain: Brain {
            kind,
            control: Control::Tree,
            current: None,
            rng: Rng::new(salt),
            dir: 1,
            ticks: 0,
        },
    }
}

/// What a pilot sees of a fighter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seen {
    pub feet: V3,
    pub head: V3,
    pub chest: V3,
    pub tip: V3,
    /// metres a tick
    pub tip_vel: V3,
    pub down: bool,
}

/// The duel as it was, a tick at a time, the newest last.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Memory {
    pub ticks: Vec<Vec<Seen>>,
}

impl Memory {
    /// Fighter `id` as it was `ago` ticks back, or as far back as is kept.
    pub fn ago(&self, ago: usize, id: usize) -> Option<Seen> {
        let n = self.ticks.len();
        if n == 0 {
            return None;
        }
        self.ticks[n - 1 - ago.min(n - 1)].get(id).copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub book: Book,
    pub memory: Memory,
    /// who drives a foe spawned from now on (`act("duel.pilot", …)`)
    pub control: Control,
}

impl State {
    pub fn new() -> State {
        State {
            book: Book::standard(),
            memory: Memory::default(),
            control: Control::Tree,
        }
    }
}

impl Default for State {
    fn default() -> State {
        State::new()
    }
}

/// How a fighter looks to others now.
pub fn seen_now(w: &World, id: usize) -> Seen {
    let f = player::fighter(w, id);
    let tip = &f.arm.pts[arm::TIP];
    Seen {
        feet: player::body(w, id).pos,
        head: f.pose.head,
        chest: f.pose.chest,
        tip: tip.p,
        tip_vel: tip.vel(),
        down: f.health.down(),
    }
}

/// The end of the tick: what everyone looked like, kept for the pilots.
pub fn remember(w: &mut World) {
    let now: Vec<Seen> = (0..player::count(w)).map(|id| seen_now(w, id)).collect();
    let m = &mut w.pilots.memory;
    m.ticks.push(now);
    if m.ticks.len() > MAX_REACTION + 1 {
        m.ticks.remove(0);
    }
}

/// Step 2 of the tick: each foe's input record, from its pilot.
pub fn step(w: &mut World) -> Vec<Input> {
    let mut out = Vec::with_capacity(w.foes.len());
    for k in 0..w.foes.len() {
        let (input, brain) = decide(w, k + 1);
        w.foes[k].brain = brain;
        out.push(input);
    }
    out
}

/// The foe that duels: of those standing and holding on the player, the
/// nearest (ties by number).
pub fn dueller(w: &World) -> Option<usize> {
    (1..player::count(w))
        .filter(|&id| !player::fighter(w, id).health.down() && player::fighter(w, id).engaged == Some(0))
        .min_by_key(|&id| (player::body(w, id).pos.dist(w.player.pos), id))
}

/// A tick of a move: where the hand's target should go on the shell
/// (degrees right and up, reach in percent of the arm), the keys, the
/// swing, the throw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    pub shell: (i32, i32, i32),
    pub keys: u32,
    pub swing: bool,
    pub fast: bool,
}

const GUARD: (i32, i32, i32) = (6, -14, 82);

/// One pilot's record for this tick, and its memory for the next. Reads the
/// world; changes nothing in it (CW-3).
pub fn decide(w: &World, id: usize) -> (Input, Brain) {
    let foe = &w.foes[id - 1];
    let mut b = foe.brain.clone();
    b.ticks += 1;
    let f = &foe.fighter;
    match &b.control {
        Control::Still => return (Input::default(), b),
        Control::Script(list) => {
            let i = list.get(b.ticks as usize - 1).copied().unwrap_or_default();
            return (i, b);
        }
        Control::Tree => {}
    }
    let Some(them) = f.engaged.map(|k| k as usize) else {
        b.current = None;
        return (Input::default(), b);
    };
    if f.health.down() {
        return (Input::default(), b);
    }
    let book = &w.pilots.book;
    let spec = &book.kinds[key_of(b.kind)];
    let ring = dueller(w) != Some(id);
    let tree = &book.trees[if ring { "ring" } else { spec.tree.as_str() }];
    let me = seen_now(w, id);
    let they = w
        .pilots
        .memory
        .ago(spec.reaction_ticks as usize, them)
        .unwrap_or_else(|| seen_now(w, them));
    let view = View { w, id, f, me, they };
    if (w.tick + id as u64).is_multiple_of(CHECK_TICKS) {
        let under_way = b.current.map(|(m, _)| m);
        let pick = match under_way {
            Some(m) if m.committed() => None,
            Some(_) => pick(tree, &view, &mut b.rng, true),
            None => pick(tree, &view, &mut b.rng, false),
        };
        if let Some(m) = pick {
            if under_way != Some(m) {
                start(&mut b, m);
            }
        }
    }
    let plan = match b.current {
        Some((m, t)) => match play(m, t, &view, b.dir) {
            Some(p) => {
                b.current = Some((m, t + 1));
                p
            }
            None => {
                b.current = None;
                hold(&view, GUARD, b.dir)
            }
        },
        None => hold(&view, GUARD, b.dir),
    };
    (to_input(f, &plan, spec), b)
}

/// A move begun: an attack picks its swing, a circle its way round.
fn start(b: &mut Brain, m: Move) {
    let m = match m {
        Move::Attack => [Move::Overhead, Move::Sweep, Move::Thrust][b.rng.below(3) as usize],
        Move::Circle | Move::Ring if b.rng.below(100) < 35 => {
            b.dir = -b.dir;
            m
        }
        m => m,
    };
    b.current = Some((m, 0));
}

/// The first rule whose conditions all hold (only those marked `interrupt`
/// when a move is under way).
fn pick(tree: &[Rule], v: &View, rng: &mut Rng, interrupts: bool) -> Option<Move> {
    tree.iter()
        .filter(|r| !interrupts || r.interrupt)
        .find(|r| r.when.iter().all(|c| holds(c, v, rng)))
        .map(|r| r.act)
}

/// What a pilot has to go on this tick.
struct View<'a> {
    w: &'a World,
    id: usize,
    f: &'a Fighter,
    me: Seen,
    they: Seen,
}

impl View<'_> {
    /// Feet to feet, across the ground, centimetres.
    fn gap_cm(&self) -> i32 {
        (self.they.feet.sub(self.me.feet).len_xz() as i64 * 100 / ONE as i64) as i32
    }

    /// My reach, centimetres.
    fn reach_cm(&self) -> i32 {
        (reach(self.f) as i64 * 100 / ONE as i64) as i32
    }

    /// The band I keep to, centimetres feet to feet.
    fn band(&self) -> (i32, i32) {
        let k = self.w.pilots.book.kinds[key_of(self.w.foes[self.id - 1].brain.kind)].keep_cm;
        (self.reach_cm() + k[0], self.reach_cm() + k[1])
    }
}

/// A fighter's reach: from its sword shoulder to its tip with the arm out
/// as far as the shell goes (`arm::REACH_MAX_PCT`) and the blade along it.
/// Short sword 1.39 m for the player, cleaver 1.44 m for an orc, sword 1.77
/// m for the leader.
pub fn reach(f: &Fighter) -> Fx {
    let w = f.arm.weapon;
    f.arm.reach() * arm::REACH_MAX_PCT / 100 + (w.len - w.grip[1])
}

fn cm(c: i32) -> Fx {
    c * ONE / 100
}

fn holds(c: &Cond, v: &View, rng: &mut Rng) -> bool {
    match c {
        Cond::HurtBelow(pct) => v.f.health.pct() < *pct,
        Cond::GapAbove(c) => v.gap_cm() > *c,
        Cond::GapBelow(c) => v.gap_cm() < *c,
        Cond::BeyondReach(c) => v.gap_cm() > v.reach_cm() + *c,
        Cond::WithinReach(c) => v.gap_cm() < v.reach_cm() + *c,
        Cond::HasThrowable => v.f.pouch.any() && v.f.throw.recover == 0 && !v.f.throw.winding(),
        Cond::TipNearHead(c) => v.they.tip.dist(v.me.head) < cm(*c),
        Cond::SwingComing(cps) => {
            let speed = v.they.tip_vel.len() as i64 * 60 * 100 / ONE as i64;
            let to_me = v.me.chest.sub(v.they.tip);
            speed > *cps as i64 && to_me.len() < cm(280) && v.they.tip_vel.dot(to_me) > 0
        }
        Cond::Opening(c) => {
            let on = arm::nearest_on(v.they.chest, v.me.chest, v.they.tip);
            v.they.tip.dist(on) > cm(*c)
        }
        Cond::Chance(pct) => rng.below(100) < *pct,
        Cond::FoeDown => v.they.down,
    }
}

/// Guard at `shell`, keeping to the band a little outside its reach.
fn hold(v: &View, shell: (i32, i32, i32), dir: i32) -> Plan {
    let gap = v.gap_cm();
    let (near, far) = v.band();
    let keys = if gap < near {
        input::BACK
    } else if gap > far {
        input::FORWARD
    } else {
        0
    };
    let _ = dir;
    Plan {
        shell,
        keys,
        swing: false,
        fast: false,
    }
}

/// An attack's footwork: in until the one it fights is `LUNGE_CM` inside
/// its reach (their body's own width brings the blade to them).
fn lunge(v: &View) -> u32 {
    if v.gap_cm() > v.reach_cm() - LUNGE_CM {
        input::FORWARD
    } else {
        0
    }
}

/// And after it, back out to the band.
fn recover(v: &View) -> u32 {
    if v.gap_cm() < v.band().0 {
        input::BACK
    } else {
        0
    }
}

/// How far inside its reach an attack steps, centimetres.
pub const LUNGE_CM: i32 = 10;

/// One tick of a move, or `None` once it is over.
fn play(m: Move, t: u32, v: &View, dir: i32) -> Option<Plan> {
    let side_key = if dir > 0 { input::RIGHT } else { input::LEFT };
    let plain = |shell, keys| Plan {
        shell,
        keys,
        swing: false,
        fast: false,
    };
    let strike = |shell, keys| Plan {
        shell,
        keys,
        swing: true,
        fast: true,
    };
    let guard = hold(v, GUARD, dir);
    Some(match m {
        Move::Guard | Move::Cast if t <= 12 => guard,
        Move::CloseIn if t <= 18 => plain(GUARD, input::FORWARD),
        Move::Circle if t <= 24 => plain(GUARD, guard.keys | side_key),
        Move::RaiseGuard if t <= 18 => plain((-30, 62, 74), guard.keys),
        Move::Block if t <= 14 => plain(block_at(v), 0),
        Move::BackOff if t <= 20 => plain(GUARD, input::BACK),
        Move::Flee if t <= 48 => plain((-20, -30, 60), input::BACK | side_key),
        Move::Throw if t <= 14 => plain(GUARD, if t == 0 { input::THROW } else { 0 }),
        Move::Ring if t <= 40 => {
            let gap = v.gap_cm();
            let keys = if gap > 900 {
                input::FORWARD
            } else if gap < 600 {
                input::BACK
            } else {
                side_key
            };
            plain(GUARD, keys)
        }
        // each attack: wound up while stepping in, struck still stepping
        // in, then back out to the band to recover
        Move::Overhead => match t {
            0..=13 => plain((10, 78, 70), lunge(v)),
            14..=29 => strike((-8, -36, 96), if t < 22 { lunge(v) } else { 0 }),
            30..=44 => plain(GUARD, recover(v)),
            _ => return None,
        },
        Move::Sweep => match t {
            0..=11 => plain((75, 8, 86), lunge(v)),
            12..=27 => strike((-65, -8, 96), if t < 20 { lunge(v) } else { 0 }),
            28..=42 => plain(GUARD, recover(v)),
            _ => return None,
        },
        Move::Thrust => match t {
            0..=9 => plain((0, -6, 45), lunge(v)),
            10..=21 => strike((-2, -6, 97), lunge(v)),
            22..=36 => plain(GUARD, recover(v)),
            _ => return None,
        },
        Move::Feint => match t {
            0..=9 => plain((10, 70, 72), 0),
            10..=18 => plain(GUARD, 0),
            _ => return None,
        },
        _ => return None,
    })
}

/// Where to put the hand to block: on the line from the sword shoulder to
/// the tip coming in, as it was seen.
fn block_at(v: &View) -> (i32, i32, i32) {
    let sh = v.f.arm.frame.shoulder[1];
    let d = v.they.tip.sub(sh);
    let yaw = player::body(v.w, v.id).facing;
    let side = -fx::wrap(yaw_of(d) - yaw) * 360 / fx::TURN;
    let pitch = fx::wrap(fx::atan2(d.y, d.len_xz().max(1))) * 360 / fx::TURN;
    (side.clamp(-70, 80), pitch.clamp(-45, 80), 72)
}

/// The record that moves the pilot's hand toward the plan's shell target at
/// its mouse speed, with the plan's keys and swing.
fn to_input(f: &Fighter, p: &Plan, spec: &KindSpec) -> Input {
    let s = &f.arm.shell;
    let px = if p.fast { spec.strike_px } else { spec.hand_px };
    let most = (px as i64 * arm::UNITS_PER_100_PX / 100) as i32;
    let dside = (deg(p.shell.0) - s.side).clamp(-most, most);
    let dpitch = (deg(p.shell.1) - s.pitch).clamp(-most, most);
    let (dx, dy) = mouse_for(dside, dpitch);
    let reach = f.arm.reach() * p.shell.2 / 100;
    let notch = arm::REACH_PER_NOTCH as i64;
    let wheel = (-((reach - s.reach) as i64) * 10_000 / notch).clamp(-20_000, 20_000) as i32;
    Input {
        keys: p.keys,
        buttons: if p.swing { input::SWING } else { 0 },
        dx,
        dy,
        wheel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::DRAW;

    fn duel(kind: &str) -> World {
        let mut w = World::new(3);
        player::act(&mut w, "spawn", kind).unwrap();
        w
    }

    #[test]
    fn the_book_reads_every_kind_and_tree_and_refuses_a_missing_tree() {
        let b = Book::standard();
        for k in ["orc", "bandit", "leader"] {
            assert!(b.kinds.contains_key(k), "{k}");
        }
        assert_eq!(b.kinds["orc"].held, "cleaver");
        let mut v: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/pilots.json")).unwrap();
        v["kinds"]["orc"]["tree"] = "nowhere".into();
        assert_eq!(
            Book::parse(&v.to_string()),
            Err("the orc names a tree \"nowhere\" that is not there".into())
        );
    }

    /// CW-3: a foe run by its tree, its records kept; then the same duel
    /// with the tree taken out and those records played back. Every body
    /// and arm comes out the same: the pilot moved nothing but the record.
    #[test]
    fn a_pilot_moves_its_body_only_through_its_input_record() {
        let player_inputs: Vec<Input> = (0..360)
            .map(|k| Input {
                keys: if k == 2 {
                    DRAW
                } else if k % 50 < 20 {
                    input::FORWARD
                } else {
                    0
                },
                dx: if k % 30 < 15 { 300 } else { -300 },
                dy: if k % 40 < 20 { 150 } else { -150 },
                buttons: if k % 60 > 40 { input::SWING } else { 0 },
                ..Input::default()
            })
            .collect();
        let mut a = duel("orc");
        let mut kept = Vec::new();
        for i in &player_inputs {
            // a tick the hit-stop holds (`feel`) asks no pilot for a record
            let held = a.feel.stop > 0;
            a.step(i);
            if !held {
                kept.push(a.foes[0].fighter.last);
            }
        }
        assert!(
            kept.iter().any(|i| i.keys != 0) && kept.iter().any(|i| i.dx != 0),
            "the pilot pressed something"
        );
        let mut b = duel("orc");
        b.foes[0].brain.control = Control::Script(kept);
        for i in &player_inputs {
            b.step(i);
        }
        assert_eq!(a.player, b.player);
        assert_eq!(a.me, b.me);
        assert_eq!(a.foes[0].body, b.foes[0].body);
        assert_eq!(a.foes[0].fighter, b.foes[0].fighter);
    }

    #[test]
    fn a_foe_out_of_reach_closes_in_and_one_in_reach_swings() {
        let mut w = duel("orc");
        let start = w.foes[0].body.pos.dist(w.player.pos);
        let mut swung = false;
        for _ in 0..240 {
            w.step(&Input::default());
            swung |= w.foes[0].fighter.last.button(input::SWING);
        }
        let now = w.foes[0].body.pos.dist(w.player.pos);
        assert!(now < start - fx::int(3), "closed in from {start} to {now}");
        assert!(swung, "and swung");
    }

    #[test]
    fn a_pilot_sees_the_others_as_they_were_its_reaction_time_ago() {
        let mut w = duel("orc");
        for k in 0..40 {
            w.player.pos.x += ONE / 10 * (k % 2);
            w.step(&Input::default());
        }
        let r = w.pilots.book.kinds["orc"].reaction_ticks as usize;
        let then = w.pilots.memory.ago(r, 0).unwrap();
        let n = w.pilots.memory.ticks.len();
        assert_eq!(then, w.pilots.memory.ticks[n - 1 - r][0]);
        assert_eq!(n, MAX_REACTION + 1, "only so much is kept");
    }

    #[test]
    fn in_a_group_one_duels_and_the_others_keep_their_distance() {
        let mut w = World::new(5);
        // a player who does not fall, so the ring holds for the whole test
        w.me.health = crate::health::Health::new(100_000);
        for k in ["bandit", "bandit", "bandit"] {
            player::act(&mut w, "spawn", k).unwrap();
        }
        for _ in 0..300 {
            w.step(&Input::default());
        }
        let d = dueller(&w).expect("one duels");
        let gaps: Vec<Fx> = (1..4)
            .map(|id| player::body(&w, id).pos.dist(w.player.pos))
            .collect();
        assert!(gaps[d - 1] < fx::int(4), "the dueller is close: {gaps:?}");
        for id in 1..4 {
            if id != d {
                assert!(gaps[id - 1] > fx::int(5), "the others ring: {gaps:?}");
            }
        }
    }

    /// CW-4: a recorded duel (a fixed list of the player's records against
    /// an orc's tree, 600 ticks) hashes the same every run, checked every
    /// second of it; one record changed by one hundredth of a pixel shows.
    #[test]
    fn a_recorded_duel_hashes_the_same_every_run() {
        let record: Vec<Input> = (0..600)
            .map(|k| Input {
                keys: match k {
                    3 => DRAW,
                    _ if k % 120 < 30 => input::FORWARD,
                    _ if k % 120 < 45 => input::LEFT,
                    _ => 0,
                },
                buttons: if k % 90 > 70 { input::SWING } else { 0 },
                dx: if k % 90 < 60 { 450 } else { -2_400 },
                dy: if k % 50 < 25 { 300 } else { -300 },
                wheel: if k % 200 == 100 { 10_000 } else { 0 },
            })
            .collect();
        let run = |rec: &[Input]| {
            let mut w = duel("orc");
            let mut marks = Vec::new();
            for (k, i) in rec.iter().enumerate() {
                w.step(i);
                if k % 60 == 59 {
                    marks.push(w.hash());
                }
            }
            (marks, w)
        };
        let (a, wa) = run(&record);
        let (b, wb) = run(&record);
        assert_eq!(a, b);
        assert_eq!(wa, wb);
        // the duel was a duel: blades met or a blow landed somewhere in it
        assert!(
            wa.me.health.hp < wa.me.health.most
                || wa.foes[0].fighter.health.hp < wa.foes[0].fighter.health.most
        );
        let mut other = record.clone();
        other[300].dx += 1;
        let (c, _) = run(&other);
        assert_eq!(c[..5], a[..5], "the same until tick 300");
        assert_ne!(c[9], a[9], "and then not");
    }

    /// By hand, from `pose::build` and M3's grip-to-tip lengths: the
    /// player's arm 20 316 + 19 005 units, 97% is 38 141, and the short
    /// sword's 806 mm (52 822): 90 963 (1.388 m). An orc's arm scaled to 2
    /// m, 23 901 + 22 358, 97% 44 871, and the cleaver's 750 mm (49 152):
    /// 94 023. A bandit's at 1.75 m, 20 913 + 19 563, 97% 39 261, and a
    /// short sword: 92 083. The leader's at 1.85 m, 22 108 + 20 681, 97%
    /// 41 505, and the sword's 1 138 mm (74 579): 116 084.
    #[test]
    fn a_fighter_s_reach_is_its_arm_at_full_stretch_and_its_blade() {
        let w = World::new(1);
        assert_eq!(reach(&w.me), 90_963);
        let book = Book::standard();
        for (kind, want) in [
            (Kind::Orc, 94_023),
            (Kind::Bandit, 92_083),
            (Kind::Leader, 116_084),
        ] {
            let f = foe(&book, kind, V3::ZERO, 0, 1);
            assert_eq!(reach(&f.fighter), want, "{kind:?}");
        }
    }

    /// Blade Symphony's rhythm, against a player who stands drawn and does
    /// nothing: each foe keeps to its band a little outside its own reach
    /// (the gaps sampled while it guards or circles, half of them inside
    /// its `keep_cm`), steps in through every attack to inside its reach,
    /// and is back out past its reach when the attack is over, unless it ran
    /// onto the player's blade and is staggering.
    #[test]
    fn a_foe_keeps_a_little_outside_its_reach_steps_in_to_attack_and_out_to_recover() {
        // each band by hand: its reach (above) and `keep_cm`: orc 143 + 15
        // to 45, bandit 140 + 10 to 40, leader 177 + 5 to 35
        for (seed, kind, band) in [
            (3, "orc", (158, 188)),
            (3, "bandit", (150, 180)),
            (3, "leader", (182, 212)),
        ] {
            let mut w = World::new(seed);
            w.me.health = crate::health::Health::new(100_000);
            player::act(&mut w, "spawn", kind).unwrap();
            let r = (reach(&w.foes[0].fighter) as i64 * 100 / ONE as i64) as i32;
            let (mut held, mut attacks, mut nearest) = (Vec::new(), 0, i32::MAX);
            // an attack in which it ran onto the player's blade and staggered
            // (`feel`) recovers more slowly: its steps back are a quarter
            let mut shaken = false;
            let mut was: Option<Move> = None;
            for t in 0..1200 {
                let keys = if t == 2 { DRAW } else { 0 };
                w.step(&Input {
                    keys,
                    ..Input::default()
                });
                let gap = (w.foes[0].body.pos.sub(w.player.pos).len_xz() as i64 * 100 / ONE as i64) as i32;
                let now = w.foes[0].brain.current.map(|(m, _)| m);
                let attacking = |m: Option<Move>| m.is_some_and(Move::committed);
                if attacking(now) {
                    nearest = nearest.min(gap);
                    shaken |= w.foes[0].fighter.gait.stagger > 0;
                }
                if attacking(was) && !attacking(now) {
                    attacks += 1;
                    assert!(
                        nearest < r,
                        "{kind}: an attack stepped only to {nearest} cm, reach {r}"
                    );
                    assert!(
                        shaken || gap > r,
                        "{kind}: still {gap} cm in after an attack, reach {r}"
                    );
                    nearest = i32::MAX;
                    shaken = false;
                }
                if t > 300 && matches!(now, None | Some(Move::Circle | Move::Guard)) {
                    held.push(gap);
                }
                was = now;
            }
            held.sort();
            let mid = held[held.len() / 2];
            assert!(attacks >= 3, "{kind}: {attacks} attacks");
            assert!(
                (band.0..=band.1).contains(&mid),
                "{kind}: held at {mid} cm, its band {band:?}"
            );
        }
    }

    #[test]
    fn a_pilot_sends_the_mouse_movement_that_moves_its_hand_toward_the_plan() {
        let w = duel("orc");
        let f = &w.foes[0].fighter;
        let spec = &w.pilots.book.kinds["orc"];
        let p = Plan {
            shell: (60, 60, 82),
            keys: 0,
            swing: false,
            fast: false,
        };
        let i = to_input(f, &p, spec);
        // far from 60°, 60°: the most a tick, hand_px pixels each way
        assert_eq!(i.dx, spec.hand_px * 100);
        assert_eq!(i.dy, -spec.hand_px * 100);
    }
}
