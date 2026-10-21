//! Fighters (M1 Duel, task D): the player and every foe are the same thing,
//! a `Body` on its feet (`motion.rs`) carrying a `Fighter`: two simulated
//! arms holding a weapon (`arm.rs`), health (`health.rs`), a gait and a
//! pose for the models to dress (`pose::Pose`, drawn through
//! `models::figure` and `models::held`). Each is moved by its `Input`
//! record alone (CW-3): the player's from the page, a foe's from its pilot.
//!
//! Fighter 0 is the player, whose feet are `World::player` and whose arms
//! are `World::me`; fighter k is `World::foes[k − 1]`. `body`, `fighter`
//! and `parts_mut` reach any of them by number.
//!
//! **The sword's two states** (journal, "The controls"). Sheathed, the mouse
//! looks and Shift runs. X draws: the right hand reaches for the hilt at the
//! left hip (`DRAW_TICKS`), takes it, and the mouse then moves the hand's
//! target on the reach shell. Drawn, the body and the camera hold on the
//! nearest foe within `HOLD_RANGE` (Blade Symphony's lock), and holding the
//! right button lets the mouse look instead. X again sheathes.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use crate::arm::{self, Arm, Frame, Goals};
use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{facing, right_of, v3, yaw_of, Affine, V3};
use crate::health::Health;
use crate::input::{self, Input, Mouse};
use crate::look::{rgb, shade, Look};
use crate::motion::Gait;
use crate::pose::{self, Build, Held, Kind, Pose};
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::world::World;

pub use crate::motion::{RUN, WALK};

/// A shadow's height over the ground: above it by more than the depth
/// buffer needs at any distance a shadow is drawn from (M3's figures read it).
pub const SHADOW_LIFT: Fx = ONE * 2 / 100;
/// A drawn fighter holds on the nearest foe this near (journal).
pub const HOLD_RANGE: Fx = fx::int(15);
/// A foe holds on the player from this far: it was set on them.
pub const FOE_RANGE: Fx = fx::int(40);
/// A draw: the hand takes `REACH_TICKS` to reach the hilt at the left hip
/// while the scabbard tips its hilt toward the sword shoulder, then the
/// blade slides out along it over `PULL_TICKS`. Sheathing: the point is
/// brought to the scabbard's mouth over `ALIGN_TICKS`, then slides home
/// over `SLIDE_TICKS`. The scabbard swings back to hang on the hip by
/// `TILT_STEP` a tick.
pub const REACH_TICKS: u8 = 18;
pub const PULL_TICKS: u8 = 10;
pub const DRAW_TICKS: u8 = REACH_TICKS + PULL_TICKS;
pub const ALIGN_TICKS: u8 = 18;
pub const SLIDE_TICKS: u8 = 12;
pub const SHEATHE_TICKS: u8 = ALIGN_TICKS + SLIDE_TICKS;
pub const TILT_STEP: Fx = ONE / 10;
/// The scabbard's leather, the brass at its mouth and its tip, as M3's
/// colours name them, and how far the throat and the chape run, mm.
const SCABBARD: &str = "hilt_leather";
const SCABBARD_BRASS: &str = "brass";
const THROAT_MM: i32 = 34;
const CHAPE_MM: i32 = 60;
/// The scabbard narrows over this much at its tip, where the point sits.
const TIP_MM: i32 = 141;
/// The camera's turn toward the foe held on: this share of the way a tick,
/// at most `HOLD_TURN`.
pub const HOLD_SHARE: Fx = ONE / 5;
pub const HOLD_TURN: i32 = deg(6);
/// Held on a foe, the view looks this far below the line to their chest
/// (the slight downward pitch of the over-the-shoulder framing, R3.10).
pub const HOLD_PITCH_BIAS: i32 = -deg(7);
pub const HOLD_PITCH_MIN: i32 = -deg(24);
pub const HOLD_PITCH_MAX: i32 = deg(10);
/// A foe spawned for a test duel stands this far ahead of the player.
pub const SPAWN_AHEAD: Fx = fx::int(6);
/// The first foe of a test duel at the start moves the player this far
/// south first: M0's test field has its lantern, rocks and the fence round
/// the start, and the view on Play looks wherever the title's drift left it,
/// so the duel is moved, not turned (R3.43).
pub const DUEL_SOUTH: Fx = fx::int(30);
/// A body sliding faster than its feet leans back by up to this.
pub const SLIDE_LEAN: i32 = deg(14);
/// A staggered body leans back along the blow by up to this, and its arms
/// lose up to this share of their strength, both wearing off with it.
pub const STAGGER_LEAN: i32 = deg(16);
pub const STAGGER_WEAK_PCT: i32 = 50;
/// Where the scabbard holds the hilt's grip on the left hip, in the body's
/// frame (right, up, forward, centimetres on the player), which way its
/// blade points hanging (down 40° and back, close along the leg: the
/// short sword's tip 0.45 m up, above the back of the knee at 0.37), and
/// tipped for a draw (down,
/// left and back: the hilt toward the sword shoulder, so the pull ends
/// within the arm's reach, 0.43 m from the shoulder for the short sword).
const SHEATH_AT: (i32, i32, i32) = (-18, 96, 10);
const SHEATH_DIR: (i32, i32, i32) = (-6, -64, -77);
/// Where the hanger meets the belt, above the mouth.
const BELT_AT: (i32, i32, i32) = (-17, 104, 6);
const DRAW_DIR: (i32, i32, i32) = (-61, -66, -44);
const TICKS: i32 = crate::ticks::TICKS_PER_SECOND as i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Body {
    /// the feet, on the ground
    pub pos: V3,
    pub prev_pos: V3,
    /// metres a tick
    pub vel: V3,
    pub prev_vel: V3,
    /// the heading the body faces
    pub facing: i32,
    pub prev_facing: i32,
}

impl Body {
    pub fn new(pos: V3, facing: i32) -> Body {
        Body {
            pos,
            prev_pos: pos,
            vel: V3::ZERO,
            prev_vel: V3::ZERO,
            facing,
            prev_facing: facing,
        }
    }

    /// The feet, drawn `alpha` of the way from the last tick to this one.
    pub fn feet(&self, alpha: Fx) -> V3 {
        self.prev_pos.lerp(self.pos, alpha)
    }
}

