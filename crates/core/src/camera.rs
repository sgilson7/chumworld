//! The camera: over the right shoulder, third person, always (CW-15, T1).
//!
//! The mouse turns the view (`Aim`); the camera sits on a pivot 0.5 m above
//! and 0.6 m right of the head, and 2.6 m back from it along the line of
//! sight, so looking down lifts it and looking up lowers it, never below
//! `MIN_EYE_HEIGHT` over the ground. Numbers from PLAN.md.
//!
//! The view a frame draws uses the mouse movement that has arrived since the
//! last tick (`input::Pending`) as if a tick had already taken it, so the
//! picture turns on the frame the mouse moves; the tick then takes the same
//! movement and the view does not jump.

use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{facing, right_of, v3, V3};
use crate::input::Mouse;
use crate::render::Camera;

/// Back from the pivot, along the line of sight.
pub const BACK: Fx = ONE * 26 / 10;
/// Right of the head.
pub const RIGHT: Fx = ONE * 6 / 10;
/// Above the head.
pub const ABOVE: Fx = ONE / 2;
/// The head's height over the feet (the top of a 1.8 m body is 10 cm above).
pub const HEAD: Fx = ONE * 17 / 10;
/// The vertical field of view.
pub const FOV: i32 = deg(60);
/// How far the view may look down and up.
pub const PITCH_MIN: i32 = -deg(55);
pub const PITCH_MAX: i32 = deg(35);
/// The pitch the game opens with: a little down, so the ground reads.
pub const PITCH_START: i32 = -deg(8);
/// The look speed a reader may set, percent.
pub const LOOK_PCT_MIN: i32 = 10;
pub const LOOK_PCT_MAX: i32 = 400;
/// The lowest the eye goes over the ground.
pub const MIN_EYE_HEIGHT: Fx = ONE * 3 / 10;
/// Turning: 0.12° a CSS pixel of mouse movement, held as angle units per
/// 100 pixels (12°, 2 184 units); the record carries hundredths of a pixel.
const LOOK_UNITS_PER_100_PX: i64 = fx::TURN as i64 * 12 / 360;

/// Where the player looks: heading and pitch, and the part of an angle unit
/// the mouse has moved that has not yet turned the view (so slow movement
/// is not rounded away tick by tick, and the same movement turns the view
/// the same amount however the ticks divide it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Aim {
    pub yaw: i32,
    pub pitch: i32,
    /// ten-thousandths of an angle unit not yet turned, each way
    pub rest: (i32, i32),
    /// the look speed, percent of `LOOK_UNITS_PER_100_PX` (a reader's
    /// setting, `act("look.speed", …)`)
    pub pct: i32,
}

impl Aim {
    pub fn new(yaw: i32) -> Aim {
        Aim {
            yaw,
            pitch: PITCH_START,
            rest: (0, 0),
            pct: 100,
        }
    }

    /// The aim after mouse movement `m` (right turns right, down looks
    /// down), the pitch held in its range.
    pub fn turned(self, m: Mouse) -> Aim {
        // ten-thousandths of a unit: hundredths of a pixel × units per 100 px
        let split = |d: i32, rest: i32| {
            let total = d as i64 * LOOK_UNITS_PER_100_PX * self.pct as i64 / 100 + rest as i64;
            (total.div_euclid(10_000) as i32, total.rem_euclid(10_000) as i32)
        };
        let (dyaw, rx) = split(m.dx, self.rest.0);
        let (dpitch, ry) = split(m.dy, self.rest.1);
        Aim {
            yaw: fx::wrap(self.yaw.wrapping_sub(dyaw)),
            pitch: (self.pitch - dpitch).clamp(PITCH_MIN, PITCH_MAX),
            rest: (rx, ry),
            pct: self.pct,
        }
    }
}

/// The camera for feet at `feet`, looking `look`, over ground at height
/// `ground` under the eye.
pub fn view(feet: V3, look: Aim, ground: impl Fn(Fx, Fx) -> Fx) -> Camera {
    view_from(feet, look, BACK, ground)
}

