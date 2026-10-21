//! `wounds` (M7 polish, task P4, T9, CW-16): hits read on the body.
//!
//! **Damage on each part.** A body has the ten parts `blade::body` gives it
//! (head, torso, each upper arm and forearm, each thigh and shin); a blow
//! lands on the capsule the blade's swept contact struck (`Hit::seg`). Health
//! still decides death (`health.rs`); each part also keeps what it has taken
//! (`Limbs::dmg`).
//!
//! **What a blow shows.** The struck part flashes red for `flash_ticks`, the
//! rest of the body a little; blood sprays along the blade's travel at the
//! contact, more the faster the blade; the part takes a stain that fades;
//! the struck part flinches as drawn (a head snaps, a trunk doubles, an
//! elbow is knocked, a knee buckles, along the blow), over `feel.rs`'s
//! stagger, which already shoves a body by the health it took. (Physical
//! reactions per part were tried and moved to drawing: R9.131.)
//!
//! **A part comes off** at its joint once its damage reaches its share in
//! `data/wounds.json`, or at once to one blow at least `one_cut_cms` fast.
//! The part becomes a `Piece`: three Verlet points held rigid, which fall,
//! tumble and lie exactly still on the ground. The stump is capped and
//! bleeds a while. A sword hand lost drops the weapon; a leg lost topples
//! the body, which then hops on the other (`motion::hop`); both legs lost,
//! it lies on its front and crawls on its arms. A fighter that has lost its
//! sword hand can take its weapon up in the other hand (the draw key, near
//! it), weaker for it; pilots choose to through their trees (CW-3).
//!
//! **Blood** is faceted drops (`Drop`), flown and landed as splats on the
//! ground (`Splat`), a pool under a fallen body; every count is capped
//! (`most`), and every fade is a dither, never a blend (CW-18). The reader's
//! setting `blood` (M4's settings) turns it all off.
//!
//! **Sounds.** Each tick's cues (`Cue`: a cut, a part taken off, gore) are
//! kept for the page's sound (`cues_since`).
//!
//! How a module joins the world (state, step, draw, scene, screen, act):
//! the extension contract in `world.rs`. `steer` runs at 3b (after the feet,
//! before the arms), `take` inside `health::step`, `step` after the
//! throwables.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::arm;
use crate::blade::{self, Hit, PARTS};
use crate::fx::{self, Fx, ONE};
use crate::geom::{facing, right_of, v3, Affine, V3};
use crate::input::{self, Input};
use crate::look::{mix, rgb, Look, Rgb};
use crate::models::{self, Dress, Model};
use crate::player::{self, Body, Fighter, Sheath};
use crate::pose::{self, Kind, Pose, LEFT, RIGHT};
use crate::prim::Mesh;
use crate::render::{Pass, Shape, Shapes};
use crate::world::World;

// ---- the parts ------------------------------------------------------------

/// The parts, by their capsule's index in `blade::body`.
pub const HEAD: usize = 0;
pub const TORSO: usize = 1;
pub const UPPER_ARM: [usize; 2] = [2, 4];
pub const FOREARM: [usize; 2] = [3, 5];
pub const THIGH: [usize; 2] = [6, 8];
pub const SHIN: [usize; 2] = [7, 9];
/// Each part's name, as the test acts take it.
pub const NAMES: [&str; PARTS] = [
    "head",
    "torso",
    "upper_arm_l",
    "forearm_l",
    "upper_arm_r",
    "forearm_r",
    "thigh_l",
    "shin_l",
    "thigh_r",
    "shin_r",
];

pub const fn bit(s: usize) -> u16 {
    1 << s
}

/// What comes off with part `s`: an upper arm takes its forearm, a thigh
/// its shin.
pub fn with(s: usize) -> u16 {
    match s {
        2 | 4 | 6 | 8 => bit(s) | bit(s + 1),
        _ => bit(s),
    }
}

/// The arm on side `side` (0 left, 1 right) has lost a part.
pub fn arm_lost(lost: u16, side: usize) -> bool {
    lost & (bit(UPPER_ARM[side]) | bit(FOREARM[side])) != 0
}

pub fn leg_lost(lost: u16, side: usize) -> bool {
    lost & (bit(THIGH[side]) | bit(SHIN[side])) != 0
}

/// How many legs are gone.
pub fn legs_lost(lost: u16) -> usize {
    leg_lost(lost, 0) as usize + leg_lost(lost, 1) as usize
}

/// The part a blow on `s` counts on once `s` is gone: a stump's blow is the
/// limb's above it, and the trunk's above that.
pub fn landed_on(s: usize, lost: u16) -> usize {
    let mut s = s;
    while lost & bit(s) != 0 {
        s = match s {
            3 | 5 | 7 | 9 => s - 1,
            _ => TORSO,
        };
    }
    s
}

// ---- the numbers: data/wounds.json ----------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TakeOff {
    pub head: i32,
    pub upper_arm: i32,
    pub forearm: i32,
    pub thigh: i32,
    pub shin: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OneCut {
    pub head: i32,
    pub limb: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DropSpec {
    pub most: usize,
    pub per_mps: i32,
    pub fewest: i32,
    pub most_a_blow: i32,
    pub carry_pct: i32,
    pub spread_cms: i32,
    pub rise_cms: i32,
    pub mm: [i32; 2],
    pub take_off_burst: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplatSpec {
    pub most: usize,
    pub life: u16,
    pub fade: u16,
    pub far_m: i32,
    pub near_m: i32,
    pub lift_mm: i32,
    pub layer_mm: i32,
    pub layers: u8,
    pub thick_mm: i32,
    pub mm: [i32; 2],
    pub grow_mm: i32,
    pub most_mm: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolSpec {
    pub grow_ticks: u16,
    pub mm: i32,
    pub after: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PieceSpec {
    pub most: usize,
    pub fade: u8,
    pub far_m: i32,
    pub rest_by: u16,
    pub kick_pct: i32,
    pub rise_cms: i32,
    pub ground_pct: i32,
    pub air_pct: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colours {
    pub flash: [i32; 3],
    pub blood: [i32; 3],
    pub blood_dark: [i32; 3],
    pub stump: [i32; 3],
    pub stump_plain: [i32; 3],
    pub bone: [i32; 3],
}

/// `data/wounds.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub take_off: TakeOff,
    pub one_cut_cms: OneCut,
    pub player_pct: i32,
    pub take_off_hp: i32,
    pub flash_ticks: u8,
    pub flash_pct: i32,
    pub body_flash_pct: i32,
    pub stain_ticks: u16,
    pub stain_pct: i32,
    pub flinch_ticks: u8,
    pub flinch_mm: i32,
    pub bleed_ticks: u16,
    pub bleed_every: u16,
    pub stump_drain_every: u16,
    pub drops: DropSpec,
    pub splats: SplatSpec,
    pub pool: PoolSpec,
    pub pieces: PieceSpec,
    pub weapon_rest_by: u16,
    pub hop_pct: i32,
    pub crawl_pct: i32,
    pub hop_mm: i32,
    pub hop_ticks: u16,
    pub switched_pct: i32,
    pub player_switched_pct: i32,
    pub off_lost_pct: i32,
    pub topple_lie: u16,
    pub topple_rise: u16,
    pub take_up_cm: i32,
    pub take_up_reach: u8,
    pub take_up_lift: u8,
    pub stoop_deg: i32,
    pub crouch_mm: i32,
    pub cap_pct: i32,
    pub colours: Colours,
}

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let mut v: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("The wounds file could not be read: {e}"))?;
        if let Some(o) = v.as_object_mut() {
            o.retain(|k, _| !k.starts_with('_'));
        }
        let s: Spec =
            serde_json::from_value(v).map_err(|e| format!("The wounds file could not be read: {e}"))?;
        if s.flash_ticks == 0 || s.hop_ticks == 0 || s.take_up_reach == 0 || s.take_up_lift == 0 {
            return Err("The wounds file's flash, hop and take-up need a tick at least.".into());
        }
        Ok(s)
    }

    /// The damage that takes part `s` off a body of `kind`.
    pub fn take_off(&self, s: usize, kind: Kind) -> i32 {
        let t = &self.take_off;
        let base = match s {
            HEAD => t.head,
            2 | 4 => t.upper_arm,
            3 | 5 => t.forearm,
            6 | 8 => t.thigh,
            7 | 9 => t.shin,
            _ => i32::MAX,
        };
        if kind == Kind::Player && base != i32::MAX {
            base * self.player_pct / 100
        } else {
            base
        }
    }

    /// The speed of one blow that takes part `s` off at once, m/s.
    pub fn one_cut(&self, s: usize) -> Fx {
        let cms = match s {
            TORSO => return Fx::MAX,
            HEAD => self.one_cut_cms.head,
            _ => self.one_cut_cms.limb,
        };
        cms * ONE / 100
    }
}

static SPEC: OnceLock<Spec> = OnceLock::new();

/// The wounds file, compiled in and parsed on first use.
pub fn spec() -> &'static Spec {
    SPEC.get_or_init(|| Spec::parse(include_str!("../../../data/wounds.json")).expect("data/wounds.json"))
}

