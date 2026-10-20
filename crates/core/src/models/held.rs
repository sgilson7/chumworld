//! What a hand holds, drawn along its grip to its tip, and the items that
//! lie in the world waiting to be picked up (the five speedster pieces and
//! the chest's key).
//!
//! An item's own frame: up runs from the grip (the middle of the hilt, where
//! the hands close) to the tip; right is the blade's width, as near the
//! world's up as stays at right angles to the blade, so a sword held level
//! has its edge up and down. Lengths are millimetres along the item from the
//! grip.

use super::*;

/// The item each kind carries at its guard (`rest_pose`).
pub fn kind_held(kind: Kind) -> Held {
    match kind {
        Kind::Player => Held::ShortSword,
        Kind::Orc => Held::Cleaver,
        Kind::Bandit => Held::Knife,
        Kind::Lookout => Held::Rock,
        Kind::Leader | Kind::Trainer => Held::Sword,
        Kind::Merchant | Kind::Villager => Held::Nothing,
    }
}

/// How each blade is laid out from the grip, millimetres: the hilt's half
/// length, the guard's thickness, the blade's length, its width at the
/// guard and its thickness.
struct Blade {
    hilt: i32,
    guard: i32,
    blade: i32,
    width: i32,
    thick: i32,
}

fn blade(item: Held) -> Blade {
    match item {
        // a 70 cm blade on a hand-and-a-half hilt
        Held::ShortSword => Blade {
            hilt: 80,
            guard: 26,
            blade: 700,
            width: 46,
            thick: 9,
        },
        // a 100 cm blade on a two-handed hilt
        Held::Sword => Blade {
            hilt: 110,
            guard: 28,
            blade: 1000,
            width: 54,
            thick: 10,
        },
        Held::Knife => Blade {
            hilt: 50,
            guard: 14,
            blade: 200,
            width: 30,
            thick: 7,
        },
        // a 60 cm slab of iron on a long haft
        Held::Cleaver => Blade {
            hilt: 150,
            guard: 0,
            blade: 600,
            width: 210,
            thick: 13,
        },
        Held::Boomerang | Held::Rock | Held::Nothing => Blade {
            hilt: 0,
            guard: 0,
            blade: 0,
            width: 0,
            thick: 0,
        },
    }
}

/// How far from the grip each item's tip is drawn: where M1's `Pose::tip`
/// should sit for the drawn blade to end there. A sword's is the hilt's
/// half, the guard and the blade: 80 + 26 + 700 mm for the short sword,
/// 110 + 28 + 1000 mm for the sword.
pub fn grip_to_tip(item: Held) -> Fx {
    match item {
        Held::Boomerang => mm(460),
        Held::Rock => mm(120),
        Held::Nothing => 0,
        _ => {
            let b = blade(item);
            mm(b.hilt + b.guard + b.blade)
        }
    }
}

