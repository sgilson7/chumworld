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
/// An elbow's and a hand's ball, millimetres on the player (M3's figure).
pub const ELBOW_MM: i32 = 57;
pub const HAND_MM: i32 = 64;
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
/// The blade's line for a hand on the shell (`blade_dir`): raised this far,
/// plus this share of the hand's pitch, and fanned this much wider than the
/// hand's side.
pub const BLADE_RISE: i32 = deg(30);
pub const BLADE_FOLLOW_PCT: i32 = 70;
pub const BLADE_FAN_PCT: i32 = 125;
/// Out to the side, or swept sideways, the rise falls by this share: a side
/// cut lies flat. Fully at `SIDE_FLAT` out, or at `SWING_FLAT` a tick of
/// the hand's sideways movement (eased, `Shell::swing`).
pub const FLAT_PCT: i32 = 75;
pub const SIDE_FLAT: i32 = deg(70);
pub const SWING_FLAT: i32 = deg(5);
/// Above `HIGH_FROM` the blade rises this share faster than the hand: raised
/// high, it stands up and back over the head, ready to come down.
pub const HIGH_FROM: i32 = deg(28);
pub const OVER_PCT: i32 = 60;
/// Raised high and to the left, it lies across above the head instead (a
/// high guard against a blow from above), fully by `HIGH_FULL` up and
/// `LEFT_FULL` left.
pub const HIGH_FULL: i32 = deg(50);
pub const LEFT_FULL: i32 = -deg(25);
/// Drawn in, reached out past the guard, or moving in or out, the blade
/// comes near level, at this share of the hand's pitch: a thrust's line.
/// Fully at the shell's nearest and farthest, or at `THRUST_FLAT` a tick of
/// the hand's reach changing.
pub const CHAMBER_PCT: i32 = 50;
pub const THRUST_FLAT: Fx = ONE * 15 / 1000;

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

/// One solid line of a weapon, as M3 draws it (`models/held.rs`): `across`
/// its width from the pommel-to-tip line (along the item's right, as near
/// the world's up as stays at right angles to the blade: `models::along`),
/// from `from` along it to the tip, and `r` thick. Against another body
/// the whole rail is solid; against its own, only from `own_from` (the
/// guard), since the hands that hold the hilt are against the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rail {
    pub across: Fx,
    pub from: Fx,
    pub own_from: Fx,
    pub r: Fx,
}

/// Millimetres to `fx` metres.
const fn mm(v: i32) -> Fx {
    (v as i64 * ONE as i64 / 1000) as Fx
}

/// Each weapon's rails. A sword's blade is one, 46 mm (54 mm for the
/// sword) wide on its line, its guard ending 106 mm past the grip (138 mm
/// for the sword). The cleaver's slab hangs off the haft's line from 150 mm
/// past the grip, from 20 mm one side of it to 194 mm the other (its edge),
/// 13 mm thick: three rails across it and the haft's own.
pub fn rails(item: Held) -> &'static [Rail] {
    const SHORT: [Rail; 1] = [Rail {
        across: 0,
        from: 0,
        own_from: cm(8) + mm(106),
        r: mm(23),
    }];
    const SWORD: [Rail; 1] = [Rail {
        across: 0,
        from: 0,
        own_from: cm(16) + mm(138),
        r: mm(27),
    }];
    const CLEAVER: [Rail; 4] = [
        Rail {
            across: 0,
            from: 0,
            own_from: cm(17) + mm(150),
            r: mm(22),
        },
        Rail {
            across: mm(40),
            from: cm(17) + mm(150),
            own_from: cm(17) + mm(150),
            r: mm(34),
        },
        Rail {
            across: mm(105),
            from: cm(17) + mm(150),
            own_from: cm(17) + mm(150),
            r: mm(34),
        },
        Rail {
            across: mm(168),
            from: cm(17) + mm(150),
            own_from: cm(17) + mm(150),
            r: mm(30),
        },
    ];
    match item {
        Held::Sword => &SWORD,
        Held::Cleaver => &CLEAVER,
        _ => &SHORT,
    }
}

