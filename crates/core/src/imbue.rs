//! `imbue` (M6 Magic, task M): the one ability in the MVP. Q
//! (`input::IMBUE`) imbues the sword for 600 ticks; the cooldown is 1,200
//! ticks counted from the cast, so Q works again 20 s after it was pressed
//! (CW-9, the journal's reading A). Before the trainer has taught it (T2)
//! Q does nothing, and during the cooldown Q does nothing. The numbers are
//! `data/items.json`'s `imbue` (M4's catalogue).
//!
//! While imbued, the sword's edge turns orbs one at a time (`orbs.rs`);
//! on a plain sword-on-sword exchange it changes nothing. The trainer may
//! also lend the imbue for a practice round (`lend`), before it is taught:
//! a lent imbue starts no cooldown.
//!
//! Steps at 6 in `World::step`, before the orbs, so the tick Q is pressed is
//! the first imbued tick. Draws the blade's glow (`draw`); the HUD's ring
//! reads `status` through `ui::imbue`.
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`.

// --- Doors, set by the coordinator so M5 (story) and M6 (magic) can run in
// parallel. M6 fills the bodies; the signatures stay. M5's trainer calls
// `learn` when the practice is passed, and `lend` during it.

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, Affine};
use crate::input::{Input, IMBUE};
use crate::look::{mix, rgb, Look};
use crate::prim::Mesh;
use crate::render::Shapes;
use crate::world::World;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    /// the tick of the last cast (`World::tick` as it stood that tick)
    pub cast_at: Option<u64>,
    /// ticks the trainer's lent imbue has left (T2's practice)
    pub lent: u32,
    /// Q was held last tick, so a held key casts once
    pub held: bool,
}

/// How long the imbue lasts and how long until it can be cast again,
/// counted from the cast, ticks (CW-9).
pub fn lasts() -> u32 {
    crate::items::catalogue().imbue.lasts_ticks
}

pub fn cooldown() -> u32 {
    crate::items::catalogue().imbue.cooldown_ticks
}

/// The trainer has taught the imbue (T2): Q works from now on.
pub fn learn(w: &mut World) {
    w.inventory.knows_imbue = true;
}

/// Whether the imbue has been taught.
pub fn learned(w: &World) -> bool {
    w.inventory.knows_imbue
}

/// The trainer imbues the player's sword for `ticks` (the practice round,
/// T2), whether or not the imbue is taught. It starts no cooldown, and a
/// lend of 0 takes a lent imbue back.
pub fn lend(w: &mut World, ticks: u32) {
    w.imbue.lent = ticks;
}

/// Ticks since the last cast, if there was one.
fn since(w: &World) -> Option<u64> {
    w.imbue.cast_at.map(|t| w.tick.saturating_sub(t))
}

/// Whether the player's own cast is on this tick.
fn cast_on(w: &World) -> bool {
    since(w).is_some_and(|s| s < lasts() as u64)
}

/// Whether the player's blade is imbued this tick (turns orbs, CW-9): a
/// cast in its 600 ticks, or a lent imbue.
pub fn on(w: &World) -> bool {
    cast_on(w) || w.imbue.lent > 0
}

/// Whether Q would cast now: taught, and 1,200 ticks since the last cast.
pub fn ready(w: &World) -> bool {
    learned(w) && since(w).is_none_or(|s| s >= cooldown() as u64)
}

/// Where the imbue stands, for the HUD's ring: none until taught (a lent
/// imbue shows while it lasts).
pub fn status(w: &World) -> Option<crate::ui::Imbue> {
    use crate::ui::Imbue;
    if cast_on(w) {
        let s = since(w).unwrap_or(0) as u32;
        return Some(Imbue::On(lasts() - s));
    }
    if w.imbue.lent > 0 {
        return Some(Imbue::On(w.imbue.lent.min(lasts())));
    }
    if !learned(w) {
        return None;
    }
    Some(match since(w) {
        Some(s) if s < cooldown() as u64 => Imbue::Cooling(cooldown() - s as u32),
        _ => Imbue::Ready,
    })
}

/// Ticks of imbue left (the cast's or the lend's, whichever is longer).
pub fn left(w: &World) -> u32 {
    let own = since(w).map_or(0, |s| (lasts() as u64).saturating_sub(s) as u32);
    own.max(w.imbue.lent)
}

/// Step 6, before the orbs: Q pressed (down this tick and not the last)
/// casts when taught and ready; a lent imbue runs down.
pub fn step(w: &mut World, input: &Input) {
    let down = input.key(IMBUE);
    let pressed = down && !w.imbue.held;
    w.imbue.held = down;
    // the lend counts down from the tick after it was given
    w.imbue.lent = w.imbue.lent.saturating_sub(1);
    if pressed && ready(w) && !w.me.health.down() {
        w.imbue.cast_at = Some(w.tick);
    }
}

// ---- the blade's glow -----------------------------------------------------

/// The glow comes up over this many ticks after the cast, and in the last
/// `WARN` ticks it pulses, `BLINK` ticks to a beat, so the end is seen
/// coming.
pub const RISE: u32 = 10;
pub const WARN: u32 = 120;
pub const BLINK: u32 = 20;
/// The glow's widths across the blade: a white-hot core, a bright inner
/// halo and a wide soft one.
const CORE_W: Fx = ONE * 24 / 1000;
const INNER_W: Fx = ONE * 9 / 100;
const OUTER_W: Fx = ONE * 24 / 100;
/// Sparkles that run up the edge, this many at once, a lap every `LAP`
/// ticks.
const MOTES: i32 = 4;
const LAP: i32 = 40;

/// How bright the glow is this tick, `ONE` full: up over `RISE`, a pulse in
/// the last `WARN` ticks.
pub fn brightness(w: &World) -> Fx {
    let left = left(w);
    if left == 0 {
        return 0;
    }
    let s = since(w).filter(|_| cast_on(w)).unwrap_or(RISE as u64) as u32;
    let rise = fx::ratio((s + 1).min(RISE) as i32, RISE as i32);
    let pulse = if left <= WARN {
        // a triangle wave between 35% and 100%
        let k = (left % BLINK) as i32;
        let tri = (k - BLINK as i32 / 2).abs();
        ONE * 35 / 100 + fx::ratio(tri * 2, BLINK as i32) * 65 / 100
    } else {
        ONE
    };
    fx::mul(rise, pulse)
}

/// The imbued blade: a white-hot line along the edge with two halos round
/// it, and sparkles running up it. Drawn while imbued and the sword in hand.
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    let b = brightness(w);
    if b == 0 || !w.me.arm.in_hand || w.me.health.down() {
        return;
    }
    let _ = look;
    let spec = crate::orbs::spec();
    let core = rgb(spec.imbue.colour);
    let halo = rgb(spec.imbue.halo);
    let p = w.me.prev_pose.lerp(&w.me.pose, alpha);
    let wpn = w.me.arm.weapon;
    let dir = p.tip.sub(p.grip).norm();
    let guard = p.grip.add(dir.scale(wpn.hilt - wpn.grip[1]));
    let len = p.tip.dist(guard);
    if len < ONE / 20 {
        return;
    }
    let mid = guard.lerp(p.tip, ONE / 2);
    // a capsule `width` across (the mesh is `prim::CAPSULE_RADIUS` × 2 of
    // its scale across) running past both ends by half its width
    let along = |width: Fx| {
        let across = fx::div(width, crate::prim::CAPSULE_RADIUS * 2);
        Affine::translate(mid)
            .then(&Affine::looking(dir))
            .then(&Affine::rot_x(fx::QUARTER_TURN))
            .then(&Affine::scale(v3(across, len + width, across)))
    };
    out.solid(Mesh::Capsule, along(CORE_W), mix(halo, core, b), ONE);
    out.glow(Mesh::Capsule, along(INNER_W), halo, fx::mul(ONE * 9 / 10, b));
    out.glow(Mesh::Capsule, along(OUTER_W), halo, fx::mul(ONE * 45 / 100, b));
    // sparkles up the edge, evenly spaced, moving with the tick
    let phase = ((w.tick % LAP as u64) as i32 * ONE + alpha) / LAP;
    for k in 0..MOTES {
        let t = (phase + ONE * k / MOTES) % ONE;
        let at = guard.lerp(p.tip, t);
        let r = fx::mul(ONE * 3 / 100, ONE - (2 * t - ONE).abs() / 2);
        out.glow(
            Mesh::Sphere,
            Affine::translate(at).then(&Affine::scale(v3(r * 2, r * 2, r * 2))),
            core,
            fx::mul(ONE * 8 / 10, b),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q() -> Input {
        Input {
            keys: IMBUE,
            ..Input::default()
        }
    }

    fn idle() -> Input {
        Input::default()
    }

    /// A world that knows the imbue, after `n` idle ticks.
    fn taught(n: u32) -> World {
        let mut w = World::new(3);
        learn(&mut w);
        for _ in 0..n {
            w.step(&idle());
        }
        w
    }

    /// CW-9, pinned by hand: Q pressed on the tick numbered T imbues the
    /// sword for that tick and the 599 after it, exactly 600, and nothing
    /// casts again until T + 1 200: Q on T + 1 199 does nothing, Q on
    /// T + 1 200 casts.
    #[test]
    fn the_imbue_is_on_for_exactly_600_ticks_and_ready_again_1200_after_the_cast() {
        let mut w = taught(5);
        w.step(&q());
        let t = w.tick;
        assert_eq!(t, 6);
        assert_eq!(w.imbue.cast_at, Some(6));
        let mut on_ticks = u32::from(on(&w));
        // Q held down the whole while: a held key casts once
        for _ in 0..599 {
            w.step(&q());
            on_ticks += u32::from(on(&w));
        }
        assert_eq!(w.tick, 605);
        assert!(on(&w), "still on at T + 599");
        w.step(&idle());
        assert!(!on(&w), "off at T + 600");
        assert_eq!(on_ticks, 600);
        // up to T + 1 198 idle, then Q on T + 1 199: nothing
        while w.tick < t + 1198 {
            w.step(&idle());
        }
        w.step(&q());
        assert_eq!(w.tick, t + 1199);
        assert_eq!(w.imbue.cast_at, Some(6), "Q during the cooldown does nothing");
        assert!(!on(&w));
        // let go, and Q again on T + 1 200: cast
        w.imbue.held = false;
        w.step(&q());
        assert_eq!(w.tick, t + 1200);
        assert_eq!(w.imbue.cast_at, Some(1206));
        assert!(on(&w));
    }

    /// Q held down from the cast through 1 300 ticks casts once: the press
    /// counts, not the key being down, so it does not cast again at
    /// T + 1 200 until it is let go and pressed again.
    #[test]
    fn holding_q_down_casts_once() {
        let mut w = taught(0);
        for _ in 0..1300 {
            w.step(&q());
        }
        assert_eq!(w.imbue.cast_at, Some(1));
        w.step(&idle());
        w.step(&q());
        assert_eq!(w.imbue.cast_at, Some(1302));
    }

    #[test]
    fn q_before_the_imbue_is_taught_does_nothing() {
        let mut w = World::new(3);
        for _ in 0..3 {
            w.step(&q());
            w.step(&idle());
        }
        assert!(!learned(&w));
        assert_eq!(w.imbue.cast_at, None);
        assert!(!on(&w));
        assert_eq!(status(&w), None, "the HUD shows no ring");
        learn(&mut w);
        w.step(&q());
        assert!(on(&w), "taught, the next press casts");
    }

    /// The trainer's lend: on for exactly the ticks lent, starting no
    /// cooldown; the HUD shows it on though the imbue is not taught.
    #[test]
    fn a_lent_imbue_lasts_the_ticks_lent_and_starts_no_cooldown() {
        let mut w = World::new(3);
        lend(&mut w, 90);
        let mut on_ticks = 0;
        for _ in 0..200 {
            w.step(&idle());
            on_ticks += u32::from(on(&w));
        }
        // lent before a tick: that tick counts it down to 89, and so on to
        // the tick it reaches 0, so 89 imbued ticks follow the lend
        assert_eq!(on_ticks, 89);
        assert_eq!(w.imbue.cast_at, None);
        lend(&mut w, 30);
        assert_eq!(status(&w), Some(crate::ui::Imbue::On(30)));
        learn(&mut w);
        w.step(&q());
        assert_eq!(w.imbue.cast_at, Some(w.tick), "taught, Q casts during a lend");
    }

    /// The HUD's ring through M4's door: cast at T, 750 ticks later it cools
    /// with 450 ticks left (8 s shown) and 62% recovered (750 / 1 200).
    #[test]
    fn the_hud_ring_reads_the_imbue_from_this_module() {
        let mut w = taught(0);
        w.step(&q());
        assert_eq!(crate::ui::imbue(&w), Some(crate::ui::Imbue::On(600)));
        for _ in 0..750 {
            w.step(&idle());
        }
        assert_eq!(crate::ui::imbue(&w), Some(crate::ui::Imbue::Cooling(450)));
        for _ in 0..450 {
            w.step(&idle());
        }
        assert_eq!(crate::ui::imbue(&w), Some(crate::ui::Imbue::Ready));
    }

    /// The glow: a fifth of the way up on the cast's tick... in whole: the
    /// cast tick is 1/10 of full, the tenth tick full; with 120 ticks left
    /// it pulses: at 110 left (k = 10) the triangle is at its foot, 35%.
    #[test]
    fn the_glow_rises_over_ten_ticks_and_pulses_before_it_ends() {
        let mut w = taught(0);
        w.step(&q());
        assert_eq!(brightness(&w), ONE / 10);
        for _ in 0..9 {
            w.step(&idle());
        }
        assert_eq!(brightness(&w), ONE);
        while left(&w) > 110 {
            w.step(&idle());
        }
        assert_eq!(brightness(&w), ONE * 35 / 100);
        while left(&w) > 0 {
            w.step(&idle());
        }
        assert_eq!(brightness(&w), 0);
    }
}