/// Where the sword is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sheath {
    Sheathed,
    /// the hand on its way to the hilt, ticks so far
    Drawing(u8),
    Drawn,
    /// the sword on its way back, ticks so far
    Sheathing(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fighter {
    pub kind: Kind,
    /// 0 the player's side, 1 the foes'
    pub team: u8,
    pub held: Held,
    pub sheath: Sheath,
    pub arm: Arm,
    pub health: Health,
    pub gait: Gait,
    /// this tick's joints and last tick's, for drawing between them
    pub pose: Pose,
    pub prev_pose: Pose,
    /// where a foe looks (the player's look is `World::aim`)
    pub look_yaw: i32,
    pub look_pitch: i32,
    /// the fighter it holds on, by number
    pub engaged: Option<u8>,
    /// last tick's input, so a key acts once per press
    pub last: Input,
    /// how strong its arms are, percent of the player's
    pub servo_pct: i32,
    /// the mouse turns the view this tick (sheathed, or the right button
    /// held); otherwise it moves the hand
    pub mouse_looks: bool,
    /// rocks and knives carried, and the throw under way (`throwables.rs`)
    pub pouch: crate::throwables::Pouch,
    pub throw: crate::throwables::Throw,
    /// how far the scabbard is tipped for a draw (`ONE`: fully)
    pub tilt: Fx,
    /// how far the view has eased to its duel framing (`framing`), this
    /// tick and last
    pub framing: Fx,
    pub prev_framing: Fx,
}

impl Fighter {
    pub fn new(kind: Kind, held: Held, team: u8, feet: V3, yaw: i32, drawn: bool) -> Fighter {
        let build = pose::build(kind);
        let body = Body::new(feet, yaw);
        let frame = frame_of(&body, &build, &Gait::default(), 0, feet.y);
        let (pommel, tip) = Scabbard::of(&body, &build, &Gait::default(), 0, 0, &arm::weapon(held)).holds(0);
        let mut a = Arm::new(&frame, arm::weapon(held), &build, pommel, tip);
        if drawn {
            a.take_in_hand(&frame);
        }
        let mut f = Fighter {
            kind,
            team,
            held,
            sheath: if drawn { Sheath::Drawn } else { Sheath::Sheathed },
            arm: a,
            health: Health::new(100),
            gait: Gait::default(),
            pose: blank_pose(feet, yaw),
            prev_pose: blank_pose(feet, yaw),
            look_yaw: yaw,
            look_pitch: 0,
            engaged: None,
            last: Input::default(),
            servo_pct: 100,
            mouse_looks: !drawn,
            pouch: crate::throwables::Pouch::default(),
            throw: crate::throwables::Throw::default(),
            tilt: 0,
            framing: 0,
            prev_framing: 0,
        };
        f.pose = pose_from(&body, &f);
        f.prev_pose = f.pose;
        f
    }

    pub fn build(&self) -> Build {
        pose::build(self.kind)
    }

    /// The sword is out of its sheath (drawn, or on its way either way).
    pub fn drawn(&self) -> bool {
        self.sheath != Sheath::Sheathed
    }

    /// A key or button down this tick and not the last: one press.
    pub fn pressed(&self, input: &Input, key: u32) -> bool {
        input.key(key) && !self.last.key(key)
    }

    pub fn clicked(&self, input: &Input, button: u32) -> bool {
        input.button(button) && !self.last.button(button)
    }
}

/// How many fighters there are: the player and the foes.
pub fn count(w: &World) -> usize {
    1 + w.foes.len()
}

pub fn body(w: &World, id: usize) -> &Body {
    if id == 0 {
        &w.player
    } else {
        &w.foes[id - 1].body
    }
}

pub fn fighter(w: &World, id: usize) -> &Fighter {
    if id == 0 {
        &w.me
    } else {
        &w.foes[id - 1].fighter
    }
}

pub fn parts_mut(w: &mut World, id: usize) -> (&mut Body, &mut Fighter) {
    if id == 0 {
        (&mut w.player, &mut w.me)
    } else {
        let f = &mut w.foes[id - 1];
        (&mut f.body, &mut f.fighter)
    }
}

/// Every fighter's arm, by number, to change two at once.
pub fn arms_mut(w: &mut World) -> Vec<&mut Arm> {
    std::iter::once(&mut w.me.arm)
        .chain(w.foes.iter_mut().map(|f| &mut f.fighter.arm))
        .collect()
}

/// The heading a fighter's movement keys turn by: the camera's for the
/// player, where it looks for a foe.
pub fn look_yaw(w: &World, id: usize) -> i32 {
    if id == 0 {
        w.aim.yaw
    } else {
        fighter(w, id).look_yaw
    }
}

/// The feet of the fighter `id` holds on, if any.
pub fn engaged_at(w: &World, id: usize) -> Option<V3> {
    fighter(w, id).engaged.map(|k| body(w, k as usize).pos)
}

/// Whether `id` may hold on `other`: on the other side, standing, in range.
fn holdable(w: &World, id: usize, other: usize, range: Fx) -> bool {
    other != id
        && other < count(w)
        && fighter(w, other).team != fighter(w, id).team
        && !fighter(w, other).health.down()
        && body(w, other).pos.dist(body(w, id).pos) <= range
}

/// The fighters `id` could hold on, nearest first (ties by number).
fn candidates(w: &World, id: usize, range: Fx) -> Vec<usize> {
    let mut c: Vec<(Fx, usize)> = (0..count(w))
        .filter(|&k| holdable(w, id, k, range))
        .map(|k| (body(w, k).pos.dist(body(w, id).pos), k))
        .collect();
    c.sort();
    c.into_iter().map(|(_, k)| k).collect()
}

/// Step 1 of the tick (`World::step`): who each fighter holds on, whether
/// the player's mouse looks this tick, and the movement that turns the view.
pub fn look_mouse(w: &mut World, input: &Input) -> Mouse {
    for id in 0..count(w) {
        let range = if id == 0 { HOLD_RANGE } else { FOE_RANGE };
        let f = fighter(w, id);
        let next = if !f.drawn() || f.health.down() {
            None
        } else {
            let keep = f.engaged.filter(|&k| holdable(w, id, k as usize, range));
            let list = candidates(w, id, range);
            if id == 0 && f.clicked(input, input::LOCK_ON) && !list.is_empty() {
                // the middle button: the next foe in reach after the one held
                let at = keep.and_then(|k| list.iter().position(|&c| c == k as usize));
                Some(list[at.map(|a| (a + 1) % list.len()).unwrap_or(0)] as u8)
            } else {
                keep.or(list.first().map(|&k| k as u8))
            }
        };
        parts_mut(w, id).1.engaged = next;
    }
    let looks = !w.me.drawn() || input.button(input::HOLD_LOOK) || w.me.health.down();
    w.me.mouse_looks = looks;
    if looks {
        Mouse {
            dx: input.dx,
            dy: input.dy,
            wheel: input.wheel,
        }
    } else {
        Mouse::default()
    }
}

/// The camera's pivot for feet at `feet` and a heading (`camera::view_from`).
fn pivot(feet: V3, yaw: i32) -> V3 {
    feet.add(v3(0, crate::camera::HEAD + crate::camera::ABOVE, 0))
        .add(right_of(yaw).scale(crate::camera::RIGHT))
}

/// The heading and pitch from `from` to a fighter's chest.
fn toward_chest(w: &World, from: V3, k: usize) -> (i32, i32) {
    let b = body(w, k);
    let chest = b.pos.add(v3(0, fighter(w, k).build().shoulder_y - ONE / 6, 0));
    let d = chest.sub(from);
    // (atan2 answers in 0 to a turn: a look down is wrapped to negative)
    (yaw_of(d), fx::wrap(fx::atan2(d.y, d.len_xz())))
}

/// Drawn and held on a foe, the view's eye moves this much further right
/// (R3.54), still over the right shoulder (CW-15): a foe's blade pointed
/// at the player then crosses the view beside the player's head rather than
/// over it. (Raising the eye too was tried: it brought the blade nearer the
/// head on screen.) It eases in and out by `FRAMING_STEP` a tick.
pub const FRAMING_RIGHT: Fx = ONE * 40 / 100;
pub const FRAMING_STEP: Fx = ONE / 12;

/// The eye's move for the duel framing, `alpha` of the way from the last
/// tick to this, for a view facing `yaw`.
pub fn framing(w: &World, alpha: Fx, yaw: i32) -> V3 {
    let f = fx::lerp(w.me.prev_framing, w.me.framing, alpha);
    right_of(yaw).scale(fx::mul(FRAMING_RIGHT, f))
}

/// Still step 1: held on a foe, the view eases round to frame them unless
/// the right button looks; every foe looks straight at whom it holds on.
pub fn hold_on(w: &mut World) {
    let goal = if w.me.engaged.is_some() && w.me.sheath == Sheath::Drawn && !w.me.mouse_looks {
        ONE
    } else {
        0
    };
    w.me.prev_framing = w.me.framing;
    w.me.framing = fx::approach(w.me.framing, goal, FRAMING_STEP);
    if let (Some(k), false) = (w.me.engaged, w.me.mouse_looks) {
        let from = pivot(w.player.pos, w.aim.yaw);
        let (yaw, pitch) = toward_chest(w, from, k as usize);
        let pitch = (pitch + HOLD_PITCH_BIAS).clamp(HOLD_PITCH_MIN, HOLD_PITCH_MAX);
        let turn = |now: i32, goal: i32| {
            let gap = fx::wrap(goal - now);
            let step = fx::mul(gap, HOLD_SHARE);
            let step = if step == 0 { gap.signum() } else { step };
            fx::wrap(now + step.clamp(-HOLD_TURN, HOLD_TURN))
        };
        w.aim.yaw = turn(w.aim.yaw, yaw);
        w.aim.pitch = turn(w.aim.pitch, pitch);
    }
    for id in 1..count(w) {
        let f = fighter(w, id);
        let look = f.engaged.map(|k| {
            let b = body(w, id);
            let eyes = b.pos.add(v3(0, f.build().shoulder_y, 0));
            toward_chest(w, eyes, k as usize)
        });
        if let Some((yaw, pitch)) = look {
            let f = parts_mut(w, id).1;
            f.look_yaw = yaw;
            f.look_pitch = pitch;
        }
    }
}

/// The shoulders and chest for a body this tick, `fall` of the way to lying
/// on its back (`ONE`: down), over ground at `ground`.
pub fn frame_of(b: &Body, build: &Build, gait: &Gait, fall: Fx, ground: Fx) -> Frame {
    let place = leaning(b, build, gait, fall);
    let shoulder = [-1, 1].map(|s| place(v3(s * build.shoulder_x, build.shoulder_y, 0)));
    let chest_r = (ONE as i64 * 15 / 100 * build.girth_pct as i64 / 100) as Fx;
    Frame {
        shoulder,
        spine: [
            place(v3(0, build.hip_y + ONE / 10, 0)),
            place(v3(0, build.shoulder_y - ONE / 12, 0)),
        ],
        chest_r,
        head: place(v3(0, build.height - build.head / 2, 0)),
        head_r: build.head * 4 / 10,
        ground,
        yaw: b.facing,
        centre: b.pos,
    }
}

/// A point in the body's frame (right, up, forward) in the world, the body
/// tipped `fall` of a quarter turn onto its back about its heels, and
/// raised as it goes so it comes to lie on its back and the back of its
/// head, not in the ground: the trunk by the belly's depth (M3's 122 mm),
/// the head by its own radius, the legs less toward the heels.
fn local(b: &Body, build: &Build, fall: Fx) -> impl Fn(V3) -> V3 {
    let turn = Affine::rot_y(b.facing).then(&Affine::rot_x(fx::mul(fall, deg(90))));
    let feet = b.pos;
    let mm = |v: i32| (v as i64 * build.height as i64 / 1700) as Fx;
    // (M7: a fall below 0 lies on the front, raised the same)
    let back = fx::mul(mm(LIE_BACK_MM) * build.girth_pct / 100, fall.abs());
    let head = fx::mul(mm(crate::blade::HEAD_MM), fall.abs());
    let (hips, neck, crown) = (build.hip_y, build.shoulder_y, build.height - build.head / 2);
    move |p: V3| {
        let lift = if p.y <= hips {
            fx::mul(back, fx::div(p.y.max(0), hips.max(1)))
        } else if p.y <= neck {
            back
        } else {
            fx::lerp(
                back,
                head,
                fx::div((p.y - neck).min(crown - neck), (crown - neck).max(1)),
            )
        };
        feet.add(turn.apply_dir(v3(p.x, p.y, -p.z))).add(v3(0, lift, 0))
    }
}

/// How high a body lying on its back holds its trunk off the ground, mm on
/// the player: M3's belly is 122 mm deep.
const LIE_BACK_MM: i32 = 122;

/// The same, leaning back against a slide (`Gait::slide`): up to `SLIDE_LEAN`
/// at 5 m/s of slide, the head away from the way the body slides; and
/// staggered (`Gait::stagger`), the head knocked back along the blow by up
/// to `STAGGER_LEAN`, easing off as the stagger wears off.
fn leaning(b: &Body, build: &Build, g: &Gait, fall: Fx) -> impl Fn(V3) -> V3 {
    // M7 (P4): bent at the hips to take up a weapon (`Gait::stoop`), and
    // lifted off the ground in a hop (`Gait::lift`)
    let unbent = local(b, build, fall);
    let bend = crate::wounds::bend(build.hip_y, g.stoop);
    let hop = v3(0, g.lift, 0);
    let base = move |p: V3| unbent(bend(p)).add(hop);
    let by = (SLIDE_LEAN as i64 * g.slide.min(fx::int(5)) as i64 / fx::int(5) as i64) as i32;
    let tip = |way: i32, by: i32| {
        Affine::rot_y(way)
            .then(&Affine::rot_x(by))
            .then(&Affine::rot_y(-way))
    };
    let shaken = STAGGER_LEAN * g.stagger as i32 / crate::feel::STAGGER_TICKS as i32;
    let tilt = tip(crate::geom::yaw_of(b.vel), by).then(&tip(crate::geom::yaw_of(g.stagger_way), -shaken));
    // M7 (R9.177): a thrust leans the body in after its point
    let tilt = tilt.then(&tip(b.facing, -fx::mul(crate::swing::LUNGE_LEAN, g.lunge)));
    let feet = b.pos;
    move |p: V3| feet.add(tilt.apply_dir(base(p).sub(feet)))
}

/// Where a fighter's scabbard is this tick: the grip of a sheathed hilt,
/// the way its blade points, and the weapon it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scabbard {
    pub grip: V3,
    pub dir: V3,
    pub wpn: arm::Weapon,
    /// where its hanger meets the belt
    pub belt: V3,
}