// ---- one body's wounds ----------------------------------------------------

/// What a fighter's body has taken, part by part (kept in its `Health`, so a
/// fighter made whole again is whole here too).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Limbs {
    /// damage taken on each part
    pub dmg: [i32; PARTS],
    /// ticks of red flash left on each part
    pub flash: [u8; PARTS],
    /// ticks of blood stain left on each part
    pub stain: [u16; PARTS],
    /// the parts gone, a bit each
    pub lost: u16,
    /// ticks the stumps bleed, and to the next drain of health
    pub bleed: u16,
    pub drain: u16,
    /// ticks since a leg went (0: none); `prone` with both gone
    pub topple: u16,
    pub prone: bool,
    /// the weapon is held in the left hand: the arm's points are swapped
    /// side for side (`swap`)
    pub switched: bool,
    /// the weapon has dropped and lies free; `settled` once it lies still,
    /// held exactly where it lies
    pub loose: bool,
    pub settled: bool,
    pub loose_ticks: u16,
    /// ticks into taking the weapon up in the other hand (0: not), and where
    /// its pommel and tip were when the hand closed on it
    pub take_up: u8,
    pub take_from: (V3, V3),
    /// a flinch on part `flinch_seg` along `flinch_way`, ticks left
    pub flinch: u8,
    pub flinch_seg: u8,
    pub flinch_way: V3,
}

impl Limbs {
    /// The arm side (0 or 1, as `arm.rs` numbers them) on physical side `s`.
    pub fn arm_side(&self, s: usize) -> usize {
        if self.switched {
            1 - s
        } else {
            s
        }
    }

    /// The physical side of the hand that holds the weapon.
    pub fn sword_side(&self) -> usize {
        self.arm_side(1)
    }

    /// The topple's lie and rise: `ONE` lying, eased in and out; negative
    /// for a body lying on its front (`prone`).
    pub fn fall(&self) -> Fx {
        let sp = spec();
        let t = self.topple as i32;
        if t == 0 {
            return 0;
        }
        let fall = crate::health::FALL_TICKS as i32;
        let lie = sp.topple_lie as i32;
        let rise = sp.topple_rise as i32;
        let m = if t <= fall {
            let u = t * ONE / fall;
            fx::mul(u, u)
        } else if self.prone || t <= fall + lie {
            ONE
        } else if t <= fall + lie + rise {
            ONE - ease((t - fall - lie) * ONE / rise)
        } else {
            0
        };
        if self.prone {
            -m
        } else {
            m
        }
    }

    /// The topple has the body on the ground, unable to walk: falling, lying
    /// or getting up (a prone body crawls once it is down).
    pub fn toppled(&self) -> bool {
        let sp = spec();
        let t = self.topple;
        let fall = crate::health::FALL_TICKS;
        if t == 0 {
            false
        } else if self.prone {
            t < fall
        } else {
            t < fall + sp.topple_lie + sp.topple_rise
        }
    }

    /// One leg gone and the body up on the other.
    pub fn hopping(&self) -> bool {
        legs_lost(self.lost) == 1 && !self.toppled()
    }

    /// Both legs gone and down on its front.
    pub fn crawling(&self) -> bool {
        self.prone && !self.toppled()
    }

    /// The last tick of the topple, past which it counts no more.
    fn topple_end(&self) -> u16 {
        let sp = spec();
        if self.prone {
            crate::health::FALL_TICKS
        } else {
            crate::health::FALL_TICKS + sp.topple_lie + sp.topple_rise
        }
    }
}

fn ease(x: Fx) -> Fx {
    let x = x.clamp(0, ONE);
    fx::mul(fx::mul(x, x), 3 * ONE - 2 * x)
}

// ---- the world's share ----------------------------------------------------

/// A drop of blood in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drop {
    pub p: V3,
    pub was: V3,
    /// metres a tick
    pub v: V3,
    pub size: Fx,
    pub age: u16,
}

/// A mark of blood on the ground: a drop's splat, or a pool under a fallen
/// body growing to `goal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Splat {
    pub at: V3,
    /// the ground's up there
    pub n: V3,
    pub r: Fx,
    pub goal: Fx,
    pub age: u16,
    /// ticks before a pool begins to spread
    pub wait: u16,
    /// which of `layers` heights it lies at: overlapping marks never share one
    pub layer: u8,
    pub tone: u8,
}

/// A part taken off: three points held rigid (its joint, its far end, and a
/// third off their line, so it can roll), in Verlet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub kind: Kind,
    pub dress: (u8, u8),
    /// the body's pose when it came off, and the model's parts it carries,
    /// placed as they were then: the piece is drawn from them
    pub pose: Pose,
    pub parts: Vec<models::Part>,
    /// the parts it carries, and the one it was cut at
    pub segs: u16,
    pub root: u8,
    /// its points when cut, now, a tick ago (Verlet), and as last drawn
    pub at0: [V3; 3],
    pub p: [V3; 3],
    pub q: [V3; 3],
    pub was: [V3; 3],
    pub len: [Fx; 3],
    pub r: Fx,
    pub age: u16,
    pub still: bool,
    pub landed: bool,
    /// ticks into its fade out (0: whole)
    pub fade: u8,
}

/// Pieces kept below `pieces.most` for those fading out.
pub const PIECE_ROOM: usize = 3;

/// What a sound would play: a cut that drew blood, a part taken off, gore
/// (a head off, a piece landing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CueKind {
    Cut,
    Sever,
    Gore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Cue {
    pub tick: u64,
    pub kind: CueKind,
    pub at: V3,
    /// the blow's speed, m/s (Q16), for loudness
    pub speed: Fx,
}

/// How many ticks of cues are kept for the page.
pub const CUE_TICKS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    pub drops: Vec<Drop>,
    pub splats: Vec<Splat>,
    pub pieces: Vec<Piece>,
    pub cues: Vec<Cue>,
    /// pools to come: the fighter, ticks left
    pub pools: Vec<(u8, u16)>,
    /// how many of this tick's hits `take` had seen (the rest are the
    /// throwables', marked in `step`)
    pub taken: usize,
    /// held still for a view (`wounds.freeze`), or from this tick on
    pub frozen: bool,
    pub freeze_at: Option<u64>,
    /// drops and splats made since the world began, for the tones
    pub made: u32,
}

fn blood_on(w: &World) -> bool {
    w.ui.settings.blood
}

/// A seeded spread from −`ONE` to `ONE`, for the drops (splitmix64): the
/// same tick, blow and drop give the same number (CW-4), and no draw is
/// taken from the world's chance, so blood changes no other outcome.
fn spread(key: u64) -> Fx {
    let mut z = key.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    ((z % (2 * ONE as u64 + 1)) as i64 - ONE as i64) as Fx
}

fn mm(v: i32) -> Fx {
    (v as i64 * ONE as i64 / 1000) as Fx
}

fn cms(v: i32) -> Fx {
    v * ONE / 100 / crate::ticks::TICKS_PER_SECOND as i32
}

fn cue(w: &mut World, kind: CueKind, at: V3, speed: Fx) {
    w.wounds.cues.push(Cue {
        tick: w.tick,
        kind,
        at,
        speed,
    });
}

/// The cues since `tick` (exclusive), oldest first: what the page's sound
/// plays (P1).
pub fn cues_since(w: &World, tick: u64) -> Vec<Cue> {
    w.wounds.cues.iter().filter(|c| c.tick > tick).copied().collect()
}

// ---- blows ----------------------------------------------------------------

/// Step 4, inside `health::step` after health is taken: each blow marked on
/// its part, and parts that have taken enough come off.
pub fn take(w: &mut World) {
    let hits = w.blades.hits.clone();
    w.wounds.taken = hits.len();
    for (k, h) in hits.iter().enumerate() {
        mark(w, h, k, true);
    }
}