/// The model of an item from `grip` to `tip`.
pub fn model(item: Held, grip: V3, tip: V3) -> Model {
    let t = table();
    let mut m = Model::default();
    if item == Held::Nothing || grip == tip {
        return m;
    }
    let f = along(grip, tip, V3::UP);
    let ink = t.ink_item;
    let len = tip.sub(grip).len();
    // a part `size` across centred `y` along the item, `x` across it
    let at = |x: Fx, y: Fx, size: V3| put(&f, v3(x, y, 0), size);
    match item {
        Held::ShortSword | Held::Sword | Held::Knife => {
            let b = blade(item);
            let (steel, edge) = (t.c("steel"), t.c("steel_edge"));
            let (grip_c, brass) = if item == Held::Knife {
                (t.c("knife_grip"), t.c("iron"))
            } else {
                (t.c("hilt_leather"), t.c("brass"))
            };
            m.add(
                Mesh::Cyl,
                at(0, 0, v3(mm(32), mm(2 * b.hilt), mm(32))),
                grip_c,
                ink,
            );
            let pommel = if item == Held::Knife { 20 } else { 30 };
            m.add(
                Mesh::Facet,
                at(
                    0,
                    -mm(b.hilt + 12),
                    v3(mm(2 * pommel), mm(2 * pommel), mm(2 * pommel)),
                ),
                brass,
                ink,
            );
            let guard_w = if item == Held::Knife { 64 } else { b.width * 4 };
            m.add(
                Mesh::Cube,
                at(0, mm(b.hilt + b.guard / 2), v3(mm(guard_w), mm(b.guard), mm(34))),
                brass,
                ink,
            );
            // the blade to where its point starts, then the point
            let start = mm(b.hilt + b.guard);
            let point = mm(b.width * 2);
            let body = (len - start - point).max(mm(10));
            m.add(
                Mesh::Cube,
                at(0, start + body / 2, v3(mm(b.width), body, mm(b.thick))),
                steel,
                ink,
            );
            // the wedge's triangle tapers along the blade to the tip, its
            // length the blade's thickness
            let p = put(
                &f,
                v3(0, start + body + point / 2, 0),
                v3(mm(b.width), point, mm(b.thick)),
            );
            m.add(Mesh::Wedge, p, edge, ink);
        }
        Held::Cleaver => {
            let b = blade(item);
            let (wood, iron, edge) = (t.c("cleaver_haft"), t.c("cleaver_iron"), t.c("steel_edge"));
            m.add(Mesh::Cyl, at(0, 0, v3(mm(38), mm(2 * b.hilt), mm(38))), wood, ink);
            m.add(
                Mesh::Facet,
                at(0, -mm(b.hilt + 10), v3(mm(54), mm(44), mm(54))),
                iron,
                ink,
            );
            // the slab hangs off the haft's line to one side, its cutting edge
            // a strip beside it, thinner, so no face of one lies on the other's
            let start = mm(b.hilt);
            let body = len - start;
            let w = mm(b.width - 40);
            m.add(
                Mesh::Cube,
                at(w / 2 - mm(20), start + body / 2, v3(w, body, mm(b.thick))),
                iron,
                ink,
            );
            m.add(
                Mesh::Cube,
                at(
                    w + mm(2),
                    start + body / 2 + mm(20),
                    v3(mm(44), body - mm(40), mm(b.thick - 5)),
                ),
                edge,
                ink,
            );
            // the haft runs up the slab's back, with two rivets
            m.add(
                Mesh::Cyl,
                at(-mm(20), start + body / 2, v3(mm(30), body, mm(30))),
                wood,
                ink,
            );
            let rivet = t.c("orc_rivet");
            for k in [1, 3] {
                let y = start + body * k / 4;
                m.add(
                    Mesh::Cyl,
                    put_turned(
                        &f,
                        v3(w / 2, y, 0),
                        &Affine::rot_x(fx::QUARTER_TURN),
                        v3(mm(30), mm(b.thick + 2 * 10), mm(30)),
                    ),
                    rivet,
                    0,
                );
            }
        }
        Held::Boomerang => {
            let (wood, paint) = (t.c("boomerang_wood"), t.c("boomerang_paint"));
            // two arms meeting at an elbow off to one side of the grip-to-tip line
            let elbow = f.apply(v3(mm(170), len / 2, 0));
            for (a, b) in [(grip, elbow), (elbow, tip)] {
                let arm = along(a, b, f.c0);
                let l = b.sub(a).len() + mm(50);
                let mid = a.lerp(b, ONE / 2);
                let ellipse = Affine { t: mid, ..arm }.then(&Affine::scale(v3(mm(80), l, mm(24))));
                let k = m.add(Mesh::Facet, ellipse, wood, ink);
                // a painted band round the middle of each arm
                let af = Affine { t: mid, ..arm };
                let r = v3(mm(40), l / 2, mm(12));
                for side in [-1, 1] {
                    let d = v3(0, 0, side * ONE);
                    decal(
                        &mut m,
                        k,
                        &af,
                        r,
                        d,
                        v3(mm(60), mm(70), mm(22)),
                        DECAL_LIFT,
                        paint,
                    );
                }
            }
            m.add(
                Mesh::Facet,
                Affine { t: elbow, ..f }.then(&Affine::scale(v3(mm(96), mm(96), mm(26)))),
                wood,
                ink,
            );
        }
        Held::Rock => {
            let rock = t.c("rock");
            let mid = grip.lerp(tip, ONE / 2);
            let a = Affine { t: mid, ..f }
                .then(&Affine::rot_z(fx::deg(30)))
                .then(&Affine::scale(v3(mm(130), mm(112), mm(118))));
            m.add(Mesh::Facet, a, rock, ink);
        }
        Held::Nothing => {}
    }
    m
}