impl Scabbard {
    /// On a body's left hip, standing, sliding or fallen (`fall`), tipped
    /// `tilt` of the way toward a draw.
    pub fn of(b: &Body, build: &Build, gait: &Gait, fall: Fx, tilt: Fx, wpn: &arm::Weapon) -> Scabbard {
        let place = leaning(b, build, gait, fall);
        let scale = |c: i32| c * ONE / 100;
        let tall =
            |c: i32| (scale(c) as i64 * build.height as i64 / pose::build(Kind::Player).height as i64) as Fx;
        let grip = place(v3(
            scale(SHEATH_AT.0) * build.girth_pct / 100,
            tall(SHEATH_AT.1),
            scale(SHEATH_AT.2),
        ));
        let way = |d: (i32, i32, i32)| v3(scale(d.0), scale(d.1), scale(d.2)).norm();
        let local = way(SHEATH_DIR).lerp(way(DRAW_DIR), tilt);
        let dir = place(local).sub(place(V3::ZERO)).norm();
        let belt = place(v3(
            scale(BELT_AT.0) * build.girth_pct / 100,
            tall(BELT_AT.1),
            scale(BELT_AT.2),
        ));
        Scabbard {
            grip,
            dir,
            wpn: *wpn,
            belt,
        }
    }

    /// How far the blade slides before its point clears the mouth: from the
    /// grip to the tip, less the hilt and guard above the mouth.
    pub fn pull(&self) -> Fx {
        (self.wpn.len - self.wpn.grip[1]) - mouth(self.wpn.item)
    }

    /// The pommel and the tip of the weapon slid `s` out of the scabbard.
    pub fn holds(&self, s: Fx) -> (V3, V3) {
        let grip = self.grip.sub(self.dir.scale(s));
        let pommel = grip.sub(self.dir.scale(self.wpn.grip[1]));
        (pommel, pommel.add(self.dir.scale(self.wpn.len)))
    }

    /// The scabbard's mouth: where the guard sits when the sword is home.
    pub fn throat(&self) -> V3 {
        self.grip.add(self.dir.scale(mouth(self.wpn.item)))
    }
}

/// From the grip to the guard's far side (M3's hilt and guard, mm): where a
/// scabbard's mouth sits.
fn mouth(item: Held) -> Fx {
    match item {
        Held::Sword => ONE * 138 / 1000,
        Held::Cleaver => ONE * 150 / 1000,
        _ => ONE * 106 / 1000,
    }
}

/// `x` from 0 to `ONE`, eased in and out (3x² − 2x³).
fn ease(x: Fx) -> Fx {
    let x = x.clamp(0, ONE);
    fx::mul(fx::mul(x, x), 3 * ONE - 2 * x)
}

fn blank_pose(feet: V3, yaw: i32) -> Pose {
    Pose {
        pelvis: feet,
        chest: feet,
        head: feet,
        facing: yaw,
        shoulder: [feet; 2],
        elbow: [feet; 2],
        hand: [feet; 2],
        hip: [feet; 2],
        knee: [feet; 2],
        foot: [feet; 2],
        grip: feet,
        tip: feet,
    }
}