/// One blow on fighter `h.on`: the part's damage, flash, stain and flinch,
/// the blood it sheds, and (with `cuts`) whether it takes the part off.
pub fn mark(w: &mut World, h: &Hit, k: usize, cuts: bool) {
    let sp = spec();
    let id = h.on as usize;
    if id >= player::count(w) {
        return;
    }
    let taken = crate::health::damage(h.speed, h.part);
    if taken <= 0 {
        return;
    }
    let kind = player::fighter(w, id).kind;
    let lost = player::fighter(w, id).health.limbs.lost;
    let s = landed_on((h.seg as usize).min(PARTS - 1), lost);
    let way = if h.vel == V3::ZERO {
        let d = player::body(w, id).pos.sub(player::body(w, h.by as usize).pos);
        v3(d.x, 0, d.z).norm()
    } else {
        h.vel.norm()
    };
    {
        let f = player::parts_mut(w, id).1;
        let l = &mut f.health.limbs;
        l.dmg[s] += taken;
        l.flash[s] = sp.flash_ticks;
        l.stain[s] = sp.stain_ticks;
        l.flinch = sp.flinch_ticks;
        l.flinch_seg = s as u8;
        l.flinch_way = v3(way.x, way.y / 3, way.z).norm();
    }
    cue(w, CueKind::Cut, h.at, h.speed);
    if blood_on(w) {
        let n = (sp.drops.fewest + fx::mul(h.speed, sp.drops.per_mps * ONE / 100) / ONE)
            .clamp(sp.drops.fewest, sp.drops.most_a_blow);
        let carry = h.vel.scale(sp.drops.carry_pct * ONE / 100);
        let key = w.tick.wrapping_mul(977) ^ ((h.by as u64) << 40) ^ ((h.on as u64) << 32) ^ (k as u64) << 16;
        spray(w, h.at, carry, n, key);
    }
    if !cuts {
        return;
    }
    let dmg = player::fighter(w, id).health.limbs.dmg[s];
    if s != TORSO && (dmg >= sp.take_off(s, kind) || h.speed >= sp.one_cut(s)) {
        take_off(w, id, s, h.vel, h.speed);
    }
}

/// `n` drops thrown from `at` with the speed `carry` (metres a tick) and a
/// seeded spread round it.
fn spray(w: &mut World, at: V3, carry: V3, n: i32, key: u64) {
    let sp = spec();
    let d = &sp.drops;
    for j in 0..n {
        if w.wounds.drops.len() >= d.most {
            break;
        }
        let r = |m: u64| spread(key ^ (j as u64).wrapping_mul(0x51_7cc1) ^ m);
        let jitter = v3(
            fx::mul(r(1), cms(d.spread_cms)),
            fx::mul(r(2), cms(d.spread_cms)) / 2 + cms(d.rise_cms),
            fx::mul(r(3), cms(d.spread_cms)),
        );
        let size = mm(d.mm[0]) + fx::mul((r(4) + ONE) / 2, mm(d.mm[1] - d.mm[0]));
        w.wounds.drops.push(Drop {
            p: at,
            was: at,
            v: carry.add(jitter),
            size,
            age: 0,
        });
    }
}

/// Part `s` of fighter `id` comes off at its joint (with what hangs from it),
/// struck along `vel` at `speed`.
pub fn take_off(w: &mut World, id: usize, s: usize, vel: V3, speed: Fx) {
    let sp = spec();
    let (kind, lost, now, before) = {
        let f = player::fighter(w, id);
        (f.kind, f.health.limbs.lost, player::pose_of(w, id), f.prev_pose)
    };
    let segs = with(s) & !lost;
    if segs == 0 || s == TORSO {
        return;
    }
    let dress = if id == 0 {
        (crate::items::speedster_mask(w), 0)
    } else {
        (0, 0)
    };
    let mut piece = piece_of(kind, dress, &now, &before, s, segs);
    let kick = vel.scale(sp.pieces.kick_pct * ONE / 100);
    let rise = v3(0, cms(sp.pieces.rise_cms), 0);
    for (k, share) in [(0, ONE / 3), (1, ONE), (2, ONE * 2 / 3)] {
        piece.q[k] = piece.q[k].sub(kick.scale(share)).sub(rise);
    }
    // near the most kept, the oldest begins to fade out; at the most, the
    // one furthest faded (or the oldest) goes, so there are never more
    let most = sp.pieces.most;
    if w.wounds.pieces.iter().filter(|p| p.fade == 0).count() + PIECE_ROOM >= most {
        if let Some(old) = w.wounds.pieces.iter_mut().find(|p| p.fade == 0) {
            old.fade = 1;
        }
    }
    if w.wounds.pieces.len() >= most {
        let k = (0..w.wounds.pieces.len())
            .max_by_key(|&k| (w.wounds.pieces[k].fade, std::cmp::Reverse(k)))
            .unwrap_or(0);
        w.wounds.pieces.remove(k);
    }
    let joint = piece.p[0];
    w.wounds.pieces.push(piece);
    let gore = s == HEAD;
    {
        let f = player::parts_mut(w, id).1;
        let l = &mut f.health.limbs;
        l.lost |= segs;
        l.bleed = sp.bleed_ticks;
        l.drain = sp.stump_drain_every;
        l.flash[s] = 0;
        if gore {
            f.health.hp = 0;
        } else {
            f.health.hp -= sp.take_off_hp;
        }
        let legs = legs_lost(l.lost);
        if segs & (bit(THIGH[0]) | bit(SHIN[0]) | bit(THIGH[1]) | bit(SHIN[1])) != 0 {
            l.topple = 1;
            l.prone = legs == 2;
        }
        let hand = arm_lost(l.lost, l.sword_side()) && segs & (bit(2) | bit(3) | bit(4) | bit(5)) != 0;
        if hand || l.prone {
            drop_weapon(f);
        }
    }
    cue(w, CueKind::Sever, joint, speed);
    if gore {
        cue(w, CueKind::Gore, joint, speed);
    }
    if blood_on(w) {
        let carry = vel.scale(ONE / 5);
        let key = w.tick.wrapping_mul(31) ^ ((id as u64) << 24) ^ (s as u64) << 8 ^ 0xb100d;
        spray(w, joint, carry, sp.drops.take_off_burst, key);
    }
}

/// The weapon falls from the hand: free until it lies still.
fn drop_weapon(f: &mut Fighter) {
    let l = &mut f.health.limbs;
    if l.loose {
        return;
    }
    l.loose = true;
    l.settled = false;
    l.loose_ticks = 0;
    l.take_up = 0;
    f.sheath = Sheath::Drawn;
}

/// The three points of part `s` cut from `pose` (and a tick before, for its
/// speed), with what it carries.
fn piece_of(kind: Kind, dress: (u8, u8), now: &Pose, before: &Pose, s: usize, segs: u16) -> Piece {
    let b = pose::build(kind);
    let pts = |p: &Pose| -> [V3; 3] {
        let up = p.head.sub(p.chest).norm();
        let fwd = facing(p.facing, 0);
        let (a, e) = match s {
            HEAD => (p.head.sub(up.scale(b.head / 2)), p.head.add(up.scale(b.head / 2))),
            2 | 4 => {
                let k = (s - 2) / 2;
                (p.shoulder[k], p.hand[k])
            }
            3 | 5 => {
                let k = (s - 3) / 2;
                (p.elbow[k], p.hand[k])
            }
            6 | 8 => {
                let k = (s - 6) / 2;
                (p.hip[k], p.foot[k])
            }
            _ => {
                let k = (s - 7) / 2;
                (p.knee[k], p.foot[k])
            }
        };
        let mid = match s {
            2 | 4 => Some(p.elbow[(s - 2) / 2]),
            6 | 8 => Some(p.knee[(s - 6) / 2]),
            _ => None,
        };
        let line = e.sub(a).norm();
        let off = |hint: V3| {
            let h = hint.sub(line.scale(hint.dot(line)));
            let h = if h.len() < ONE / 8 { right_of(p.facing) } else { h };
            a.lerp(e, ONE / 2).add(h.norm().scale(ONE * 8 / 100))
        };
        let c = match mid {
            Some(m) if arm::nearest_on(a, e, m).dist(m) > ONE * 3 / 100 => m,
            _ => off(fwd),
        };
        [a, e, c]
    };
    let p = pts(now);
    let q = pts(before);
    let r = match s {
        HEAD => b.head * 45 / 100,
        2..=5 => mm(blade::FOREARM_MM) * b.height / pose::build(Kind::Player).height,
        _ => mm(blade::SHIN_MM) * b.height / pose::build(Kind::Player).height,
    };
    let len = [p[0].dist(p[1]), p[1].dist(p[2]), p[0].dist(p[2])];
    let d = Dress {
        worn: dress.0,
        variant: dress.1,
    };
    let m = models::figure_model(kind, now, d);
    let labels = labels(kind, d);
    let parts = if labels.len() == m.parts.len() {
        m.parts
            .iter()
            .zip(labels.iter())
            .filter(|(_, &l)| segs & bit(l as usize) != 0)
            .map(|(p, _)| *p)
            .collect()
    } else {
        Vec::new()
    };
    Piece {
        kind,
        dress,
        pose: *now,
        parts,
        segs,
        root: s as u8,
        at0: p,
        p,
        q,
        was: p,
        len,
        r,
        age: 0,
        still: false,
        landed: false,
        fade: 0,
    }
}

