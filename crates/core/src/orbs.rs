//! `orbs` (M6 Magic, task M): magic as Long Time Comin's bullet patterns,
//! in 3D at duel distance. A caster throws a volley from before its chest
//! at the player's: fans, rings that hang and then close on you or fly
//! past round you, spirals (a ring of one that turns each repeat) and waves
//! with a hole to step into. Each kind of orb has its own shape and colour
//! (CW-10) and a halo; the kinds and both MVP patterns are numbers in
//! `data/orbs.json`. Ported from Long Time Comin's `orbs.rs` (`fire`'s
//! fans, rings, spin and sweep, the curl, the held formation's tell), with
//! what a sword needs added:
//!
//! - **Turning one orb.** While the player's blade is imbued (`imbue::on`),
//!   its edge, swept from where it began the tick to where it ended it as
//!   M1's blades are (`blade::sweep`), turns each orb it touches, one at a
//!   time: the orb is reflected about the edge's direction of travel
//!   (`turned_way`; a still edge sends it straight back), and an orb turned to within
//!   45° of its caster flies straight at it (`ASSIST_COS`). A turned orb never turns
//!   again, takes the turned colour, flies faster, and hits what it meets
//!   on the foes' side, its caster most of all. An edge that is not
//!   imbued does nothing to an orb: it passes through and hits you.
//! - **Landing.** An orb not turned hits the player's body (health by its
//!   kind), and stops on the ground and on the props M5 gives the
//!   throwables (`throwables::State::props`). It is spent after
//!   `LIFE` ticks, `REACH` from where it was cast, or `PAST` behind you.
//!
//! Steps at 6 in `World::step`, after the imbue. `deflected` counts the
//! orbs the player has turned from each caster since its last `cast`.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

// --- Doors, set by the coordinator so M5 (story) and M6 (magic) can run in
// parallel. M6 fills the bodies; the signatures stay. M5 calls them.

pub mod shapes;

use std::sync::OnceLock;

use serde::Deserialize;

use crate::blade::{self, Swept};
use crate::fx::{self, Fx, ONE};
use crate::geom::{facing, right_of, v3, yaw_of, Affine, V3};
use crate::look::{mix, rgb, Look, Rgb};
use crate::player;
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::world::World;

/// Which pattern a caster throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    /// the bandit leader's, cast at the player in the camp
    Leader,
    /// the trainer's practice round (T2): slower, fewer orbs, harmless
    Practice,
}

// ---- the data: data/orbs.json ---------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Sphere,
    Tetra,
    Octa,
    Cube,
    Star,
    Torus,
}

impl Shape {
    pub fn mesh(self) -> Mesh {
        match self {
            Shape::Sphere => Mesh::Sphere,
            Shape::Tetra => Mesh::Tetra,
            Shape::Octa => Mesh::Octa,
            Shape::Cube => Mesh::Cube,
            Shape::Star => Mesh::Star,
            Shape::Torus => Mesh::Torus,
        }
    }
}

/// What turning or landing an orb does besides its damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    None,
    /// turning it jars the sword arm: both arms weakened `knock_ticks`
    Knock,
    /// turning it pushes the player back along its flight at `shove_cms`
    Shove,
    /// what it hits has both arms weakened in full (`health::WEAK_TICKS`)
    Stun,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kind {
    pub id: String,
    pub shape: Shape,
    pub colour: [i32; 3],
    pub halo: [i32; 3],
    pub radius_cm: i32,
    pub damage: i32,
    pub turnable: bool,
    pub effect: Effect,
}

impl Kind {
    pub fn radius(&self) -> Fx {
        self.radius_cm * ONE / 100
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Form {
    Fan,
    Ring,
    Wave,
}

/// How a held orb sets off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Launch {
    /// along the line from the caster to the player as it was cast
    #[default]
    Along,
    /// each at the player's chest as it sets off
    Aim,
}

fn one() -> u32 {
    1
}

fn one_i() -> i32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Volley {
    pub form: Form,
    pub kind: String,
    #[serde(default = "one_i")]
    pub count: i32,
    #[serde(default)]
    pub spread_deg: i32,
    #[serde(default)]
    pub radius_cm: i32,
    #[serde(default)]
    pub hold: u32,
    #[serde(default)]
    pub launch: Launch,
    #[serde(default)]
    pub spacing_cm: i32,
    #[serde(default)]
    pub gap: i32,
    pub speed_cms: i32,
    #[serde(default)]
    pub delay: u32,
    #[serde(default = "one")]
    pub repeat: u32,
    #[serde(default = "one")]
    pub every: u32,
    #[serde(default)]
    pub spin_deg: i32,
    #[serde(default)]
    pub sweep_deg: i32,
    #[serde(default)]
    pub curl_degs: i32,
    #[serde(default)]
    pub alternate: bool,
}

impl Volley {
    /// The tick of the cast's clock its last repeat fires on.
    pub fn last_tick(&self) -> u32 {
        self.delay + self.every * self.repeat.saturating_sub(1)
    }

