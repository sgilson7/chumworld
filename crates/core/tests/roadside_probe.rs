//! A probe, not a check (M7 roadside, T12): where each roadside stop stands
//! in the shipped layout, side by side, for SECOND-ORDER.md. It prints and
//! asserts nothing.
//!
//! ```bash
//! cargo test -p tool_core --test roadside_probe -- --ignored --nocapture
//! ```

use tool_core::fx::ONE;
use tool_core::places::Layout;
use tool_core::roadside::{centre, sides, stops, top};

#[test]
#[ignore]
fn probe_where_the_stops_stand() {
    let l = Layout::shipped();
    let c = centre(&l);
    let all = stops(&l);
    for (k, side) in sides(&l).iter().enumerate() {
        let p = side.path[0];
        println!(
            "{} (on a road: {}), the boundary's line at ({}, {}):",
            side.name,
            side.on_road,
            p.x / ONE,
            p.z / ONE
        );
        let mut mine: Vec<_> = all.iter().filter(|s| s.side == k).collect();
        mine.sort_by_key(|s| s.at.sub(c).len_xz());
        for s in mine {
            println!(
                "  {} {:?} at ({}, {}), {} m out, {}.{} m tall",
                s.id,
                s.kind,
                s.at.x / ONE,
                s.at.z / ONE,
                s.at.sub(c).len_xz() / ONE,
                top(s) / ONE,
                top(s) * 10 / ONE % 10
            );
        }
    }
}