// ---- before the arms: the body's own state --------------------------------

/// Step 3b, after the feet and before the arms: each fighter's wounds tick
/// (flash, stain, flinch, bleeding, the topple), its weapon settles where it
/// fell, and the draw key near it takes it up in the other hand.
pub fn steer(w: &mut World, input: &Input, foes: &[Input]) {
    if w.wounds.frozen {
        return;
    }
    let sp = spec();
    for id in 0..player::count(w) {
        let inp = if id == 0 {
            *input
        } else {
            foes.get(id - 1).copied().unwrap_or_default()
        };
        let ground = {
            let b = player::body(w, id);
            crate::solid::surface(w, b.pos.x, b.pos.z)
        };
        let near = can_take_up(w, id);
        let (b, f) = player::parts_mut(w, id);
        let pressed = f.pressed(&inp, input::DRAW);
        let down = f.health.down();
        let l = &mut f.health.limbs;
        for s in 0..PARTS {
            l.flash[s] = l.flash[s].saturating_sub(1);
            l.stain[s] = l.stain[s].saturating_sub(1);
        }
        l.flinch = l.flinch.saturating_sub(1);
        if l.topple > 0 && l.topple < l.topple_end() {
            l.topple += 1;
        }
        if l.bleed > 0 {
            l.bleed -= 1;
            // a foe's stumps drain its health a little while they bleed
            if id != 0 && !down {
                l.drain = l.drain.saturating_sub(1);
                if l.drain == 0 {
                    l.drain = sp.stump_drain_every;
                    f.health.hp -= 1;
                }
            }
        }
        let l = &mut f.health.limbs;
        // the weapon lying free: still once its points barely move
        if l.loose && !l.settled {
            l.loose_ticks += 1;
            let a = &f.arm;
            let quiet = [arm::POMMEL, arm::TIP].iter().all(|&k| {
                let v = a.pts[k].vel();
                v.x.abs() <= arm::REST * 2 && v.y.abs() <= arm::REST * 2 && v.z.abs() <= arm::REST * 2
            });
            let low = [arm::POMMEL, arm::TIP]
                .iter()
                .all(|&k| a.pts[k].p.y <= ground + ONE * 15 / 100);
            if (quiet && low) || l.loose_ticks >= sp.weapon_rest_by {
                l.settled = true;
                for k in [arm::POMMEL, arm::TIP] {
                    f.arm.pts[k].q = f.arm.pts[k].p;
                }
            }
        }
        let l = &mut f.health.limbs;
        if l.take_up == 0 && l.loose && l.settled && pressed && near && !down {
            l.take_up = 1;
        } else if l.take_up > 0 {
            l.take_up += 1;
            if l.take_up == sp.take_up_reach + 1 {
                // the other hand has the hilt: it is the sword hand now
                l.take_from = (f.arm.pts[arm::POMMEL].p, f.arm.pts[arm::TIP].p);
                swap(f);
                let l = &mut f.health.limbs;
                l.loose = false;
                l.settled = false;
            } else if l.take_up > sp.take_up_reach + sp.take_up_lift {
                l.take_up = 0;
                f.sheath = Sheath::Drawn;
            }
        }
        // stooping to the weapon and straightening after
        let l = &f.health.limbs;
        let goal = if l.take_up > 0 && l.take_up <= sp.take_up_reach + sp.take_up_lift / 2 {
            ONE
        } else {
            0
        };
        f.gait.stoop = fx::approach(f.gait.stoop, goal, ONE / 8);
        let _ = b;
    }
}

/// Whether fighter `id` stands near enough its dropped weapon, with a whole
/// hand to take it in, to take it up.
pub fn can_take_up(w: &World, id: usize) -> bool {
    let f = player::fighter(w, id);
    let l = &f.health.limbs;
    if !l.loose || l.prone || l.toppled() || f.health.down() {
        return false;
    }
    let other = 1 - l.sword_side();
    if arm_lost(l.lost, other) {
        return false;
    }
    let grip = f.arm.grip_point();
    let feet = player::body(w, id).pos;
    grip.sub(feet).len_xz() <= spec().take_up_cm * ONE / 100
}

/// The arm's sides swapped: its points and its frame's shoulders, so `arm.rs`
/// steers what was the free hand as the sword hand.
pub fn swap(f: &mut Fighter) {
    let a = &mut f.arm;
    a.pts.swap(arm::ELBOW[0], arm::ELBOW[1]);
    a.pts.swap(arm::HAND[0], arm::HAND[1]);
    a.frame.shoulder.swap(0, 1);
    f.health.limbs.switched = !f.health.limbs.switched;
}

/// The frame the arms are given this tick: its shoulders swapped once the
/// weapon is in the left hand (`arm::step`).
pub fn sided(f: &Fighter, mut frame: arm::Frame) -> arm::Frame {
    if f.health.limbs.switched {
        frame.shoulder.swap(0, 1);
    }
    frame
}

/// The arms' goals as wounds change them (`player::goals`): a lost arm is
/// limp and holds nothing; the weapon left in a weaker hand; a dropped
/// weapon lying where it fell; the reach and lift of taking it up; a
/// crawl's hands pulling along the ground.
pub fn goals(
    w: &World,
    id: usize,
    (mut sheath, mut tilt, mut g): (Sheath, Fx, arm::Goals),
) -> (Sheath, Fx, arm::Goals) {
    let f = player::fighter(w, id);
    let l = &f.health.limbs;
    if l.lost == 0 && !l.loose && l.take_up == 0 && l.topple == 0 && !l.switched {
        return (sheath, tilt, g);
    }
    let sp = spec();
    let a = &f.arm;
    if l.loose || l.take_up > 0 {
        sheath = f.sheath;
        tilt = f.tilt;
    }
    for s in 0..2 {
        if arm_lost(l.lost, if l.switched { 1 - s } else { s }) {
            g.hand[s] = None;
            g.hold[s] = false;
            g.strength[s] = 0;
        }
    }
    if l.switched {
        let pct = if id == 0 {
            sp.player_switched_pct
        } else {
            sp.switched_pct
        };
        g.strength[1] = g.strength[1] * pct / 100;
    } else if arm_lost(l.lost, LEFT) {
        g.strength[1] = g.strength[1] * sp.off_lost_pct / 100;
    }
    let lying = (a.pts[arm::POMMEL].p, a.pts[arm::TIP].p);
    let still_weapon = |g: &mut arm::Goals| {
        g.in_hand = false;
        g.sheath = lying;
        g.hold = [false, false];
        g.ride = None;
        g.tip = None;
        g.target = None;
        g.swing = false;
    };
    if l.loose && l.take_up == 0 {
        if l.settled {
            still_weapon(&mut g);
        } else {
            // free, falling: no hand pushes it
            g.in_hand = true;
            g.hold = [false, false];
            g.hand[1] = None;
            g.ride = None;
            g.tip = None;
            g.target = None;
            g.swing = false;
        }
    }
    if l.take_up > 0 {
        if l.take_up <= sp.take_up_reach {
            // the free hand (arm side 0, not yet swapped) reaches for the hilt
            still_weapon(&mut g);
            g.hand[0] = Some(a.grip_point());
            g.strength[0] = 200;
        } else {
            // swapped: the hilt rises into the new sword hand
            let u = (l.take_up - sp.take_up_reach) as i32 * ONE / sp.take_up_lift as i32;
            let e = ease(u);
            let fr = &a.frame;
            let hand = a.shell.point(fr.shoulder[1], fr.yaw);
            let dir = arm::blade_dir(fr.yaw, &a.shell, a.reach());
            let pommel = hand.sub(dir.scale(a.weapon.grip[1]));
            let tip = pommel.add(dir.scale(a.weapon.len));
            let (p0, t0) = l.take_from;
            g.in_hand = true;
            g.hold = [false, true];
            g.hand[1] = Some(hand);
            g.target = Some(hand);
            g.tip = None;
            g.swing = false;
            g.ride = Some((p0.lerp(pommel, e), t0.lerp(tip, e), ONE));
        }
    }
    if l.toppled() {
        // falling and lying: the arms go slack
        g.hand = [None, None];
        g.tip = None;
        g.swing = false;
        g.target = None;
    } else if l.crawling() && !f.health.down() {
        let b = player::body(w, id);
        let fwd = facing(b.facing, 0);
        let reach = a.reach();
        let phase = f.gait.phase;
        for s in 0..2 {
            let phys = if l.switched { 1 - s } else { s };
            if arm_lost(l.lost, phys) || (s == 1 && l.loose && !l.settled) {
                continue;
            }
            let sh = a.frame.shoulder[s];
            let swing = fx::sin(phase.wrapping_add(if phys == 0 { 0 } else { fx::HALF_TURN }));
            let out = right_of(b.facing).scale(if phys == 0 { -ONE / 8 } else { ONE / 8 });
            let mut t = sh.add(fwd.scale(fx::mul(reach, ONE / 2 + swing / 3))).add(out);
            t.y = w.ground(t.x, t.z) + ONE / 20;
            g.hand[s] = Some(t);
            g.strength[s] = 140;
        }
        g.tip = None;
        g.swing = false;
        g.target = None;
    }
    (sheath, tilt, g)
}