    /// How many orbs it throws in all.
    pub fn orbs(&self) -> u32 {
        let per = match self.form {
            Form::Wave => (self.count - self.gap.clamp(0, self.count)).max(0),
            _ => self.count.max(0),
        };
        per as u32 * self.repeat
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Colours {
    pub colour: [i32; 3],
    pub halo: [i32; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patterns {
    pub leader: Vec<Volley>,
    pub practice: Vec<Volley>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub turned: Colours,
    pub imbue: Colours,
    pub knock_ticks: u16,
    pub shove_cms: i32,
    pub kinds: Vec<Kind>,
    pub patterns: Patterns,
}

impl Spec {
    /// Reads `data/orbs.json` and checks it: kinds named once, each with a
    /// shape of its own (CW-10), every volley of a kind there is.
    pub fn parse(json: &str) -> Result<Spec, String> {
        let s: Spec =
            serde_json::from_str(json).map_err(|e| format!("The orbs file could not be read: {e}"))?;
        for (i, k) in s.kinds.iter().enumerate() {
            for other in &s.kinds[..i] {
                if other.id == k.id {
                    return Err(format!("The orbs file names the kind \"{}\" twice.", k.id));
                }
                if other.shape == k.shape {
                    return Err(format!(
                        "The orbs file gives \"{}\" and \"{}\" one shape; each kind needs its own.",
                        other.id, k.id
                    ));
                }
            }
        }
        for v in s.patterns.leader.iter().chain(&s.patterns.practice) {
            if s.kind(&v.kind).is_none() {
                return Err(format!(
                    "The orbs file's pattern throws a kind \"{}\" it does not list.",
                    v.kind
                ));
            }
            if v.every == 0 || v.speed_cms <= 0 {
                return Err(
                    "The orbs file's volleys need a speed and at least a tick between repeats.".into(),
                );
            }
        }
        Ok(s)
    }

    /// A kind's number by its name.
    pub fn kind(&self, id: &str) -> Option<u8> {
        self.kinds.iter().position(|k| k.id == id).map(|k| k as u8)
    }

    pub fn pattern(&self, p: Pattern) -> &[Volley] {
        match p {
            Pattern::Leader => &self.patterns.leader,
            Pattern::Practice => &self.patterns.practice,
        }
    }
}

/// The spec the game runs on, read once.
pub fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| Spec::parse(include_str!("../../../data/orbs.json")).expect("data/orbs.json"))
}

// ---- the rules' numbers -----------------------------------------------------

/// An orb is spent after this many ticks (7 s), this far from where it was
/// cast, or this far behind the player's chest (it has missed).
pub const LIFE: u16 = 420;
pub const REACH: Fx = fx::int(60);
pub const PAST: Fx = fx::int(4);
/// A volley sets off this far before the caster's chest and this high
/// above it: where its hands are raised.
pub const HANDS_AHEAD: Fx = ONE * 45 / 100;
pub const HANDS_UP: Fx = ONE / 10;
/// The edge turns an orb that comes within its radius, plus the blade's
/// half thickness (`blade::BLADE_GAP` / 2), plus this: a little
/// forgiveness, so a near miss with the edge counts.
pub const FORGIVE: Fx = ONE * 4 / 100;
/// A body is hit by an orb within this share of its radius.
pub const BODY_PCT: i32 = 90;
/// An edge moving under this a tick (0.3 m/s) is still: it turns an orb
/// off its flat, not off its sweep.
pub const STILL: Fx = ONE / 200;
/// A turned orb sent to within this angle of its caster's chest flies
/// straight at it (the cosine of 45°: 0.707 11).
pub const ASSIST_COS: Fx = 46_341;
/// A turned orb flies at 140% of its speed, and at least 9 m/s.
pub const TURNED_PCT: i32 = 140;
pub const TURNED_MIN: Fx = ONE * 9 / 60;
/// A turned orb takes this share of its kind's damage from what it hits
/// (200% let a well-swung imbue fell the leader with its own pattern).
pub const TURNED_DAMAGE_PCT: i32 = 100;
/// The most substeps an orb and the edge are swept in.
pub const SWEEP_MAX: u32 = 32;

// ---- the state ------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Orb {
    /// in the order thrown, which is the order drawn (CW-18)
    pub id: u32,
    pub kind: u8,
    pub caster: u8,
    pub pos: V3,
    pub prev: V3,
    /// unit direction of travel
    pub dir: V3,
    /// metres a tick
    pub speed: Fx,
    /// the turn about the vertical each tick
    pub curl: i32,
    /// ticks it still hangs before it sets off, of `held` in all
    pub hold: u16,
    pub held: u16,
    pub launch: Launch,
    pub age: u16,
    /// where it was cast from, for `REACH`
    pub from: V3,
    /// the age it was turned at
    pub turned: Option<u16>,
}

impl Orb {
    pub fn kind(&self) -> &'static Kind {
        &spec().kinds[self.kind as usize]
    }
}

/// A pattern under way: whose, which, and its clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cast {
    pub caster: u8,
    pub pattern: Pattern,
    pub age: u32,
}

