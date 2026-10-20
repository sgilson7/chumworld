//! `arm`: the simulated arms (M1 Duel, task D). Vagrancy's arm rebuilt in
//! 3D (journal, "The arm"; R1.2): points joined by sticks, moved by position
//! Verlet and relaxed `PASSES` times a tick in a fixed order (Jakobsen,
//! "Advanced Character Physics", GDC 2001), in Q16.16 metres (CW-2).
//!
//! Each fighter has six points: an elbow and a hand per arm, and the weapon
//! as two, its pommel and its tip. The shoulders are anchors the body
//! carries (`Frame`); they have no mass and nothing moves them. Sticks hold
//! the upper arms, the forearms and the weapon to their lengths; pins hold
//! a hand to its place on the hilt, both hands for a two-handed weapon, the
//! right alone for the short sword.
//!
//! **Nothing is animated.** The mouse moves a target on a shell of reach in
//! front of the sword shoulder (`Shell`); a servo pulls the sword hand
//! toward it, and a second one turns the blade toward the line the target's
//! place on the shell asks for (`blade_dir`). Both are capped in speed and
//! in the change of speed they may make a tick, so a heavy weapon, whose
//! points weigh against the hand through the pins, starts late and carries
//! past where it was aimed. The left button (`Goals::swing`) raises the
//! caps for a committed swing and loosens the blade's servo, so the blade
//! trails the hand and whips through.
//!
//! **The free-energy rule** (Vagrancy's D9). Every correction a stick or a
//! pin makes is an exact pair shift: when it moves one point by `k·m_b` it
//! moves the other by `−k·m_a`, with one integer `k` an axis, so the pair's
//! momentum does not change by a single unit, and a stick only ever takes
//! energy out. The servos are the one source; drag (`DRAG_PCT`) and the
//! speed cap (`CAP`) bound what they put in. `nothing_gains_energy_without_a_servo`
//! and `a_stick_and_a_pin_move_no_momentum` hold it.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{facing, v3, Affine, V3};
use crate::input::{Input, Mouse};
use crate::player::{self, Sheath};
use crate::pose::{Build, Held};
use crate::world::World;

/// The points, by index.
pub const N: usize = 6;
pub const ELBOW: [usize; 2] = [0, 1];
pub const HAND: [usize; 2] = [2, 3];
pub const POMMEL: usize = 4;
pub const TIP: usize = 5;
/// Relaxation passes a tick (Vagrancy, PLAN.md).
pub const PASSES: usize = 8;
/// Gravity: 9.8 m/s² is 9.8 / 3600 m a tick a tick, 178.4 units.
pub const GRAVITY: Fx = 178;
/// The share of a point's speed drag takes each tick, percent: the weapon's
/// points, and the elbows and hands, which their muscles damp as well.
pub const DRAG_PCT: i32 = 3;
pub const LIMB_DRAG_PCT: i32 = 10;
/// No point moves faster than 32 m/s (a sword's tip in a full swing).
pub const CAP: Fx = ONE * 32 / 60;
/// An elbow's and a hand's mass (the weapon's are in `Weapon::mass`).
pub const LIMB_MASS: i32 = 2;
/// The hand's servo: the speed it asks for is this share of the distance
/// to the target a tick, at most `HAND_SPEED`, reached by changes of at
/// most `HAND_ACCEL` a tick. A committed swing has its own caps.
pub const GAIN: Fx = ONE * 35 / 100;
pub const HAND_SPEED: Fx = ONE * 65 / 600;
pub const HAND_ACCEL: Fx = ONE * 9 / 600;
pub const SWING_SPEED: Fx = ONE * 130 / 600;
pub const SWING_ACCEL: Fx = ONE * 17 / 600;
/// The blade's servo, which turns the tip toward the line the shell asks
/// for; in a committed swing it keeps `SWING_TIP_PCT` of its strength.
pub const TIP_SPEED: Fx = ONE * 90 / 600;
pub const TIP_ACCEL: Fx = ONE * 10 / 600;
pub const SWING_TIP_SPEED: Fx = ONE * 200 / 600;
pub const SWING_TIP_ACCEL: Fx = ONE * 22 / 600;
/// A hand with no target (the sword sheathed, the arm at rest) is held at
/// this share of the servo's strength.
pub const REST_PCT: i32 = 45;
/// Elbows are pushed a little down and out each tick, as the muscles that
/// keep them from folding the wrong way would.
pub const ELBOW_POLE: Fx = ONE * 12 / 6000;
/// A still body's arm whose points would each move less than this a tick
/// (0.37 mm) holds still instead: rounding would otherwise rock it by a
/// unit or two for ever.
pub const REST: Fx = 24;
/// No point goes nearer the ground than this.
pub const FLOOR: Fx = ONE * 2 / 100;
/// The shell's edges: how far right and left of straight ahead the hand's
/// target goes, how far down and up, and its reach as a share of the arm.
pub const SIDE_MIN: i32 = -deg(75);
pub const SIDE_MAX: i32 = deg(85);
pub const PITCH_MIN: i32 = -deg(50);
pub const PITCH_MAX: i32 = deg(85);
pub const REACH_MIN_PCT: i32 = 45;
pub const REACH_MAX_PCT: i32 = 97;
/// The guard the hand starts in: a little right of straight ahead, below
/// the shoulder, so the blade stands clear of the body seen from behind.
pub const GUARD_SIDE: i32 = deg(6);
pub const GUARD_PITCH: i32 = -deg(14);
pub const GUARD_REACH_PCT: i32 = 82;
/// The mouse moves the target 0.15° a CSS pixel: 2 730 angle units a 100 px.
pub const UNITS_PER_100_PX: i64 = fx::TURN as i64 * 15 / 360;
/// The wheel moves it 3 cm a notch (100 of the browser's units).
pub const REACH_PER_NOTCH: Fx = ONE * 3 / 100;
/// The blade's line for a hand on the shell: raised this far, plus this
/// share of the hand's pitch, and fanned this much wider than the hand's
/// side.
pub const BLADE_RISE: i32 = deg(40);
pub const BLADE_FOLLOW_PCT: i32 = 70;
pub const BLADE_FAN_PCT: i32 = 125;
/// Above this pitch the blade swings over to a high guard across the head,
/// fully by `HIGH_FULL` above it.
pub const HIGH_FROM: i32 = deg(28);
pub const HIGH_FULL: i32 = deg(50);