// ---- the feet: hop and crawl ----------------------------------------------

/// The speed a wounded body's keys get: none while it topples or takes up
/// its weapon, a hop's share of a walk on one leg, a crawl's on none.
pub fn gait_want(f: &Fighter, want: V3, n: u8) -> V3 {
    let l = &f.health.limbs;
    if l.lost == 0 && l.take_up == 0 {
        return want;
    }
    let sp = spec();
    if l.toppled() || l.take_up > 0 {
        return V3::ZERO;
    }
    let walk = crate::motion::per_tick(crate::motion::WALK, n);
    if l.crawling() {
        want.clamp_len(walk * sp.crawl_pct / 100)
    } else if l.hopping() {
        want.clamp_len(walk * sp.hop_pct / 100)
    } else {
        want
    }
}

/// The body's stoop and crouch for taking up a weapon (`Gait::stoop`): a
/// point of the body's own frame (right, up, forward) bent forward at the
/// hips by up to `stoop_deg`, and lowered by up to `crouch_mm` (the knees
/// bend to it).
pub fn bend(hip_y: Fx, stoop: Fx) -> impl Fn(V3) -> V3 {
    let sp = spec();
    let a = fx::mul(stoop, fx::deg(sp.stoop_deg));
    let (c, s) = (fx::cos(a), fx::sin(a));
    let crouch = fx::mul(mm(sp.crouch_mm), stoop);
    move |p: V3| {
        if stoop == 0 {
            return p;
        }
        let mut q = p;
        if p.y > hip_y {
            let dy = p.y - hip_y;
            q.y = hip_y + fx::mul(dy, c);
            q.z = p.z + fx::mul(dy, s);
        }
        q.y -= fx::mul(crouch, fx::div(p.y.clamp(0, hip_y), hip_y.max(1)));
        q
    }
}

/// The joints as wounds change them, after `player::pose_from`: back to the
/// body's own sides once the weapon has changed hands, the hop's
/// standing leg, and every lost part folded into its joint, so the capsules
/// (`blade::body`) have no part where nothing is drawn.
pub fn shape_pose(b: &Body, f: &Fighter, p: &mut Pose) {
    let l = &f.health.limbs;
    if l.switched {
        p.shoulder.swap(0, 1);
        p.elbow.swap(0, 1);
        p.hand.swap(0, 1);
    }
    if l.hopping() && f.health.fall() == 0 {
        let build = pose::build(f.kind);
        let stand = if leg_lost(l.lost, LEFT) { RIGHT } else { LEFT };
        let mut foot = p.hip[stand];
        foot.y = b.pos.y + fx::mul(f.gait.lift, ONE * 3 / 4);
        let fwd = facing(b.facing, 0);
        let foot = foot.add(fwd.scale(ONE * 3 / 100));
        p.foot[stand] = foot;
        p.knee[stand] = crate::motion::knee(p.hip[stand], foot, build.thigh, build.shin, fwd);
    }
    if l.lost == 0 {
        return;
    }
    for side in [LEFT, RIGHT] {
        if l.lost & bit(UPPER_ARM[side]) != 0 {
            p.elbow[side] = p.shoulder[side];
            p.hand[side] = p.shoulder[side];
        } else if l.lost & bit(FOREARM[side]) != 0 {
            p.hand[side] = p.elbow[side];
        }
        if l.lost & bit(THIGH[side]) != 0 {
            p.knee[side] = p.hip[side];
            p.foot[side] = p.hip[side];
        } else if l.lost & bit(SHIN[side]) != 0 {
            p.foot[side] = p.knee[side];
        }
    }
    if l.lost & bit(HEAD) != 0 {
        p.head = p.chest;
    }
}

// ---- the world's step: blood and pieces ------------------------------------

/// Step 5b, after the throwables: their blows marked (no part comes off to a
/// rock), pools begun under the fallen, drops flown and landed, splats aged
/// and pools spread, pieces fallen and come to rest, cues kept.
pub fn step(w: &mut World) {
    if let Some(at) = w.wounds.freeze_at {
        if w.tick >= at {
            w.wounds.frozen = true;
            w.wounds.freeze_at = None;
        }
    }
    let taken = w.wounds.taken.min(w.blades.hits.len());
    let later: Vec<Hit> = w.blades.hits[taken..].to_vec();
    w.wounds.taken = 0;
    if w.wounds.frozen {
        return;
    }
    for (k, h) in later.iter().enumerate() {
        mark(w, h, k + 32, false);
    }
    let sp = spec();
    let tick = w.tick;
    w.wounds.cues.retain(|c| c.tick + CUE_TICKS > tick);
    let blood = blood_on(w);
    if !blood {
        w.wounds.drops.clear();
        w.wounds.splats.clear();
        w.wounds.pools.clear();
    }
    if blood {
        // a body's first tick down, however it went down
        for id in 0..player::count(w) {
            if player::fighter(w, id).health.down_ticks == 1 {
                w.wounds.pools.push((id as u8, sp.pool.after));
            }
        }
        bleed(w);
        pools(w);
        drops(w);
        for s in &mut w.wounds.splats {
            if s.wait > 0 {
                s.wait -= 1;
            } else if s.r < s.goal {
                s.r = (s.r + s.goal / sp.pool.grow_ticks.max(1) as i32 + 1).min(s.goal);
            }
            s.age = s.age.saturating_add(1);
        }
        let life = sp.splats.life;
        w.wounds.splats.retain(|s| s.age < life);
    }
    pieces(w);
}

/// Each stump bleeds while its body's `bleed` lasts, thinning out, and a
/// piece from its cut end for a while.
fn bleed(w: &mut World) {
    let sp = spec();
    for id in 0..player::count(w) {
        let f = player::fighter(w, id);
        let l = &f.health.limbs;
        if l.bleed == 0 || l.lost == 0 {
            continue;
        }
        let every = sp.bleed_every as u64 + (sp.bleed_ticks - l.bleed.min(sp.bleed_ticks)) as u64 / 40;
        if !(w.tick + id as u64).is_multiple_of(every.max(1)) {
            continue;
        }
        let p = &f.pose;
        let stumps: Vec<V3> = stumps(l.lost).iter().map(|&(s, _)| joint(p, s)).collect();
        for (k, at) in stumps.into_iter().enumerate() {
            let key = w.tick.wrapping_mul(131) ^ ((id as u64) << 20) ^ (k as u64) << 12 ^ 0x5717;
            spray(w, at, v3(0, -cms(40), 0), 1, key);
        }
    }
    let n = w.wounds.pieces.len();
    for k in 0..n {
        let pc = &w.wounds.pieces[k];
        if pc.age > 70 || !pc.age.is_multiple_of(5) || pc.fade > 0 {
            continue;
        }
        let at = pc.p[0];
        let key = w.tick.wrapping_mul(733) ^ (k as u64) << 10 ^ 0x9eca;
        spray(w, at, V3::ZERO, 1, key);
    }
}

/// Pools due under the fallen.
fn pools(w: &mut World) {
    let sp = spec();
    let mut due = Vec::new();
    for (id, left) in &mut w.wounds.pools {
        *left = left.saturating_sub(1);
        if *left == 0 {
            due.push(*id);
        }
    }
    w.wounds.pools.retain(|(_, left)| *left > 0);
    for id in due {
        if (id as usize) >= player::count(w) {
            continue;
        }
        let p = player::fighter(w, id as usize).pose;
        let at = p.pelvis.lerp(p.chest, ONE / 3);
        let ground = crate::solid::surface(w, at.x, at.z);
        lay(w, v3(at.x, ground, at.z), mm(sp.pool.mm), true);
    }
}

/// A tick of hit-stop (`feel::hold`): drops and pieces hold where they are,
/// drawn there.
pub fn hold(w: &mut World) {
    for d in &mut w.wounds.drops {
        d.was = d.p;
    }
    for pc in &mut w.wounds.pieces {
        pc.was = pc.p;
    }
}