/// The joints of a fighter this tick: the trunk and legs from the body and
/// its gait, the arms and weapon from the simulated points.
pub fn pose_from(b: &Body, f: &Fighter) -> Pose {
    let build = f.build();
    let fall = f.health.fall();
    let place = leaning(b, &build, &f.gait, fall);
    let bob = fx::mul(
        fx::cos(f.gait.phase.wrapping_mul(2)).abs(),
        fx::mul(ONE / 40, f.gait.swing.min(ONE)),
    );
    let pelvis = place(v3(0, build.hip_y - bob, 0));
    let hip = [-1, 1].map(|s| place(v3(s * build.hip_x, build.hip_y - bob, 0)));
    let flat = crate::motion::feet(b, &f.gait, &build, b.pos);
    // a fallen body's feet stay where they were; standing, the gait's
    let foot = if fall != 0 {
        [-1, 1].map(|s| place(v3(s * build.hip_x, ONE / 20, 0)))
    } else {
        flat
    };
    let fwd = facing(b.facing, 0);
    let knee = [0, 1].map(|s| crate::motion::knee(hip[s], foot[s], build.thigh, build.shin, fwd));
    let a = &f.arm;
    let mut pose = Pose {
        pelvis,
        chest: place(v3(0, build.shoulder_y - ONE / 10, 0)),
        head: place(v3(0, build.height - build.head / 2, 0)),
        facing: b.facing,
        shoulder: a.frame.shoulder,
        elbow: [a.pts[arm::ELBOW[0]].p, a.pts[arm::ELBOW[1]].p],
        hand: [a.pts[arm::HAND[0]].p, a.pts[arm::HAND[1]].p],
        hip,
        knee,
        foot,
        grip: a.grip_point(),
        tip: a.pts[arm::TIP].p,
    };
    // M7 (P4): the wounds' changes to the joints (lost parts folded away)
    crate::wounds::shape_pose(b, f, &mut pose);
    pose
}

/// The joints of fighter `id` this tick.
pub fn pose_of(w: &World, id: usize) -> Pose {
    pose_from(body(w, id), fighter(w, id))
}

/// What each fighter's arms aim at this tick, from its sword's state, its
/// shell, its throw and its health: the sword's next state, how far the
/// scabbard is tipped, and the arms' goals.
pub fn goals(w: &World, id: usize, input: &Input) -> (Sheath, Fx, Goals) {
    let b = body(w, id);
    let f = fighter(w, id);
    let build = f.build();
    let place = local(b, &build, 0);
    let wpn = f.arm.weapon;
    let reach = build.upper_arm + build.forearm;
    // the sword's state: X toggles, the hand moves, the sword changes hands
    let down = f.health.down();
    let press = !down && f.pressed(input, input::DRAW);
    let sheath = match f.sheath {
        Sheath::Sheathed if press => Sheath::Drawing(0),
        Sheath::Drawing(t) if t + 1 >= DRAW_TICKS => Sheath::Drawn,
        Sheath::Drawing(t) => Sheath::Drawing(t + 1),
        Sheath::Drawn if press => Sheath::Sheathing(0),
        Sheath::Sheathing(t) if t + 1 >= SHEATHE_TICKS => Sheath::Sheathed,
        Sheath::Sheathing(t) => Sheath::Sheathing(t + 1),
        s => s,
    };
    // the scabbard tips for a draw or a sheathing and swings back after
    let tilt = match sheath {
        Sheath::Drawing(t) => ease(((t as i32 + 1) * ONE / REACH_TICKS as i32).min(ONE)).max(f.tilt),
        Sheath::Sheathing(t) => ease(((t as i32 + 1) * ONE / ALIGN_TICKS as i32).min(ONE)).max(f.tilt),
        _ => (f.tilt - TILT_STEP).max(0),
    };
    let sc = Scabbard::of(b, &build, &f.gait, f.health.fall(), tilt, &wpn);
    let (pommel, tip) = sc.holds(0);
    let rest = |s: usize| {
        place(v3(
            (2 * s as i32 - 1) * (build.shoulder_x + ONE / 25),
            build.shoulder_y - reach * 92 / 100,
            ONE / 12,
        ))
    };
    let free = |s: usize| {
        place(v3(
            (2 * s as i32 - 1) * build.shoulder_x,
            build.shoulder_y - reach * 70 / 100,
            reach * 35 / 100,
        ))
    };
    let shell_hand = f.arm.shell.point(f.arm.frame.shoulder[1], b.facing);
    let blade = arm::blade_dir(b.facing, &f.arm.shell, f.arm.reach());
    // M6: imbued, the player's guard holds the blade across the line orbs come down
    let blade = crate::imbue::guard_line(w, id, b.facing, &f.arm.shell, blade);
    // M7 (T10): a cut under way aims the hand along its arc and the blade
    // along its plane (`swing.rs`)
    let cut = f
        .arm
        .swing
        .cut
        .filter(|_| f.sheath == Sheath::Drawn && !f.health.down());
    let (shell_hand, blade) = match cut {
        Some(c) => crate::swing::targets(
            &c,
            b.facing,
            f.arm.frame.shoulder[1],
            f.arm.reach(),
            f.arm.grip_point(),
        ),
        None => (shell_hand, blade),
    };
    let two = wpn.two_handed();
    let winding = f.throw.winding();
    let shaken = 100 - STAGGER_WEAK_PCT * f.gait.stagger as i32 / crate::feel::STAGGER_TICKS as i32;
    // M7 (T10): a cut's wind-up gathers at part strength (`swing::strength_pct`)
    let gather = f
        .arm
        .swing
        .cut
        .map(|c| crate::swing::strength_pct(&c))
        .unwrap_or(100);
    let full = [0, 1].map(|s| f.servo_pct * f.health.strength(s) / 100 * shaken / 100 * gather / 100);
    let mut g = Goals {
        hand: [Some(rest(0)), Some(rest(1))],
        tip: None,
        strength: full,
        swing: cut.is_some(),
        in_hand: false,
        hold: [false, false],
        sheath: (pommel, tip),
        ride: None,
        grasp: false,
        target: None,
        // M7 (T10): a cut's strike turns the weapon about its pivot
        turn: cut.and_then(|c| {
            crate::swing::turn(
                &c,
                b.facing,
                f.arm.frame.shoulder[1],
                f.arm.reach(),
                f.arm.grip_point(),
            )
        }),
    };
    // the free hand steadies the scabbard at its mouth while the sword
    // goes in or out
    let steady = sc.throat().add(right_of(b.facing).scale(-ONE / 25));
    match sheath {
        Sheath::Sheathed => {}
        Sheath::Drawing(t) if t < REACH_TICKS => {
            // the sword hand reaches across for the hilt at full strength,
            // round in front of the belly first rather than through it
            let round = rest(1)
                .lerp(sc.grip, ONE / 2)
                .add(facing(b.facing, 0).scale(ONE / 4));
            let to = if t < REACH_TICKS / 2 { round } else { sc.grip };
            g.hand = [Some(steady), Some(to)];
            g.grasp = true;
        }
        Sheath::Drawing(t) => {
            // the blade slides out along the tipped scabbard, the hand on it
            let u = ((t - REACH_TICKS) as i32 + 1) * ONE / PULL_TICKS as i32;
            let (p, q) = sc.holds(fx::mul(sc.pull(), ease(u)));
            g.in_hand = true;
            g.hold = [false, true];
            g.hand = [Some(steady), Some(sc.grip)];
            g.ride = Some((p, q, ONE));
        }
        Sheath::Drawn => {
            g.in_hand = true;
            // the second hand takes the hilt once it has come to it
            let second = shell_hand.sub(blade.scale(wpn.grip[1] - wpn.grip[0]));
            let near = f.arm.pts[arm::HAND[0]].p.dist(f.arm.hilt_at(wpn.grip[0])) < ONE / 20;
            // (a level cut swings wider than the free arm reaches: it lets
            // go of the hilt and takes it again after, T10)
            let both = two && !winding && !cut.is_some_and(|c| crate::swing::wide(&c));
            g.hold = [both && (f.arm.hold[0] || near), true];
            g.hand[1] = Some(shell_hand);
            g.target = Some(shell_hand);
            g.hand[0] = Some(if both { second } else { free(0) });
            // (a cut turns its blade within its own plane, `swing::toward`)
            let line = match cut {
                Some(_) => crate::swing::toward(&f.arm, blade),
                None => through_front(&f.arm, blade, b.facing),
            };
            g.tip = Some(shell_hand.add(line.scale(wpn.len - wpn.grip[1])));
        }
        Sheath::Sheathing(t) if t < ALIGN_TICKS => {
            // the point brought to the mouth, the blade along the scabbard,
            // turned with the quickness of a swing
            let (p, q) = sc.holds(sc.pull());
            let grip = p.add(sc.dir.scale(wpn.grip[1]));
            g.in_hand = true;
            g.hold = [false, true];
            g.swing = true;
            g.hand = [Some(steady), Some(grip)];
            g.tip = Some(q);
        }
        Sheath::Sheathing(t) => {
            // and slid home, eased over from where the arm brought it
            let k = (t - ALIGN_TICKS) as i32 + 1;
            let u = k * ONE / SLIDE_TICKS as i32;
            let (p, q) = sc.holds(fx::mul(sc.pull(), ONE - ease(u)));
            g.in_hand = true;
            g.hold = [false, true];
            g.hand = [Some(steady), Some(sc.grip)];
            g.ride = Some((p, q, (k * ONE / 4).min(ONE)));
        }
    }
    if winding {
        // the free hand draws back over the shoulder, then throws
        g.hand[0] = Some(place(v3(
            -build.shoulder_x,
            build.shoulder_y + ONE / 8,
            -reach * 55 / 100,
        )));
    }
    if down {
        // the arms go limp and a weapon in hand falls from them
        g.hand = [None, None];
        g.tip = None;
        g.target = None;
        g.ride = None;
        g.hold = [false, false];
    }
    // M7 (P4): a lost arm, a dropped weapon, taking it up, a crawl
    crate::wounds::goals(w, id, (sheath, tilt, g))
}

