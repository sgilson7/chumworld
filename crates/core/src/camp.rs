//! `camp` (M5 Places and story, task S): the bandit camp in the north-east
//! forest.
//!
//! The brief: "one little forest encapment of bandits you can fight", and
//! its example: "defeating this encampment will drop a key for a chest that
//! opens to a boomerang". The camp's people stand once the player comes
//! within `SPAWN_AT` of it (`data/story.json` places them): a lookout on the
//! trail where it enters the clearing, three bandits at the fire, and the
//! leader by the chest, all M1's foes with their own trees (the lookout is
//! a bandit by its tree and wears the lookout's model). They stand with
//! their blades put away until the lookout sees the player (`LOOKOUT_SIGHT`)
//! or the player comes near any of them (`NOTICE`) or hurts one: then the
//! alarm goes up, every blade comes out, and they fight. One that has no
//! one within reach of its tree walks home with its blade away.
//!
//! The leader casts its pattern at the player (`orbs::cast`, M6's door):
//! once when it first holds on the player, then every `CAST_EVERY` while it
//! stands more than `CAST_FROM` off, and once more when it is first hurt
//! below half.
//!
//! What they drop (CW-6's routes, `data/items.json`): each bandit coins and
//! knives ("bandits"), the lookout the speedster's gloves ("lookout"), the
//! leader the sword ("leader_duel") and, once the whole camp is down, the
//! chest key ("leader_drop"). The key opens the chest ("camp_chest"), which
//! holds the boomerang; the chest's lid swings open (`LID_TICKS`).

use crate::fx::{self, deg, Fx, ONE};
use crate::geom::{v3, yaw_of, Affine, V3};
use crate::input::Input;
use crate::items::Item;
use crate::look::Look;
use crate::models::{self, Prop};
use crate::orbs::{self, Pattern};
use crate::pilot::Control;
use crate::player::{self, Sheath};
use crate::pose::Kind;
use crate::render::Shapes;
use crate::story::{self, Gift};
use crate::world::World;

/// The camp's people stand once the player is this near its centre.
pub const SPAWN_AT: Fx = fx::int(300);
/// The lookout sees the player this far; anyone notices them this near.
pub const LOOKOUT_SIGHT: Fx = fx::int(32);
pub const NOTICE: Fx = fx::int(12);
/// A bandit fights a player within this of it (M1's `player::FOE_RANGE`),
/// and walks home past this far from home.
pub const FIGHT_IN: Fx = player::FOE_RANGE;
pub const LEASH: Fx = fx::int(70);
/// The leader casts every this many ticks (7 s) while the player stands
/// farther than `CAST_FROM`.
pub const CAST_EVERY: u64 = 420;
pub const CAST_FROM: Fx = fx::int(4);
/// The chest's lid opens over this many ticks.
pub const LID_TICKS: u32 = 30;
/// E opens the chest from this near its middle.
pub const CHEST_FROM: Fx = ONE * 23 / 10;

/// Who a camp foe is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Lookout,
    Bandit,
    Leader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Member {
    pub foe: usize,
    pub role: Role,
    pub home: V3,
    /// what it carried lies on the ground
    pub dropped: bool,
    /// the moment it went down, for the order of the drops
    pub acc: Fx,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    pub members: Vec<Member>,
    pub spawned: bool,
    pub alarm: bool,
    /// the leader's casts so far and the tick of the last
    pub casts: u32,
    pub cast_at: u64,
    pub hurt_cast: bool,
    /// the key lies on the ground (the camp is cleared)
    pub key_dropped: bool,
    /// the chest is open, and how far its lid has swung (ticks)
    pub open: bool,
    pub lid: u32,
    /// M7 (R9.25): the tick the alarm went up, and the order the camp comes
    /// in after it (members by number, nearest the player first)
    pub alarm_at: Option<u64>,
    pub order: Vec<usize>,
}

/// The camp's centre, on the ground.
pub fn centre(w: &World) -> V3 {
    crate::places::at(w, "camp").unwrap_or(crate::world::START)
}

/// The chest's place.
pub fn chest(w: &World) -> Option<crate::places::Spot> {
    crate::places::prop_spots(w, Prop::Chest).into_iter().next()
}

/// The fighter id of the camp's leader, if it stands.
pub fn leader(w: &World) -> Option<usize> {
    w.camp
        .members
        .iter()
        .find(|m| m.role == Role::Leader)
        .map(|m| m.foe + 1)
        .filter(|&id| id < player::count(w))
}

