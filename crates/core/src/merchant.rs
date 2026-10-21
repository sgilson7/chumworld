//! `merchant` (M5 Places and story, task S): the merchant and the donkey
//! cart on the road.
//!
//! The brief: "a shopkeeper being pulled by a donkey that you can stop by
//! flinging something at their cart, then which you can buy stuff". The
//! cart walks M2's road (`places::road`) at `SPEED` (1.4 m/s, PLAN.md), on a
//! beat between two points of it (`data/story.json`: west of the bridge to
//! the village's crossroads), over the bridge on its deck (`solid::surface`),
//! turning round on the spot at each end. Its body is a box throwables stop
//! against (`throwables::State::props`); one that lands on it (an `Impact`
//! on that box, or the boomerang passing through it) stops the cart, and
//! the merchant waits `WAIT_TICKS` (1,800, 30 s) from the hit, then walks on.
//! While it waits, E by the cart greets you and opens M4's shop.
//!
//! The merchant sits on the cart's seat; the donkey walks `DONKEY_TO_CART`
//! ahead (M3's models), its legs stepping with the distance walked.

use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{facing, v3, yaw_of, Affine, V3};
use crate::look::{rgb, Look};
use crate::models::{self, DONKEY_TO_CART};
use crate::pose::{Held, Kind, Pose};
use crate::render::Shapes;
use crate::solid;
use crate::throwables::{Block, Struck};
use crate::world::World;

/// 1.4 m/s (PLAN.md), metres a tick.
pub const SPEED: Fx = ONE * 14 / 10 / 60;
/// The wait after a hit (PLAN.md).
pub const WAIT_TICKS: u32 = 1800;
/// Turning round at the end of its beat takes this long (3 s).
pub const TURN_TICKS: u32 = 180;
/// The cart's half length along the road, and where its wheels touch, for
/// its pitch.
pub const HALF_LEN: Fx = ONE * 12 / 10;
/// The box throwables stop against, round the cart's body and canopy: half
/// across, half high, half along; its middle this far over the cart's foot.
pub const BOX_HALF: (Fx, Fx, Fx) = (ONE * 80 / 100, ONE * 90 / 100, ONE * 125 / 100);
pub const BOX_UP: Fx = ONE * 140 / 100;
/// E talks to the merchant from this near the cart's middle.
pub const TALK_FROM: Fx = ONE * 36 / 10;
/// The donkey takes a stride (a whole turn of its step) every this far.
pub const STRIDE: Fx = ONE * 12 / 10;
/// M7 (R5.14): the cart's wheels go round once every this far: M3's wheel
/// is 1.15 m across, so 1.15 m × π = 3.613 m.
pub const WHEEL_ROUND: Fx = ONE * 3613 / 1000;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    /// the beat: points along the road, and the distance to each from the
    /// beat's west end
    pub path: Vec<V3>,
    pub along: Vec<Fx>,
    /// where the cart's middle is along the beat, and which way it goes
    /// (1 east, −1 west)
    pub s: Fx,
    pub dir: i32,
    /// ticks left to wait after a hit
    pub wait: u32,
    /// ticks into a turn at an end of the beat
    pub turning: u32,
    /// the donkey's step, an angle
    pub step: i32,
    /// the cart as drawn: its foot, heading and pitch, this tick and last
    pub at: V3,
    pub yaw: i32,
    pub pitch: i32,
    pub prev_at: V3,
    pub prev_yaw: i32,
    pub prev_pitch: i32,
    /// which of `throwables::State::props` is the cart's box
    pub prop: Option<usize>,
    /// hits so far (for tests and the shop's greeting)
    pub hits: u32,
    /// M7: how far round the wheels have rolled, an angle, this tick and last
    pub wheel: i32,
    pub prev_wheel: i32,
}

impl State {
    pub fn active(&self) -> bool {
        self.path.len() >= 2
    }

    /// Whether the cart stands waiting after a hit.
    pub fn waiting(&self) -> bool {
        self.wait > 0
    }
}