/// Every drop flies a tick; one that reaches the ground leaves a splat.
fn drops(w: &mut World) {
    let sp = spec();
    let mut landed = Vec::new();
    let mut keep = Vec::with_capacity(w.wounds.drops.len());
    let list = std::mem::take(&mut w.wounds.drops);
    for mut d in list {
        d.was = d.p;
        d.v.y -= arm::GRAVITY;
        d.p = d.p.add(d.v);
        d.age += 1;
        let g = crate::solid::surface(w, d.p.x, d.p.z);
        if d.p.y <= g {
            landed.push((v3(d.p.x, g, d.p.z), d.size));
        } else if d.age < 240 {
            keep.push(d);
        }
    }
    w.wounds.drops = keep;
    for (at, size) in landed {
        let r = (size * 5 / 2).clamp(mm(sp.splats.mm[0]), mm(sp.splats.mm[1]));
        lay(w, at, r, false);
    }
}

/// Places among `splats.most` that only a pool may take.
pub const POOL_ROOM: usize = 4;

/// A mark of blood at `at`, `r` round (a pool grows to `r`): it spreads an
/// old mark it lands in, or lies at a height no mark it overlaps lies at.
fn lay(w: &mut World, at: V3, r: Fx, pool: bool) {
    let sp = spec();
    let ss = &sp.splats;
    let overlapping: Vec<usize> = (0..w.wounds.splats.len())
        .filter(|&k| {
            let s = &w.wounds.splats[k];
            s.at.sub(at).len_xz() < s.goal.max(s.r) + r
        })
        .collect();
    // landing inside a mark: the mark spreads (a pool takes no more)
    if !pool {
        if let Some(&k) = overlapping
            .iter()
            .find(|&&k| w.wounds.splats[k].at.sub(at).len_xz() < w.wounds.splats[k].r)
        {
            // it spreads unless that would reach a mark at its own height
            let s = w.wounds.splats[k];
            let grown = (s.r + mm(ss.grow_mm)).min(mm(ss.most_mm)).max(s.r);
            let clear = w.wounds.splats.iter().enumerate().all(|(j, o)| {
                j == k || o.layer != s.layer || o.at.sub(s.at).len_xz() >= o.goal.max(o.r) + grown
            });
            let s = &mut w.wounds.splats[k];
            if s.goal == s.r && clear {
                s.r = grown;
                s.goal = grown;
            }
            s.age = s.age.min(ss.life / 2);
            return;
        }
    }
    let used: Vec<u8> = overlapping.iter().map(|&k| w.wounds.splats[k].layer).collect();
    // a drop's mark takes the lowest height free among those it overlaps; a
    // pool one of the two above every drop's, so the small marks it spreads
    // over never take its height
    let heights = if pool {
        ss.layers..ss.layers + 2
    } else {
        0..ss.layers
    };
    let Some(layer) = heights.into_iter().find(|l| !used.contains(l)) else {
        return;
    };
    // the last `POOL_ROOM` places are kept for pools
    let room = if pool {
        ss.most
    } else {
        ss.most.saturating_sub(POOL_ROOM)
    };
    if w.wounds.splats.len() >= room {
        // full: this mark is not made, and unless one is already going,
        // the oldest begins its fade to make room
        let fade_from = ss.life - ss.fade;
        let going = w.wounds.splats.iter().any(|s| s.age >= fade_from);
        if !going {
            if let Some(k) = (0..w.wounds.splats.len()).max_by_key(|&k| (w.wounds.splats[k].age, k)) {
                w.wounds.splats[k].age = fade_from;
            }
        }
        return;
    }
    let h = |x: Fx, z: Fx| crate::solid::surface(w, x, z);
    let e = r.max(ONE / 10);
    let n = v3(
        h(at.x - e, at.z) - h(at.x + e, at.z),
        2 * e,
        h(at.x, at.z - e) - h(at.x, at.z + e),
    )
    .norm();
    w.wounds.made = w.wounds.made.wrapping_add(1);
    let tone = (w.wounds.made % 4) as u8;
    w.wounds.splats.push(Splat {
        at,
        n,
        r: if pool { 0 } else { r },
        goal: r,
        age: 0,
        wait: 0,
        layer,
        tone,
    });
}

/// Every piece falls and tumbles, comes to rest exactly, and fades out once
/// the player is far from it or it is the oldest of too many.
fn pieces(w: &mut World) {
    let sp = spec();
    let ps = &sp.pieces;
    let me = w.player.pos;
    let far = ps.far_m * ONE;
    let mut thuds = Vec::new();
    let mut list = std::mem::take(&mut w.wounds.pieces);
    for pc in &mut list {
        pc.was = pc.p;
        if pc.fade > 0 {
            pc.fade = pc.fade.saturating_add(1);
        } else if pc.p[0].sub(me).len_xz() > far {
            pc.fade = 1;
        }
        if pc.still {
            continue;
        }
        pc.age += 1;
        let ground: Vec<Fx> =
            pc.p.iter()
                .map(|p| crate::solid::surface(w, p.x, p.z) + pc.r)
                .collect();
        for (k, &floor) in ground.iter().enumerate() {
            let mut v = pc.p[k].sub(pc.q[k]);
            v = v.sub(v.scale(ps.air_pct * ONE / 100));
            if pc.p[k].y <= floor + ONE / 200 {
                v.x -= v.x * ps.ground_pct / 100;
                v.z -= v.z * ps.ground_pct / 100;
            }
            v.y -= arm::GRAVITY;
            pc.q[k] = pc.p[k];
            pc.p[k] = pc.p[k].add(v.clamp_len(arm::CAP));
        }
        for _ in 0..4 {
            for (i, j, len) in [(0, 1, pc.len[0]), (1, 2, pc.len[1]), (0, 2, pc.len[2])] {
                let d = pc.p[j].sub(pc.p[i]);
                let dist = d.len();
                if dist == 0 {
                    continue;
                }
                let half = d.scale(fx::div(dist - len, dist) / 2);
                pc.p[i] = pc.p[i].add(half);
                pc.p[j] = pc.p[j].sub(half);
            }
            for (k, &floor) in ground.iter().enumerate() {
                if pc.p[k].y < floor {
                    pc.p[k].y = floor;
                    pc.q[k].y = pc.q[k].y.max(pc.p[k].y);
                    if !pc.landed {
                        pc.landed = true;
                        thuds.push(pc.p[k]);
                    }
                }
            }
        }
        // a piece come down on its end would stand balanced: lying, its top
        // end is nudged over, the way it leans or along its third point
        if pc.landed {
            let (lo, hi) = if pc.p[0].y < pc.p[1].y { (0, 1) } else { (1, 0) };
            if pc.p[hi].y - pc.p[lo].y > pc.r * 2 {
                let lean = pc.p[hi].sub(pc.p[lo]);
                let side = pc.p[2].sub(pc.p[lo]);
                let way = [v3(lean.x, 0, lean.z), v3(side.x, 0, side.z), v3(ONE, 0, 0)]
                    .into_iter()
                    .find(|v| v.len() > ONE / 100)
                    .unwrap_or(v3(ONE, 0, 0));
                pc.p[hi] = pc.p[hi].add(way.norm().scale(ONE / 400));
            }
        }
        let quiet = (0..3).all(|k| {
            let d = pc.p[k].sub(pc.was[k]);
            d.x.abs() <= arm::REST && d.y.abs() <= arm::REST && d.z.abs() <= arm::REST
        });
        if (pc.landed && quiet && pc.age > 10) || pc.age >= ps.rest_by {
            pc.still = true;
            pc.p = pc.was;
            pc.q = pc.was;
        }
    }
    list.retain(|pc| pc.fade <= ps.fade);
    w.wounds.pieces = list;
    for at in thuds {
        cue(w, CueKind::Gore, at, 0);
    }
}

// ---- drawing --------------------------------------------------------------

/// The part (`blade::body`'s index) each part of a kind's model is drawn on:
/// built on a pose with the arms and legs spread wide, each model part goes
/// to the capsule whose surface its centre is nearest (the joint balls to the
/// part above: the elbow to the upper arm, the knee to the thigh).
pub fn labels(kind: Kind, dress: Dress) -> std::borrow::Cow<'static, [u8]> {
    // worked out once for each kind and set of speedster pieces worn (a
    // villager's looks are not cut: they are not fighters)
    let at = models::KINDS.iter().position(|&k| k == kind);
    match at {
        Some(k) if dress.variant == 0 && dress.worn < 32 => std::borrow::Cow::Borrowed(
            LABELS[k * 32 + dress.worn as usize].get_or_init(|| labels_of(kind, dress)),
        ),
        _ => std::borrow::Cow::Owned(labels_of(kind, dress)),
    }
}

static LABELS: [OnceLock<Vec<u8>>; 8 * 32] = [const { OnceLock::new() }; 8 * 32];