/// Centimetres to `fx` metres.
const fn cm(c: i32) -> Fx {
    c * ONE / 100
}

/// One point: where it is, where it was a tick ago (the difference is its
/// velocity), and its mass (0: an anchor nothing moves).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub p: V3,
    pub q: V3,
    pub m: i32,
}

impl Point {
    pub fn at(p: V3, m: i32) -> Point {
        Point { p, q: p, m }
    }

    pub fn vel(&self) -> V3 {
        self.p.sub(self.q)
    }
}

/// A weapon's sizes: pommel to tip, the hilt's length, each hand's place on
/// it from the pommel (left −1: one-handed), and its two points' masses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weapon {
    pub item: Held,
    pub len: Fx,
    pub hilt: Fx,
    pub grip: [Fx; 2],
    pub mass: [i32; 2],
    /// how much of the servo's change of speed a tick the weapon allows,
    /// percent: a heavy one starts late and carries past
    pub heft_pct: i32,
}

impl Weapon {
    pub fn two_handed(&self) -> bool {
        self.grip[0] >= 0
    }

    pub fn blade(&self) -> Fx {
        self.len - self.hilt
    }
}

/// Each weapon's sizes. The short sword (70 cm blade, PLAN.md) is held in
/// the right hand; the sword (100 cm) and the orc's cleaver in both. The
/// cleaver's weight is in its head.
pub fn weapon(item: Held) -> Weapon {
    // the model's own grip-to-tip length (M3's door, R5.5), so the drawn
    // blade ends where the simulated one does
    let w = sized(item);
    Weapon {
        len: w.grip[1] + crate::models::grip_to_tip(w.item).max(cm(30)),
        ..w
    }
}

/// The hilt, the hands and the masses of each weapon (the length is the
/// model's, `weapon`).
fn sized(item: Held) -> Weapon {
    match item {
        Held::Sword => Weapon {
            item,
            len: 0,
            hilt: cm(22),
            grip: [cm(5), cm(16)],
            mass: [3, 3],
            heft_pct: 75,
        },
        Held::Cleaver => Weapon {
            item,
            len: 0,
            hilt: cm(24),
            grip: [cm(5), cm(17)],
            mass: [2, 8],
            heft_pct: 45,
        },
        _ => Weapon {
            item,
            len: 0,
            hilt: cm(15),
            grip: [-1, cm(8)],
            mass: [2, 1],
            heft_pct: 100,
        },
    }
}

/// The hand's target on the reach shell, in the body's frame: `side` right
/// of straight ahead (angle units, right positive), `pitch` up, `reach`
/// from the sword shoulder; and what of the mouse has not yet moved it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shell {
    pub side: i32,
    pub pitch: i32,
    pub reach: Fx,
    pub rest: (i32, i32),
}

impl Shell {
    pub fn guard(arm: Fx) -> Shell {
        Shell {
            side: GUARD_SIDE,
            pitch: GUARD_PITCH,
            reach: arm * GUARD_REACH_PCT / 100,
            rest: (0, 0),
        }
    }

    /// The shell after mouse movement `m` (right moves right, down moves
    /// down, the wheel toward the reader draws the hand in), held to its
    /// edges, and how far past the side edges the mouse pushed (angle units,
    /// right positive): the body turns by that when it holds on no one.
    pub fn moved(self, m: Mouse, arm: Fx) -> (Shell, i32) {
        let split = |d: i32, rest: i32| {
            let total = d as i64 * UNITS_PER_100_PX + rest as i64;
            (total.div_euclid(10_000) as i32, total.rem_euclid(10_000) as i32)
        };
        let (dside, rx) = split(m.dx, self.rest.0);
        let (dpitch, ry) = split(m.dy, self.rest.1);
        let side = self.side + dside;
        let clamped = side.clamp(SIDE_MIN, SIDE_MAX);
        let reach = self.reach - (m.wheel as i64 * REACH_PER_NOTCH as i64 / 10_000) as Fx;
        (
            Shell {
                side: clamped,
                pitch: (self.pitch - dpitch).clamp(PITCH_MIN, PITCH_MAX),
                reach: reach.clamp(arm * REACH_MIN_PCT / 100, arm * REACH_MAX_PCT / 100),
                rest: (rx, ry),
            },
            side - clamped,
        )
    }

    /// The target in the world, for the sword shoulder at `shoulder` and the
    /// body facing `yaw`.
    pub fn point(&self, shoulder: V3, yaw: i32) -> V3 {
        shoulder.add(facing(yaw - self.side, self.pitch).scale(self.reach))
    }
}