/// M7 (R9.25): after the alarm the camp comes in one at a time, a member
/// every `JOIN_EVERY` (2.5 s), nearest the player first; of those come in, at
/// most `PRESS_MOST` press the player at once (the leader, once in, is one
/// of them, from where it casts), the nearest; the rest stand drawn at their
/// posts and wait their turn.
pub const JOIN_EVERY: u64 = 150;
pub const PRESS_MOST: usize = 2;

/// Whether member `k` has come in: the alarm is up and its beat has come
/// (an alarm set with no tick, as a test sets it, brings everyone in).
pub fn joined(w: &World, k: usize) -> bool {
    if !w.camp.alarm {
        return false;
    }
    let Some(at) = w.camp.alarm_at else {
        return true;
    };
    let place = w.camp.order.iter().position(|&m| m == k).unwrap_or(k) as u64;
    w.tick >= at + place * JOIN_EVERY
}

/// The members pressing the player now, at most `PRESS_MOST`: those come in,
/// standing, within reach of a fight and of home; the leader first, then the
/// nearest.
pub fn pressing(w: &World) -> Vec<usize> {
    let me = w.player.pos;
    let mut can: Vec<(bool, Fx, usize)> = Vec::new();
    for (k, m) in w.camp.members.iter().enumerate() {
        let Some(foe) = w.foes.get(m.foe) else {
            continue;
        };
        let at = foe.body.pos;
        if foe.fighter.health.down()
            || !joined(w, k)
            || me.sub(at).len_xz() >= FIGHT_IN
            || at.sub(m.home).len_xz() >= LEASH
        {
            continue;
        }
        can.push((m.role != Role::Leader, me.sub(at).len_xz(), k));
    }
    can.sort();
    can.into_iter().take(PRESS_MOST).map(|(_, _, k)| k).collect()
}

/// A test duel forgets the camp: its foes are gone with the duel's reset.
pub fn forget(w: &mut World) {
    w.camp.members.clear();
    w.camp.spawned = false;
    w.camp.alarm = false;
    w.camp.alarm_at = None;
    w.camp.order.clear();
}

fn spawn(w: &mut World) {
    let sp = &story::spec().camp;
    let fire = centre(w);
    let put = |w: &mut World, at: [i32; 2], role: Role| {
        let p = story::near_place(w, "camp", at);
        let (kind, look) = match role {
            Role::Lookout => (Kind::Bandit, Kind::Lookout),
            Role::Bandit => (Kind::Bandit, Kind::Bandit),
            Role::Leader => (Kind::Leader, Kind::Leader),
        };
        // the lookout faces down the trail; the rest face the fire
        let yaw = if role == Role::Lookout {
            yaw_of(p.sub(fire))
        } else {
            yaw_of(fire.sub(p))
        };
        let foe = story::sheathed_foe(w, kind, look, p, yaw);
        // M7 save: one beaten in the save this world came from lies down
        let key = crate::save::camp_keys()
            .get(w.camp.members.len())
            .cloned()
            .unwrap_or_default();
        if crate::save::beaten(w, &key) {
            crate::save::lay_down(w, foe);
        }
        w.camp.members.push(Member {
            foe,
            role,
            home: p,
            dropped: false,
            acc: 0,
        });
    };
    put(w, sp.lookout, Role::Lookout);
    for b in &sp.bandits {
        put(w, *b, Role::Bandit);
    }
    put(w, sp.leader, Role::Leader);
    w.camp.spawned = true;
}