/// Puts the cart on its beat (the opening calls it).
pub fn begin(w: &mut World) {
    let sp = &crate::story::spec().merchant;
    let road = crate::places::road(w);
    let (lo, hi) = (fx::int(sp.west_x), fx::int(sp.east_x));
    let mut path: Vec<V3> = road
        .points
        .iter()
        .filter(|p| p.x >= lo && p.x <= hi)
        .copied()
        .collect();
    for p in &mut path {
        p.y = solid::surface(w, p.x, p.z);
    }
    let mut along = Vec::with_capacity(path.len());
    let mut d = 0;
    for k in 0..path.len() {
        if k > 0 {
            d += path[k].sub(path[k - 1]).len_xz();
        }
        along.push(d);
    }
    let start = fx::int(sp.start_x);
    let s = path
        .iter()
        .zip(&along)
        .min_by_key(|(p, _)| (p.x - start).abs())
        .map(|(_, a)| *a)
        .unwrap_or(0);
    let m = &mut w.merchant;
    m.path = path;
    m.along = along;
    m.s = s;
    m.dir = -1;
    place(w);
    let m = &mut w.merchant;
    m.prev_at = m.at;
    m.prev_yaw = m.yaw;
    m.prev_pitch = m.pitch;
    let b = cart_box(m);
    w.throwables.props.push(b);
    w.merchant.prop = Some(w.throwables.props.len() - 1);
}

/// The beat's length.
pub fn length(m: &State) -> Fx {
    m.along.last().copied().unwrap_or(0)
}

/// The point `s` along the beat, on the ground (or the bridge's deck).
pub fn point(w: &World, s: Fx) -> V3 {
    let m = &w.merchant;
    let s = s.clamp(0, length(m));
    let k = m.along.partition_point(|a| *a <= s).clamp(1, m.path.len() - 1);
    let (a, b) = (m.path[k - 1], m.path[k]);
    let seg = (m.along[k] - m.along[k - 1]).max(1);
    let mut p = a.lerp(b, fx::div(s - m.along[k - 1], seg));
    p.y = solid::surface(w, p.x, p.z);
    p
}

/// The cart's foot, heading and pitch from where it is along the beat: its
/// heading from its back wheels' point to its front's, so it follows the
/// road's bends; tipped by the heights under them.
fn place(w: &mut World) {
    let m = &w.merchant;
    let ahead = if m.dir >= 0 { 1 } else { -1 };
    let front = point(w, m.s + ahead * HALF_LEN);
    let back = point(w, m.s - ahead * HALF_LEN);
    let mut at = front.lerp(back, ONE / 2);
    at.y = (front.y + back.y) / 2;
    let mut yaw = yaw_of(front.sub(back));
    let pitch = fx::wrap(fx::atan2(front.y - back.y, front.sub(back).len_xz().max(1)));
    let m = &w.merchant;
    if m.turning > 0 {
        // turning round on the spot: half a turn over `TURN_TICKS`, eased
        let t = (m.turning as i64 * ONE as i64 / TURN_TICKS as i64) as Fx;
        let eased = fx::mul(fx::mul(t, t), 3 * ONE - 2 * t);
        yaw = fx::wrap(yaw + fx::HALF_TURN - fx::mul(fx::HALF_TURN, eased));
    }
    let m = &mut w.merchant;
    m.at = at;
    m.yaw = yaw;
    m.pitch = pitch;
}

/// The cart's box for throwables: round its body and canopy, as wide as it
/// is across whichever way it heads (a box on the world's axes).
pub fn cart_box(m: &State) -> Block {
    let (s, c) = (fx::sin(m.yaw).abs(), fx::cos(m.yaw).abs());
    let (hx, hy, hz) = BOX_HALF;
    Block {
        centre: m.at.add(v3(0, BOX_UP, 0)),
        half: v3(
            fx::mul(hx, c) + fx::mul(hz, s),
            hy,
            fx::mul(hx, s) + fx::mul(hz, c),
        ),
    }
}

/// Step 7: walk the beat (or wait, or turn), then stop for a throwable
/// that landed on the cart this tick.
pub fn step(w: &mut World) {
    if !w.merchant.active() {
        return;
    }
    let m = &mut w.merchant;
    m.prev_at = m.at;
    m.prev_yaw = m.yaw;
    m.prev_pitch = m.pitch;
    m.prev_wheel = m.wheel;
    if w.story.still {
        // held for a picture: nothing moves
    } else if m.wait > 0 {
        m.wait -= 1;
    } else if m.turning > 0 {
        m.turning += 1;
        if m.turning >= TURN_TICKS {
            m.turning = 0;
        }
        m.step = m.step.wrapping_add(fx::TURN / 120);
    } else {
        let end = length(m);
        m.s += m.dir * SPEED;
        // the wheels roll with the ground the cart covers
        m.wheel = m
            .wheel
            .wrapping_add((SPEED as i64 * fx::TURN as i64 / WHEEL_ROUND as i64) as i32);
        m.step = m
            .step
            .wrapping_add((SPEED as i64 * fx::TURN as i64 / STRIDE as i64) as i32);
        if m.s >= end - HALF_LEN || m.s <= HALF_LEN {
            m.s = m.s.clamp(HALF_LEN, end - HALF_LEN);
            m.dir = -m.dir;
            m.turning = 1;
        }
    }
    place(w);
    let b = cart_box(&w.merchant);
    if let Some(k) = w.merchant.prop {
        if let Some(slot) = w.throwables.props.get_mut(k) {
            *slot = b;
        }
        let hit = w
            .throwables
            .landed
            .iter()
            .any(|i| i.struck == Struck::Prop(k as u16));
        if hit {
            let m = &mut w.merchant;
            m.wait = WAIT_TICKS;
            m.hits += 1;
            if !w.story.still {
                crate::story::banner(w, "story_cart_stopped");
            }
        }
    }
}