/// A capsule of a body an arm keeps its weapon and hands out of: another
/// fighter's every part, or its own head, torso and legs (`own`: only the
/// blade from the guard keeps out of those, and the hands out of the head).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fence {
    pub a: V3,
    pub b: V3,
    pub r: Fx,
    pub own: bool,
    pub head: bool,
    /// the weapon keeps out of it too: always its own body; another body
    /// only once the blades' sweep has found what each blade met
    /// (`blade::step`), so a blow lands at the speed it was swung
    pub blade: bool,
    /// where the body it belongs to stands: the way out of its trunk across
    /// the ground, for a part pressed between two of its capsules; none for
    /// an arm held out more level than upright, whose own way out is across
    /// it
    pub axis: Option<V3>,
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
    /// how fast the target moves across and up (angle units a tick) and in
    /// and out (metres a tick), eased (two thirds of it kept each tick, so
    /// it comes to exactly nothing): the mouse's path, which lays a sweep
    /// flat and holds a thrust to its line
    pub swing: (i32, i32, Fx),
}

impl Shell {
    pub fn guard(arm: Fx) -> Shell {
        Shell {
            side: GUARD_SIDE,
            pitch: GUARD_PITCH,
            reach: arm * GUARD_REACH_PCT / 100,
            rest: (0, 0),
            swing: (0, 0, 0),
        }
    }