/// Step 2b: before the alarm, the camp stands; after it, each draws and
/// its tree fights a player within reach, or it walks home sheathed.
pub fn steer(w: &mut World, inputs: &mut [Input]) {
    let press = pressing(w);
    for k in 0..w.camp.members.len() {
        let m = w.camp.members[k];
        let Some(foe) = w.foes.get(m.foe) else {
            continue;
        };
        if foe.brain.control != Control::Tree || foe.fighter.health.down() {
            continue;
        }
        let f = &foe.fighter;
        let at = foe.body.pos;
        // M7 (R9.25): only those pressing fight; one come in and waiting
        // stands drawn at its post
        let fight = press.contains(&k);
        let waiting = !fight && joined(w, k) && at.sub(m.home).len_xz() <= ONE * 15 / 10;
        inputs[m.foe] = if waiting {
            Input {
                keys: if f.sheath == Sheath::Sheathed {
                    story::sheath_keys(f, true)
                } else {
                    0
                },
                ..Input::default()
            }
        } else if fight {
            if f.sheath == Sheath::Sheathed {
                Input {
                    keys: story::sheath_keys(f, true),
                    ..Input::default()
                }
            } else {
                inputs[m.foe]
            }
        } else if f.drawn() {
            Input {
                keys: story::sheath_keys(f, false),
                ..Input::default()
            }
        } else if at.sub(m.home).len_xz() > ONE * 15 / 10 {
            let mut acc = m.acc;
            let keys = story::keys_toward(f.look_yaw, m.home.sub(at), &mut acc);
            w.camp.members[k].acc = acc;
            Input {
                keys,
                ..Input::default()
            }
        } else {
            Input::default()
        };
    }
}

/// Step 7: the camp stands when the player comes near, the alarm, the
/// leader's casts, the drops, the chest's lid.
pub fn step(w: &mut World) {
    if !w.camp.spawned {
        if w.story.phase != story::Phase::Off && centre(w).sub(w.player.pos).len_xz() < SPAWN_AT {
            spawn(w);
        }
        return;
    }
    let me = w.player.pos;
    if !w.camp.alarm {
        let seen = w.camp.members.iter().any(|m| {
            let Some(foe) = w.foes.get(m.foe) else {
                return false;
            };
            let f = &foe.fighter;
            let gap = foe.body.pos.sub(me).len_xz();
            !f.health.down()
                && ((m.role == Role::Lookout && gap < LOOKOUT_SIGHT)
                    || gap < NOTICE
                    || f.health.hp < f.health.most)
        });
        if seen && !w.player_down() {
            w.camp.alarm = true;
            story::banner(w, "story_alarm");
            // M7 (R9.25): the camp comes in nearest first, a beat apart
            w.camp.alarm_at = Some(w.tick);
            let mut order: Vec<(Fx, usize)> = (0..w.camp.members.len())
                .map(|k| {
                    let at = w.foes.get(w.camp.members[k].foe).map_or(me, |f| f.body.pos);
                    (at.sub(me).len_xz(), k)
                })
                .collect();
            order.sort();
            w.camp.order = order.into_iter().map(|(_, k)| k).collect();
        }
    }
    casts(w);
    drops(w);
    if w.camp.open && w.camp.lid < LID_TICKS {
        w.camp.lid += 1;
    }
}

fn casts(w: &mut World) {
    let Some(id) = leader(w) else {
        return;
    };
    let f = player::fighter(w, id);
    // M7 (R9.24): held still, the leader casts nothing; (R9.25) nor before
    // it presses the player
    let k = w.camp.members.iter().position(|m| m.role == Role::Leader);
    let presses = k.is_some_and(|k| w.camp.alarm_at.is_none() || pressing(w).contains(&k));
    if story::holding(w) || !w.camp.alarm || !presses || f.health.down() || f.engaged != Some(0) {
        return;
    }
    let gap = player::body(w, id).pos.sub(w.player.pos).len_xz();
    let hurt = f.health.pct() < 50;
    let due = w.camp.casts == 0
        || (gap > CAST_FROM && w.tick >= w.camp.cast_at + CAST_EVERY)
        || (hurt && !w.camp.hurt_cast);
    if due {
        if hurt {
            w.camp.hurt_cast = true;
        }
        w.camp.casts += 1;
        w.camp.cast_at = w.tick;
        orbs::cast(w, id, Pattern::Leader);
    }
}

