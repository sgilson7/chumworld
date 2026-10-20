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
use crate::page::Strings;
use crate::pose::{self, Build, Held, Kind, Pose};
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::world::{HudItem, World};

pub use crate::motion::{RUN, WALK};

/// The soft shadow under a body: its width at the player's girth.
pub const SHADOW_WIDTH: Fx = ONE * 11 / 10;
/// The shadow's height over the ground: above it by more than the depth
/// buffer needs at any distance the shadow is drawn from.
pub const SHADOW_LIFT: Fx = ONE * 2 / 100;
/// A drawn fighter holds on the nearest foe this near (journal).
pub const HOLD_RANGE: Fx = fx::int(15);
/// A foe holds on the player from this far: it was set on them.
pub const FOE_RANGE: Fx = fx::int(40);
/// Ticks the hand takes to reach the hilt, and to put the sword away.
pub const DRAW_TICKS: u8 = 9;
pub const SHEATHE_TICKS: u8 = 12;
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
/// Where the scabbard sits on the left hip, in the body's frame (right, up,
/// forward), and which way its blade points (down and back).
const SHEATH_AT: (i32, i32, i32) = (-19, 92, 6);
const SHEATH_DIR: (i32, i32, i32) = (-18, -72, -62);
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
}

impl Fighter {
    pub fn new(kind: Kind, held: Held, team: u8, feet: V3, yaw: i32, drawn: bool) -> Fighter {
        let build = pose::build(kind);
        let body = Body::new(feet, yaw);
        let frame = frame_of(&body, &build, 0, feet.y);
        let (pommel, tip) = sheath_points(&body, &build, &arm::weapon(held));
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
    (yaw_of(d), fx::atan2(d.y, d.len_xz()))
}

/// Still step 1: held on a foe, the view eases round to frame them unless
/// the right button looks; every foe looks straight at whom it holds on.
pub fn hold_on(w: &mut World) {
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
pub fn frame_of(b: &Body, build: &Build, fall: Fx, ground: Fx) -> Frame {
    let place = local(b, fall);
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
/// tipped `fall` of a quarter turn onto its back about its heels.
fn local(b: &Body, fall: Fx) -> impl Fn(V3) -> V3 {
    let turn = Affine::rot_y(b.facing).then(&Affine::rot_x(fx::mul(fall, deg(84))));
    let feet = b.pos;
    move |p: V3| feet.add(turn.apply_dir(v3(p.x, p.y, -p.z)))
}

/// The scabbard's hilt and blade for a body: where a sheathed weapon is.
pub fn sheath_points(b: &Body, build: &Build, wpn: &arm::Weapon) -> (V3, V3) {
    let place = local(b, 0);
    let scale = |c: i32| c * ONE / 100;
    let grip = place(v3(
        scale(SHEATH_AT.0) * build.girth_pct / 100,
        (scale(SHEATH_AT.1) as i64 * build.height as i64 / pose::build(Kind::Player).height as i64) as Fx,
        scale(SHEATH_AT.2),
    ));
    let dir = place(v3(scale(SHEATH_DIR.0), scale(SHEATH_DIR.1), scale(SHEATH_DIR.2)))
        .sub(b.pos)
        .norm();
    let pommel = grip.sub(dir.scale(wpn.grip[1]));
    (pommel, pommel.add(dir.scale(wpn.len)))
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
    let place = local(b, fall);
    let bob = fx::mul(
        fx::cos(f.gait.phase.wrapping_mul(2)).abs(),
        fx::mul(ONE / 40, f.gait.swing.min(ONE)),
    );
    let pelvis = place(v3(0, build.hip_y - bob, 0));
    let hip = [-1, 1].map(|s| place(v3(s * build.hip_x, build.hip_y - bob, 0)));
    let flat = crate::motion::feet(b, &f.gait, &build, b.pos);
    // a fallen body's feet stay where they were; standing, the gait's
    let foot = if fall > 0 {
        [-1, 1].map(|s| place(v3(s * build.hip_x, ONE / 20, 0)))
    } else {
        flat
    };
    let fwd = facing(b.facing, 0);
    let knee = [0, 1].map(|s| crate::motion::knee(hip[s], foot[s], build.thigh, build.shin, fwd));
    let a = &f.arm;
    Pose {
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
    }
}

/// The joints of fighter `id` this tick.
pub fn pose_of(w: &World, id: usize) -> Pose {
    pose_from(body(w, id), fighter(w, id))
}

/// What each fighter's arms aim at this tick, from its sword's state, its
/// shell, its throw and its health. Also moves the sheath on a tick.
pub fn goals(w: &World, id: usize, input: &Input) -> (Sheath, Goals) {
    let b = body(w, id);
    let f = fighter(w, id);
    let build = f.build();
    let place = local(b, 0);
    let wpn = f.arm.weapon;
    let reach = build.upper_arm + build.forearm;
    let (pommel, tip) = sheath_points(b, &build, &wpn);
    let sheath_grip = pommel.add(tip.sub(pommel).norm().scale(wpn.grip[1]));
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
    let blade = arm::blade_dir(b.facing, &f.arm.shell);
    let two = wpn.two_handed();
    let winding = f.throw.winding();
    let mut g = Goals {
        hand: [Some(rest(0)), Some(rest(1))],
        tip: None,
        strength: [0, 1].map(|s| f.servo_pct * f.health.strength(s) / 100),
        swing: input.button(input::SWING) && !down,
        in_hand: false,
        hold: [false, false],
        sheath: (pommel, tip),
        target: None,
    };
    match sheath {
        Sheath::Sheathed => {}
        Sheath::Drawing(_) => {
            g.hand[1] = Some(sheath_grip);
        }
        Sheath::Drawn => {
            g.in_hand = true;
            g.hold = [two && !winding, true];
            g.hand[1] = Some(shell_hand);
            g.target = Some(shell_hand);
            // the second hand on the hilt goes where the hilt is asked to be
            g.hand[0] = Some(if two && !winding {
                shell_hand.sub(blade.scale(wpn.grip[1] - wpn.grip[0]))
            } else {
                free(0)
            });
            g.tip = Some(shell_hand.add(blade.scale(wpn.len - wpn.grip[1])));
        }
        Sheath::Sheathing(_) => {
            g.in_hand = true;
            g.hold = [false, true];
            g.hand[1] = Some(sheath_grip);
            // the scabbard's own line: the blade goes in point first
            g.tip = Some(tip);
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
        g.hand = [None, None];
        g.tip = None;
        g.target = None;
    }
    (sheath, g)
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
        crate::models::figure(f.kind, &p, look, out);
        if f.held != Held::Nothing {
            crate::models::held(f.held, p.grip, p.tip, look, out);
        }
        let feet = body(w, id).feet(alpha);
        let wide = (SHADOW_WIDTH as i64 * f.build().girth_pct as i64 / 100) as Fx;
        out.shadow(
            feet.add(v3(0, SHADOW_LIFT, 0)),
            wide,
            rgb(look.palette.shadow),
            ONE,
        );
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
    if w.sticks {
        sticks(w, alpha, look, out);
    }
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

/// The player's health, and the health of the foe they hold on, as text and
/// a share (percent) M4's bars draw.
pub fn hud(w: &World, s: &Strings, out: &mut Vec<HudItem>) {
    let h = &w.me.health;
    out.push(HudItem {
        id: "health".into(),
        text: format!("{} {}", s.get("health_you"), h.hp.max(0)),
        shown: true,
        value: Some(h.pct()),
    });
    let foe =
        w.me.engaged
            .map(|k| k as usize)
            .filter(|&k| k > 0 && k < count(w));
    let (text, value) = match foe {
        Some(k) => {
            let f = fighter(w, k);
            (
                format!("{} {}", s.get(kind_key(f.kind)), f.health.hp.max(0)),
                Some(f.health.pct()),
            )
        }
        None => (String::new(), None),
    };
    out.push(HudItem {
        id: "foe_health".into(),
        text,
        shown: foe.is_some(),
        value,
    });
}

/// The strings key that names a kind of foe.
pub fn kind_key(k: Kind) -> &'static str {
    match k {
        Kind::Orc => "foe_orc",
        Kind::Leader => "foe_leader",
        _ => "foe_bandit",
    }
}

/// `act("duel.<name>", value)`: `spawn` puts an `orc`, `bandit` or `leader`
/// 6 m ahead of the player; `reset` clears the duel; `sticks` shows the
/// physics view (`on`, `off`).
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
        _ => Err(format!("Unknown action \"duel.{name}\".")),
    }
}

/// A foe of `kind` ahead of the player, facing them, sword drawn. A second
/// and later foe stands to one side or the other of the first.
pub fn spawn(w: &mut World, kind: Kind) {
    let k = w.foes.len() as i32;
    let side = if k % 2 == 0 { 1 } else { -1 } * ((k + 1) / 2) * deg(28);
    let way = fx::wrap(w.aim.yaw + side);
    let mut at = w.player.pos.add(facing(way, 0).scale(SPAWN_AHEAD));
    at.y = w.ground(at.x, at.z);
    let yaw = yaw_of(w.player.pos.sub(at));
    let salt = w.rng.next_u64();
    let foe = crate::pilot::foe(&w.pilots.book, kind, at, yaw, salt);
    w.foes.push(foe);
}

/// The duel cleared: every foe gone, the player whole, sheathed, the pouch
/// full, nothing in the air.
pub fn reset(w: &mut World) {
    w.foes.clear();
    let held = crate::items::held(w);
    w.me = Fighter::new(Kind::Player, held, 0, w.player.pos, w.player.facing, false);
    w.me.pouch = crate::throwables::Pouch::player();
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
        w.me.health.hp = 3;
        act(&mut w, "reset", "").unwrap();
        assert!(w.foes.is_empty());
        assert_eq!(w.me.health.hp, w.me.health.most);
    }

    #[test]
    fn health_reaches_the_hud_as_text_and_a_share() {
        let mut w = World::new(1);
        act(&mut w, "spawn", "orc").unwrap();
        for _ in 0..DRAW_TICKS + 1 {
            w.step(&press(DRAW));
        }
        w.me.health.hp = 64;
        let s = crate::page::tests::strings();
        let mut out = Vec::new();
        hud(&w, &s, &mut out);
        assert_eq!(out[0].text, "Your health 64");
        assert_eq!(out[0].value, Some(64));
        // the orc's 140 of 140: 100%
        assert_eq!(out[1].text, "Orc raider 140");
        assert_eq!(out[1].value, Some(100));
        assert!(out[1].shown);
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