/// The line the blade's servo turns it toward this tick: the line asked
/// for, or, while the blade points more than a quarter turn away from it,
/// a line between the two and ahead of the body. A cut from one side to
/// the other, or from up behind the head down to the front, so passes
/// through the space in front, as a cut does, rather than in whatever
/// plane a straight pull across happens to find.
pub fn through_front(a: &Arm, want: V3, yaw: i32) -> V3 {
    let now = a.pts[arm::TIP].p.sub(a.grip_point()).norm();
    if now.dot(want) >= 0 {
        return want;
    }
    let mid = now.add(want).add(facing(yaw, 0));
    if mid == V3::ZERO {
        want
    } else {
        mid.norm()
    }
}

/// The end of the tick: last tick's pose kept for drawing between, this
/// tick's built, and each fighter's input kept for the next press.
pub fn finish(w: &mut World, input: &Input, foes: &[Input]) {
    for id in 0..count(w) {
        let now = pose_of(w, id);
        let f = parts_mut(w, id).1;
        f.prev_pose = f.pose;
        f.pose = now;
        f.last = if id == 0 { *input } else { foes[id - 1] };
    }
}

// ---- drawing --------------------------------------------------------------

/// Every fighter, in number order: its figure and weapon (the models'
/// doors), its shadow, and for the player the hand's target while drawn.
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    for id in 0..count(w) {
        let f = fighter(w, id);
        let p = f.prev_pose.lerp(&f.pose, alpha);
        // the player wears the speedster pieces M4's inventory says (or,
        // with the test setting, the first n); the figure draws its shadow
        let worn = match (id, w.motion.forced_pieces) {
            (0, Some(n)) => (1u8 << n.min(crate::motion::PIECES)) - 1,
            (0, None) => crate::items::speedster_mask(w),
            _ => 0,
        };
        let dress = crate::models::Dress { worn, variant: 0 };
        // M7 (P4): through wounds, which draw what is still on the body
        crate::wounds::figure(w, id, &p, dress, look, out);
        if f.held != Held::Nothing && !w.sticks {
            crate::models::held(f.held, p.grip, p.tip, look, out);
        }
        if matches!(f.held, Held::ShortSword | Held::Sword) && !w.sticks {
            let b = body(w, id);
            let at = Body {
                pos: b.feet(alpha),
                facing: fx::lerp_angle(b.prev_facing, b.facing, alpha),
                ..*b
            };
            let sc = Scabbard::of(&at, &f.build(), &f.gait, f.health.fall(), f.tilt, &f.arm.weapon);
            scabbard(&sc, out, look);
        }
    }
    if w.me.sheath == Sheath::Drawn && !w.me.health.down() {
        let a = &w.me.arm;
        let at = a.prev_target.lerp(a.target, alpha);
        let r = ONE * 35 / 1000;
        out.glow(
            Mesh::Sphere,
            Affine::translate(at).then(&Affine::scale(v3(r, r, r))),
            rgb(look.palette.lamp),
            ONE / 2,
        );
    }
    // M7 (T10): the cut the mouse would pick, as a line of beads
    crate::swing::draw(w, look, out);
    crate::swing::draw_trails(w, look, out); // and every blade's trail (R9.175)
    if w.sticks {
        sticks(w, alpha, look, out);
    }
}

/// A scabbard drawn on the hip: a leather sheath from the mouth to past
/// where the sheathed point sits, wider and thicker than M3's blade by more
/// than its outline (so a sheathed blade is hidden inside and no face lies
/// on another, CW-18), set across the same way as the blade (`models::along`
/// with the world's up), brass at its mouth and its tip.
/// A scabbard's width and thickness at its mouth, mm: M3's blade (46 by 9
/// mm for the short sword, 54 by 10 for the sword) and its 4 mm outline,
/// with room.
pub fn scabbard_size(item: Held) -> (i32, i32) {
    match item {
        Held::Sword => (74, 32),
        _ => (66, 30),
    }
}

fn scabbard(sc: &Scabbard, out: &mut Shapes, look: &Look) {
    let t = crate::models::table();
    let (wide, thick) = scabbard_size(sc.wpn.item);
    let mm = crate::models::mm;
    let along = |d: Fx| sc.grip.add(sc.dir.scale(d));
    // from the mouth (the guard's far side) to past where the point sits:
    // the scabbard is as long as the blade it carries
    let (mouth, end) = (mouth(sc.wpn.item), sc.wpn.len - sc.wpn.grip[1] + mm(25));
    let point = end - mm(TIP_MM);
    let mut m = crate::models::Model::default();
    let ink = t.ink_item;
    let bar = |a: Fx, b: Fx, w: i32, k: i32| crate::models::bar(along(a), along(b), (mm(w), mm(k)), V3::UP);
    // the straight body, then the last part narrowing to 60% over the point
    m.add(
        Mesh::Cube,
        bar(mouth, point + mm(4), wide, thick),
        t.c(SCABBARD),
        ink,
    );
    m.add(Mesh::Taper, bar(point, end, wide, thick), t.c(SCABBARD), ink);
    // brass at the mouth and a chape over the tip
    m.add(
        Mesh::Cube,
        bar(mouth - mm(4), mouth + mm(THROAT_MM), wide + 10, thick + 10),
        t.c(SCABBARD_BRASS),
        ink,
    );
    let chape_w = |w: i32| w * (100 - 40 * (TIP_MM - CHAPE_MM) / TIP_MM) / 100 + 8;
    m.add(
        Mesh::Taper,
        bar(end - mm(CHAPE_MM), end + mm(6), chape_w(wide), chape_w(thick)),
        t.c(SCABBARD_BRASS),
        ink,
    );
    // and a leather hanger from the throat up to the belt
    let belt = sc.belt;
    let hang = crate::models::bar(belt, along(mouth + mm(THROAT_MM / 2)), (mm(26), mm(10)), V3::UP);
    m.add(Mesh::Cube, hang, t.c(SCABBARD), ink);
    m.emit(look.ink(), out);
}