fn drops(w: &mut World) {
    let sp = &story::spec().camp;
    for k in 0..w.camp.members.len() {
        let m = w.camp.members[k];
        let Some(foe) = w.foes.get(m.foe) else {
            continue;
        };
        if m.dropped || !foe.fighter.health.down() {
            continue;
        }
        let at = foe.body.pos;
        let yaw = foe.body.facing;
        let beside = |d: Fx, turn: i32| at.add(crate::geom::facing(yaw + turn, 0).scale(d));
        // M7 save: each drop by its stable key (`save::drop_key`), never
        // laid again once taken
        let key = crate::save::camp_keys().get(k).cloned().unwrap_or_default();
        let slot = |s: usize| crate::save::drop_key(&key, s);
        match m.role {
            Role::Lookout => {
                save_drop(
                    w,
                    &slot(0),
                    Gift::Item(Item::Gloves, 1),
                    beside(ONE * 8 / 10, deg(90)),
                );
                save_drop(
                    w,
                    &slot(1),
                    Gift::Coins(sp.bandit_coins),
                    beside(ONE * 8 / 10, -deg(90)),
                );
            }
            Role::Bandit => {
                save_drop(
                    w,
                    &slot(0),
                    Gift::Coins(sp.bandit_coins),
                    beside(ONE * 8 / 10, deg(90)),
                );
                save_drop(
                    w,
                    &slot(1),
                    Gift::Item(Item::Knife, sp.bandit_knives),
                    beside(ONE * 8 / 10, -deg(90)),
                );
            }
            Role::Leader => {
                save_drop(w, &slot(0), Gift::Item(Item::Sword, 1), beside(ONE, deg(90)));
                save_drop(
                    w,
                    &slot(1),
                    Gift::Coins(sp.leader_coins),
                    beside(ONE * 8 / 10, -deg(90)),
                );
            }
        }
        w.camp.members[k].dropped = true;
    }
    let cleared = !w.camp.members.is_empty()
        && w.camp
            .members
            .iter()
            .all(|m| w.foes.get(m.foe).is_none_or(|f| f.fighter.health.down()));
    if cleared && !w.camp.key_dropped {
        w.camp.key_dropped = true;
        let at = leader(w)
            .map(|id| player::body(w, id).pos)
            .unwrap_or_else(|| centre(w));
        // M7 (R9.25): toward the player, clear of the fallen leader's body
        // (the camp now falls where it waits, by the tents and the chest)
        let to = w.player.pos.sub(at);
        let way = if to.len_xz() > 0 {
            v3(to.x, 0, to.z).norm()
        } else {
            v3(0, 0, ONE)
        };
        save_drop(
            w,
            crate::save::KEY_DROP,
            Gift::Item(Item::ChestKey, 1),
            at.add(way.scale(ONE * 12 / 10)),
        );
        // M7 save: cleared in the save this world came from, it is not said again
        if !crate::save::was_cleared(w) {
            story::banner(w, "story_camp_cleared");
        }
    }
}

fn save_drop(w: &mut World, key: &str, gift: Gift, at: V3) {
    crate::save::drop(w, key, gift, at);
}

/// How far the player is from opening the chest, near it.
pub fn chest_reach(w: &World) -> Option<Fx> {
    let c = chest(w)?;
    if w.story.phase == story::Phase::Off && !w.camp.spawned {
        return None;
    }
    let d = c.at.sub(w.player.pos).len_xz();
    (d <= CHEST_FROM).then(|| (d - CHEST_FROM + story::REACH).max(0))
}

/// E at the chest: with the key, it opens and gives the boomerang; without,
/// a line says it is locked; open, that it is empty.
pub fn open_chest(w: &mut World) {
    if w.camp.open {
        story::banner(w, "story_chest_empty");
    } else if w.inventory.has(Item::ChestKey) {
        crate::inventory::take(w, Item::ChestKey, 1);
        w.camp.open = true;
        w.camp.lid = 0;
        crate::inventory::give(w, Item::Boomerang, 1);
    } else {
        story::banner(w, "story_chest_locked");
    }
}

/// The chest, its lid shut or swinging open about its back edge.
/// M7 (R7.13, R9.90): the speedster pieces fighter `id` (a foe's number
/// plus one, as `player::fighter`) is drawn wearing: the lookout wears the
/// gloves until it drops them.
pub fn worn(w: &World, id: usize) -> u8 {
    let wears = w
        .camp
        .members
        .iter()
        .any(|m| m.role == Role::Lookout && m.foe + 1 == id && !m.dropped);
    if wears {
        models::GLOVES
    } else {
        0
    }
}