/// A flash where an orb was turned (`turn` true: the ping) or where one
/// landed, burning out over `TURN_SPARK` or `BURST` ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spark {
    pub id: u32,
    pub at: V3,
    pub dir: V3,
    pub kind: u8,
    pub turn: bool,
    pub age: u8,
}

pub const TURN_SPARK: u8 = 20;
pub const BURST: u8 = 12;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    pub casts: Vec<Cast>,
    pub flying: Vec<Orb>,
    /// orbs turned from each caster since its last cast, by caster
    pub turned: Vec<(u8, u32)>,
    pub sparks: Vec<Spark>,
    /// held still for a view (`magic.freeze`): nothing moves or lands
    pub frozen: bool,
    /// freeze once the newest cast's clock reaches this
    pub freeze_at: Option<u32>,
    pub next: u32,
}

// ---- the doors ------------------------------------------------------------

/// Fighter `caster` (`player::fighter` ids; 0 is the player) starts a
/// volley of `pattern` aimed at the player. A cast while its last is still
/// under way starts the pattern again; orbs already thrown fly on. The
/// count of orbs turned from it starts again at 0.
pub fn cast(w: &mut World, caster: usize, pattern: Pattern) {
    if caster >= player::count(w) || caster > u8::MAX as usize {
        return;
    }
    let c = caster as u8;
    w.orbs.casts.retain(|k| k.caster != c);
    w.orbs.casts.push(Cast {
        caster: c,
        pattern,
        age: 0,
    });
    w.orbs.turned.retain(|(k, _)| *k != c);
}

/// Orbs from `caster` that the player has turned with a blade since its
/// last `cast`. The trainer's practice passes on this.
pub fn deflected(w: &World, caster: usize) -> u32 {
    w.orbs
        .turned
        .iter()
        .find(|(k, _)| *k as usize == caster)
        .map_or(0, |(_, n)| *n)
}

/// Whether any orb from `caster` is still in flight, or its pattern still
/// has volleys to throw.
pub fn flying(w: &World, caster: usize) -> bool {
    w.orbs.casts.iter().any(|c| c.caster as usize == caster)
        || w.orbs.flying.iter().any(|o| o.caster as usize == caster)
}

// ---- throwing ---------------------------------------------------------------

/// Where a fighter's volley sets off: before its chest, its hands raised.
pub fn hands(w: &World, id: usize) -> V3 {
    let f = player::fighter(w, id);
    f.pose
        .chest
        .add(facing(f.pose.facing, 0).scale(HANDS_AHEAD))
        .add(v3(0, HANDS_UP, 0))
}

/// The player's chest, where every volley aims.
pub fn target(w: &World) -> V3 {
    w.me.pose.chest
}

/// cm a second to metres a tick.
fn per_tick(cms: i32) -> Fx {
    (cms as i64 * ONE as i64 / 100 / crate::ticks::TICKS_PER_SECOND) as Fx
}

