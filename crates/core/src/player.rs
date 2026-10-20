//! The player's body, as M0 has it: a capsule that walks and runs on flat
//! ground, moved only by its `Input` (CW-3).
//!
//! W, A, S and D move relative to where the camera looks; Shift runs. The
//! body speeds up and slows down at `ACCEL`, so starting and stopping are
//! smooth, and turns to face the way it moves at `TURN_RATE`. M1's
//! `motion.rs` replaces this rule with feet, grip and ice (CW-8) and M3's
//! `models.rs` replaces the capsule with the player's model; both keep
//! `step` driven by the input record alone.

use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{facing, right_of, v3, yaw_of, Affine, V3};
use crate::input::{self, Input};
use crate::look::{rgb, Palette, Rgb};
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::ticks::TICKS_PER_SECOND;

/// The body's height, top of the head to the soles.
pub const HEIGHT: Fx = ONE * 18 / 10;
/// The body's width: the capsule's 0.32 m radius drawn at 78% across, about
/// a person's 0.5 m shoulders.
pub const SLIM_PCT: i32 = 78;
/// Walking and running speeds, metres a second.
pub const WALK: Fx = ONE * 22 / 10;
pub const RUN: Fx = fx::int(5);
/// How fast the body speeds up or slows down, metres a second each second.
pub const ACCEL: Fx = fx::int(18);
/// The most the body turns in a tick toward the way it moves (600° a second).
pub const TURN_RATE: i32 = deg(10);
/// The body leans forward with speed, this far at a run.
pub const LEAN_AT_RUN: i32 = deg(9);
/// The soft shadow under the body: its width.
pub const SHADOW_WIDTH: Fx = ONE * 12 / 10;
/// The shadow's height over the ground: above it by more than the depth
/// buffer needs at any distance the shadow is drawn from.
pub const SHADOW_LIFT: Fx = ONE * 2 / 100;