/// The physics view (`act("duel.sticks", "on")`, `?sticks=1`): every arm's
/// sticks and every weapon as the solver has them, for tuning and for
/// screenshots while the models' `held` door is still a placeholder.
fn sticks(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    let ink = look.ink();
    for id in 0..count(w) {
        let f = fighter(w, id);
        let p = f.prev_pose.lerp(&f.pose, alpha);
        let arm_rgb = rgb(look.palette.post);
        for s in 0..2 {
            bar(out, p.shoulder[s], p.elbow[s], ONE * 45 / 1000, arm_rgb, ink);
            bar(out, p.elbow[s], p.hand[s], ONE * 40 / 1000, arm_rgb, ink);
        }
        let blade = if id == 0 {
            look.palette.visor
        } else {
            look.palette.rock[2]
        };
        let wpn = f.arm.weapon;
        let dir = p.tip.sub(p.grip).norm();
        let pommel = p.grip.sub(dir.scale(wpn.grip[1]));
        let guard = pommel.add(dir.scale(wpn.hilt));
        bar(out, pommel, guard, ONE * 3 / 100, rgb(look.palette.trunk), ink);
        let width = if wpn.item == Held::Cleaver {
            ONE * 9 / 100
        } else {
            ONE * 4 / 100
        };
        bar(out, guard, p.tip, width, shade(rgb(blade), 110), ink);
    }
    crate::throwables::draw_sticks(w, alpha, look, out);
}

/// A thin solid from `a` to `b`, `width` across, inked.
pub fn bar(out: &mut Shapes, a: V3, b: V3, width: Fx, colour: crate::look::Rgb, ink: crate::look::Rgb) {
    let d = b.sub(a);
    let len = d.len();
    if len < ONE / 200 {
        return;
    }
    let basis = Affine::looking(d.norm());
    let at = Affine::translate(a.add(d.scale(ONE / 2)))
        .then(&basis)
        .then(&Affine::scale(v3(width, width, len)));
    out.inked(Mesh::Cube, at, colour, ink);
}

// ---- the screen and the door ---------------------------------------------

/// What the duel's lines over the game show, in one value: the page reads
/// the screen again when it changes (`game.rs`).
pub fn hud_stamp(w: &World) -> (i32, Option<(u8, i32)>, Option<&'static str>) {
    let foe = w.me.engaged.filter(|&k| (k as usize) < count(w));
    (
        w.me.health.hp,
        foe.map(|k| (k, fighter(w, k as usize).health.hp)),
        // M7 (T10): the swing cue's word
        crate::swing::cue(w).map(|c| crate::swing::cue_key(c.0)),
    )
}

/// `act("duel.<name>", value)`: `spawn` puts an `orc`, `bandit` or `leader`
/// 6 m ahead of the player; `reset` clears the duel; `sticks` shows the
/// physics view (`on`, `off`); `pilot` runs every foe by its tree (`tree`)
/// or holds it still (`still`), `gap` stands every foe that many
/// centimetres ahead of the player, and `health` sets every foe's health
/// (0 puts them down), all for tests and screenshots.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    match name {
        "spawn" => {
            let kind = crate::pilot::kind_named(value.trim()).ok_or_else(|| {
                format!("A test duel is with an orc, a bandit or a leader, not \"{value}\".")
            })?;
            spawn(w, kind);
            Ok(())
        }
        "reset" => {
            reset(w);
            Ok(())
        }
        "sticks" => {
            w.sticks = matches!(value.trim(), "on" | "1" | "true");
            Ok(())
        }
        // every foe moved to this many centimetres straight ahead of the
        // player, facing them: staging for a test or a picture
        "gap" => {
            let cm: i32 = value
                .trim()
                .parse()
                .ok()
                .filter(|c| (40..=4000).contains(c))
                .ok_or_else(|| {
                    format!("A gap is a whole number of centimetres from 40 to 4000, not \"{value}\".")
                })?;
            let ahead = facing(w.aim.yaw, 0);
            let feet = w.player.pos;
            for k in 0..w.foes.len() {
                let mut at = feet.add(ahead.scale(cm * ONE / 100));
                at.y = w.ground(at.x, at.z);
                let yaw = yaw_of(feet.sub(at));
                let f = &mut w.foes[k];
                f.body = Body::new(at, yaw);
                // the arms come too, carried as the body would carry them
                let frame = frame_of(&f.body, &f.fighter.build(), &f.fighter.gait, 0, at.y);
                f.fighter.arm.carry(&frame);
                f.fighter.arm.frame = frame;
            }
            Ok(())
        }
        // every foe's hand target on its shell, degrees right and up
        // ("-40,-10"): staging a guard (a still foe holds it) for a test or a
        // picture
        "guard" => {
            let parsed: Option<Vec<i32>> = value.split(',').map(|v| v.trim().parse().ok()).collect();
            let [side, pitch] = parsed.as_deref().unwrap_or(&[]) else {
                return Err(format!(
                    "A guard is degrees right and up, as \"-40,-10\", not \"{value}\"."
                ));
            };
            for f in &mut w.foes {
                let a = &mut f.fighter.arm;
                a.shell.side = deg(*side).clamp(arm::SIDE_MIN, arm::SIDE_MAX);
                a.shell.pitch = deg(*pitch).clamp(arm::PITCH_MIN, arm::PITCH_MAX);
            }
            Ok(())
        }
        // every foe's health set, 0 to put them down: staging a fall for a
        // test or a picture
        "health" => {
            let hp: i32 = value
                .trim()
                .parse()
                .ok()
                .filter(|h| (0..=1000).contains(h))
                .ok_or_else(|| {
                    format!("A foe's health is a whole number from 0 to 1000, not \"{value}\".")
                })?;
            for f in &mut w.foes {
                f.fighter.health.hp = hp.min(f.fighter.health.most);
            }
            Ok(())
        }
        // every foe's pilot on (`tree`) or off (`still`): a test, a picture
        "pilot" => {
            let control = match value.trim() {
                "tree" => crate::pilot::Control::Tree,
                "still" => crate::pilot::Control::Still,
                other => {
                    return Err(format!(
                        "A foe's pilot is \"tree\" or \"still\", not \"{other}\"."
                    ))
                }
            };
            w.pilots.control = control.clone();
            for f in &mut w.foes {
                f.brain.control = control.clone();
            }
            Ok(())
        }
        // M7 (T10): every foe makes one cut through its record now (`level`,
        // `backhand`, `chop`, `rising`, `thrust`), then stands still: staging
        // for a picture
        "cut" => {
            let way = match value.trim() {
                "level" => (-6_000, 0, 0),
                "backhand" => (6_000, 0, 0),
                "chop" => (0, 6_000, 0),
                "rising" => (0, -6_000, 0),
                "thrust" => (0, 0, -100),
                _ => {
                    return Err(format!(
                        "A cut is level, backhand, chop, rising or thrust, not \"{value}\"."
                    ))
                }
            };
            for f in &mut w.foes {
                let mut list = vec![Input::default(); f.brain.ticks as usize];
                list.push(Input {
                    buttons: input::SWING,
                    dx: way.0,
                    dy: way.1,
                    wheel: way.2,
                    ..Input::default()
                });
                list.extend(std::iter::repeat_n(
                    Input {
                        buttons: input::SWING,
                        ..Input::default()
                    },
                    40,
                ));
                f.brain.control = crate::pilot::Control::Script(list);
            }
            Ok(())
        }
        _ => Err(format!("Unknown action \"duel.{name}\".")),
    }
}

/// A foe of `kind` ahead of the player, facing them, sword drawn. A second
/// and later foe stands to one side or the other of the first.
pub fn spawn(w: &mut World, kind: Kind) {
    if w.foes.is_empty() && w.player.pos == crate::world::START {
        let mut at = duel_spot();
        at.y = w.ground(at.x, at.z);
        w.player.pos = at;
        w.player.prev_pos = at;
        let frame = frame_of(&w.player, &w.me.build(), &w.me.gait, 0, w.ground(at.x, at.z));
        w.me.arm.carry(&frame);
        w.me.arm.frame = frame;
        w.me.pose = pose_from(&w.player, &w.me);
        w.me.prev_pose = w.me.pose;
    }
    let k = w.foes.len() as i32;
    let side = if k % 2 == 0 { 1 } else { -1 } * ((k + 1) / 2) * deg(28);
    let way = fx::wrap(w.aim.yaw + side);
    let mut at = w.player.pos.add(facing(way, 0).scale(SPAWN_AHEAD));
    at.y = w.ground(at.x, at.z);
    let yaw = yaw_of(w.player.pos.sub(at));
    let salt = w.rng.next_u64();
    let mut foe = crate::pilot::foe(&w.pilots.book, kind, at, yaw, salt);
    foe.brain.control = w.pilots.control.clone();
    w.foes.push(foe);
}