/// An item lying in the world to be picked up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pickup {
    Hood,
    Jacket,
    Gloves,
    Trousers,
    Boots,
    ChestKey,
}

impl Pickup {
    pub const ALL: [Pickup; 6] = [
        Pickup::Hood,
        Pickup::Jacket,
        Pickup::Gloves,
        Pickup::Trousers,
        Pickup::Boots,
        Pickup::ChestKey,
    ];
}

/// Draws an item lying on the ground at `at`, turned by `yaw`, with a soft
/// glow over it so it reads from afar, and its shadow. M5 draws these where
/// the pieces are hidden and takes them away when picked up.
pub fn pickup(item: Pickup, at: V3, yaw: i32, look: &Look, out: &mut Shapes) {
    let place = Affine::translate(at).then(&Affine::rot_y(yaw));
    pickup_model(item).placed(&place).emit(look.ink(), out);
    let glow = table().c(if item == Pickup::ChestKey {
        "brass"
    } else {
        "speed_stripe"
    });
    out.glow(
        Mesh::Dot,
        Affine::translate(at.add(v3(0, mm(160), 0))).then(&Affine::scale(v3(mm(700), mm(400), mm(700)))),
        glow,
        ONE / 5,
    );
    out.shadow(
        at.add(v3(0, crate::player::SHADOW_LIFT, 0)),
        mm(600),
        rgb(look.palette.shadow),
        ONE * 2 / 3,
    );
}

