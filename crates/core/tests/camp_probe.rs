//! M7 (R9.25): what the camp does to a player who stands by its fire with
//! the guard up and does nothing else. A probe, not a check.
//!     cargo test --release -p tool_core --test camp_probe -- --ignored --nocapture
use tool_core::fx::{self, ONE};
use tool_core::geom::v3;
use tool_core::input::Input;

#[test]
#[ignore]
fn probe_the_camp() {
    for (name, at) in [
        ("by the fire", (0, 6)),
        ("on the trail, at the lookout", (-28, 30)),
    ] {
        let mut w = tool_core::story::opening(4);
        tool_core::story::step_aside(&mut w);
        w.story.phase = tool_core::story::Phase::Free;
        let c = tool_core::camp::centre(&w);
        let mut p = c.add(v3(-fx::int(60), 0, fx::int(60)));
        w.player = tool_core::player::Body::new(p, 0);
        w.me = tool_core::player::Fighter::new(
            tool_core::pose::Kind::Player,
            tool_core::pose::Held::ShortSword,
            0,
            p,
            0,
            false,
        );
        w.step(&Input::default());
        p = c.add(v3(fx::int(at.0), 0, fx::int(at.1)));
        p.y = w.ground(p.x, p.z);
        let yaw = tool_core::geom::yaw_of(c.sub(p));
        w.player = tool_core::player::Body::new(p, yaw);
        w.me = tool_core::player::Fighter::new(
            tool_core::pose::Kind::Player,
            tool_core::pose::Held::ShortSword,
            0,
            p,
            yaw,
            false,
        );
        w.aim.yaw = yaw;
        w.me.health = tool_core::health::Health::new(100_000);
        let mut taken = [0i32; 3]; // blades, orbs and thrown things
        let mut attackers = std::collections::BTreeSet::new();
        let (mut most_near, mut hp10, mut hp20, mut down) = (0, 0, 0, None);
        for t in 0..3600u32 {
            let keys = if t == 2 { tool_core::input::DRAW } else { 0 };
            let before = w.me.health.hp;
            w.step(&Input {
                keys,
                ..Input::default()
            });
            let lost = before - w.me.health.hp;
            if lost > 0 {
                let by_blade = w.blades.hits.iter().any(|h| h.on == 0);
                let by_thrown = w
                    .throwables
                    .landed
                    .iter()
                    .any(|i| matches!(i.struck, tool_core::throwables::Struck::Body(0)));
                let k = if by_blade {
                    0
                } else if by_thrown {
                    2
                } else {
                    1
                };
                taken[k] += lost;
                for h in w.blades.hits.iter().filter(|h| h.on == 0) {
                    attackers.insert(h.by);
                }
            }
            let near = w
                .foes
                .iter()
                .filter(|f| {
                    !f.fighter.health.down()
                        && f.fighter.drawn()
                        && f.body.pos.sub(w.player.pos).len_xz() < ONE * 4
                })
                .count();
            most_near = most_near.max(near);
            if t == 600 {
                hp10 = w.me.health.hp;
            }
            if t == 1200 {
                hp20 = w.me.health.hp;
            }
            if down.is_none() && w.me.health.down() {
                down = Some(t);
            }
        }
        let _ = (hp10, hp20, down);
        println!("{name}, 60 s standing with the guard up: taken by blades {}, orbs {}, thrown {}; struck by {} different foes; most drawn foes within 4 m at once {most_near}; casts {}", taken[0], taken[1], taken[2], attackers.len(), w.camp.casts);
    }
}