/// How far the player is from talking to the merchant, while the cart
/// waits; `None` while it walks on (the merchant never stops for a shout).
pub fn reach(w: &World) -> Option<Fx> {
    let m = &w.merchant;
    if !m.active() || !m.waiting() {
        return None;
    }
    let d = m.at.sub(w.player.pos).len_xz();
    (d <= TALK_FROM).then(|| (d - TALK_FROM + crate::story::REACH).max(0))
}

pub fn prompt_key(_w: &World) -> &'static str {
    "does_talk_merchant"
}

/// E by the waiting cart: a greeting, then the shop (M4: the dialogue shows
/// first, the shop the tick after it closes).
pub fn talk(w: &mut World) {
    crate::ui::say(w, "merchant", &["merchant_hello"]);
    crate::ui::open_shop(w);
}

/// `merchant.<name>`, test-only, for views of the shop on the road: `stop`
/// stops the cart as a hit would; `beside` stands the player by the cart's
/// side, facing it.
pub fn act(w: &mut World, name: &str, _value: &str) -> Result<(), String> {
    match name {
        "stop" if w.merchant.active() => {
            w.merchant.wait = WAIT_TICKS;
            Ok(())
        }
        "beside" if w.merchant.active() => {
            let m = &w.merchant;
            let mut at = m.at.add(crate::geom::right_of(m.yaw).scale(ONE * 26 / 10));
            at.y = solid::surface(w, at.x, at.z);
            let yaw = yaw_of(m.at.sub(at)) - deg(20);
            w.player = crate::player::Body::new(at, yaw);
            let held = crate::items::held(w);
            w.me = crate::player::Fighter::new(Kind::Player, held, 0, at, yaw, false);
            w.aim.yaw = yaw;
            Ok(())
        }
        "stop" | "beside" => Err("There is no merchant on the road yet.".into()),
        _ => Err(format!("Unknown action \"merchant.{name}\".")),
    }
}

// ---- drawing ------------------------------------------------------------------

/// The cart, the donkey ahead of it and the merchant on its seat.
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    let m = &w.merchant;
    if !m.active() {
        return;
    }
    let at = m.prev_at.lerp(m.at, alpha);
    let yaw = fx::lerp_angle(m.prev_yaw, m.yaw, alpha);
    let pitch = fx::lerp(m.prev_pitch, m.pitch, alpha);
    let place = Affine::translate(at)
        .then(&Affine::rot_y(yaw))
        .then(&Affine::rot_x(pitch));
    // M7 (R5.14): the wheels turned as far as the cart has rolled
    let wheel = fx::lerp_angle(m.prev_wheel, m.wheel, alpha);
    models::cart_model(wheel).placed(&place).emit(look.ink(), out);
    out.shadow(
        at.add(v3(0, crate::player::SHADOW_LIFT, 0)),
        ONE * 26 / 10,
        rgb(look.palette.shadow),
        ONE * 2 / 3,
    );
    // the donkey ahead, its feet on the ground under it
    let mut donkey = at.add(facing(yaw, 0).scale(DONKEY_TO_CART));
    donkey.y = solid::surface(w, donkey.x, donkey.z);
    let walking = m.wait == 0;
    models::donkey(donkey, yaw, if walking { m.step } else { 0 }, look, out);
    // the merchant on the seat (M3's cart: its top 1.2 m up, 0.9 m forward)
    let seat = place.apply(v3(0, ONE * 120 / 100, -ONE * 90 / 100));
    let pose = seated(Kind::Merchant, seat, yaw, pitch);
    models::figure(Kind::Merchant, &pose, look, out);
}