/// The orbs of repeat `r` of a volley thrown from `origin` at `target`.
pub fn fire(v: &Volley, origin: V3, target: V3, r: u32, caster: u8, next: &mut u32) -> Vec<Orb> {
    let s = spec();
    let kind = s.kind(&v.kind).unwrap_or(0);
    let to = target.sub(origin);
    let aim = to.norm();
    let yaw = yaw_of(to);
    // across the line to the player, and up in the plane facing them
    let across = right_of(yaw);
    let up = across.cross(aim).norm();
    let speed = per_tick(v.speed_cms);
    let curl = fx::deg(v.curl_degs) / crate::ticks::TICKS_PER_SECOND as i32;
    let r = r as i32;
    let n = v.count.max(0);
    let mut out = Vec::new();
    let mut push = |pos: V3, dir: V3, i: i32, hold: u32| {
        let curl = if v.alternate && i % 2 == 1 { -curl } else { curl };
        out.push(Orb {
            id: *next,
            kind,
            caster,
            pos,
            prev: pos,
            dir,
            speed,
            curl,
            hold: hold.min(u16::MAX as u32) as u16,
            held: hold.min(u16::MAX as u32) as u16,
            launch: v.launch,
            age: 0,
            from: origin,
            turned: None,
        });
        *next = next.wrapping_add(1);
    };
    match v.form {
        Form::Fan => {
            let spread = fx::deg(v.spread_deg);
            let sweep = if v.sweep_deg != 0 && v.repeat > 1 {
                -fx::deg(v.sweep_deg) / 2 + fx::deg(v.sweep_deg) * r / (v.repeat as i32 - 1)
            } else {
                0
            };
            for i in 0..n {
                let off = if n == 1 {
                    0
                } else {
                    -spread / 2 + spread * i / (n - 1)
                };
                push(origin, turn_y(aim, off + sweep), i, 0);
            }
        }
        Form::Ring => {
            let radius = v.radius_cm * ONE / 100;
            let base = fx::deg(v.spin_deg).wrapping_mul(r);
            for i in 0..n {
                let a = base + fx::TURN / n.max(1) * i;
                let out_of = across.scale(fx::cos(a)).add(up.scale(fx::sin(a)));
                let pos = origin.add(out_of.scale(radius));
                let dir = match v.launch {
                    Launch::Aim => target.sub(pos).norm(),
                    Launch::Along => aim,
                };
                push(pos, dir, i, v.hold);
            }
        }
        Form::Wave => {
            let g = v.gap.clamp(0, n);
            // the hole moves left, middle, right as the volley repeats
            let hole = [n / 4, (n - g) / 2, n - n / 4 - g][r.rem_euclid(3) as usize].clamp(0, (n - g).max(0));
            let spacing = v.spacing_cm * ONE / 100;
            for i in 0..n {
                if (hole..hole + g).contains(&i) {
                    continue;
                }
                let off = spacing * (2 * i - (n - 1)) / 2;
                push(origin.add(across.scale(off)), aim, i, 0);
            }
        }
    }
    out
}

/// `d` turned by `a` about the vertical, the way a heading turns
/// (`geom::facing`): x' = x cos a + z sin a, z' = z cos a − x sin a.
pub fn turn_y(d: V3, a: i32) -> V3 {
    if a == 0 {
        return d;
    }
    let (s, c) = (fx::sin(a), fx::cos(a));
    v3(
        fx::mul(d.x, c) + fx::mul(d.z, s),
        d.y,
        fx::mul(d.z, c) - fx::mul(d.x, s),
    )
}

// ---- turning ----------------------------------------------------------------

/// The way an orb flying along `d` leaves an edge that met it moving
/// `travel` this tick: `d` reflected about the edge's direction of travel
/// `t` (turned half a turn round it), 2 (d · t) t − d. What it carried
/// along the swing it keeps; the rest turns back, so a swing across its
/// line sends it back the way it came, whichever way the blade points. An
/// edge moving under `STILL` is a block: the orb goes back the way it
/// came, −d. Unit.
pub fn turned_way(d: V3, travel: V3) -> V3 {
    if travel.len() < STILL {
        return d.neg();
    }
    let t = travel.norm();
    t.scale(2 * d.dot(t)).sub(d).norm()
}

/// The edge of the player's blade swept over this tick, guard to tip, when
/// it can turn orbs: imbued, in hand, standing.
pub fn imbued_edge(w: &World) -> Option<Swept> {
    if !crate::imbue::on(w) || !w.me.arm.in_hand || w.me.health.down() {
        return None;
    }
    Some(edge(&w.me.arm))
}

/// An arm's edge, from the guard to the tip, at the start and end of the
/// tick (as `blade.rs` sweeps it).
pub fn edge(a: &crate::arm::Arm) -> Swept {
    let (p, t) = (&a.pts[crate::arm::POMMEL], &a.pts[crate::arm::TIP]);
    let share = fx::div(a.weapon.hilt, a.weapon.len.max(1));
    Swept {
        a0: p.q.lerp(t.q, share),
        b0: t.q,
        a1: p.p.lerp(t.p, share),
        b1: t.p,
    }
}

/// What an orb meets first this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Meets {
    Edge(blade::Contact),
    Body(usize),
    Ground,
}