pub fn draw(w: &World, alpha: Fx, look: &Look, out: &mut Shapes) {
    let Some(c) = chest(w) else {
        return;
    };
    let place = Affine::translate(c.at).then(&Affine::rot_y(c.yaw));
    let model = models::prop_model(Prop::Chest);
    let t = (w.camp.lid as i64 * ONE as i64
        + if w.camp.open && w.camp.lid < LID_TICKS {
            alpha as i64
        } else {
            0
        })
        / LID_TICKS as i64;
    let t = (t as Fx).min(ONE);
    let eased = fx::mul(fx::mul(t, t), 3 * ONE - 2 * t);
    // M3's chest: the lid (its rounded top and the bands over it) above
    // 560 mm, hinged along its back edge 500 mm up, 280 mm back
    let hinge = v3(0, ONE / 2, ONE * 28 / 100);
    let open = Affine::translate(hinge)
        .then(&Affine::rot_x(fx::mul(deg(105), eased)))
        .then(&Affine::translate(hinge.neg()));
    let mut m = models::Model::default();
    for p in &model.parts {
        let lid = p.at.t.y > ONE * 56 / 100;
        let mut q = *p;
        if lid {
            q.at = open.then(&p.at);
        }
        m.parts.push(q);
    }
    m.placed(&place).emit(look.ink(), out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_camp() -> World {
        let mut w = story::opening(4);
        story::step_aside(&mut w);
        w.story.phase = story::Phase::Free;
        let c = centre(&w);
        let p = c.add(v3(-fx::int(60), 0, fx::int(60)));
        w.player = crate::player::Body::new(p, 0);
        w.me = crate::player::Fighter::new(Kind::Player, crate::pose::Held::ShortSword, 0, p, 0, false);
        w.step(&Input::default());
        w
    }

    #[test]
    fn the_camp_stands_when_the_player_comes_near_with_a_lookout_three_bandits_and_a_leader() {
        let w = at_camp();
        let roles: Vec<Role> = w.camp.members.iter().map(|m| m.role).collect();
        assert_eq!(
            roles,
            [
                Role::Lookout,
                Role::Bandit,
                Role::Bandit,
                Role::Bandit,
                Role::Leader
            ]
        );
        // every one stands clear of every wall
        for m in &w.camp.members {
            let p = w.foes[m.foe].body.pos;
            assert!(
                !w.solid
                    .walls
                    .iter()
                    .any(|k| k.push(p.x, p.z, crate::solid::RADIUS).is_some()),
                "{:?} stands in a wall",
                m.role
            );
            assert!(!w.foes[m.foe].fighter.drawn(), "blades away before the alarm");
        }
        assert!(!w.camp.alarm);
    }

    #[test]
    fn the_lookout_raises_the_alarm_at_thirty_two_metres_and_not_before() {
        let mut w = at_camp();
        let look = w.foes[w.camp.members[0].foe].body.pos;
        // 33 m from the lookout, on the trail's side: no alarm
        let away = look.sub(centre(&w)).norm();
        for (d, alarm) in [(33, false), (31, true)] {
            let mut p = look.add(away.scale(fx::int(d)));
            p.y = w.ground(p.x, p.z);
            w.player = crate::player::Body::new(p, 0);
            w.step(&Input::default());
            assert_eq!(w.camp.alarm, alarm, "at {d} m");
        }
    }

    #[test]
    fn the_chest_opens_only_with_the_key_and_gives_the_boomerang() {
        let mut w = at_camp();
        open_chest(&mut w);
        assert!(!w.camp.open);
        assert!(!w.inventory.has(Item::Boomerang));
        assert_eq!(w.story.banner.as_ref().unwrap().key, "story_chest_locked");
        crate::inventory::give(&mut w, Item::ChestKey, 1);
        open_chest(&mut w);
        assert!(w.camp.open);
        assert!(w.inventory.has(Item::Boomerang));
        assert!(!w.inventory.has(Item::ChestKey), "the key stays in the lock");
        open_chest(&mut w);
        assert_eq!(w.inventory.count(Item::Boomerang), 1, "once");
    }

    #[test]
    fn the_whole_camp_down_drops_the_key_and_each_drops_its_own() {
        let mut w = at_camp();
        for m in w.camp.members.clone() {
            w.foes[m.foe].fighter.health.hp = 0;
        }
        w.step(&Input::default());
        let gifts: Vec<Gift> = w.story.pickups.iter().map(|p| p.gift).collect();
        let sp = &story::spec().camp;
        for g in [
            Gift::Item(Item::Gloves, 1),
            Gift::Item(Item::Sword, 1),
            Gift::Item(Item::ChestKey, 1),
            Gift::Item(Item::Knife, sp.bandit_knives),
            Gift::Coins(sp.leader_coins),
        ] {
            assert!(gifts.contains(&g), "{g:?} lies on the ground");
        }
        // one key, however long they lie
        w.step(&Input::default());
        let keys = w
            .story
            .pickups
            .iter()
            .filter(|p| p.gift == Gift::Item(Item::ChestKey, 1))
            .count();
        assert_eq!(keys, 1);
    }

    /// M7 (R7.13): the lookout is drawn in the speedster's gloves (the
    /// piece's colour on its hands) until it falls and they lie on the
    /// ground; no other foe wears them.
    #[test]
    fn the_lookout_wears_the_gloves_until_it_drops_them() {
        let mut w = at_camp();
        let look_id = w.camp.members[0].foe + 1;
        let bandit_id = w.camp.members[1].foe + 1;
        assert_eq!((worn(&w, look_id), worn(&w, bandit_id)), (models::GLOVES, 0));
        let gloved = |w: &World| {
            let mut out = Shapes::default();
            crate::player::draw(w, 0, &crate::look::tests::look(), &mut out);
            let speed = models::table().c("speed_base");
            out.list.iter().filter(|s| s.rgb == speed).count()
        };
        // two hands in the piece's colour (the player wears nothing)
        assert_eq!(gloved(&w), 2);
        w.foes[look_id - 1].fighter.health.hp = 0;
        w.step(&Input::default());
        assert_eq!(worn(&w, look_id), 0);
        assert_eq!(gloved(&w), 0, "dropped, not worn");
    }

    #[test]
    fn the_leader_casts_when_it_first_holds_on_the_player_and_then_every_seven_seconds() {
        let mut w = at_camp();
        w.camp.alarm = true;
        let id = leader(&w).unwrap();
        let p = player::body(&w, id).pos.add(v3(fx::int(9), 0, 0));
        w.player = crate::player::Body::new(p, 0);
        // the leader holds on a fighter it can see with its blade out: here
        // it is put there as M1 would on the next tick
        w.foes[id - 1].fighter.engaged = Some(0);
        casts(&mut w);
        assert_eq!(w.camp.casts, 1);
        casts(&mut w);
        assert_eq!(w.camp.casts, 1, "not again at once");
        w.tick += CAST_EVERY;
        casts(&mut w);
        assert_eq!(w.camp.casts, 2);
    }

    /// M7 (R9.25): after the alarm the camp comes in nearest first, one
    /// every 150 ticks (2.5 s): five members at 0, 150, 300, 450 and 600
    /// ticks after it; and no more than two ever press the player at once.
    #[test]
    fn the_camp_comes_in_a_beat_apart_and_no_more_than_two_press_at_once() {
        let mut w = at_camp();
        w.me.health = crate::health::Health::new(100_000);
        let fire = centre(&w);
        let mut p = fire.add(v3(0, 0, fx::int(6)));
        p.y = w.ground(p.x, p.z);
        w.player = crate::player::Body::new(p, 0);
        w.step(&Input::default());
        assert!(w.camp.alarm, "by the fire, the camp is up");
        let at = w.camp.alarm_at.unwrap();
        let n = w.camp.members.len();
        assert_eq!(n, 5);
        let mut came: Vec<Option<u64>> = (0..n).map(|k| joined(&w, k).then_some(0)).collect();
        let mut most = 0;
        for _ in 0..900 {
            w.step(&Input::default());
            for (k, c) in came.iter_mut().enumerate() {
                if c.is_none() && joined(&w, k) {
                    *c = Some(w.tick - at);
                }
            }
            most = most.max(pressing(&w).len());
        }
        let mut beats: Vec<u64> = came.iter().map(|c| c.unwrap()).collect();
        beats.sort();
        assert_eq!(beats, vec![0, 150, 300, 450, 600]);
        assert_eq!(most, 2, "two press at most, and two do");
    }

    /// M7 (R9.24): held still (`story.still`), the leader casts nothing,
    /// though everything else says it should.
    #[test]
    fn held_still_the_leader_casts_nothing() {
        let mut w = at_camp();
        w.camp.alarm = true;
        let id = leader(&w).unwrap();
        let p = player::body(&w, id).pos.add(v3(fx::int(9), 0, 0));
        w.player = crate::player::Body::new(p, 0);
        w.foes[id - 1].fighter.engaged = Some(0);
        w.story.still = true;
        casts(&mut w);
        assert_eq!(w.camp.casts, 0);
        w.story.still = false;
        casts(&mut w);
        assert_eq!(w.camp.casts, 1, "let go, it casts");
    }
}
