//! A probe, not a check: the triangles and instances each model draws, for
//! MEASUREMENTS.md (M3). Run with
//! `cargo test -p tool_core --test model_counts -- --ignored --nocapture`.

use tool_core::geom::v3;
use tool_core::models::{self, Dress, Pickup, Prop, KINDS, SPEEDSTER_ALL, VILLAGER_VARIANTS};
use tool_core::pose::{Held, Kind};

#[test]
#[ignore]
fn probe_model_counts() {
    let mut figures = Vec::new();
    for k in KINDS {
        let n = if k == Kind::Villager { VILLAGER_VARIANTS } else { 1 };
        for v in 0..n {
            figures.push((k, Dress { worn: 0, variant: v }));
        }
    }
    figures.push((
        Kind::Player,
        Dress {
            worn: SPEEDSTER_ALL,
            variant: 0,
        },
    ));
    for (k, d) in figures {
        let pose = models::rest_pose(k, v3(0, 0, 0), 0);
        let m = models::figure_model(k, &pose, d);
        let h = models::held_model(models::kind_held(k), pose.grip, pose.tip);
        println!(
            "figure {k:?} variant {} worn {}: {} parts, {} triangles, {} instances; held {:?}: {} triangles, {} instances",
            d.variant,
            d.worn,
            m.parts.len(),
            m.triangles(),
            m.instances() + 1,
            models::kind_held(k),
            h.triangles(),
            h.instances()
        );
    }
    for item in [
        Held::ShortSword,
        Held::Sword,
        Held::Cleaver,
        Held::Knife,
        Held::Boomerang,
        Held::Rock,
    ] {
        let m = models::held_model(item, v3(0, 0, 0), v3(models::grip_to_tip(item), 0, 0));
        println!(
            "held {item:?}: {} triangles, {} instances",
            m.triangles(),
            m.instances()
        );
    }
    for p in Prop::ALL {
        let m = models::prop_model(p);
        let glow = m.parts.iter().filter(|q| q.glow != 0).count();
        println!(
            "prop {p:?}: {} parts, {} triangles in its mesh (as many again in its outline); static instances 2 + {} glowing, {} as separate shapes",
            m.parts.len(),
            m.triangles(),
            glow,
            m.instances()
        );
    }
    for p in Pickup::ALL {
        let m = models::pickup_model(p);
        println!(
            "pickup {p:?}: {} triangles, {} instances",
            m.triangles(),
            m.instances() + 2
        );
    }
    let d = models::donkey_model(0);
    println!(
        "donkey: {} triangles, {} instances",
        d.triangles(),
        d.instances() + 1
    );
}