/// Turns orb `o` off the edge at contact `c`.
fn turn(w: &mut World, o: &mut Orb, e: &Swept, c: &blade::Contact) {
    let at = c.on_a;
    let start = e.a0.lerp(e.b0, c.f);
    let end = e.a1.lerp(e.b1, c.f);
    let came = o.dir;
    let mut dir = turned_way(came, end.sub(start));
    if (o.caster as usize) < player::count(w) && o.caster != 0 {
        let home = player::fighter(w, o.caster as usize).pose.chest.sub(at).norm();
        if dir.dot(home) >= ASSIST_COS {
            dir = home;
        }
    }
    o.dir = dir;
    o.pos = at;
    o.speed = (o.speed as i64 * TURNED_PCT as i64 / 100).max(TURNED_MIN as i64) as Fx;
    o.curl = 0;
    o.turned = Some(o.age);
    match w.orbs.turned.iter_mut().find(|(k, _)| *k == o.caster) {
        Some((_, n)) => *n += 1,
        None => w.orbs.turned.push((o.caster, 1)),
    }
    let s = spec();
    match o.kind().effect {
        Effect::Knock => {
            for weak in &mut w.me.health.weak {
                *weak = (*weak).max(s.knock_ticks);
            }
        }
        Effect::Shove => {
            let push = v3(came.x, 0, came.z).norm().scale(per_tick(s.shove_cms));
            w.player.vel = w.player.vel.add(push);
        }
        Effect::None | Effect::Stun => {}
    }
    spark(w, o, c.on_b, dir, true);
}

fn spark(w: &mut World, o: &Orb, at: V3, dir: V3, turn: bool) {
    w.orbs.sparks.push(Spark {
        id: o.id,
        at,
        dir,
        kind: o.kind,
        turn,
        age: 0,
    });
}

/// Orb `o` lands on fighter `id`: health by its kind (`TURNED_DAMAGE_PCT`
/// of that turned), a stun weakening both arms.
fn land_on(w: &mut World, o: &Orb, id: usize) {
    let k = o.kind();
    let dmg = if o.turned.is_some() {
        k.damage * TURNED_DAMAGE_PCT / 100
    } else {
        k.damage
    };
    let h = &mut player::parts_mut(w, id).1.health;
    if h.down() {
        return;
    }
    h.hp -= dmg;
    if k.effect == Effect::Stun {
        h.weak = [crate::health::WEAK_TICKS; 2];
    }
}

/// Moves one orb a tick; false once spent.
fn fly(w: &mut World, o: &mut Orb, edge: Option<&Swept>, bodies: &[(bool, Vec<Swept>, Vec<Fx>)]) -> bool {
    o.prev = o.pos;
    o.age = o.age.saturating_add(1);
    if o.hold > 0 {
        o.hold -= 1;
        if o.hold > 0 {
            return o.age < LIFE;
        }
        if o.launch == Launch::Aim {
            let d = target(w).sub(o.pos).norm();
            if d != V3::ZERO {
                o.dir = d;
            }
        }
    }
    if o.curl != 0 {
        o.dir = turn_y(o.dir, o.curl);
        if o.age.is_multiple_of(16) {
            o.dir = o.dir.norm();
        }
    }
    o.pos = o.pos.add(o.dir.scale(o.speed));
    let k = o.kind();
    let r = k.radius();
    let path = Swept {
        a0: o.prev,
        b0: o.prev,
        a1: o.pos,
        b1: o.pos,
    };
    // the earliest of the edge, a body, a prop and the ground, as shares of
    // the tick
    let mut best: Option<(i64, Meets)> = None;
    let mut consider = |share: i64, m: Meets| {
        if best.is_none_or(|(b, _)| share < b) {
            best = Some((share, m));
        }
    };
    if let Some(e) = edge.filter(|_| o.turned.is_none() && k.turnable) {
        let gap = r + blade::BLADE_GAP / 2 + FORGIVE;
        if let Some(c) = blade::sweep(&path, e, gap, SWEEP_MAX, 0) {
            consider(c.k as i64 * ONE as i64 / c.s as i64, Meets::Edge(c));
        }
    }
    for (id, (open, caps, radii)) in bodies.iter().enumerate() {
        // not turned, it hits the player; turned, the foes' side
        let mine = (id == 0) == o.turned.is_none();
        if !open || !mine {
            continue;
        }
        for (cap, cr) in caps.iter().zip(radii) {
            let reach = cr + r * BODY_PCT / 100;
            if let Some(c) = blade::sweep(&path, cap, reach, 16, 0) {
                consider(c.k as i64 * ONE as i64 / c.s as i64, Meets::Body(id));
            }
        }
    }
    for b in &w.throwables.props {
        if let Some(share) = crate::throwables::through_box(o.prev, o.pos, b, r) {
            consider(share, Meets::Ground);
        }
    }
    let ground = w.ground(o.pos.x, o.pos.z);
    if o.pos.y <= ground + r / 2 {
        let drop = (o.prev.y - o.pos.y).max(1) as i64;
        let share = ((o.prev.y - ground - r / 2).max(0) as i64 * ONE as i64 / drop).min(ONE as i64);
        consider(share, Meets::Ground);
    }
    match best {
        Some((_, Meets::Edge(c))) => {
            if let Some(e) = edge {
                turn(w, o, e, &c);
            }
        }
        Some((share, Meets::Body(id))) => {
            let at = o.prev.lerp(o.pos, share as Fx);
            land_on(w, o, id);
            spark(w, o, at, o.dir, false);
            return false;
        }
        Some((share, Meets::Ground)) => {
            let at = o.prev.lerp(o.pos, share as Fx);
            spark(w, o, at, o.dir, false);
            return false;
        }
        None => {}
    }
    if o.age >= LIFE || o.pos.dist(o.from) > REACH {
        return false;
    }
    // not turned, and gone on `PAST` beyond the player: it missed
    if o.turned.is_none() && o.pos.sub(target(w)).dot(o.dir) > PAST {
        return false;
    }
    true
}