/// The mouse movement (hundredths of a pixel) that moves the shell's target
/// by `dside` and `dpitch`: what a pilot sends to put its hand somewhere.
pub fn mouse_for(dside: i32, dpitch: i32) -> (i32, i32) {
    let px = |d: i32| (d as i64 * 10_000 / UNITS_PER_100_PX) as i32;
    (px(dside), -px(dpitch))
}

/// The line the blade is asked to hold for a hand at `shell`, the body
/// facing `yaw`: raised above the hand and fanned out with it, so a hand
/// in front points the blade a little up, at a foe's face; a hand to the
/// right lays it out
/// to the right, and a hand high swings it across above the head.
pub fn blade_dir(yaw: i32, shell: &Shell) -> V3 {
    let mid = facing(
        yaw - shell.side * BLADE_FAN_PCT / 100,
        BLADE_RISE + shell.pitch * BLADE_FOLLOW_PCT / 100,
    );
    let top = facing(yaw + deg(80), deg(10)).add(facing(yaw, 0).scale(ONE / 3));
    let high = ((shell.pitch - HIGH_FROM) as i64 * ONE as i64 / (HIGH_FULL - HIGH_FROM) as i64)
        .clamp(0, ONE as i64) as Fx;
    mid.lerp(top.norm(), high).norm()
}

/// What the body gives the arms this tick: the shoulders, the spine and
/// head to keep elbows, hands and the blade's tip out of, the ground, which
/// way it faces and where it stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub shoulder: [V3; 2],
    pub spine: [V3; 2],
    pub chest_r: Fx,
    pub head: V3,
    pub head_r: Fx,
    pub ground: Fx,
    pub yaw: i32,
    pub centre: V3,
}

/// What the arms aim at this tick (`player::goals`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Goals {
    /// each hand's target; none goes limp
    pub hand: [Option<V3>; 2],
    /// the blade's tip's target
    pub tip: Option<V3>,
    /// each arm's servo, percent
    pub strength: [i32; 2],
    /// the left button: a committed swing
    pub swing: bool,
    /// the weapon is in the hand (otherwise it rides in the scabbard)
    pub in_hand: bool,
    /// which hands are pinned to the hilt
    pub hold: [bool; 2],
    /// the scabbard's pommel and tip
    pub sheath: (V3, V3),
    /// the shell's target, for the reticle
    pub target: Option<V3>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arm {
    pub pts: [Point; N],
    pub weapon: Weapon,
    pub frame: Frame,
    pub shell: Shell,
    /// upper arm and forearm (elbow to the grip)
    pub bones: (Fx, Fx),
    pub in_hand: bool,
    pub hold: [bool; 2],
    /// the shell's target this tick and last, for the reticle
    pub target: V3,
    pub prev_target: V3,
}

/// `v` × `num` / `den`, in i64.
fn mul_div(v: V3, num: i64, den: i64) -> V3 {
    let f = |c: i32| (c as i64 * num / den) as i32;
    v3(f(v.x), f(v.y), f(v.z))
}

fn div(v: V3, k: i32) -> V3 {
    v3(v.x / k, v.y / k, v.z / k)
}

fn times(v: V3, k: i32) -> V3 {
    v3(v.x * k, v.y * k, v.z * k)
}

impl Arm {
    /// Arms hanging at the sides, the weapon in its scabbard.
    pub fn new(frame: &Frame, weapon: Weapon, build: &Build, pommel: V3, tip: V3) -> Arm {
        let bones = (build.upper_arm, build.forearm);
        let down = v3(0, -ONE, 0);
        let mut pts = [Point::at(V3::ZERO, LIMB_MASS); N];
        for s in 0..2 {
            let e = frame.shoulder[s].add(down.scale(bones.0));
            pts[ELBOW[s]] = Point::at(e, LIMB_MASS);
            pts[HAND[s]] = Point::at(e.add(down.scale(bones.1)), LIMB_MASS);
        }
        pts[POMMEL] = Point::at(pommel, weapon.mass[0]);
        pts[TIP] = Point::at(tip, weapon.mass[1]);
        let arm = bones.0 + bones.1;
        Arm {
            pts,
            weapon,
            frame: *frame,
            shell: Shell::guard(arm),
            bones,
            in_hand: false,
            hold: [false, false],
            target: frame.shoulder[1],
            prev_target: frame.shoulder[1],
        }
    }

    /// The weapon in hand at the guard at once (a foe that spawns drawn).
    pub fn take_in_hand(&mut self, frame: &Frame) {
        let w = self.weapon;
        let hand = self.shell.point(frame.shoulder[1], frame.yaw);
        let dir = blade_dir(frame.yaw, &self.shell);
        let pommel = hand.sub(dir.scale(w.grip[1]));
        self.pts[POMMEL] = Point::at(pommel, w.mass[0]);
        self.pts[TIP] = Point::at(pommel.add(dir.scale(w.len)), w.mass[1]);
        self.pts[HAND[1]] = Point::at(hand, LIMB_MASS);
        if w.two_handed() {
            self.pts[HAND[0]] = Point::at(pommel.add(dir.scale(w.grip[0])), LIMB_MASS);
        }
        for s in 0..2 {
            let mid = frame.shoulder[s]
                .lerp(self.pts[HAND[s]].p, ONE / 2)
                .add(v3(0, -ONE / 8, 0));
            self.pts[ELBOW[s]] = Point::at(mid, LIMB_MASS);
        }
        self.in_hand = true;
        self.hold = [w.two_handed(), true];
        self.target = hand;
        self.prev_target = hand;
        for _ in 0..PASSES * 4 {
            self.relax();
        }
        for p in &mut self.pts {
            p.q = p.p;
        }
    }