    /// The swing eased one tick toward a movement of `d`.
    pub fn eased(swing: (i32, i32, Fx), d: (i32, i32, Fx)) -> (i32, i32, Fx) {
        let e = |a: i32, b: i32| (a * 2 + b) / 3;
        (e(swing.0, d.0), e(swing.1, d.1), e(swing.2, d.2))
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
        let reach = reach.clamp(arm * REACH_MIN_PCT / 100, arm * REACH_MAX_PCT / 100);
        let pitch = (self.pitch - dpitch).clamp(PITCH_MIN, PITCH_MAX);
        let moved = (clamped - self.side, pitch - self.pitch, reach - self.reach);
        (
            Shell {
                side: clamped,
                pitch,
                reach,
                rest: (rx, ry),
                swing: Shell::eased(self.swing, moved),
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

/// `x` from `a` (0) to `b` (`ONE`), held between.
fn ramp(x: i32, a: i32, b: i32) -> Fx {
    if a == b {
        return 0;
    }
    ((x - a) as i64 * ONE as i64 / (b - a) as i64).clamp(0, ONE as i64) as Fx
}

/// The line the blade is asked to hold for a hand at `shell` on an arm
/// `arm` long, the body facing `yaw`, so each cut reads from the mouse's
/// path. At the guard the blade points a little up, at a foe's face, fanned
/// out with the hand. Out to the side, or swept sideways, it lies flat: a
/// side cut. Raised, it stands up and back over the head, and brought down
/// it falls through the upright plane in front: an overhead. Raised and to
/// the left, it lies across above the head: the high guard. Drawn in it is
/// near level, and reached out it lies along the arm: a thrust.
pub fn blade_dir(yaw: i32, shell: &Shell, arm: Fx) -> V3 {
    let flat = ramp(shell.side.abs(), 0, SIDE_FLAT).max(ramp(shell.swing.0.abs(), 0, SWING_FLAT));
    let rise = BLADE_RISE - fx::mul(BLADE_RISE * FLAT_PCT / 100, flat);
    let mut up =
        rise + shell.pitch * BLADE_FOLLOW_PCT / 100 + (shell.pitch - HIGH_FROM).max(0) * OVER_PCT / 100;
    let at = |pct: i32| arm * pct / 100;
    let guard = at(GUARD_REACH_PCT);
    let line = ramp(shell.reach, guard, at(REACH_MAX_PCT))
        .max(ramp(shell.reach, guard, at(REACH_MIN_PCT)))
        .max(ramp(shell.swing.2.abs(), 0, THRUST_FLAT));
    up = fx::lerp(up, shell.pitch * CHAMBER_PCT / 100, line);
    let mid = facing(yaw - shell.side * BLADE_FAN_PCT / 100, up);
    let top = facing(yaw + deg(80), deg(10)).add(facing(yaw, 0).scale(ONE / 3));
    let high = fx::mul(
        ramp(shell.pitch, HIGH_FROM, HIGH_FULL),
        ramp(shell.side, 0, LEFT_FULL),
    );
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
    /// the weapon sliding in or out of its scabbard: where the scabbard puts
    /// its pommel and tip this tick, and how much of the way from where the
    /// arm had it (`ONE`: all); the sword hand holds on
    pub ride: Option<(V3, V3, Fx)>,
    /// the sword hand reaches for something (the hilt at the hip) at the
    /// servo's whole strength, not a resting arm's `REST_PCT`
    pub grasp: bool,
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
    /// the bodies near enough to meet this tick (`step` sets them)
    pub fences: Vec<Fence>,
    /// how round an elbow and a hand are, as M3 draws them
    pub joint_r: (Fx, Fx),
    /// the weapon slides along its scabbard this tick (`Goals::ride`)
    pub riding: bool,
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
            fences: Vec::new(),
            riding: false,
            joint_r: (
                (ELBOW_MM as i64 * build.height as i64 * build.girth_pct as i64 / 170_000) as Fx,
                (HAND_MM as i64 * build.height as i64 * build.girth_pct as i64 / 170_000) as Fx,
            ),
        }
    }

    /// The weapon in hand at the guard at once (a foe that spawns drawn).
    pub fn take_in_hand(&mut self, frame: &Frame) {
        let w = self.weapon;
        let hand = self.shell.point(frame.shoulder[1], frame.yaw);
        let dir = blade_dir(frame.yaw, &self.shell, self.reach());
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

    /// The point `at` from the pommel along the weapon.
    pub fn hilt_at(&self, at: Fx) -> V3 {
        let (a, b) = (self.pts[POMMEL].p, self.pts[TIP].p);
        a.add(mul_div(b.sub(a), at as i64, self.weapon.len.max(1) as i64))
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
                    let rest = if s == 1 && g.grasp { 100 } else { REST_PCT };
                    let ac = HAND_ACCEL * rest / 100;
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
        self.riding = g.in_hand && g.ride.is_some();
        if let (true, Some((pommel, tip, blend))) = (g.in_hand, g.ride) {
            // the scabbard guides the blade: it moves where the scabbard puts
            // it (from where the arm had it, by `blend`), with the speed of
            // that move, so it leaves the scabbard moving
            for (k, at) in [(POMMEL, pommel), (TIP, tip)] {
                let pt = &mut self.pts[k];
                pt.p = pt.p.lerp(at, blend);
            }
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
        if still && settled && g.in_hand == before.2 && !self.riding {
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
            if !self.riding {
                self.stick(POMMEL, TIP, self.weapon.len);
            }
            for (s, hand) in HAND.into_iter().enumerate() {
                if self.hold[s] && self.weapon.grip[s] >= 0 {
                    if self.riding {
                        // the scabbard holds the weapon; the hand follows it
                        self.pts[hand].p = self.hilt_at(self.weapon.grip[s]);
                    } else {
                        self.pin(hand, self.weapon.grip[s]);
                    }
                }
            }
        }
        let f = self.frame;
        for k in [ELBOW[0], ELBOW[1], HAND[0], HAND[1], TIP] {
            if k == TIP && (!self.in_hand || self.riding) {
                continue;
            }
            self.keep_out(k, f.spine[0], f.spine[1], f.chest_r + ONE / 25);
            if k == TIP {
                self.keep_out(k, f.head, f.head, f.head_r + ONE / 25);
            }
        }
        self.fence();
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

    /// The weapon's right, across its width, as M3 draws it (`models::along`
    /// from the grip to the tip, the world's up for a hint), now or a tick
    /// ago.
    pub fn across(&self, prev: bool) -> V3 {
        let (g, t) = if prev {
            (self.grip_was(), self.pts[TIP].q)
        } else {
            (self.grip_point(), self.pts[TIP].p)
        };
        crate::models::along(g, t, V3::UP).c0
    }

    /// A rail's segment (against its own body, `own`), now or a tick ago,
    /// and the share of the way from the pommel to the tip each end is at.
    pub fn rail(&self, r: &Rail, own: bool, prev: bool) -> (V3, V3, Fx) {
        self.rail_at(r, own, prev, self.across(prev))
    }

    /// The same, with the weapon's right already found.
    fn rail_at(&self, r: &Rail, own: bool, prev: bool, right: V3) -> (V3, V3, Fx) {
        let (p, t) = if prev {
            (self.pts[POMMEL].q, self.pts[TIP].q)
        } else {
            (self.pts[POMMEL].p, self.pts[TIP].p)
        };
        let side = right.scale(r.across);
        let from = if own { r.own_from } else { r.from };
        let len = self.weapon.len.max(1) as i64;
        let a = p.add(mul_div(t.sub(p), from as i64, len)).add(side);
        (a, t.add(side), (from as i64 * ONE as i64 / len) as Fx)
    }

    /// Every hand, elbow and the weapon's rails kept out of the fences: each
    /// pushed back the way it came (out of the side of the fence it was on a
    /// tick ago, so a fast blade is never pushed through to the far side),
    /// with where it was moved alike so the push adds no speed, and its
    /// speed into the fence taken away (Vagrancy's D9: the body stops it,
    /// inelastically). A weapon riding in its scabbard is the scabbard's.
    fn fence(&mut self) {
        if self.fences.is_empty() {
            return;
        }
        let fences = std::mem::take(&mut self.fences);
        let mut rails = self.rail_lines();
        // rounds until nothing moves (at most `FENCE_ROUNDS`), so a hand or a
        // blade pressed between two parts of a body comes out of both; from
        // halfway, still in, it goes straight out across the ground from the
        // body's upright axis, which leads out of all of its parts at once
        for round in 0..FENCE_ROUNDS {
            let away = round >= FENCE_ROUNDS / 2;
            let mut moved = false;
            for f in &fences {
                if !f.own || f.head {
                    for (k, r) in [
                        (ELBOW[0], self.joint_r.0),
                        (ELBOW[1], self.joint_r.0),
                        (HAND[0], self.joint_r.1),
                        (HAND[1], self.joint_r.1),
                    ] {
                        if f.own && k != HAND[0] && k != HAND[1] {
                            continue;
                        }
                        let (p, q) = (self.pts[k].p, self.pts[k].q);
                        if let Some(d) = push_out(p, q, f, r, away) {
                            let pt = &mut self.pts[k];
                            pt.p = pt.p.add(d.0);
                            pt.q = pt.q.add(d.0);
                            let into = pt.vel().dot(d.1);
                            if into < 0 {
                                pt.q = pt.q.add(d.1.scale(into));
                            }
                            moved = true;
                        }
                    }
                }
                for k in 0..rails.len() {
                    if !f.blade {
                        break;
                    }
                    let (rail, now, was) = rails[k];
                    let (a, b, t0) = now[f.own as usize];
                    let (a0, b0, _) = was[f.own as usize];
                    let Some((d, out, s)) = push_rail(a, b, a0, b0, f, rail.r, away) else {
                        continue;
                    };
                    // the share of the way along the weapon the push lands at
                    let t = t0 + fx::mul(ONE - t0, s);
                    self.shove(t, d, out);
                    rails = self.rail_lines();
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        self.fences = fences;
    }

    /// Each rail now and a tick ago, against another body and its own; none
    /// for a weapon in its scabbard.
    #[allow(clippy::type_complexity)]
    fn rail_lines(&self) -> Vec<(Rail, [(V3, V3, Fx); 2], [(V3, V3, Fx); 2])> {
        if !self.in_hand || self.riding {
            return Vec::new();
        }
        let (now, was) = (self.across(false), self.across(true));
        rails(self.weapon.item)
            .iter()
            .map(|r| {
                let at = |own| (self.rail_at(r, own, false, now), self.rail_at(r, own, true, was));
                let (a, a0) = at(false);
                let (b, b0) = at(true);
                (*r, [a, b], [a0, b0])
            })
            .collect()
    }

    /// The weapon moved so its point `t` of the way from the pommel to the
    /// tip moves by `d`, as a turn and a slide that move it least: the
    /// pommel by `(1 − t)·k·d`, the tip by `t·k·d`, `k = 1 / ((1 − t)² + t²)`.
    /// Where it was moves alike (no speed added), and the speed of that
    /// point along `out` into the fence is taken away.
    fn shove(&mut self, t: Fx, d: V3, out: V3) {
        let t = t.clamp(0, ONE) as i64;
        let u = ONE as i64 - t;
        let den = ((u * u + t * t) >> 16).max(1);
        let wp = (u * ONE as i64 / den) as Fx;
        let wt = (t * ONE as i64 / den) as Fx;
        let v = self.pts[POMMEL]
            .vel()
            .scale(ONE - t as Fx)
            .add(self.pts[TIP].vel().scale(t as Fx));
        let stop = out.scale(v.dot(out).min(0));
        for (k, w) in [(POMMEL, wp), (TIP, wt)] {
            let pt = &mut self.pts[k];
            let m = d.scale(w);
            pt.p = pt.p.add(m);
            pt.q = pt.q.add(m).add(stop.scale(w));
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

/// How far and which way to move a point (`p`, a tick ago `q`) of radius
/// `r` out of a fence: to the fence's surface on the side the point was a
/// tick ago, or (`away`) straight out across the ground from its body's
/// axis. `None` when it is clear.
fn push_out(p: V3, q: V3, f: &Fence, r: Fx, away: bool) -> Option<(V3, V3)> {
    let room = f.r + r;
    if !near_box(p, p, f, room) {
        return None;
    }
    let c = nearest_on(f.a, f.b, p);
    let d = p.sub(c);
    if sq(d) >= room as i64 * room as i64 {
        return None;
    }
    if let (true, Some(axis)) = (away, f.axis) {
        return Some(escape(p, d, axis, room));
    }
    let c0 = nearest_on(f.a, f.b, q);
    let was = q.sub(c0);
    let out = side(d, was);
    Some((c.add(out.scale(room)).sub(p), out))
}

/// The same for a rail `a`–`b` (a tick ago `a0`–`b0`): the move of its
/// nearest point, the way out, and how far along the rail that point is.
fn push_rail(a: V3, b: V3, a0: V3, b0: V3, f: &Fence, r: Fx, away: bool) -> Option<(V3, V3, Fx)> {
    let room = f.r + r;
    if !near_box(a, b, f, room) {
        return None;
    }
    let (s, _, pr, pf) = crate::blade::closest(a, b, f.a, f.b);
    let d = pr.sub(pf);
    if sq(d) >= room as i64 * room as i64 {
        return None;
    }
    if let (true, Some(axis)) = (away, f.axis) {
        let (m, out) = escape(pr, d, axis, room);
        return Some((m, out, s));
    }
    let (_, _, pr0, pf0) = crate::blade::closest(a0, b0, f.a, f.b);
    let out = side(d, pr0.sub(pf0));
    Some((pf.add(out.scale(room)).sub(pr), out, s))
}

/// A point `p`, `d` from the fence's nearest point and `room` needed, moved
/// straight out across the ground from the fence's body's axis: by the
/// sink, over the share of that way that leads out of this capsule (at
/// least a third), and a unit more.
fn escape(p: V3, d: V3, axis: V3, room: Fx) -> (V3, V3) {
    let flat = v3(p.x - axis.x, 0, p.z - axis.z);
    let out = if flat == V3::ZERO { side(d, d) } else { flat.norm() };
    let sink = room - d.len();
    let lead = if d == V3::ZERO {
        ONE
    } else {
        out.dot(d.norm()).max(ONE / 3)
    };
    (out.scale(fx::div(sink, lead) + 1), out)
}

/// The way out of a fence: the way the point is from it now, unless a tick
/// ago it was on the other side (it came through the middle), then that
/// side; straight up when there is neither.
fn side(now: V3, was: V3) -> V3 {
    let through = now.dot(was) < 0 || now == V3::ZERO;
    let pick = if through && was != V3::ZERO { was } else { now };
    if pick == V3::ZERO {
        V3::UP
    } else {
        pick.norm()
    }
}

/// Whether a segment's box, grown by `room`, meets a fence's.
fn near_box(a: V3, b: V3, f: &Fence, room: Fx) -> bool {
    let lo = |x: i32, y: i32, u: i32, v: i32| x.min(y) - room <= u.max(v);
    lo(a.x, b.x, f.a.x, f.b.x)
        && lo(f.a.x, f.b.x, a.x, b.x)
        && lo(a.y, b.y, f.a.y, f.b.y)
        && lo(f.a.y, f.b.y, a.y, b.y)
        && lo(a.z, b.z, f.a.z, f.b.z)
        && lo(f.a.z, f.b.z, a.z, b.z)
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
    // the bodies as they stand after the feet, each arm as it moves: each
    // arm keeps out of them
    let mut caps = all_capsules(w);
    for id in 0..player::count(w) {
        let inp = if id == 0 { *input } else { foes[id - 1] };
        move_shell(w, id, &inp);
        let (sheath, tilt, goals) = player::goals(w, id, &inp);
        let b = *player::body(w, id);
        let f = player::fighter(w, id);
        let frame = player::frame_of(
            &b,
            &f.build(),
            &f.gait,
            f.health.fall(),
            w.ground(b.pos.x, b.pos.z),
        );
        let near = fences_of(w, id, &caps, false);
        let f = player::parts_mut(w, id).1;
        f.sheath = sheath;
        f.tilt = tilt;
        f.arm.fences = near;
        f.arm.step(&frame, &goals);
        caps[id] = crate::blade::body(player::fighter(w, id).kind, &player::pose_of(w, id));
    }
}

/// Times each relaxation pass goes over the fences.
pub const FENCE_ROUNDS: usize = 8;
/// Fighter `id`'s mouse moves its hand's target on the shell, unless it is
/// looking; otherwise the target's swing eases to nothing.
pub fn move_shell(w: &mut World, id: usize, inp: &Input) {
    let f = player::fighter(w, id);
    if f.sheath == Sheath::Drawn && !f.mouse_looks && !f.health.down() {
        // the player's hand moves at the reader's setting (M4's mouse
        // sensitivity, percent); a pilot's at its own
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
            // pushed past the side edge, holding on no one: the view turns,
            // and the body after it
            w.aim.yaw = fx::wrap(w.aim.yaw - over);
        }
    } else {
        let s = &mut player::parts_mut(w, id).1.arm.shell;
        s.swing = Shell::eased(s.swing, (0, 0, 0));
    }
}

/// Bodies this far apart, feet to feet, cannot meet each other's arms.
pub const FENCE_RANGE: Fx = fx::int(4);

/// The fences each fighter's arm keeps out of: its own head, torso and
/// legs, and every part of each body within `FENCE_RANGE`, standing or
/// down, as they stand now (`blade::body`), by fighter.
pub fn fences(w: &World) -> Vec<Vec<Fence>> {
    let caps = all_capsules(w);
    (0..caps.len()).map(|id| fences_of(w, id, &caps, true)).collect()
}

/// Every body's capsules as it stands now.
pub fn all_capsules(w: &World) -> Vec<[crate::blade::Capsule; crate::blade::PARTS]> {
    (0..player::count(w))
        .map(|k| crate::blade::body(player::fighter(w, k).kind, &player::pose_of(w, k)))
        .collect()
}

/// The fences of fighter `id` among bodies standing as `caps`; the weapon
/// keeps out of other bodies only with `blades`.
pub fn fences_of(
    w: &World,
    id: usize,
    caps: &[[crate::blade::Capsule; crate::blade::PARTS]],
    blades: bool,
) -> Vec<Fence> {
    let me = player::body(w, id).pos;
    let mut out = Vec::new();
    for (k, caps) in caps.iter().enumerate() {
        let at = player::body(w, k).pos;
        if k != id && v3(at.x - me.x, 0, at.z - me.z).len() > FENCE_RANGE {
            continue;
        }
        for (n, c) in caps.iter().enumerate() {
            let own = k == id;
            if own && !crate::blade::TRUNK.contains(&n) {
                continue;
            }
            out.push(Fence {
                a: c.a,
                b: c.b,
                r: c.r,
                own,
                head: c.part == crate::blade::Part::Head,
                blade: own || blades,
                axis: (crate::blade::TRUNK.contains(&n) || {
                    let d = c.b.sub(c.a);
                    d.y.abs() >= d.len_xz()
                })
                .then_some(at),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::int;
    use crate::player::{Body, Fighter};
    use crate::pose::Kind;
    use crate::world::World;

    fn frame() -> Frame {
        let b = Body::new(v3(0, 0, 0), 0);
        player::frame_of(&b, &crate::pose::build(Kind::Player), &Default::default(), 0, 0)
    }

    fn arm(item: Held) -> Arm {
        let f = Fighter::new(Kind::Player, item, 0, V3::ZERO, 0, true);
        f.arm
    }

    fn goals(a: &Arm, target: V3, swing: bool) -> Goals {
        let dir = blade_dir(0, &a.shell, a.reach());
        Goals {
            hand: [None, Some(target)],
            tip: Some(target.add(dir.scale(a.weapon.len - a.weapon.grip[1]))),
            strength: [100, 100],
            swing,
            in_hand: true,
            hold: [a.weapon.two_handed(), true],
            sheath: (V3::ZERO, V3::ZERO),
            ride: None,
            grasp: false,
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
            ride: None,
            grasp: false,
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

    /// The player alone, the sword drawn and settled at the guard, facing
    /// north; then `path`, and the tip each tick in the body's own frame
    /// (right, up, ahead) from the feet.
    fn cut(path: &[Input]) -> Vec<V3> {
        let mut w = World::new(1);
        w.aim.yaw = 0;
        w.player.facing = 0;
        let draw = Input {
            keys: crate::input::DRAW,
            ..Input::default()
        };
        w.step(&draw);
        for _ in 0..60 {
            w.step(&Input::default());
        }
        path.iter()
            .map(|i| {
                w.step(i);
                let t = w.me.arm.pts[TIP].p.sub(w.player.pos);
                v3(t.x, t.y, -t.z)
            })
            .collect()
    }

    fn mouse(dx: i32, dy: i32, wheel: i32, swing: bool, ticks: usize) -> Vec<Input> {
        vec![
            Input {
                dx: dx * 100,
                dy: dy * 100,
                wheel,
                buttons: if swing { crate::input::SWING } else { 0 },
                ..Input::default()
            };
            ticks
        ]
    }

    /// Each cut reads from the mouse's path, the tip followed through the
    /// whole arm. Overhead: the mouse up 600 px, then down 900 px with the
    /// button: the tip comes from over 2 m up and behind the feet down through
    /// the space in front, more than 1.2 m, and keeps within 0.35 m of its
    /// line across. Side: the mouse right 450 px, then left 1 000 px: the
    /// tip crosses more than 1.3 m and keeps within 0.4 m of one height.
    /// Thrust: the wheel draws the hand in ten notches (as near as the shell
    /// goes), then out twelve (as far): the tip goes 0.25 m ahead or more,
    /// under 0.15 m across and under 0.25 m up or down.
    #[test]
    fn an_overhead_a_side_cut_and_a_thrust_each_read_from_the_mouse_s_path() {
        let span = |v: &[V3], f: fn(&V3) -> i32| {
            let (lo, hi) = (v.iter().map(f).min().unwrap(), v.iter().map(f).max().unwrap());
            hi - lo
        };
        // overhead
        let mut path = mouse(0, -60, 0, false, 10);
        path.extend(mouse(0, 0, 0, false, 30));
        let n = path.len();
        path.extend(mouse(0, 150, 0, true, 6));
        path.extend(mouse(0, 0, 0, true, 14));
        let tips = cut(&path);
        let (raised, chop) = (tips[n - 1], &tips[n..]);
        assert!(
            raised.y > fx::int(2) && raised.z < -ONE / 10,
            "raised above and behind: {raised:?}"
        );
        let low = chop.iter().map(|t| t.y).min().unwrap();
        let ahead = chop.iter().map(|t| t.z).max().unwrap();
        assert!(
            raised.y - low > ONE * 12 / 10,
            "fell {} from {raised:?}",
            raised.y - low
        );
        assert!(ahead > ONE, "came down in front: {ahead}");
        assert!(
            span(chop, |t| t.x) < ONE * 35 / 100,
            "kept to its line: {}",
            span(chop, |t| t.x)
        );
        // side
        let mut path = mouse(45, 0, 0, false, 10);
        path.extend(mouse(0, 0, 0, false, 30));
        let n = path.len();
        path.extend(mouse(-170, 0, 0, true, 6));
        path.extend(mouse(0, 0, 0, true, 20));
        let tips = cut(&path);
        let sweep = &tips[n - 1..];
        assert!(
            span(sweep, |t| t.x) > ONE * 13 / 10,
            "crossed {}",
            span(sweep, |t| t.x)
        );
        assert!(
            span(sweep, |t| t.y) < ONE * 4 / 10,
            "rose and fell {}",
            span(sweep, |t| t.y)
        );
        // thrust
        let mut path = mouse(0, 0, 10_000, false, 10);
        path.extend(mouse(0, 0, 0, false, 30));
        let n = path.len();
        path.extend(mouse(0, 0, -10_000, true, 12));
        path.extend(mouse(0, 0, 0, true, 10));
        let tips = cut(&path);
        let thrust = &tips[n - 1..];
        assert!(
            span(thrust, |t| t.z) > ONE / 4,
            "went {} ahead",
            span(thrust, |t| t.z)
        );
        assert!(
            span(thrust, |t| t.x) < ONE * 15 / 100,
            "moved {} across",
            span(thrust, |t| t.x)
        );
        assert!(
            span(thrust, |t| t.y) < ONE / 4,
            "rose and fell {}",
            span(thrust, |t| t.y)
        );
    }

    /// Worked by hand for an arm 1 m long, the hand at the guard's reach
    /// (82%) unless given: angles to sines through `fx::deg`'s units.
    #[test]
    fn the_blade_points_up_at_the_guard_flat_to_the_side_up_and_back_raised_and_along_a_thrust() {
        let at = |side: i32, pitch: i32, pct: i32, swing: i32| {
            blade_dir(
                0,
                &Shell {
                    side: deg(side),
                    pitch: deg(pitch),
                    reach: ONE * pct / 100,
                    rest: (0, 0),
                    swing: (deg(swing), 0, 0),
                },
                ONE,
            )
        };
        let near = |a: Fx, b: Fx| (a - b).abs() < 40;
        // the guard: 30° less 70% of 14° (5 461 − 1 783 units) = 20.2° up,
        // sin 20.2° = 0.345: 22 630
        let guard = at(0, -14, 82, 0);
        assert!(near(guard.y, 22_630) && guard.z < -ONE * 9 / 10, "{guard:?}");
        // 70° out: flat, 30° less 75% = 7.5° up, fanned to 87.5° right
        let right = at(70, 0, 82, 0);
        assert!(right.x > ONE * 9 / 10 && near(right.y, 8_554), "{right:?}");
        // swept sideways at 6° a tick, in front: flat the same, sin 7.5° =
        // 0.1305: 8 554
        let sweep = at(0, 0, 82, 6);
        assert!(near(sweep.y, 8_554), "{sweep:?}");
        // raised to 78°, 10° right: 30° less 75% of a seventh = 26.8°, plus
        // 70% of 78° (54.6°) and 60% of 50° (30°): 111.4°, up and back over
        // the head (cos 111.4° = −0.365 behind, sin = 0.931 up)
        let raised = at(10, 78, 82, 0);
        assert!(raised.y > ONE * 9 / 10 && raised.z > ONE * 3 / 10, "{raised:?}");
        // raised and to the left: across above the head, the high guard:
        // (facing 80° left, 10° up) + a third ahead, normalised to
        // (−0.876, 0.157, −0.455)
        let high = at(-30, 70, 82, 0);
        assert!(high.x < -ONE * 85 / 100 && high.y.abs() < ONE / 5, "{high:?}");
        // reached out all the way: on the thrust's line, half the hand's 6°
        // down (−1 092 units, −546)
        let thrust = at(0, -6, 97, 0);
        assert!(thrust.dist(facing(0, -546)) < 40, "{thrust:?}");
        // drawn in all the way: half the hand's 20° down, sin −10° = −0.174
        let drawn_in = at(0, -20, 45, 0);
        assert!(
            near(drawn_in.y, -11_380) && drawn_in.z < -ONE * 9 / 10,
            "{drawn_in:?}"
        );
    }
}