/// Step 6, after the imbue: the casts throw what is due, every orb flies a
/// tick and lands or turns, and the sparks burn down.
pub fn step(w: &mut World) {
    let e = imbued_edge(w);
    advance(w, e);
}

/// The step with the edge given: what `step` does with `imbued_edge`, and
/// what a test stages with an edge of its own.
pub fn advance(w: &mut World, edge: Option<Swept>) {
    if w.orbs.frozen {
        return;
    }
    // the casts' clocks: each volley's repeats on their ticks
    let mut casts = std::mem::take(&mut w.orbs.casts);
    let aim = target(w);
    let mut next = w.orbs.next;
    let mut thrown = Vec::new();
    casts.retain(|c| {
        let id = c.caster as usize;
        id < player::count(w) && !player::fighter(w, id).health.down()
    });
    for c in &mut casts {
        let from = hands(w, c.caster as usize);
        let volleys = spec().pattern(c.pattern);
        for v in volleys {
            if c.age >= v.delay && (c.age - v.delay) % v.every == 0 {
                let r = (c.age - v.delay) / v.every;
                if r < v.repeat {
                    thrown.extend(fire(v, from, aim, r, c.caster, &mut next));
                }
            }
        }
        c.age += 1;
    }
    if let Some(n) = w.orbs.freeze_at {
        if casts.last().is_some_and(|c| c.age > n) {
            w.orbs.frozen = true;
            w.orbs.freeze_at = None;
        }
    }
    casts.retain(|c| spec().pattern(c.pattern).iter().any(|v| c.age <= v.last_tick()));
    w.orbs.casts = casts;
    w.orbs.next = next;
    w.orbs.flying.extend(thrown);
    // every body's capsules this tick, open when standing
    let bodies: Vec<(bool, Vec<Swept>, Vec<Fx>)> = (0..player::count(w))
        .map(|id| {
            let f = player::fighter(w, id);
            let caps = blade::capsules(&f.prev_pose, &f.pose, f.build().girth_pct, f.build().head);
            (
                !f.health.down(),
                caps.iter().map(|(_, s, _)| *s).collect(),
                caps.iter().map(|(_, _, r)| *r).collect(),
            )
        })
        .collect();
    let mut keep = Vec::with_capacity(w.orbs.flying.len());
    for mut o in std::mem::take(&mut w.orbs.flying) {
        if fly(w, &mut o, edge.as_ref(), &bodies) {
            keep.push(o);
        }
    }
    w.orbs.flying = keep;
    for s in &mut w.orbs.sparks {
        s.age += 1;
    }
    w.orbs
        .sparks
        .retain(|s| s.age < if s.turn { TURN_SPARK } else { BURST });
}

// ---- drawing ----------------------------------------------------------------

/// A shape turns this far a tick as it flies (a tumble), its halo is this
/// many times its radius, and its body glows this much (`ONE` unlit).
const SPIN: i32 = fx::TURN / 90;
const HALO_X: i32 = 26;
const BODY_GLOW: Fx = ONE * 30 / 100;
const HALO: Fx = ONE * 45 / 100;
/// A turned orb takes the turned colour over this many ticks, its halo
/// flaring for the first `FLARE`.
const FADE_IN: i32 = 8;
const FLARE: i32 = 5;
/// A held orb hangs at this share of its size and swells to whole as it is
/// about to fly (Long Time Comin's tell).
const HELD_PCT: i32 = 45;

/// The shape's turn about its own axes: a tumble for the solids, the star
/// and the mote facing where they fly and turning about it.
fn spin(o: &Orb, angle: i32) -> Affine {
    match o.kind().shape {
        Shape::Sphere => Affine::IDENTITY,
        Shape::Star | Shape::Torus => Affine::looking(o.dir).then(&Affine::rot_z(angle)),
        _ => Affine::rot_y(angle).then(&Affine::rot_x(angle / 2 + (o.id as i32 % 8) * fx::TURN / 16)),
    }
}

fn size(s: Fx) -> Affine {
    Affine::scale(v3(s, s, s))
}