    /// The reach of the arm from shoulder to grip.
    pub fn reach(&self) -> Fx {
        self.bones.0 + self.bones.1
    }

    /// Where the sword hand holds the hilt.
    pub fn grip_point(&self) -> V3 {
        let (a, b) = (self.pts[POMMEL].p, self.pts[TIP].p);
        a.add(mul_div(
            b.sub(a),
            self.weapon.grip[1] as i64,
            self.weapon.len.max(1) as i64,
        ))
    }

    /// Where the grip was a tick ago.
    pub fn grip_was(&self) -> V3 {
        let (a, b) = (self.pts[POMMEL].q, self.pts[TIP].q);
        a.add(mul_div(
            b.sub(a),
            self.weapon.grip[1] as i64,
            self.weapon.len.max(1) as i64,
        ))
    }

    /// Every point carried with the body as it moves and turns: from the
    /// last frame's centre and heading to this one's.
    pub fn carry(&mut self, to: &Frame) {
        let from = self.frame;
        let turn = Affine::rot_y(fx::wrap(to.yaw - from.yaw));
        for pt in &mut self.pts {
            pt.p = turn.apply_dir(pt.p.sub(from.centre)).add(to.centre);
            pt.q = turn.apply_dir(pt.q.sub(from.centre)).add(to.centre);
        }
    }

    /// One tick: carried with the body, pulled by the servos, moved, then
    /// relaxed `PASSES` times.
    pub fn step(&mut self, frame: &Frame, g: &Goals) {
        let before = (self.pts, self.frame, self.in_hand);
        let still = frame.centre == self.frame.centre && frame.yaw == self.frame.yaw;
        self.carry(frame);
        self.frame = *frame;
        self.prev_target = self.target;
        if let Some(t) = g.target {
            self.target = t;
        }
        // the weapon changes hands: into the hand still, out of it into the
        // scabbard
        if g.in_hand && !self.in_hand {
            for k in [POMMEL, TIP] {
                self.pts[k].q = self.pts[k].p;
            }
        }
        self.in_hand = g.in_hand;
        self.hold = g.hold;
        let mut acc = [V3::ZERO; N];
        let heft = self.weapon.heft_pct;
        let (speed, accel, tip_speed, tip_accel) = if g.swing {
            (SWING_SPEED, SWING_ACCEL, SWING_TIP_SPEED, SWING_TIP_ACCEL)
        } else {
            (HAND_SPEED, HAND_ACCEL, TIP_SPEED, TIP_ACCEL)
        };
        let held = |s: usize| g.in_hand && g.hold[s];
        for s in 0..2 {
            let pct = g.strength[s];
            match g.hand[s] {
                // a hand on the hilt moves with the weapon (below)
                Some(_) if held(s) => {}
                Some(t) => {
                    let ac = HAND_ACCEL * REST_PCT / 100;
                    acc[HAND[s]] = servo(&self.pts[HAND[s]], t, HAND_SPEED * pct / 100, ac * pct / 100);
                }
                None => {}
            }
            // a limp arm (no target) has no muscle at the elbow either
            if g.hand[s].is_some() {
                let out = crate::geom::right_of(frame.yaw).scale(if s == 0 { -ONE / 2 } else { ONE / 2 });
                acc[ELBOW[s]] = out.add(v3(0, -ONE, 0)).norm().scale(ELBOW_POLE);
            }
        }
        if let (true, Some(t)) = (g.in_hand, g.hand[1]) {
            // the hand and the weapon it holds are pushed as one: the grip
            // toward the target, the weapon's heft taking from the caps
            let pct = g.strength[1] * heft / 100;
            let grip = Point {
                p: self.grip_point(),
                q: self.grip_was(),
                m: 1,
            };
            let dv = servo(&grip, t, speed, accel * pct / 100);
            for k in [HAND[0], HAND[1], POMMEL, TIP] {
                if k == POMMEL || k == TIP || held(if k == HAND[0] { 0 } else { 1 }) {
                    acc[k] = acc[k].add(dv);
                }
            }
            // and the blade is turned toward its line about the grip, as a
            // wrist (or two hands, one against the other) turns it: each
            // held point moves across the blade in proportion to how far it
            // is from the grip, the tip one way, the pommel the other
            if let Some(tt) = g.tip {
                let along = self.pts[TIP].p.sub(self.pts[POMMEL].p).norm();
                let mut dt = servo(&self.pts[TIP], tt, tip_speed, tip_accel * pct / 100).sub(dv);
                dt = dt.sub(along.scale(dt.dot(along)));
                let w = self.weapon;
                let reach = (w.len - w.grip[1]).max(1) as i64;
                let at = |s: Fx| mul_div(dt, (s - w.grip[1]) as i64, reach);
                acc[TIP] = acc[TIP].add(dt);
                acc[POMMEL] = acc[POMMEL].add(at(0));
                if held(0) {
                    acc[HAND[0]] = acc[HAND[0]].add(at(w.grip[0]));
                }
            }
        }
        self.integrate(&acc);
        if !self.in_hand {
            // the scabbard carries the weapon: no speed of its own
            let (pommel, tip) = g.sheath;
            self.pts[POMMEL] = Point::at(pommel, self.weapon.mass[0]);
            self.pts[TIP] = Point::at(tip, self.weapon.mass[1]);
        }
        for _ in 0..PASSES {
            self.relax();
        }
        self.hold_to_cap();
        // at rest: a body standing still whose every point would move less
        // than `REST` keeps the tick before exactly, so a still fighter
        // draws the same frame tick after tick (CW-18)
        let settled = self.pts.iter().zip(before.0.iter()).all(|(n, o)| {
            let d = n.p.sub(o.p);
            d.x.abs() <= REST && d.y.abs() <= REST && d.z.abs() <= REST
        });
        // (never while the weapon changes hands)
        if still && settled && g.in_hand == before.2 {
            self.pts = before.0;
            self.frame = before.1;
        }
    }