fn labels_of(kind: Kind, dress: Dress) -> Vec<u8> {
    let p = spread_pose(kind);
    let caps = blade::body(kind, &p);
    let m = models::figure_model(kind, &p, dress);
    m.parts
        .iter()
        .map(|part| {
            let c = part.at.t;
            let mut best = (i64::MAX, 0u8);
            for (k, cap) in caps.iter().enumerate() {
                let on = arm::nearest_on(cap.a, cap.b, c);
                let d = on.dist(c) as i64 - cap.r as i64;
                if d < best.0 {
                    best = (d, k as u8);
                }
            }
            best.1
        })
        .collect()
}

/// A kind standing with its arms straight out and its legs apart: no part
/// near another's.
fn spread_pose(kind: Kind) -> Pose {
    let b = pose::build(kind);
    let mut p = models::rest_pose(kind, V3::ZERO, 0);
    let r = right_of(0);
    for (s, side) in [(LEFT, -1), (RIGHT, 1)] {
        let out = r.scale(side * ONE);
        p.elbow[s] = p.shoulder[s].add(out.scale(b.upper_arm));
        p.hand[s] = p.elbow[s].add(out.scale(b.forearm));
        let leg = v3(0, -ONE, 0).add(out.scale(ONE / 3)).norm();
        p.knee[s] = p.hip[s].add(leg.scale(b.thigh));
        p.foot[s] = p.knee[s].add(leg.scale(b.shin));
    }
    p
}

/// The joint a stump on part `s` is capped at, on pose `p`.
fn joint(p: &Pose, s: usize) -> V3 {
    match s {
        HEAD => p.chest.lerp(p.head, ONE * 4 / 10),
        2 | 4 => p.shoulder[(s - 2) / 2],
        3 | 5 => p.elbow[(s - 3) / 2],
        6 | 8 => p.hip[(s - 6) / 2],
        _ => p.knee[(s - 7) / 2],
    }
}

/// The stumps a body has: each lost part whose parent is still on it, and
/// the way out of it.
fn stumps(lost: u16) -> Vec<(usize, usize)> {
    (0..PARTS)
        .filter(|&s| s != TORSO && lost & bit(s) != 0)
        .filter(|&s| match s {
            3 | 5 | 7 | 9 => lost & bit(s - 1) == 0,
            _ => true,
        })
        .map(|s| (s, s))
        .collect()
}

/// The way out of a stump at part `s`: from the parent joint through the cut.
fn outward(p: &Pose, s: usize) -> V3 {
    let d = match s {
        HEAD => p.head.sub(p.chest),
        2 | 4 => p.shoulder[(s - 2) / 2].sub(p.chest),
        3 | 5 => p.elbow[(s - 3) / 2].sub(p.shoulder[(s - 3) / 2]),
        6 | 8 => v3(0, -ONE, 0),
        _ => p.knee[(s - 7) / 2].sub(p.hip[(s - 7) / 2]),
    };
    if d == V3::ZERO {
        V3::UP
    } else {
        d.norm()
    }
}

/// A limb's radius at part `s` for kind `k`, as M3 draws it, mm on the player.
fn limb_r(k: Kind, s: usize) -> Fx {
    let b = pose::build(k);
    let base = match s {
        HEAD => 60,
        2 | 4 => 56,
        3 | 5 => 52,
        6 | 8 => 76,
        _ => 63,
    };
    mm(base) * b.height / pose::build(Kind::Player).height * b.girth_pct / 100
}

fn colours() -> (Rgb, Rgb, Rgb, Rgb, Rgb, Rgb) {
    let c = &spec().colours;
    (
        rgb(c.flash),
        rgb(c.blood),
        rgb(c.blood_dark),
        rgb(c.stump),
        rgb(c.stump_plain),
        rgb(c.bone),
    )
}

/// A capped stump at `at`, its way out `out`: a faceted red cap a little
/// wider than the limb, and the bone's end standing out of it.
fn cap(out: &mut Shapes, at: V3, way: V3, r: Fx, blood: bool, look: &Look, place: &Affine) {
    let (_, _, _, stump, plain, bone) = colours();
    let rr = r * spec().cap_pct / 100;
    let mut m = Model::default();
    let ink = models::table().ink_figure;
    m.add(
        Mesh::Facet,
        Affine::translate(at).then(&Affine::scale(v3(2 * rr, 2 * rr, 2 * rr))),
        if blood { stump } else { plain },
        ink,
    );
    let br = rr * 38 / 100;
    m.add(
        Mesh::Facet,
        Affine::translate(at.add(way.scale(rr * 80 / 100))).then(&Affine::scale(v3(2 * br, 2 * br, 2 * br))),
        bone,
        ink,
    );
    m.placed(place).emit(look.ink(), out);
}

/// A model's parts on `labels`, those on `keep` drawn, each tinted by the
/// red flash and stain of its part (and the body's share of the strongest).
fn tinted(m: &Model, labels: &[u8], keep: u16, l: &Limbs, blood: bool) -> Model {
    let sp = spec();
    let (flash, _, stain, _, _, _) = colours();
    let amount = |s: usize| l.flash[s] as i32 * ONE / sp.flash_ticks as i32;
    let body = (0..PARTS).map(amount).max().unwrap_or(0);
    let parts = m
        .parts
        .iter()
        .zip(labels.iter())
        .filter(|(_, &s)| keep & bit(s as usize) != 0)
        .map(|(p, &s)| {
            let s = s as usize;
            let mut p = *p;
            if p.glow == 0 {
                let st = if blood {
                    (l.stain[s] as i64 * (sp.stain_pct * ONE / 100) as i64 / sp.stain_ticks.max(1) as i64)
                        as Fx
                } else {
                    0
                };
                if st > 0 {
                    p.rgb = mix(p.rgb, stain, st);
                }
                let k = fx::mul(amount(s), sp.flash_pct * ONE / 100)
                    .max(fx::mul(body, sp.body_flash_pct * ONE / 100));
                if k > 0 {
                    p.rgb = mix(p.rgb, flash, k);
                }
            }
            p
        })
        .collect();
    Model {
        parts,
        decals: Vec::new(),
    }
}

/// The flinch of a struck head, trunk or knee, drawn: the part pushed along
/// the blow, easing back over `flinch_ticks`. Only drawn, so no capsule
/// moves into a blade as it eases back (the arms' flinch is their points'
/// own speed, `mark`).
fn flinched(l: &Limbs, p: &Pose) -> Pose {
    let sp = spec();
    let mut p = *p;
    if l.flinch == 0 {
        return p;
    }
    let k = mm(sp.flinch_mm) * l.flinch as i32 / sp.flinch_ticks as i32;
    let d = l.flinch_way.scale(k);
    match l.flinch_seg as usize {
        HEAD => p.head = p.head.add(d),
        TORSO => {
            p.chest = p.chest.add(d.scale(ONE * 2 / 3));
            p.head = p.head.add(d);
        }
        s @ 2..=5 => {
            // the elbow knocked; the hand stays on what it holds
            let side = (s - 2) / 2;
            p.elbow[side] = p.elbow[side].add(d.scale(ONE * 3 / 4));
        }
        s @ 6..=9 => {
            // the knee buckles
            let side = (s - 6) / 2;
            p.knee[side] = p.knee[side].add(d.scale(ONE * 3 / 4)).sub(v3(0, k / 2, 0));
        }
        _ => {}
    }
    p
}

/// A fighter drawn (`player::draw`'s door to M3's `figure_dressed`): its parts
/// that are still on it, the struck ones flashing and flinching, its stumps
/// capped.
pub fn figure(w: &World, id: usize, p: &Pose, dress: Dress, look: &Look, out: &mut Shapes) {
    let f = player::fighter(w, id);
    let l = &f.health.limbs;
    let p = &flinched(l, p);
    let blood = blood_on(w);
    let marked = l.flash.iter().any(|&t| t > 0) || (blood && l.stain.iter().any(|&t| t > 0));
    if l.lost == 0 && !marked {
        models::figure_dressed(f.kind, p, dress, look, out);
        return;
    }
    let m = models::figure_model(f.kind, p, dress);
    let labels = labels(f.kind, dress);
    if labels.len() == m.parts.len() {
        tinted(&m, &labels, !l.lost, l, blood).emit(look.ink(), out);
    } else {
        m.emit(look.ink(), out);
    }
    for (s, _) in stumps(l.lost) {
        cap(
            out,
            joint(p, s),
            outward(p, s),
            limb_r(f.kind, s),
            blood,
            look,
            &Affine::IDENTITY,
        );
    }
    models::figure_shadow(f.kind, p, look, out);
}

/// How far the player stands from `at` decides a mark's fade: whole within
/// `near_m`, gone by `far_m`.
fn by_distance(me: V3, at: V3) -> Fx {
    let ss = &spec().splats;
    let d = at.sub(me).len_xz();
    let (near, far) = (ss.near_m * ONE, ss.far_m * ONE);
    if d <= near {
        ONE
    } else if d >= far {
        0
    } else {
        fx::div(far - d, far - near)
    }
}