/// Every orb in the order thrown, then the sparks in the order struck.
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    let alpha = if w.orbs.frozen { ONE } else { alpha };
    let s = spec();
    let ink = look.ink();
    let (turned_body, turned_halo) = (rgb(s.turned.colour), rgb(s.turned.halo));
    let chest = w.me.prev_pose.chest.lerp(w.me.pose.chest, alpha);
    let ahead = facing(w.aim.yaw, 0);
    for o in &w.orbs.flying {
        let k = o.kind();
        let at = o.prev.lerp(o.pos, alpha);
        let mut d = k.radius() * 2;
        if o.hold > 0 && o.held > 0 {
            let grown = fx::ratio((o.held - o.hold) as i32, o.held as i32);
            d = fx::mul(
                d,
                ONE * HELD_PCT / 100 + fx::mul(grown, ONE * (100 - HELD_PCT) / 100),
            );
        }
        let (mut body, mut halo, mut halo_d) = (rgb(k.colour), rgb(k.halo), d * HALO_X / 10);
        if let Some(t) = o.turned {
            let since = o.age.saturating_sub(t) as i32;
            let share = fx::ratio((since + 1).min(FADE_IN), FADE_IN);
            body = mix(body, turned_body, share);
            halo = mix(halo, turned_halo, share);
            if since < FLARE {
                halo_d = halo_d * (16 - since) / 10;
            }
        }
        // passed by the player, between them and the camera: a faint halo
        // alone, so it does not fill the view
        if o.turned.is_none() && chest.sub(at).dot(ahead) > 0 {
            out.glow(
                Mesh::Sphere,
                Affine::translate(at).then(&size(halo_d)),
                halo,
                HALO / 3,
            );
            continue;
        }
        let angle = (o.age as i32 - 1) * SPIN + fx::mul(alpha, SPIN);
        let place = Affine::translate(at).then(&spin(o, angle)).then(&size(d));
        out.solid(k.shape.mesh(), place, body, BODY_GLOW);
        out.outline(k.shape.mesh(), &place, ink, ONE);
        out.glow(
            Mesh::Sphere,
            Affine::translate(at).then(&size(halo_d)),
            halo,
            HALO,
        );
    }
    for sp in &w.orbs.sparks {
        spark_shapes(sp, alpha, turned_halo, out);
    }
}

/// Six ways out from a spark, turned by its number so no two look alike.
const SHARDS: [(i32, i32, i32); 6] = [
    (100, 20, 0),
    (-60, 70, 30),
    (-40, -50, 75),
    (30, 80, -50),
    (10, -70, -70),
    (-90, 10, -40),
];

fn spark_shapes(sp: &Spark, alpha: Fx, turned_halo: Rgb, out: &mut Shapes) {
    let k = &spec().kinds[sp.kind as usize];
    let life = if sp.turn { TURN_SPARK } else { BURST } as i32;
    let t = sp.age as i32 * ONE + alpha;
    // 1 at the strike, 0 burnt out
    let left = (ONE - t / life).max(0);
    if left == 0 {
        return;
    }
    let colour = if sp.turn { turned_halo } else { rgb(k.halo) };
    let turn = Affine::rot_y(sp.id as i32 * 9_973).then(&Affine::rot_x(sp.id as i32 * 4_211));
    if sp.turn {
        // the ping: a flash, a ring that opens across the new flight, and
        // shards thrown out
        let flash = fx::mul(ONE * 5 / 10, fx::mul(left, left));
        out.glow(
            Mesh::Sphere,
            Affine::translate(sp.at).then(&size(ONE * 7 / 10)),
            colour,
            flash,
        );
        let open = ONE * 15 / 100 + fx::mul(ONE - left, ONE * 85 / 100);
        let ring = Affine::translate(sp.at)
            .then(&Affine::looking(sp.dir))
            .then(&Affine::scale(v3(open, open, ONE * 4 / 10)));
        out.glow(Mesh::Torus, ring, colour, fx::mul(ONE * 9 / 10, left));
    } else {
        out.glow(
            Mesh::Sphere,
            Affine::translate(sp.at).then(&size(ONE * 45 / 100)),
            colour,
            fx::mul(ONE * 6 / 10, left),
        );
    }
    let n = if sp.turn { 6 } else { 4 };
    let reach = if sp.turn { ONE * 7 / 10 } else { ONE * 4 / 10 };
    let gone = fx::mul(ONE - fx::mul(left, left), reach);
    for (x, y, z) in SHARDS.iter().take(n) {
        let way = turn
            .apply_dir(v3(x * ONE / 100, y * ONE / 100, z * ONE / 100))
            .norm();
        let at = sp.at.add(way.scale(gone));
        let d = fx::mul(ONE * 6 / 100, left);
        let place = Affine::translate(at).then(&Affine::looking(way)).then(&size(d));
        out.solid(Mesh::Tetra, place, colour, ONE);
    }
}