    /// Velocity first (drag, the servos' change, gravity, the cap), then
    /// position.
    fn integrate(&mut self, acc: &[V3; N]) {
        let cap_sq = CAP as i64 * CAP as i64;
        for (k, pt) in self.pts.iter_mut().enumerate() {
            let mut v = pt.vel();
            // limbs are damped by their muscles as well as the air
            let drag = if k == POMMEL || k == TIP {
                DRAG_PCT
            } else {
                LIMB_DRAG_PCT
            };
            v = v.sub(mul_div(v, drag as i64, 100));
            v = v.add(acc[k]);
            v.y -= GRAVITY;
            if sq(v) > cap_sq {
                v = v.clamp_len(CAP);
            }
            pt.q = pt.p;
            pt.p = pt.p.add(v);
        }
    }

    /// The relaxation passes can push a point past the cap after
    /// integration held it there; this holds next tick's speed to the cap by
    /// moving where it was (Vagrancy's `hold_to_cap`).
    fn hold_to_cap(&mut self) {
        let cap_sq = CAP as i64 * CAP as i64;
        for pt in &mut self.pts {
            let v = pt.vel();
            if sq(v) > cap_sq {
                pt.q = pt.p.sub(v.clamp_len(CAP));
            }
        }
    }

    /// One relaxation pass, in a fixed order: the arms from the shoulders
    /// out, the weapon, the hands on the hilt, then nothing inside the body
    /// and nothing under the ground.
    pub fn relax(&mut self) {
        for s in 0..2 {
            self.anchored(self.frame.shoulder[s], ELBOW[s], self.bones.0);
            self.stick(ELBOW[s], HAND[s], self.bones.1);
        }
        if self.in_hand {
            self.stick(POMMEL, TIP, self.weapon.len);
            for (s, hand) in HAND.into_iter().enumerate() {
                if self.hold[s] && self.weapon.grip[s] >= 0 {
                    self.pin(hand, self.weapon.grip[s]);
                }
            }
        }
        let f = self.frame;
        for k in [ELBOW[0], ELBOW[1], HAND[0], HAND[1], TIP] {
            if k == TIP && !self.in_hand {
                continue;
            }
            self.keep_out(k, f.spine[0], f.spine[1], f.chest_r + ONE / 25);
            if k == TIP {
                self.keep_out(k, f.head, f.head, f.head_r + ONE / 25);
            }
        }
        for pt in &mut self.pts {
            if pt.m > 0 && pt.p.y < f.ground + FLOOR {
                // the ground stops a point dead downward and gives it
                // nothing back upward: no bounce, no energy (D9)
                pt.p.y = f.ground + FLOOR;
                pt.q.y = pt.q.y.max(pt.p.y);
            }
        }
    }

    /// A stick from an anchor: the point alone moves.
    fn anchored(&mut self, anchor: V3, i: usize, len: Fx) {
        let d = self.pts[i].p.sub(anchor);
        let dist = d.len();
        if dist == 0 {
            return;
        }
        self.pts[i].p = anchor.add(mul_div(d, len as i64, dist as i64));
    }

    /// A stick between two points: an exact pair shift.
    pub fn stick(&mut self, a: usize, b: usize, len: Fx) {
        let d = self.pts[b].p.sub(self.pts[a].p);
        let dist = d.len();
        if dist == 0 {
            return;
        }
        let delta = mul_div(d, (dist - len) as i64, dist as i64);
        self.shift_pair(a, b, delta);
    }

    /// Move `i` by `+k·m_j` and `j` by `−k·m_i`, one integer `k` an axis.
    fn shift_pair(&mut self, i: usize, j: usize, d: V3) {
        let (mi, mj) = (self.pts[i].m, self.pts[j].m);
        let k = div(d, mi + mj);
        self.pts[i].p = self.pts[i].p.add(times(k, mj));
        self.pts[j].p = self.pts[j].p.sub(times(k, mi));
    }

    /// A hand held at `at` from the pommel along the weapon: the hand and
    /// the weapon's two points shift as a pair, the weapon as one.
    pub fn pin(&mut self, hand: usize, at: Fx) {
        let (a, b) = (self.pts[POMMEL].p, self.pts[TIP].p);
        let target = a.add(mul_div(b.sub(a), at as i64, self.weapon.len.max(1) as i64));
        let d = target.sub(self.pts[hand].p);
        let mh = self.pts[hand].m;
        let mg = self.pts[POMMEL].m + self.pts[TIP].m;
        let k = div(d, mh + mg);
        self.pts[hand].p = self.pts[hand].p.add(times(k, mg));
        for g in [POMMEL, TIP] {
            self.pts[g].p = self.pts[g].p.sub(times(k, mh));
        }
    }

    /// A point pushed out of the capsule `a`–`b`, `r` round.
    fn keep_out(&mut self, i: usize, a: V3, b: V3, r: Fx) {
        let p = self.pts[i].p;
        let c = nearest_on(a, b, p);
        let d = p.sub(c);
        let l = d.len();
        if l >= r {
            return;
        }
        let out = if l == 0 {
            v3(0, 0, ONE)
        } else {
            mul_div(d, ONE as i64, l as i64)
        };
        // pushed out with where it was, so the push adds no speed, and its
        // speed into the body taken away: the body stops it, inelastically
        let pt = &mut self.pts[i];
        let moved = c.add(out.scale(r)).sub(pt.p);
        pt.p = pt.p.add(moved);
        pt.q = pt.q.add(moved);
        let into = pt.vel().dot(out);
        if into < 0 {
            pt.q = pt.q.add(out.scale(into));
        }
    }