/// The same, `back` from the pivot instead of `BACK`: the title screen's wide
/// view, and the ease from it into play.
pub fn view_from(feet: V3, look: Aim, back: Fx, ground: impl Fn(Fx, Fx) -> Fx) -> Camera {
    let head = feet.add(v3(0, HEAD, 0));
    let pivot = head.add(v3(0, ABOVE, 0)).add(right_of(look.yaw).scale(RIGHT));
    let fwd = facing(look.yaw, look.pitch);
    let mut eye = pivot.sub(fwd.scale(back));
    eye.y = eye.y.max(ground(eye.x, eye.z) + MIN_EYE_HEIGHT);
    Camera { eye, fwd, fov: FOV }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::int;

    fn flat(_: Fx, _: Fx) -> Fx {
        0
    }

    #[test]
    fn level_and_facing_north_the_eye_is_behind_right_of_and_above_the_head() {
        // feet at the origin, head at 1.7 m; heading 0 faces −z, so behind
        // is +z and right is +x: the eye at (0.6, 2.2, 2.6)
        let c = view(
            V3::ZERO,
            Aim {
                yaw: 0,
                pitch: 0,
                rest: (0, 0),
                pct: 100,
            },
            flat,
        );
        let want = v3(ONE * 6 / 10, ONE * 22 / 10, ONE * 26 / 10);
        assert!(
            (c.eye.x - want.x).abs() <= 4 && (c.eye.y - want.y).abs() <= 4 && (c.eye.z - want.z).abs() <= 4,
            "{:?}",
            c.eye
        );
        assert_eq!(c.fov, deg(60));
    }

    #[test]
    fn turned_a_quarter_left_the_shoulder_turns_with_the_view() {
        // heading a quarter turn left faces −x (west): behind is +x, right
        // is −z; the eye at (2.6, 2.2, −0.6)
        let c = view(
            V3::ZERO,
            Aim {
                yaw: fx::QUARTER_TURN,
                pitch: 0,
                rest: (0, 0),
                pct: 100,
            },
            flat,
        );
        assert!(
            (c.eye.x - ONE * 26 / 10).abs() <= 4 && (c.eye.z + ONE * 6 / 10).abs() <= 4,
            "{:?}",
            c.eye
        );
    }

    #[test]
    fn looking_down_lifts_the_camera_and_the_ground_stops_it_going_under() {
        let level = view(
            V3::ZERO,
            Aim {
                yaw: 0,
                pitch: 0,
                rest: (0, 0),
                pct: 100,
            },
            flat,
        );
        let down = view(
            V3::ZERO,
            Aim {
                yaw: 0,
                pitch: -deg(30),
                rest: (0, 0),
                pct: 100,
            },
            flat,
        );
        // 2.6 m × sin 30° = 1.3 m higher
        assert!(
            (down.eye.y - level.eye.y - ONE * 13 / 10).abs() < ONE / 100,
            "{}",
            down.eye.y
        );
        // on ground 2 m up (a step the feet are below), the eye stays 0.3 m over it
        let up = view(
            V3::ZERO,
            Aim {
                yaw: 0,
                pitch: PITCH_MAX,
                rest: (0, 0),
                pct: 100,
            },
            |_, _| int(2),
        );
        assert_eq!(up.eye.y, int(2) + MIN_EYE_HEIGHT);
    }

    #[test]
    fn mouse_right_turns_right_and_down_looks_down_within_range() {
        // 100 px right (10 000 hundredths) is 12°: 65 536 × 12 / 360 =
        // 2 184.5, which the integer constant holds as 2 184 units
        let l = Aim::new(0).turned(Mouse {
            dx: 10_000,
            dy: 0,
            wheel: 0,
        });
        assert_eq!(l.yaw, -2184, "right is a negative heading");
        let d = Aim {
            yaw: 0,
            pitch: 0,
            rest: (0, 0),
            pct: 100,
        }
        .turned(Mouse {
            dx: 0,
            dy: 10_000,
            wheel: 0,
        });
        assert_eq!(d.pitch, -2184);
        let far = Aim::new(0).turned(Mouse {
            dx: 0,
            dy: 1_000_000,
            wheel: 0,
        });
        assert_eq!(far.pitch, PITCH_MIN);
    }

    #[test]
    fn slow_movement_turns_the_view_as_much_as_the_same_movement_at_once() {
        // a quarter pixel a tick for 400 ticks is 100 px: 2 184 units, the
        // same as 100 px in one tick, though each tick's share is 5.46 units
        let mut slow = Aim::new(0);
        for _ in 0..400 {
            slow = slow.turned(Mouse {
                dx: 25,
                dy: 0,
                wheel: 0,
            });
        }
        let fast = Aim::new(0).turned(Mouse {
            dx: 10_000,
            dy: 0,
            wheel: 0,
        });
        assert_eq!(slow.yaw, fast.yaw);
        assert_eq!(slow.yaw, -2184);
    }

    #[test]
    fn a_quarter_look_speed_turns_a_quarter_as_far() {
        // 100 px at 25%: 2 184 × 25 / 100 = 546 units
        let slow = Aim {
            pct: 25,
            ..Aim::new(0)
        }
        .turned(Mouse {
            dx: 10_000,
            dy: 0,
            wheel: 0,
        });
        assert_eq!(slow.yaw, -546);
        assert_eq!(slow.pct, 25, "the setting stays with the aim");
    }
}