// ---- the door for settings and test staging ------------------------------

/// `act("magic.<name>", value)`: `leader` and `practice` throw that pattern
/// from the first foe standing (a leader is put 9 m ahead, held still,
/// when there is none), for tests, screenshots and the browser check;
/// `freeze` holds every orb still (`on`, `off`, or a whole number of ticks:
/// once the newest cast has run that long); `clear` takes every orb, cast
/// and spark away. For views: `lineup` hangs one orb of each kind in a row
/// 2 m before the player, frozen; `lend` lends the imbue for that many
/// ticks (`imbue::lend`); `stage_turn` sends one sphere from the first foe
/// at the drawn blade, 45 cm off it, and lends the imbue if it is not on.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    match name {
        "leader" | "practice" => {
            let pattern = if name == "leader" {
                Pattern::Leader
            } else {
                Pattern::Practice
            };
            let caster = match (1..player::count(w)).find(|&id| !player::fighter(w, id).health.down()) {
                Some(id) => id,
                None => {
                    player::spawn(w, crate::pose::Kind::Leader);
                    let id = player::count(w) - 1;
                    w.foes[id - 1].brain.control = crate::pilot::Control::Still;
                    player::act(w, "gap", "900")?;
                    id
                }
            };
            cast(w, caster, pattern);
            Ok(())
        }
        "freeze" => {
            match value.trim() {
                "on" | "1" | "true" | "" => w.orbs.frozen = true,
                "off" | "0" | "false" => {
                    w.orbs.frozen = false;
                    w.orbs.freeze_at = None;
                }
                other => {
                    let n: u32 = other.parse().map_err(|_| {
                        format!("Freeze takes on, off or a whole number of ticks, not \"{other}\".")
                    })?;
                    w.orbs.freeze_at = Some(n);
                }
            }
            Ok(())
        }
        "clear" => {
            w.orbs = State::default();
            Ok(())
        }
        "lineup" => {
            lineup(w);
            Ok(())
        }
        "lend" => {
            let n: u32 = value
                .trim()
                .parse()
                .map_err(|_| format!("A lend is a whole number of ticks, not \"{value}\"."))?;
            crate::imbue::lend(w, n);
            Ok(())
        }
        "stage_turn" => {
            if !w.me.arm.in_hand {
                return Err("Draw the sword first: a turn is staged at the drawn blade.".into());
            }
            let caster = (1..player::count(w)).find(|&id| !player::fighter(w, id).health.down());
            let e = edge(&w.me.arm);
            let mid = e.a1.lerp(e.b1, ONE * 6 / 10);
            let from = caster.map_or(mid.add(facing(w.aim.yaw, 0).scale(fx::int(9))), |id| hands(w, id));
            let dir = mid.sub(from).norm();
            let mut o = fire(
                &spec().patterns.leader[0],
                from,
                mid,
                1,
                caster.unwrap_or(0) as u8,
                &mut w.orbs.next,
            )
            .swap_remove(2);
            o.pos = mid.sub(dir.scale(ONE * 45 / 100));
            o.prev = o.pos;
            o.from = from;
            o.dir = dir;
            w.orbs.flying.push(o);
            if !crate::imbue::on(w) {
                crate::imbue::lend(w, crate::imbue::lasts());
            }
            Ok(())
        }
        _ => Err(format!("Unknown action \"magic.{name}\".")),
    }
}

/// One orb of each kind hung in a row 2 m before the player and 60 cm over
/// its chest, 55 cm apart, each tumbled a little differently, and the orbs
/// frozen: the lineup for a close view.
fn lineup(w: &mut World) {
    w.orbs = State::default();
    let ahead = facing(w.aim.yaw, 0);
    let across = right_of(w.aim.yaw);
    let centre = target(w).add(ahead.scale(fx::int(2))).add(v3(0, ONE * 6 / 10, 0));
    let n = spec().kinds.len() as i32;
    for k in 0..n {
        let at = centre.add(across.scale(ONE * 55 / 100 * (2 * k - (n - 1)) / 2));
        w.orbs.flying.push(Orb {
            id: k as u32,
            kind: k as u8,
            caster: 0,
            pos: at,
            prev: at,
            dir: ahead.neg(),
            speed: 0,
            curl: 0,
            hold: 0,
            held: 0,
            launch: Launch::Along,
            age: 12 + 7 * k as u16,
            from: at,
            turned: None,
        });
    }
    w.orbs.next = n as u32;
    w.orbs.frozen = true;
}

#[cfg(test)]
mod tests;