    /// Σ m·v, each axis.
    pub fn momentum(&self) -> (i64, i64, i64) {
        self.pts.iter().fold((0, 0, 0), |(x, y, z), p| {
            let v = p.vel();
            (
                x + p.m as i64 * v.x as i64,
                y + p.m as i64 * v.y as i64,
                z + p.m as i64 * v.z as i64,
            )
        })
    }

    /// Σ m·v², raw units squared times mass.
    pub fn kinetic(&self) -> i64 {
        self.pts.iter().map(|p| p.m as i64 * sq(p.vel())).sum()
    }
}

fn sq(v: V3) -> i64 {
    v.x as i64 * v.x as i64 + v.y as i64 * v.y as i64 + v.z as i64 * v.z as i64
}

/// The point of segment `a`–`b` nearest `p`.
pub fn nearest_on(a: V3, b: V3, p: V3) -> V3 {
    let ab = b.sub(a);
    let den = sq(ab);
    if den == 0 {
        return a;
    }
    let ap = p.sub(a);
    let num = ap.x as i64 * ab.x as i64 + ap.y as i64 * ab.y as i64 + ap.z as i64 * ab.z as i64;
    let t = (num as i128 * ONE as i128 / den as i128).clamp(0, ONE as i128) as i64;
    a.add(mul_div(ab, t, ONE as i64))
}

/// The servo's change of velocity this tick for a point and its target: the
/// speed it asks for is `GAIN` of the distance, at most `speed`; the change
/// toward that speed is at most `accel`.
pub fn servo(pt: &Point, target: V3, speed: Fx, accel: Fx) -> V3 {
    let want = target.sub(pt.p).scale(GAIN).clamp_len(speed);
    want.sub(pt.vel()).clamp_len(accel)
}