/// Where a test duel is staged: 30 m south of the start (`DUEL_SOUTH`), on
/// M2's level ground in the village, 17 m from the nearest house and 35 m
/// from the well (R3.52).
pub fn duel_spot() -> V3 {
    crate::world::START.add(v3(0, 0, DUEL_SOUTH))
}

/// The duel cleared: every foe gone, the player whole, sheathed, nothing in
/// the air (what the player carries is the inventory's).
pub fn reset(w: &mut World) {
    w.foes.clear();
    let held = crate::items::held(w);
    w.me = Fighter::new(Kind::Player, held, 0, w.player.pos, w.player.facing, false);
    w.blades = crate::blade::State::default();
    w.throwables.flying.clear();
    w.throwables.landed.clear();
    w.throwables.resting.clear();
}

/// Ticks of the speed a point moves at, metres a second (the HUD and tests
/// read speeds this way).
pub fn mps(per_tick: Fx) -> Fx {
    per_tick * TICKS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{DRAW, FORWARD, HOLD_LOOK};

    fn press(keys: u32) -> Input {
        Input {
            keys,
            ..Input::default()
        }
    }

    #[test]
    fn x_draws_the_sword_in_the_ticks_the_hand_takes_and_x_again_sheathes_it() {
        let mut w = World::new(1);
        assert_eq!(w.me.sheath, Sheath::Sheathed);
        w.step(&press(DRAW));
        assert_eq!(w.me.sheath, Sheath::Drawing(0));
        // held down, X acts once; the hand takes DRAW_TICKS to reach the hilt
        for _ in 0..DRAW_TICKS - 1 {
            w.step(&press(DRAW));
        }
        assert_eq!(w.me.sheath, Sheath::Drawing(DRAW_TICKS - 1));
        w.step(&press(DRAW));
        assert_eq!(w.me.sheath, Sheath::Drawn);
        assert!(w.me.arm.in_hand);
        w.step(&Input::default());
        w.step(&press(DRAW));
        assert_eq!(w.me.sheath, Sheath::Sheathing(0));
        for _ in 0..SHEATHE_TICKS {
            w.step(&Input::default());
        }
        assert_eq!(w.me.sheath, Sheath::Sheathed);
        assert!(!w.me.arm.in_hand);
    }

    #[test]
    fn sheathed_the_mouse_looks_and_drawn_it_moves_the_hand_unless_the_right_button_is_held() {
        let mut w = World::new(1);
        let turn = Input {
            dx: 5_000,
            ..Input::default()
        };
        let yaw = w.aim.yaw;
        w.step(&turn);
        assert_ne!(w.aim.yaw, yaw, "sheathed, the mouse turns the view");
        for _ in 0..DRAW_TICKS + 1 {
            w.step(&press(DRAW));
        }
        let (yaw, side) = (w.aim.yaw, w.me.arm.shell.side);
        w.step(&turn);
        assert_eq!(w.aim.yaw, yaw, "drawn, the view holds");
        // 50 px at 0.15° a pixel is 7.5°: 1 365 units (2 730 a 100 px)
        assert_eq!(
            w.me.arm.shell.side,
            side + 1365,
            "and the hand's target moves right"
        );
        let side = w.me.arm.shell.side;
        w.step(&Input {
            buttons: HOLD_LOOK,
            ..turn
        });
        assert_ne!(w.aim.yaw, yaw, "the right button looks");
        assert_eq!(w.me.arm.shell.side, side, "and leaves the hand where it was");
    }

    #[test]
    fn drawn_the_view_holds_on_the_nearest_foe_within_fifteen_metres() {
        let mut w = World::new(1);
        act(&mut w, "pilot", "still").unwrap();
        act(&mut w, "spawn", "orc").unwrap();
        // turn away first: the foe is behind
        w.aim.yaw = fx::wrap(w.aim.yaw + fx::HALF_TURN);
        for _ in 0..DRAW_TICKS + 1 {
            w.step(&press(DRAW));
        }
        assert_eq!(w.me.engaged, Some(1));
        for _ in 0..90 {
            w.step(&Input::default());
        }
        let from = pivot(w.player.pos, w.aim.yaw);
        let (yaw, _) = toward_chest(&w, from, 1);
        // within a degree and a little: the view's own turn rounds by a unit
        assert!(
            fx::wrap(w.aim.yaw - yaw).abs() <= deg(1) + 8,
            "{} {yaw}",
            w.aim.yaw
        );
        // and looks down at them: from the pivot 1.9 m up to the orc's chest
        // (its shoulders at 1.24 × 200 / 170 = 1.459 m, less 1/6 m: 1.292 m)
        // 6.06 m off is atan(0.608 / 6.06) = 5.7° down, and 7° more: 12.7°
        let want = -deg(127) / 10;
        assert!((w.aim.pitch - want).abs() <= deg(1), "{} {want}", w.aim.pitch);
        // the body faces the foe too
        let to = yaw_of(w.foes[0].body.pos.sub(w.player.pos));
        assert!(fx::wrap(w.player.facing - to).abs() <= deg(2));
        // 16 m off, no foe is held
        let mut far = w.clone();
        far.foes[0].body.pos = far.player.pos.add(facing(0, 0).scale(fx::int(16)));
        far.step(&Input::default());
        assert_eq!(far.me.engaged, None);
    }

    #[test]
    fn a_test_duel_spawns_six_metres_ahead_facing_the_player_and_reset_clears_it() {
        let mut w = World::new(1);
        act(&mut w, "spawn", "bandit").unwrap();
        let f = &w.foes[0];
        assert_eq!(f.fighter.kind, Kind::Bandit);
        // 30 m south of the start, clear of the test field's lantern (2.2 m
        // west and 4.5 m north of it): the player 34.6 m from it, the foe at
        // least 28.6
        let lantern = crate::world::START.add(v3(-ONE * 22 / 10, 0, -ONE * 45 / 10));
        assert!(
            w.player.pos.dist(lantern) > fx::int(34),
            "{}",
            w.player.pos.dist(lantern)
        );
        assert!(
            f.body.pos.dist(lantern) > fx::int(28),
            "{}",
            f.body.pos.dist(lantern)
        );
        let d = f.body.pos.sub(w.player.pos);
        assert!((d.len() - fx::int(6)).abs() <= 2, "{}", d.len());
        assert!(
            fx::wrap(yaw_of(d) - w.aim.yaw).abs() <= deg(1) / 2,
            "ahead, where the player looks"
        );
        assert!(
            fx::wrap(f.body.facing - yaw_of(d.neg())).abs() <= deg(1) / 2,
            "facing the player"
        );
        assert_eq!(
            act(&mut w, "spawn", "dragon"),
            Err("A test duel is with an orc, a bandit or a leader, not \"dragon\".".into())
        );
        act(&mut w, "guard", "-40,-10").unwrap();
        assert_eq!(
            (
                w.foes[0].fighter.arm.shell.side,
                w.foes[0].fighter.arm.shell.pitch
            ),
            (deg(-40), deg(-10))
        );
        assert_eq!(
            act(&mut w, "guard", "low"),
            Err("A guard is degrees right and up, as \"-40,-10\", not \"low\".".into())
        );
        act(&mut w, "health", "0").unwrap();
        assert!(w.foes[0].fighter.health.down(), "health 0 puts a foe down");
        assert_eq!(
            act(&mut w, "health", "lots"),
            Err("A foe's health is a whole number from 0 to 1000, not \"lots\".".into())
        );
        w.me.health.hp = 3;
        act(&mut w, "reset", "").unwrap();
        assert!(w.foes.is_empty());
        assert_eq!(w.me.health.hp, w.me.health.most);
    }

    #[test]
    fn health_reaches_the_hud_bars_through_m4_s_doors() {
        let mut w = World::new(1);
        act(&mut w, "spawn", "orc").unwrap();
        assert_eq!(crate::ui::foe(&w), None, "sheathed, no foe is held on");
        for _ in 0..DRAW_TICKS + 1 {
            w.step(&press(DRAW));
        }
        w.me.health = Health::new(80);
        w.me.health.hp = 64;
        let h = crate::ui::health(&w);
        assert_eq!((h.now, h.most), (64, 80));
        w.foes[0].fighter.health.hp = 90;
        let (who, g) = crate::ui::foe(&w).expect("drawn, the orc is held on");
        assert_eq!((who.as_str(), g.now, g.most), ("orc", 90, 140));
        w.me.health.hp = -9;
        assert_eq!(crate::ui::health(&w).now, 0, "never below none");
    }

    /// CW-18: the sheathed sword, its scabbard and the figure share no
    /// face: scanned as M3 scans its models (`flicker::fights`), seen from
    /// near and from 992 m round, the farthest a figure is drawn.
    #[test]
    fn a_sheathed_sword_its_scabbard_and_the_figure_share_no_face() {
        let w = World::new(1);
        let look = crate::look::tests::look();
        let mut out = Shapes::default();
        draw(&w, ONE, &look, &mut out);
        let insts: Vec<crate::flicker::Inst> = out
            .list
            .iter()
            .filter(|s| s.pass == crate::render::Pass::Solid)
            .map(|s| crate::flicker::Inst {
                mesh: s.mesh,
                at: s.at,
                rgb: s.rgb,
                value: s.value,
                pass: s.pass,
            })
            .collect();
        assert!(insts.len() > 30, "{} parts", insts.len());
        let centre = w.player.pos;
        let far = crate::models::FIGURE_FAR - fx::int(8);
        let eyes = (0..8)
            .flat_map(|k| {
                let a = fx::TURN * k / 8;
                [ONE * 2, far / 2]
                    .map(|up| centre.add(v3(fx::mul(fx::cos(a), far), up, fx::mul(fx::sin(a), far))))
            })
            .collect();
        let table = crate::prim::MeshTable::default();
        for seen in [
            crate::flicker::Seen {
                bottom: centre.y,
                ..crate::flicker::Seen::near()
            },
            crate::flicker::Seen {
                bottom: centre.y,
                eyes,
                far: crate::models::FIGURE_FAR,
            },
        ] {
            let bad: Vec<String> = crate::flicker::fights(&insts, &table, &seen)
                .iter()
                .map(|f| crate::flicker::describe(&insts, f))
                .collect();
            assert!(bad.is_empty(), "{bad:?}");
        }
    }

    /// X draws through the scabbard: the hand reaches the hilt at the left
    /// hip while the scabbard tips, then the blade slides out along it, the
    /// hand on the hilt all the way, and nothing jumps: no point of the arm
    /// or the weapon moves more than 25 cm in a tick (15 m/s) and the hand
    /// is never more than 2 cm off the grip once it holds it. Sheathing
    /// runs it back, the point into the mouth first.
    #[test]
    fn the_sword_slides_out_of_its_scabbard_and_back_in_without_a_jump() {
        let mut w = World::new(1);
        let mut last = w.me.arm.pts;
        let mut tilted = 0;
        for t in 0..(DRAW_TICKS as i32 + 30 + SHEATHE_TICKS as i32 + 20) {
            let keys = if t == 0 || t == DRAW_TICKS as i32 + 30 {
                DRAW
            } else {
                0
            };
            w.step(&press(keys));
            for (k, (a, b)) in last.iter().zip(w.me.arm.pts.iter()).enumerate() {
                assert!(
                    a.p.dist(b.p) < ONE / 4,
                    "tick {t}: point {k} jumped {}",
                    a.p.dist(b.p)
                );
            }
            if w.me.arm.riding {
                let grip = w.me.arm.grip_point();
                assert!(
                    w.me.arm.pts[arm::HAND[1]].p.dist(grip) < ONE / 50,
                    "tick {t}: the hand let go"
                );
            }
            tilted = tilted.max(w.me.tilt);
            last = w.me.arm.pts;
        }
        assert_eq!(tilted, ONE, "the scabbard tipped for the draw");
        assert_eq!(w.me.sheath, Sheath::Sheathed);
        assert_eq!(w.me.tilt, 0, "and swung back after");
        // home: the hilt's grip where the scabbard holds it
        let b = &w.player;
        let sc = Scabbard::of(b, &w.me.build(), &w.me.gait, 0, 0, &w.me.arm.weapon);
        assert!(
            w.me.arm.grip_point().dist(sc.grip) < 2,
            "{:?} {:?}",
            w.me.arm.grip_point(),
            sc.grip
        );
    }

    /// R3.54: drawn and held on an orc standing still at its band (1.75 m),
    /// the view eases fully into its duel framing, and on a 1920 × 1080
    /// screen the nearest point of the orc's blade sits further from the
    /// player's head (its capsule, cap and all) than it does without:
    /// measured 107 px against 93.
    #[test]
    fn held_on_a_foe_the_view_frames_the_duel_with_its_blade_clear_of_the_player_s_head() {
        let mut w = World::new(1);
        act(&mut w, "spawn", "orc").unwrap();
        act(&mut w, "pilot", "still").unwrap();
        act(&mut w, "gap", "175").unwrap();
        w.step(&press(DRAW));
        for _ in 0..120 {
            w.step(&Input::default());
        }
        assert_eq!(w.me.framing, ONE, "eased fully in");
        // the nearest a point of the blade comes to the head's capsule on
        // screen, pixels of a 1920-wide view
        let clear = |w: &World| {
            let aspect = ONE * 16 / 9;
            let cam = w.camera(ONE, Mouse::default(), crate::camera::BACK);
            let head = crate::blade::body(Kind::Player, &w.me.pose)[0];
            let hs: Vec<(Fx, Fx)> = (0..=4)
                .filter_map(|k| cam.screen(aspect, head.a.lerp(head.b, ONE * k / 4)))
                .collect();
            let a = &w.foes[0].fighter.arm;
            let mut best = i64::MAX;
            for rail in arm::rails(a.weapon.item) {
                let (p, q, _) = a.rail(rail, false, false);
                for k in 0..=16 {
                    if let Some(s) = cam.screen(aspect, p.lerp(q, ONE * k / 16)) {
                        for h in &hs {
                            let dx = (s.0 - h.0) as i64;
                            let dy = (s.1 - h.1) as i64 * 9 / 16;
                            best = best.min(fx::isqrt_u64((dx * dx + dy * dy) as u64) as i64);
                        }
                    }
                }
            }
            best * 1920 / ONE as i64
        };
        let mut flat = w.clone();
        flat.me.framing = 0;
        flat.me.prev_framing = 0;
        let (with, without) = (clear(&w), clear(&flat));
        assert!(with > without + 10, "framed {with} px, unframed {without} px");
        // a head is about 47 px round here: the blade clears it
        assert!(with > 47, "{with} px");
    }

    /// By hand: a scabbard hides its blade and the blade's 4 mm outline
    /// (M3's `ink_item`) all along. The short sword's blade is 46 by 9 mm,
    /// so 54 by 17 with its outline; the sword's 54 by 10, so 62 by 18. The
    /// scabbard narrows to 60% over its last 141 mm, from 690 mm past the
    /// grip; the point begins at 714 (806 less twice the 46 mm width), 24
    /// mm in, where the short sword's scabbard is 66 × (1 − 0.4 × 24 / 141)
    /// = 61.5 mm wide and 28 thick, still round the 54 by 17 there.
    #[test]
    fn a_scabbard_is_wider_and_thicker_than_the_blade_it_hides() {
        for (item, blade) in [(Held::ShortSword, (54, 17)), (Held::Sword, (62, 18))] {
            let (w, t) = scabbard_size(item);
            assert!(w > blade.0 && t > blade.1, "{item:?}: {w} by {t}");
        }
        let (w, t) = scabbard_size(Held::ShortSword);
        let at = |x: i32| x * (100 - 40 * 24 / 141) / 100;
        assert!(at(w) > 54 && at(t) > 17, "{} by {} at the point", at(w), at(t));
    }

    #[test]
    fn walking_forward_still_walks_north_with_the_figure_on_its_feet() {
        let mut w = World::new(1);
        w.aim.yaw = 0;
        for _ in 0..60 {
            w.step(&press(FORWARD));
        }
        assert!(w.player.pos.z < crate::world::START.z - ONE, "{:?}", w.player.pos);
        let p = w.me.pose;
        assert!(p.foot[0].y >= 0 && p.foot[1].y >= 0, "feet on or over the ground");
        assert!(p.head.y > p.chest.y && p.chest.y > p.pelvis.y);
    }
}