const TICKS: i32 = TICKS_PER_SECOND as i32;

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

    /// One tick: the movement keys, relative to the camera's heading `yaw`,
    /// and the ground's height under the feet.
    pub fn step(&mut self, input: &Input, yaw: i32, ground: Fx) {
        self.prev_pos = self.pos;
        self.prev_facing = self.facing;
        self.prev_vel = self.vel;
        let (x, z) = input.walk_axes();
        let way = right_of(yaw)
            .scale(x * ONE)
            .sub(facing(yaw, 0).scale(z * ONE))
            .norm();
        let speed = if input.key(input::RUN) { RUN } else { WALK };
        let target = way.scale(speed / TICKS);
        let change = target.sub(self.vel).clamp_len(ACCEL / (TICKS * TICKS));
        self.vel = self.vel.add(change);
        if self.vel.len() <= 1 {
            self.vel = V3::ZERO;
        }
        self.pos = self.pos.add(self.vel);
        self.pos.y = ground;
        if self.vel != V3::ZERO {
            let want = yaw_of(self.vel);
            let turn = fx::wrap(want - self.facing).clamp(-TURN_RATE, TURN_RATE);
            self.facing = fx::wrap(self.facing + turn);
        }
    }

    /// The feet, drawn `alpha` of the way from the last tick to this one.
    pub fn feet(&self, alpha: Fx) -> V3 {
        self.prev_pos.lerp(self.pos, alpha)
    }

    /// The capsule, its visor (which way it faces) and its soft shadow.
    pub fn draw(&self, alpha: Fx, palette: &Palette, ink: Rgb, out: &mut Shapes) {
        let feet = self.feet(alpha);
        let turn = Affine::rot_y(fx::lerp_angle(self.prev_facing, self.facing, alpha));
        // a lean toward where the body moves, from its feet, by its speed
        let vel = self.prev_vel.lerp(self.vel, alpha);
        let speed = vel.len() as i64 * TICKS as i64;
        let lean_by = (LEAN_AT_RUN as i64 * speed / RUN as i64).min(LEAN_AT_RUN as i64) as i32;
        let lean = Affine::rot_y(crate::geom::yaw_of(vel))
            .then(&Affine::rot_x(-lean_by))
            .then(&Affine::rot_y(-crate::geom::yaw_of(vel)));
        let turn = lean.then(&turn);
        let wide = HEIGHT * SLIM_PCT / 100;
        let body = Affine::translate(feet)
            .then(&turn)
            .then(&Affine::translate(v3(0, HEIGHT / 2, 0)))
            .then(&Affine::scale(v3(wide, HEIGHT, wide)));
        out.inked(Mesh::Capsule, body, rgb(palette.body), ink);
        // a dark band across the face, a little proud of the capsule's front
        let visor = Affine::translate(feet)
            .then(&turn)
            .then(&Affine::translate(v3(0, ONE * 156 / 100, -ONE * 19 / 100)))
            .then(&Affine::scale(v3(ONE * 28 / 100, ONE * 11 / 100, ONE * 14 / 100)));
        out.inked(Mesh::Sphere, visor, rgb(palette.visor), ink);
        out.shadow(
            feet.add(v3(0, SHADOW_LIFT, 0)),
            SHADOW_WIDTH,
            rgb(palette.shadow),
            ONE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hold(keys: u32) -> Input {
        Input {
            keys,
            ..Input::default()
        }
    }

    #[test]
    fn holding_w_walks_north_and_reaches_walking_pace_smoothly() {
        let mut b = Body::new(V3::ZERO, 0);
        for _ in 0..60 {
            b.step(&hold(input::FORWARD), 0, 0);
        }
        // a tick's pace: 2.2 m/s is 144 179 Q16, / 60 = 2 402 (2402.98
        // truncated); speeding up 18 m/s² / 3600 = 327 a tick: 327, 654, … 2289 over 7 ticks (sum
        // 327 × 28 = 9 156), then 53 ticks at 2 402 (127 306): 136 462 in all,
        // 2.08 m north (−z)
        assert!((b.pos.z + 136_462).abs() <= 60, "{}", b.pos.z);
        assert_eq!(b.pos.x, 0);
        assert_eq!(b.vel.z, -2402);
    }

    #[test]
    fn shift_runs_faster_than_walking() {
        let mut w = Body::new(V3::ZERO, 0);
        let mut r = Body::new(V3::ZERO, 0);
        for _ in 0..120 {
            w.step(&hold(input::FORWARD), 0, 0);
            r.step(&hold(input::FORWARD | input::RUN), 0, 0);
        }
        // 5 m/s / 60 = 5 461 a tick (5461.3 truncated)
        assert_eq!(r.vel.z, -5461);
        assert!(r.pos.z < w.pos.z * 2, "{} {}", r.pos.z, w.pos.z);
    }

    #[test]
    fn movement_is_relative_to_the_camera_heading() {
        // the camera turned a quarter left faces west (−x): W walks west, D
        // walks north (the camera's right)
        let mut b = Body::new(V3::ZERO, 0);
        for _ in 0..30 {
            b.step(&hold(input::FORWARD), fx::QUARTER_TURN, 0);
        }
        assert!(b.pos.x < -ONE / 2 && b.pos.z.abs() < ONE / 100, "{:?}", b.pos);
        let mut d = Body::new(V3::ZERO, 0);
        for _ in 0..30 {
            d.step(&hold(input::RIGHT), fx::QUARTER_TURN, 0);
        }
        assert!(d.pos.z < -ONE / 2 && d.pos.x.abs() < ONE / 100, "{:?}", d.pos);
    }

    #[test]
    fn letting_go_slows_to_a_stop_and_the_body_turns_to_face_its_way() {
        let mut b = Body::new(V3::ZERO, 0);
        for _ in 0..40 {
            b.step(&hold(input::LEFT), 0, 0);
        }
        // walking west: facing a quarter turn left, reached at 10° a tick
        assert_eq!(b.facing, fx::QUARTER_TURN);
        for _ in 0..20 {
            b.step(&Input::default(), 0, 0);
        }
        assert_eq!(b.vel, V3::ZERO);
        assert_eq!(b.prev_pos, b.pos, "a still body is drawn still");
    }

    #[test]
    fn the_body_is_drawn_between_ticks() {
        let mut b = Body::new(V3::ZERO, 0);
        b.pos = v3(ONE, 0, 0);
        assert_eq!(b.feet(ONE / 2), v3(ONE / 2, 0, 0));
    }
}