/// Step 4 of the tick: every fighter's sword state moves on, the mouse moves
/// its hand's target, and its arms are pulled, moved and relaxed.
pub fn step(w: &mut World, input: &Input, foes: &[Input]) {
    for id in 0..player::count(w) {
        let inp = if id == 0 { *input } else { foes[id - 1] };
        // the shell takes the mouse unless it is looking
        let f = player::fighter(w, id);
        if f.sheath == Sheath::Drawn && !f.mouse_looks && !f.health.down() {
            // the player's hand moves at the reader's setting (M4's
            // mouse sensitivity, percent); a pilot's at its own
            let pct = if id == 0 { w.ui.settings.hand_pct } else { 100 };
            let (shell, over) = f.arm.shell.moved(
                Mouse {
                    dx: (inp.dx as i64 * pct as i64 / 100) as i32,
                    dy: (inp.dy as i64 * pct as i64 / 100) as i32,
                    wheel: inp.wheel,
                },
                f.arm.reach(),
            );
            let free = f.engaged.is_none();
            player::parts_mut(w, id).1.arm.shell = shell;
            if id == 0 && free && over != 0 {
                // pushed past the side edge, holding on no one: the view
                // turns, and the body after it
                w.aim.yaw = fx::wrap(w.aim.yaw - over);
            }
        }
        let (sheath, goals) = player::goals(w, id, &inp);
        let b = *player::body(w, id);
        let f = player::fighter(w, id);
        let frame = player::frame_of(
            &b,
            &f.build(),
            &f.gait,
            f.health.fall(),
            w.ground(b.pos.x, b.pos.z),
        );
        let f = player::parts_mut(w, id).1;
        f.sheath = sheath;
        f.arm.step(&frame, &goals);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::int;
    use crate::player::{Body, Fighter};
    use crate::pose::Kind;

    fn frame() -> Frame {
        let b = Body::new(v3(0, 0, 0), 0);
        player::frame_of(&b, &crate::pose::build(Kind::Player), &Default::default(), 0, 0)
    }

    fn arm(item: Held) -> Arm {
        let f = Fighter::new(Kind::Player, item, 0, V3::ZERO, 0, true);
        f.arm
    }

    fn goals(a: &Arm, target: V3, swing: bool) -> Goals {
        let dir = blade_dir(0, &a.shell);
        Goals {
            hand: [None, Some(target)],
            tip: Some(target.add(dir.scale(a.weapon.len - a.weapon.grip[1]))),
            strength: [100, 100],
            swing,
            in_hand: true,
            hold: [a.weapon.two_handed(), true],
            sheath: (V3::ZERO, V3::ZERO),
            target: Some(target),
        }
    }

    #[test]
    fn a_stick_and_a_pin_move_no_momentum() {
        // two points 1 m apart on a 0.6 m stick, masses 2 and 3: the stick
        // pulls each in, and m₁Δ₁ + m₂Δ₂ is zero to the unit on every axis
        let mut a = arm(Held::Sword);
        a.pts[ELBOW[0]] = Point::at(v3(0, int(1), 0), 2);
        a.pts[TIP] = Point::at(v3(ONE * 3 / 10, ONE * 4 / 10, -ONE * 7 / 10), 3);
        let before = a.pts;
        a.stick(ELBOW[0], TIP, ONE * 6 / 10);
        let moved = |k: usize, m: i64| {
            let d = a.pts[k].p.sub(before[k].p);
            (m * d.x as i64, m * d.y as i64, m * d.z as i64)
        };
        let (e, t) = (moved(ELBOW[0], 2), moved(TIP, 3));
        assert_eq!((e.0 + t.0, e.1 + t.1, e.2 + t.2), (0, 0, 0));
        assert!(a.pts[ELBOW[0]].p.dist(a.pts[TIP].p) - ONE * 6 / 10 <= 8);
        // a hand pinned to the hilt: the hand (2) against the weapon (3 + 3)
        let mut b = arm(Held::Sword);
        b.pts[HAND[1]].p = b.pts[HAND[1]].p.add(v3(ONE / 10, -ONE / 20, ONE / 7));
        let before = b.pts;
        b.pin(HAND[1], b.weapon.grip[1]);
        let mut sum = (0i64, 0i64, 0i64);
        for k in [HAND[1], POMMEL, TIP] {
            let d = b.pts[k].p.sub(before[k].p);
            let m = b.pts[k].m as i64;
            sum = (
                sum.0 + m * d.x as i64,
                sum.1 + m * d.y as i64,
                sum.2 + m * d.z as i64,
            );
        }
        assert_eq!(sum, (0, 0, 0));
        assert!(
            b.grip_point().dist(b.pts[HAND[1]].p) <= 64,
            "the hand is on the hilt"
        );
    }

    #[test]
    fn nothing_gains_energy_without_a_servo() {
        // limp arms (no target), a sword in both hands, set whirling; with
        // gravity on, kinetic energy plus height may only fall: sticks and
        // pins take energy out, drag takes more, and nothing puts any in
        let mut a = arm(Held::Sword);
        for (k, pt) in a.pts.iter_mut().enumerate() {
            let spin = v3(ONE / 9 * (k as i32 % 3 - 1), ONE / 25, ONE / 11 * (k as i32 % 2));
            pt.q = pt.p.sub(spin);
        }
        let limp = Goals {
            hand: [None, None],
            tip: None,
            strength: [100, 100],
            swing: false,
            in_hand: true,
            hold: [true, true],
            sheath: (V3::ZERO, V3::ZERO),
            target: None,
        };
        let f = frame();
        // energy as m·v² / 2 + m·g·h, in units² of a tick
        let energy = |a: &Arm| -> i64 {
            a.kinetic() / 2
                + a.pts
                    .iter()
                    .map(|p| p.m as i64 * GRAVITY as i64 * p.p.y as i64)
                    .sum::<i64>()
        };
        let start = energy(&a);
        let mut most = start;
        for _ in 0..240 {
            a.step(&f, &limp);
            most = most.max(energy(&a));
        }
        // Verlet's energy rocks a little tick to tick; it never climbs past
        // where it began, and drag has taken most of it by the end
        let end = energy(&a);
        let floor = a
            .pts
            .iter()
            .map(|p| p.m as i64 * GRAVITY as i64 * FLOOR as i64)
            .sum::<i64>();
        assert!(most <= start, "energy rose from {start} to {most}");
        assert!(end - floor < (start - floor) * 3 / 4, "from {start} to {end}");
    }

    #[test]
    fn no_point_ever_moves_faster_than_the_cap() {
        let mut a = arm(Held::ShortSword);
        let f = frame();
        // the tip flung at 60 m/s: the next tick holds it to 32 m/s, 32 / 60
        // m a tick, 34 952 units
        a.pts[TIP].q = a.pts[TIP].p.sub(v3(ONE, 0, 0));
        a.step(&f, &goals(&a, a.pts[HAND[1]].p, false));
        assert!(a.pts[TIP].vel().len() <= 34_952 + 2, "{}", a.pts[TIP].vel().len());
        for t in 0..200 {
            // the target flung from side to side every 10 ticks, swinging
            let x = if (t / 10) % 2 == 0 { ONE } else { -ONE };
            a.step(&f, &goals(&a, v3(x, int(1), -ONE / 2), true));
            for p in &a.pts {
                assert!(p.vel().len() <= 34_952 + 2, "tick {t}: {}", p.vel().len());
            }
        }
    }

    #[test]
    fn the_servo_is_capped_in_speed_and_in_change_of_speed() {
        // a point at rest, a target 2 m off: it asks 35% of 2 m = 0.7 m a
        // tick, held to 6.5 m/s (7 099 a tick), and the first tick changes
        // its speed by 0.9 m/s (983) alone
        let p = Point::at(V3::ZERO, 2);
        let dv = servo(&p, v3(int(2), 0, 0), HAND_SPEED, HAND_ACCEL);
        // (983.04 held as 983; shortening a vector to it rounds a unit down)
        assert_eq!(dv, v3(982, 0, 0));
        // moving at 7 099 already, toward it: no change
        let moving = Point {
            p: V3::ZERO,
            q: v3(-7099, 0, 0),
            m: 2,
        };
        let dv = servo(&moving, v3(int(2), 0, 0), HAND_SPEED, HAND_ACCEL);
        assert!(dv.len() <= 1, "{dv:?}");
    }

    /// The shell's target for `side` and `pitch` (degrees) at the guard's
    /// reach, for the arm `a` at the origin facing north.
    fn on_shell(a: &Arm, side: i32, pitch: i32) -> V3 {
        let s = Shell {
            side: deg(side),
            pitch: deg(pitch),
            ..a.shell
        };
        s.point(a.frame.shoulder[1], 0)
    }

    /// From a guard on the right to a guard on the left, 90° across the
    /// shell: the short sword's hand gets there in a fraction of a second,
    /// the cleaver's later, its 8 at the tip weighing on both hands; and
    /// the cleaver carries on past where it was sent.
    #[test]
    fn a_heavy_weapon_starts_late_and_carries_past() {
        let ticks_to = |item: Held| {
            let mut a = arm(item);
            let f = frame();
            let from = on_shell(&a, 45, -10);
            for _ in 0..90 {
                a.step(&f, &goals(&a, from, false));
            }
            let to = on_shell(&a, -45, -10);
            let mut reached = None;
            let mut past = 0;
            for t in 0..120 {
                a.step(&f, &goals(&a, to, false));
                let x = a.pts[HAND[1]].p.x;
                if reached.is_none() && x <= to.x + ONE / 25 {
                    reached = Some(t);
                }
                past = past.max(to.x - x);
            }
            (reached.unwrap_or(999), past)
        };
        let (short, short_past) = ticks_to(Held::ShortSword);
        let (heavy, heavy_past) = ticks_to(Held::Cleaver);
        assert!(short <= 20, "short sword: {short} ticks");
        assert!(heavy >= short + 2, "cleaver {heavy} ticks, short sword {short}");
        assert!(
            heavy_past > short_past,
            "past by {heavy_past} against {short_past}"
        );
    }

    #[test]
    fn a_committed_swing_moves_the_hand_faster() {
        let speed = |swing: bool| {
            let mut a = arm(Held::Sword);
            let f = frame();
            let from = on_shell(&a, 20, 70);
            for _ in 0..90 {
                a.step(&f, &goals(&a, from, false));
            }
            let to = on_shell(&a, -15, -40);
            let mut top = 0;
            for _ in 0..30 {
                a.step(&f, &goals(&a, to, swing));
                top = top.max(a.pts[HAND[1]].vel().len());
            }
            top
        };
        let (plain, swing) = (speed(false), speed(true));
        assert!(swing > plain * 5 / 4, "swing {swing} against {plain}");
        // the plain servo never asks more than 6.5 m/s of the hand
        assert!(plain <= HAND_SPEED + HAND_ACCEL, "{plain}");
    }

    #[test]
    fn both_hands_stay_on_a_two_handed_hilt_and_the_bones_keep_their_lengths() {
        let mut a = arm(Held::Sword);
        let f = frame();
        for t in 0..120 {
            let x = if (t / 20) % 2 == 0 { ONE / 2 } else { -ONE / 2 };
            a.step(&f, &goals(&a, v3(x, ONE * 13 / 10, -ONE / 2), t % 40 < 20));
            let (p, tip) = (a.pts[POMMEL].p, a.pts[TIP].p);
            for (s, hand) in HAND.into_iter().enumerate() {
                let hold = p.add(mul_div(tip.sub(p), a.weapon.grip[s] as i64, a.weapon.len as i64));
                assert!(
                    hold.dist(a.pts[hand].p) < ONE / 50,
                    "tick {t}: hand {s} off the hilt by {}",
                    hold.dist(a.pts[hand].p)
                );
            }
            assert!((p.dist(tip) - a.weapon.len).abs() < ONE / 100);
            let fore = a.pts[ELBOW[1]].p.dist(a.pts[HAND[1]].p);
            assert!((fore - a.bones.1).abs() < ONE / 50, "tick {t}: forearm {fore}");
        }
    }

    #[test]
    fn the_mouse_moves_the_shell_target_and_a_pilot_can_send_the_movement_back() {
        let s = Shell::guard(int(1) / 2 + ONE / 10);
        // 100 px right: 15° (2 730 units); 100 px down: 15° lower
        let (m, over) = s.moved(
            Mouse {
                dx: 10_000,
                dy: 10_000,
                wheel: 0,
            },
            int(1),
        );
        assert_eq!((m.side - s.side, m.pitch - s.pitch, over), (2730, -2730, 0));
        let (dx, dy) = mouse_for(2730, -2730);
        assert_eq!((dx, dy), (10_000, 10_000));
        // far right: held at the edge, and the rest is the overflow
        let (m, over) = s.moved(
            Mouse {
                dx: 200_000,
                dy: 0,
                wheel: 0,
            },
            int(1),
        );
        assert_eq!(m.side, SIDE_MAX);
        assert_eq!(over, s.side + 20 * 2730 - SIDE_MAX);
        // a notch of the wheel toward the reader draws the hand in 3 cm
        let (m, _) = s.moved(
            Mouse {
                dx: 0,
                dy: 0,
                wheel: 10_000,
            },
            int(1),
        );
        assert_eq!(s.reach - m.reach, ONE * 3 / 100);
    }

    #[test]
    fn the_blade_points_ahead_from_the_guard_out_to_the_side_and_across_a_high_guard() {
        let at = |side: i32, pitch: i32| {
            blade_dir(
                0,
                &Shell {
                    side,
                    pitch,
                    reach: ONE / 2,
                    rest: (0, 0),
                },
            )
        };
        let guard = at(0, -deg(14));
        // 40° + 70% of −14° = 30.2° up: sin 30.2° = 0.503, 32 971 units
        assert!(
            (guard.y - 32_971).abs() < 40 && guard.z < -ONE * 4 / 5,
            "ahead, a little up: {guard:?}"
        );
        let right = at(deg(70), 0);
        assert!(right.x > ONE / 2, "out to the right: {right:?}");
        let high = at(0, deg(70));
        assert!(
            high.x < -ONE * 3 / 4 && high.y.abs() < ONE / 2,
            "across to the left: {high:?}"
        );
    }
}