/// A rotation and move taking the frame of three points `a` to that of `b`.
fn carried(a: &[V3; 3], b: &[V3; 3]) -> Affine {
    let basis = |p: &[V3; 3]| {
        let x = p[1].sub(p[0]).norm();
        let y0 = p[2].sub(p[0]);
        let y = y0.sub(x.scale(y0.dot(x))).norm();
        let z = x.cross(y).norm();
        (x, y, z)
    };
    let (x0, y0, z0) = basis(a);
    let (x1, y1, z1) = basis(b);
    // R = R1 · R0ᵀ: column j is x1·x0ⱼ + y1·y0ⱼ + z1·z0ⱼ
    let comp = |v: V3, j: usize| [v.x, v.y, v.z][j];
    let col = |j: usize| {
        x1.scale(comp(x0, j))
            .add(y1.scale(comp(y0, j)))
            .add(z1.scale(comp(z0, j)))
    };
    let rot = Affine {
        c0: col(0),
        c1: col(1),
        c2: col(2),
        t: V3::ZERO,
    };
    let t = b[0].sub(rot.apply_dir(a[0]));
    Affine { t, ..rot }
}

/// Everything wounds draw: the pieces (dithered out as they go), the drops
/// in flight and the marks on the ground, in a fixed order (CW-18).
pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    let sp = spec();
    let blood = blood_on(w);
    let (_, red, dark, _, _, _) = colours();
    let ink = look.ink();
    for pc in &w.wounds.pieces {
        let now: [V3; 3] = [0, 1, 2].map(|k| pc.was[k].lerp(pc.p[k], alpha));
        let place = carried(&pc.at0, &now);
        let kept = Model {
            parts: pc.parts.clone(),
            decals: Vec::new(),
        }
        .placed(&place);
        let fade = if pc.fade > 0 {
            ONE - pc.fade as i32 * ONE / sp.pieces.fade.max(1) as i32
        } else {
            ONE
        };
        if fade >= ONE {
            kept.emit(ink, out);
            let root = pc.root as usize;
            cap(
                out,
                joint(&pc.pose, root),
                outward(&pc.pose, root).neg(),
                limb_r(pc.kind, root),
                blood,
                look,
                &place,
            );
        } else if fade > 0 {
            for p in &kept.parts {
                out.dithered(p.mesh, p.at, p.rgb, fade);
                if p.ink > 0 {
                    out.push(Shape {
                        mesh: p.mesh.into(),
                        at: crate::render::inflate_by(&p.at, models::ink_width(&p.at, p.ink)),
                        rgb: ink,
                        value: fade,
                        pass: Pass::Ink,
                    });
                }
            }
        }
    }
    if !blood {
        return;
    }
    for d in &w.wounds.drops {
        let at = d.was.lerp(d.p, alpha);
        let way = if d.v == V3::ZERO { V3::UP } else { d.v.norm() };
        let half = d.size;
        let shape = models::bar(
            at.sub(way.scale(half)),
            at.add(way.scale(half)),
            (d.size, d.size),
            V3::UP,
        );
        out.solid(Mesh::Octa, shape, red, 0);
    }
    let me = w.player.feet(alpha);
    let ss = &sp.splats;
    for s in &w.wounds.splats {
        if s.r < mm(4) {
            continue;
        }
        let life = if s.age + ss.fade >= ss.life {
            (ss.life - s.age) as i32 * ONE / ss.fade.max(1) as i32
        } else {
            ONE
        };
        let fade = life.min(by_distance(me, s.at));
        if fade <= 0 {
            continue;
        }
        let lift = mm(ss.lift_mm + ss.layer_mm * s.layer as i32);
        let top = s.at.add(s.n.scale(lift));
        let bottom = top.sub(s.n.scale(mm(ss.thick_mm)));
        let shape = models::bar(bottom, top, (2 * s.r, 2 * s.r), v3(ONE, 0, 0));
        let tone = crate::look::shade(dark, 88 + 8 * s.tone as i32);
        out.dithered(Mesh::Cyl, shape, tone, fade);
    }
}

// ---- the screen -----------------------------------------------------------

/// The line over the game while the player can take up their weapon.
pub fn hud(w: &World, s: &crate::page::Strings, out: &mut Vec<crate::world::HudItem>) {
    let shown = stamp(w);
    out.push(crate::world::HudItem {
        id: "wound".into(),
        text: if shown {
            s.get("wound_take_up")
        } else {
            String::new()
        },
        shown,
    });
}

/// What the line shows, in one value: the page reads the screen again when
/// it changes (`game.rs`).
pub fn stamp(w: &World) -> bool {
    can_take_up(w, 0) && w.me.health.limbs.settled && w.me.health.limbs.take_up == 0
}

// ---- acts -----------------------------------------------------------------

fn part_named(name: &str) -> Result<usize, String> {
    NAMES
        .iter()
        .position(|n| *n == name.trim())
        .ok_or_else(|| format!("A part is one of {}, not \"{}\".", NAMES.join(", "), name.trim()))
}

/// `act("wounds.<name>", value)`, for tests and views: `cut` strikes every foe
/// on a part at a speed (`forearm_r,10`: metres a second) as a blade of the
/// player's would; `take_off` takes a part off every foe, `take_off_me` off
/// the player; `left_hand_me` takes the player's right forearm off and puts
/// the sword in the left hand at once; `freeze` holds blood, pieces and every body's wounds still
/// (`on`, `off`, or a whole number of ticks from now); `blood` turns blood
/// on or off as the setting does.
pub fn act(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    match name {
        "cut" => {
            let (part, speed) = value.split_once(',').ok_or_else(|| {
                format!(
                    "A cut is a part and a speed in metres a second, as \"forearm_r,10\", not \"{value}\"."
                )
            })?;
            let s = part_named(part)?;
            let mps: i32 = speed
                .trim()
                .parse()
                .ok()
                .filter(|v| (0..=40).contains(v))
                .ok_or_else(|| {
                    format!(
                        "A cut's speed is a whole number of metres a second from 0 to 40, not \"{}\".",
                        speed.trim()
                    )
                })?;
            for id in 1..player::count(w) {
                let caps = blade::body(player::fighter(w, id).kind, &player::pose_of(w, id));
                let c = &caps[s];
                let vel = right_of(player::body(w, id).facing).scale(mps * ONE / 60).neg();
                let hit = Hit {
                    by: 0,
                    on: id as u8,
                    part: c.part,
                    seg: s as u8,
                    speed: mps * ONE,
                    at: c.a.lerp(c.b, ONE / 2),
                    vel,
                };
                stage(w, hit);
            }
            Ok(())
        }
        "take_off" | "take_off_me" => {
            let s = part_named(value)?;
            if s == TORSO {
                return Err("The torso does not come off.".into());
            }
            let ids: Vec<usize> = if name == "take_off_me" {
                vec![0]
            } else {
                (1..player::count(w)).collect()
            };
            for id in ids {
                let way = right_of(player::body(w, id).facing).scale(ONE / 10);
                take_off(w, id, s, way, fx::int(12));
            }
            Ok(())
        }
        // the player's right forearm off and the sword in the left hand at
        // once, as if taken up: a view of fighting one-handed
        "left_hand_me" => {
            take_off(
                w,
                0,
                FOREARM[1],
                right_of(w.player.facing).scale(ONE / 10),
                fx::int(12),
            );
            let f = &mut w.me;
            swap(f);
            f.health.limbs.loose = false;
            f.health.limbs.settled = false;
            f.sheath = Sheath::Drawn;
            let frame = f.arm.frame;
            f.arm.take_in_hand(&frame);
            Ok(())
        }
        "freeze" => {
            match value.trim() {
                "on" => w.wounds.frozen = true,
                "off" => {
                    w.wounds.frozen = false;
                    w.wounds.freeze_at = None;
                }
                n => {
                    let t: u64 = n
                        .parse()
                        .map_err(|_| format!("Freeze is on, off or a whole number of ticks, not \"{n}\"."))?;
                    w.wounds.freeze_at = Some(w.tick + t);
                }
            }
            Ok(())
        }
        "blood" => {
            w.ui.settings.blood = match value.trim() {
                "on" | "true" => true,
                "off" | "false" => false,
                other => return Err(format!("Blood is on or off, not \"{other}\".")),
            };
            Ok(())
        }
        _ => Err(format!("Unknown action \"wounds.{name}\".")),
    }
}

/// A blow staged as the blades would land it: health taken, then marked.
pub fn stage(w: &mut World, hit: Hit) {
    let id = hit.on as usize;
    player::parts_mut(w, id).1.health.take(hit.speed, hit.part);
    mark(w, &hit, 0, true);
}

#[cfg(test)]
mod tests;