/// A figure sitting with its hips at `seat`: `rest_pose` with the thighs
/// forward and the shins down.
pub fn seated(kind: Kind, seat: V3, yaw: i32, pitch: i32) -> Pose {
    let b = crate::pose::build(kind);
    let fw = facing(yaw, pitch);
    let up = v3(0, ONE, 0);
    let feet = seat.sub(up.scale(b.hip_y));
    let mut p = models::rest_pose(kind, feet, yaw);
    // the rest pose sinks its hips a little: lift it back to the seat
    let lift = seat.y - p.pelvis.y;
    let up_by = |v: &mut V3| v.y += lift;
    for j in [&mut p.pelvis, &mut p.chest, &mut p.head, &mut p.grip, &mut p.tip] {
        up_by(j);
    }
    for s in 0..2 {
        up_by(&mut p.shoulder[s]);
        up_by(&mut p.elbow[s]);
        up_by(&mut p.hand[s]);
        up_by(&mut p.hip[s]);
    }
    for s in 0..2 {
        p.knee[s] = p.hip[s].add(fw.scale(b.thigh));
        p.foot[s] = p.knee[s].sub(up.scale(b.shin));
    }
    // the hands forward on the reins
    for s in 0..2 {
        p.hand[s] = p.hand[s]
            .add(fw.scale(ONE * 25 / 100))
            .add(up.scale(ONE * 30 / 100));
        p.elbow[s] = p.shoulder[s].lerp(p.hand[s], ONE / 2).sub(up.scale(ONE / 10));
    }
    p.grip = p.hand[1];
    p.tip = p.hand[1];
    let _ = (deg(0), Held::Nothing);
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;
    use crate::throwables::{Impact, Struck};

    fn world() -> World {
        let mut w = crate::story::opening(9);
        crate::story::step_aside(&mut w);
        w
    }

    /// A landed throwable on the cart's box this tick, as `throwables`
    /// reports one.
    fn hit(w: &mut World, k: usize) {
        let at = w.merchant.at;
        w.throwables.landed.push(Impact {
            item: Held::Rock,
            by: 0,
            at,
            speed: ONE * 15,
            struck: Struck::Prop(k as u16),
        });
    }

    /// M7 (R5.14): the wheels roll with the road the cart covers. A tick's
    /// walk is SPEED = 65 536 × 14 / 10 / 60 = 1 529 units; a round is
    /// 65 536 × 3 613 / 1 000 = 236 781 units; so 1 529 × 65 536 / 236 781 =
    /// 423 angle units a tick, 25 380 over 60 ticks. Stopped by a hit, they
    /// stand still.
    #[test]
    fn the_cart_s_wheels_roll_as_far_as_the_cart_goes() {
        let mut w = world();
        // clear of the beat's ends, so it walks straight on
        w.merchant.s = length(&w.merchant) / 2;
        w.merchant.turning = 0;
        let w0 = w.merchant.wheel;
        for _ in 0..60 {
            step(&mut w);
        }
        assert_eq!(w.merchant.wheel.wrapping_sub(w0), 25_380);
        assert_eq!(w.merchant.wheel.wrapping_sub(w.merchant.prev_wheel), 423);
        w.merchant.wait = 100;
        let held = w.merchant.wheel;
        step(&mut w);
        assert_eq!(w.merchant.wheel, held, "waiting, the wheels stand");
        // the drawn spokes turn with it: spoke 0 stands up at 0 and lies
        // along the cart, its top forward (−z), a quarter round on
        let spoke = |m: &crate::models::Model| {
            m.parts
                .iter()
                .find(|p| p.mesh == crate::prim::Mesh::Cube && p.at.c1.len() > ONE * 9 / 10)
                .map(|p| p.at.c1)
                .unwrap()
        };
        let up = spoke(&crate::models::cart_model(0));
        assert!(up.y > ONE * 99 / 100 && up.z.abs() < ONE / 100, "{up:?}");
        let along = spoke(&crate::models::cart_model(fx::QUARTER_TURN));
        assert!(
            along.z < -ONE * 99 / 100 && along.y.abs() < ONE / 100,
            "{along:?}"
        );
        assert_eq!(
            crate::models::cart_model(0),
            crate::models::prop_model(crate::models::Prop::Cart)
        );
    }

    #[test]
    fn the_cart_walks_the_road_at_one_point_four_metres_a_second() {
        let mut w = world();
        let s0 = w.merchant.s;
        for _ in 0..600 {
            w.step(&Input::default());
        }
        // 600 ticks at 1.4/60 m a tick: 14 m west, give or take the
        // rounding of SPEED (91.75 units a tick, kept as 91): 600 × 91
        assert_eq!(s0 - w.merchant.s, 600 * SPEED);
        assert_eq!(SPEED, 1529);
        // on the road: within its half width of a road point
        let road = crate::places::road(&w);
        let at = w.merchant.at;
        let near = road.points.iter().map(|p| p.sub(at).len_xz()).min().unwrap();
        assert!(near < road.half, "{near}");
    }

    #[test]
    fn a_throwable_landing_on_the_cart_stops_it_for_exactly_1800_ticks() {
        let mut w = world();
        let k = w.merchant.prop.expect("the cart's box");
        w.step(&Input::default());
        hit(&mut w, k);
        // the hit is read in the tick's step 7; landed is cleared at step 5
        // of the next, so it is put in by hand between the two halves here:
        // the merchant reads it now
        step(&mut w);
        let held = w.merchant.s;
        assert_eq!(w.merchant.wait, 1800);
        for t in 0..1800 {
            w.step(&Input::default());
            assert_eq!(w.merchant.s, held, "still waiting at tick {t}");
        }
        assert_eq!(w.merchant.wait, 0);
        w.step(&Input::default());
        assert_eq!(held - w.merchant.s, SPEED, "and on its way the tick after");
    }

    #[test]
    fn a_rock_thrown_onto_the_cart_stops_it_and_one_that_misses_does_not() {
        // a real throw: the player stands 7 m from the cart's middle, square
        // to the road, and throws at it; then the same throw aimed 30° off
        for (off, stops) in [(0, true), (30, false)] {
            let mut w = world();
            let m = &w.merchant;
            let side = crate::geom::right_of(m.yaw);
            let mut at = m.at.add(side.scale(fx::int(7)));
            at.y = w.ground(at.x, at.z);
            let yaw = yaw_of(m.at.sub(at)) + deg(off);
            w.player = crate::player::Body::new(at, yaw);
            w.me = crate::player::Fighter::new(Kind::Player, Held::ShortSword, 0, at, yaw, false);
            w.aim.yaw = yaw;
            w.aim.pitch = 0;
            crate::inventory::give(&mut w, crate::items::Item::Rock, 1);
            // the cart held still while the throw flies, so only aim decides
            w.story.still = true;
            w.step(&Input {
                keys: crate::input::THROW,
                ..Input::default()
            });
            for _ in 0..60 {
                w.step(&Input::default());
            }
            assert_eq!(w.merchant.hits > 0, stops, "aimed {off}° off");
            assert_eq!(w.merchant.waiting(), stops);
        }
    }

    #[test]
    fn the_cart_turns_round_at_each_end_of_its_beat() {
        let mut w = world();
        // just short of the west end, heading west
        w.merchant.s = HALF_LEN + SPEED * 3;
        w.merchant.dir = -1;
        for _ in 0..4 {
            step(&mut w);
        }
        assert_eq!(w.merchant.dir, 1);
        assert!(w.merchant.turning > 0);
        for _ in 0..TURN_TICKS {
            step(&mut w);
        }
        assert_eq!(w.merchant.turning, 0);
        let s = w.merchant.s;
        step(&mut w);
        assert_eq!(w.merchant.s, s + SPEED, "east now");
    }

    #[test]
    fn over_the_bridge_the_cart_rides_on_the_deck() {
        let mut w = world();
        let bridge = crate::places::prop_spots(&w, crate::models::Prop::Bridge)[0];
        let s = w
            .merchant
            .path
            .iter()
            .zip(&w.merchant.along)
            .min_by_key(|(p, _)| p.sub(bridge.at).len_xz())
            .map(|(_, a)| *a)
            .unwrap();
        w.merchant.s = s;
        place(&mut w);
        assert!(
            (w.merchant.at.y - (bridge.at.y + solid::DECK)).abs() < ONE / 20,
            "{} {}",
            w.merchant.at.y,
            bridge.at.y + solid::DECK
        );
        // and its pitch never jumps more than 2° a tick crossing it
        w.merchant.s = s + fx::int(12);
        w.merchant.dir = -1;
        let mut last: Option<i32> = None;
        for _ in 0..(fx::int(24) / SPEED) {
            step(&mut w);
            if let Some(p) = last {
                assert!(
                    (w.merchant.pitch - p).abs() <= deg(2),
                    "{} {}",
                    p,
                    w.merchant.pitch
                );
            }
            last = Some(w.merchant.pitch);
        }
    }
}