/// The model of a pickup lying with its base at the origin, its front to −z.
pub fn pickup_model(item: Pickup) -> Model {
    let t = table();
    let ink = t.ink_item;
    let mut m = Model::default();
    let (base, stripe, wing, sole) = (
        t.c("speed_base"),
        t.c("speed_stripe"),
        t.c("speed_wing"),
        t.c("speed_sole"),
    );
    let o = Affine::IDENTITY;
    match item {
        Pickup::Hood => {
            let k = m.add(
                Mesh::Facet,
                put(&o, v3(0, mm(70), 0), v3(mm(300), mm(170), mm(320))),
                base,
                ink,
            );
            let r = v3(mm(150), mm(85), mm(160));
            let f = Affine::translate(v3(0, mm(70), 0));
            decal(
                &mut m,
                k,
                &f,
                r,
                v3(0, ONE, 0),
                v3(mm(70), mm(30), mm(240)),
                DECAL_LIFT,
                stripe,
            );
            for side in [-1, 1] {
                let sweep = Affine::rot_y(side * fx::deg(20)).then(&Affine::rot_x(-fx::deg(80)));
                m.add(
                    Mesh::Wedge,
                    put_turned(
                        &o,
                        v3(side * mm(150), mm(70), mm(70)),
                        &sweep,
                        v3(mm(10), mm(130), mm(60)),
                    ),
                    wing,
                    ink,
                );
            }
        }
        Pickup::Jacket => {
            m.add(
                Mesh::Cube,
                put(&o, v3(0, mm(40), 0), v3(mm(380), mm(80), mm(300))),
                base,
                ink,
            );
            // a stripe across, its top a centimetre proud of the jacket's
            for (x, turn) in [(-mm(90), 30), (0, -30), (mm(90), 30)] {
                let a = put_turned(
                    &o,
                    v3(x, mm(80), 0),
                    &Affine::rot_y(fx::deg(turn)),
                    v3(mm(130), 2 * DECAL_LIFT, mm(40)),
                );
                m.add(Mesh::Cube, a, stripe, 0);
            }
            let collar = put_turned(
                &o,
                v3(0, mm(95), mm(120)),
                &Affine::rot_z(fx::QUARTER_TURN),
                v3(mm(70), mm(320), mm(70)),
            );
            m.add(Mesh::Cyl, collar, stripe, ink);
        }
        Pickup::Gloves => {
            for (x, turn) in [(-mm(80), 20), (mm(90), -15)] {
                let r = Affine::rot_y(fx::deg(turn));
                m.add(
                    Mesh::Facet,
                    put_turned(&o, v3(x, mm(50), 0), &r, v3(mm(120), mm(90), mm(170))),
                    base,
                    ink,
                );
                let cuff = put_turned(
                    &o,
                    v3(x, mm(50), mm(90)),
                    &r.then(&Affine::rot_x(fx::QUARTER_TURN)),
                    v3(mm(100), mm(50), mm(84)),
                );
                m.add(Mesh::Cyl, cuff, stripe, ink);
            }
        }
        Pickup::Trousers => {
            m.add(
                Mesh::Cube,
                put(&o, v3(0, mm(45), 0), v3(mm(320), mm(90), mm(360))),
                base,
                ink,
            );
            for x in [-mm(110), mm(110)] {
                m.add(
                    Mesh::Cube,
                    put(&o, v3(x, mm(90), 0), v3(mm(36), 2 * DECAL_LIFT, mm(330))),
                    stripe,
                    0,
                );
            }
        }
        Pickup::Boots => {
            for x in [-mm(100), mm(100)] {
                m.add(
                    Mesh::Facet,
                    put(&o, v3(x, mm(60), -mm(40)), v3(mm(130), mm(110), mm(250))),
                    base,
                    ink,
                );
                m.add(
                    Mesh::Cyl,
                    put(&o, v3(x, mm(150), mm(40)), v3(mm(120), mm(200), mm(120))),
                    base,
                    ink,
                );
                m.add(
                    Mesh::Cyl,
                    put(&o, v3(x, mm(240), mm(40)), v3(mm(150), mm(60), mm(150))),
                    stripe,
                    ink,
                );
                m.add(
                    Mesh::Facet,
                    put(&o, v3(x, mm(14), -mm(40)), v3(mm(140), mm(28), mm(270))),
                    sole,
                    ink,
                );
                let fin = put_turned(
                    &o,
                    v3(x + x / 2, mm(170), mm(80)),
                    &Affine::rot_x(-fx::deg(60)),
                    v3(mm(10), mm(120), mm(70)),
                );
                m.add(Mesh::Wedge, fin, wing, ink);
            }
        }
        Pickup::ChestKey => {
            let brass = t.c("brass");
            let flat = Affine::IDENTITY;
            m.add(
                Mesh::Tube,
                put(&flat, v3(0, mm(12), mm(110)), v3(mm(110), mm(24), mm(110))),
                brass,
                ink,
            );
            let shaft = put_turned(
                &flat,
                v3(0, mm(14), -mm(40)),
                &Affine::rot_x(fx::QUARTER_TURN),
                v3(mm(26), mm(220), mm(26)),
            );
            m.add(Mesh::Cyl, shaft, brass, ink);
            m.add(
                Mesh::Cube,
                put(&flat, v3(mm(30), mm(14), -mm(120)), v3(mm(50), mm(22), mm(26))),
                brass,
                ink,
            );
            m.add(
                Mesh::Cube,
                put(&flat, v3(mm(30), mm(14), -mm(84)), v3(mm(36), mm(22), mm(20))),
                brass,
                ink,
            );
            let gem = t.c("key_gem");
            m.add_glow(
                Mesh::Dot,
                put(&flat, v3(0, mm(26), mm(110)), v3(mm(30), mm(30), mm(30))),
                gem,
                ONE / 2,
            );
        }
    }
    m
}
