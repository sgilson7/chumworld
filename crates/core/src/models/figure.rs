//! The characters: one model per `pose::Kind`, dressed on the joints of a
//! `Pose`. Limbs run between the joints (upper arm shoulder to elbow,
//! forearm elbow to hand, thigh hip to knee, shin knee to foot), a ball sits
//! on each joint, the torso runs from pelvis to chest, and the head sits at
//! `pose.head` facing `pose.facing`. Sizes are millimetres on the player's
//! 1.70 m, scaled to each kind's height and, across the body, its girth
//! (`pose::build`).

use super::*;
use crate::geom::{facing, right_of};
use crate::pose::{build, Build, LEFT, RIGHT};

/// Speedster pieces worn, as bits of `Dress::worn` (R5.4).
pub const HOOD: u8 = 1;
pub const JACKET: u8 = 2;
pub const GLOVES: u8 = 4;
pub const TROUSERS: u8 = 8;
pub const BOOTS: u8 = 16;
pub const SPEEDSTER_ALL: u8 = HOOD | JACKET | GLOVES | TROUSERS | BOOTS;

/// What a figure wears beyond its kind's own clothes: the speedster pieces
/// (`HOOD` … bits) and, for a villager, which of `VILLAGER_VARIANTS` looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dress {
    pub worn: u8,
    pub variant: u8,
}

/// Limb radii, millimetres on the player (widened by girth).
const THIGH: i32 = 76;
const SHIN: i32 = 63;
const UPPER_ARM: i32 = 56;
const FOREARM: i32 = 52;

/// The player's height, which the millimetres below are written against.
const PLAYER_HEIGHT: i64 = 1700;

/// The colours a figure's body is dressed in.
#[derive(Debug, Clone, Copy)]
struct Clothes {
    skin: Rgb,
    shirt: Rgb,
    sleeve: Rgb,
    forearm: Rgb,
    hand: Rgb,
    legs: Rgb,
    shin: Rgb,
    shoe: Rgb,
}

struct Fig<'a> {
    m: Model,
    t: &'a Table,
    b: Build,
    ink: Fx,
    right: V3,
    up: V3,
    back: V3,
    fwd: V3,
    pose: Pose,
    worn: u8,
}

impl Fig<'_> {
    /// Millimetres on the player, scaled to this figure's height.
    fn s(&self, v: i32) -> Fx {
        (v as i64 * ONE as i64 * self.b.height as i64 / (PLAYER_HEIGHT * ONE as i64)) as Fx
    }

    /// The same, widened by the figure's girth.
    fn w(&self, v: i32) -> Fx {
        (self.s(v) as i64 * self.b.girth_pct as i64 * crate::pose::BODY_GIRTH_PCT as i64 / 10_000) as Fx
    }

    fn c(&self, name: &str) -> Rgb {
        self.t.c(name)
    }

    /// The body's frame with its origin at `p`.
    fn at(&self, p: V3) -> Affine {
        frame(self.right, self.up, self.back, p)
    }

    /// A point `x` right, `y` up and `z` back (millimetres, scaled) of `p`.
    fn off(&self, p: V3, x: i32, y: i32, z: i32) -> V3 {
        p.add(self.right.scale(self.w(x)))
            .add(self.up.scale(self.s(y)))
            .add(self.back.scale(self.w(z)))
    }

    /// A faceted ball of radii `r` (in the body's axes) at `p`.
    fn ball(&mut self, p: V3, r: V3, rgb: Rgb) -> usize {
        let a = put(&self.at(p), V3::ZERO, v3(2 * r.x, 2 * r.y, 2 * r.z));
        self.m.add(Mesh::Facet, a, rgb, self.ink)
    }

    /// A limb of radius `r` from `a` to `b`.
    fn limb(&mut self, a: V3, b: V3, r: Fx, rgb: Rgb) -> usize {
        let at = bar(a, b, (2 * r, 2 * r), self.right);
        self.m.add(Mesh::Cyl, at, rgb, self.ink)
    }

    /// A ring round the limb from `a` to `b` of radius `r`, at share `t` of
    /// the way, `h` long, standing `DECAL_LIFT` off it: cuffs and bands. Both
    /// are ten-sided and turned alike, so the gap between their faces is the
    /// gap between radii times cos 18° (0.951).
    fn band(&mut self, a: V3, b: V3, r: Fx, t: Fx, h: Fx, rgb: Rgb) -> usize {
        let mid = a.lerp(b, t);
        let d = b.sub(a).norm();
        let rr = r + fx::div(DECAL_LIFT, ONE * 951 / 1000) + mm(1);
        let at = bar(
            mid.sub(d.scale(h / 2)),
            mid.add(d.scale(h / 2)),
            (2 * rr, 2 * rr),
            self.right,
        );
        self.m.add(Mesh::Cyl, at, rgb, self.ink)
    }

    /// A mark `size` across (millimetres, scaled) laid on the ellipsoid
    /// part `base`, radii `r` in frame `fr`, in the direction `d`.
    fn mark(&mut self, base: usize, fr: &Affine, r: V3, d: V3, size: (i32, i32, i32), rgb: Rgb) -> usize {
        let size = v3(self.s(size.0), self.s(size.1), self.s(size.2));
        decal(&mut self.m, base, fr, r, d, size, DECAL_LIFT, rgb)
    }

    /// A ball the size of the head's radii `r` scaled, placed in the head's
    /// frame at `local` (millimetres, scaled).
    fn head_part(
        &mut self,
        local: (i32, i32, i32),
        r: (i32, i32, i32),
        turn: &Affine,
        mesh: Mesh,
        rgb: Rgb,
    ) -> usize {
        let f = self.at(self.pose.head);
        let l = v3(self.s(local.0), self.s(local.1), self.s(local.2));
        let size = v3(2 * self.s(r.0), 2 * self.s(r.1), 2 * self.s(r.2));
        let a = put_turned(&f, l, turn, size);
        self.m.add(mesh, a, rgb, self.ink)
    }
}

/// The colours each kind wears.
fn clothes(kind: Kind, variant: u8, t: &Table) -> Clothes {
    let c = |n: &str| t.c(n);
    match kind {
        Kind::Player => Clothes {
            skin: c("skin_light"),
            shirt: c("player_shirt"),
            sleeve: c("player_shirt"),
            forearm: c("skin_light"),
            hand: c("skin_light"),
            legs: c("player_trousers"),
            shin: c("player_trousers"),
            shoe: c("player_slipper"),
        },
        Kind::Orc => Clothes {
            skin: c("orc_skin"),
            shirt: c("orc_cloth"),
            sleeve: c("orc_skin"),
            forearm: c("orc_skin"),
            hand: c("orc_skin"),
            legs: c("orc_cloth"),
            shin: c("orc_wrap"),
            shoe: c("orc_leather"),
        },
        Kind::Bandit => Clothes {
            skin: c("skin_tan"),
            shirt: c("bandit_tunic"),
            sleeve: c("bandit_tunic"),
            forearm: c("bandit_wrap"),
            hand: c("skin_tan"),
            legs: c("bandit_trousers"),
            shin: c("bandit_trousers"),
            shoe: c("bandit_boot"),
        },
        Kind::Lookout => Clothes {
            skin: c("skin_warm"),
            shirt: c("lookout_tunic"),
            sleeve: c("lookout_tunic"),
            forearm: c("skin_warm"),
            hand: c("skin_warm"),
            legs: c("bandit_trousers"),
            shin: c("lookout_wrap"),
            shoe: c("bandit_boot"),
        },
        Kind::Leader => Clothes {
            skin: c("skin_tan"),
            shirt: c("leader_tunic"),
            sleeve: c("leader_tunic"),
            forearm: c("leader_tunic"),
            hand: c("skin_tan"),
            legs: c("leader_trousers"),
            shin: c("leader_trousers"),
            shoe: c("leader_boot"),
        },
        Kind::Merchant => Clothes {
            skin: c("skin_light"),
            shirt: c("merchant_shirt"),
            sleeve: c("merchant_shirt"),
            forearm: c("skin_light"),
            hand: c("skin_light"),
            legs: c("merchant_trousers"),
            shin: c("merchant_trousers"),
            shoe: c("merchant_shoe"),
        },
        Kind::Trainer => Clothes {
            skin: c("skin_old"),
            shirt: c("trainer_coat"),
            sleeve: c("trainer_coat"),
            forearm: c("trainer_coat"),
            hand: c("skin_old"),
            legs: c("trainer_trousers"),
            shin: c("trainer_boot"),
            shoe: c("trainer_boot"),
        },
        Kind::Villager => {
            let v = variant % VILLAGER_VARIANTS;
            let shirt = c([
                "villager_tunic_0",
                "villager_dress_1",
                "villager_vest_2",
                "villager_dress_3",
            ][v as usize]);
            let skin = c(["skin_warm", "skin_light", "skin_old", "skin_tan"][v as usize]);
            Clothes {
                skin,
                shirt,
                sleeve: if v == 2 { c("villager_shirt_2") } else { shirt },
                forearm: skin,
                hand: skin,
                legs: c("villager_trousers"),
                shin: c("villager_trousers"),
                shoe: c("villager_shoe"),
            }
        }
    }
}

/// The head's radii, millimetres on the player: wider for an orc.
fn head_radii(kind: Kind) -> (i32, i32, i32) {
    match kind {
        Kind::Orc => (222, 200, 205),
        Kind::Merchant => (200, 205, 190),
        _ => (190, 205, 182),
    }
}

/// The model of a kind on a pose, dressed.
pub fn model(kind: Kind, pose: &Pose, dress: Dress) -> Model {
    let t = table();
    let b = build(kind);
    let fwd = facing(pose.facing, 0);
    let up0 = pose.chest.sub(pose.pelvis).norm();
    let up = if up0 == V3::ZERO { V3::UP } else { up0 };
    let back0 = fwd.neg();
    let back = back0.sub(up.scale(back0.dot(up))).norm();
    let back = if back == V3::ZERO { v3(0, 0, ONE) } else { back };
    let right = up.cross(back).norm();
    let mut f = Fig {
        m: Model::default(),
        t,
        b,
        ink: t.ink_figure,
        right,
        up,
        back,
        fwd: back.neg(),
        pose: *pose,
        worn: dress.worn,
    };
    let mut cl = clothes(kind, dress.variant, t);
    let speed = t.c("speed_base");
    if dress.worn & JACKET != 0 {
        cl.shirt = speed;
        cl.sleeve = speed;
        cl.forearm = speed;
    }
    if dress.worn & GLOVES != 0 {
        cl.hand = speed;
    }
    if dress.worn & TROUSERS != 0 {
        cl.legs = speed;
        cl.shin = speed;
    }
    if dress.worn & BOOTS != 0 {
        cl.shoe = speed;
    }
    legs(&mut f, &cl);
    let torso = body(&mut f, kind, &cl);
    arms(&mut f, &cl);
    let hr = head_radii(kind);
    let head = head(&mut f, kind, &cl, hr);
    face(&mut f, kind, head, hr, dress);
    outfit(&mut f, kind, dress, &cl, head, hr, torso);
    if dress.worn != 0 {
        speedster(&mut f, head, hr, torso);
    }
    f.m
}

fn legs(f: &mut Fig, cl: &Clothes) {
    let p = f.pose;
    for s in [LEFT, RIGHT] {
        let ankle = p.foot[s].add(V3::UP.scale(f.s(70)));
        f.limb(p.hip[s], p.knee[s], f.w(THIGH), cl.legs);
        f.ball(p.knee[s], v3(f.w(70), f.s(70), f.w(70)), cl.shin);
        f.limb(p.knee[s], ankle, f.w(SHIN), cl.shin);
        // the shoe, along the way the body faces
        let shoe = p.foot[s].add(V3::UP.scale(f.s(44))).add(f.fwd.scale(f.s(34)));
        let r = if f.worn & BOOTS != 0 {
            v3(f.w(80), f.s(62), f.s(130))
        } else {
            v3(f.w(70), f.s(52), f.s(120))
        };
        f.ball(shoe, r, cl.shoe);
    }
}

/// The torso: hips, belly and the shoulders' yoke. Returns the belly's
/// index, radii and centre, for marks laid on it.
fn body(f: &mut Fig, kind: Kind, cl: &Clothes) -> (usize, V3, V3) {
    let p = f.pose;
    let hips = f.off(p.pelvis, 0, 12, 0);
    // under a skirt or a coat, the hips are the skirt's colour
    let skirted = matches!(kind, Kind::Player | Kind::Trainer | Kind::Villager);
    f.ball(
        hips,
        v3(f.w(152), f.s(108), f.w(112)),
        if skirted { cl.shirt } else { cl.legs },
    );
    let mid = p.pelvis.lerp(p.chest, ONE / 2);
    let half = p.chest.sub(p.pelvis).len() / 2 + f.s(60);
    let deep = if kind == Kind::Merchant { 150 } else { 122 };
    let belly_c = mid.add(f.up.scale(f.s(20)));
    let belly_r = v3(f.w(160), half, f.w(deep));
    let belly = f.ball(belly_c, belly_r, cl.shirt);
    let yoke = f.off(p.chest, 0, -26, 4);
    f.ball(yoke, v3(f.w(188), f.s(100), f.w(118)), cl.shirt);
    // the neck, under the head
    let chin = p.head.sub(f.up.scale(f.s(150)));
    f.limb(p.chest, chin, f.s(52), cl.skin);
    (belly, belly_c, belly_r)
}

fn arms(f: &mut Fig, cl: &Clothes) {
    let p = f.pose;
    for s in [LEFT, RIGHT] {
        f.ball(p.shoulder[s], v3(f.w(62), f.s(62), f.w(62)), cl.sleeve);
        f.limb(p.shoulder[s], p.elbow[s], f.w(UPPER_ARM), cl.sleeve);
        f.ball(p.elbow[s], v3(f.w(57), f.s(57), f.w(57)), cl.forearm);
        f.limb(p.elbow[s], p.hand[s], f.w(FOREARM), cl.forearm);
        f.ball(p.hand[s], v3(f.w(64), f.s(64), f.w(64)), cl.hand);
        if f.worn & GLOVES != 0 {
            let speed_stripe = f.c("speed_stripe");
            f.band(
                p.elbow[s],
                p.hand[s],
                f.w(FOREARM),
                ONE * 80 / 100,
                f.s(44),
                speed_stripe,
            );
        }
    }
}

fn head(f: &mut Fig, kind: Kind, cl: &Clothes, r: (i32, i32, i32)) -> usize {
    let _ = kind;
    f.head_part((0, 0, 0), r, &Affine::IDENTITY, Mesh::Facet, cl.skin)
}

/// The head's radii as a vector, scaled.
fn hv(f: &Fig, r: (i32, i32, i32)) -> V3 {
    v3(f.s(r.0), f.s(r.1), f.s(r.2))
}

/// Dot eyes with a catch-light each, and blush: the faces of Protection of
/// the Spheres. Each mark stands `DECAL_LIFT` off the surface under it.
fn face(f: &mut Fig, kind: Kind, head: usize, hr: (i32, i32, i32), dress: Dress) {
    let hf = f.at(f.pose.head);
    let r = hv(f, hr);
    let (eye_w, eye_h, spread, el) = match kind {
        Kind::Orc => (44, 52, 24, -2),
        Kind::Merchant => (46, 56, 21, -2),
        _ => (52, 64, 21, -1),
    };
    let dark = f.c("eye");
    let white = f.c("catch_light");
    let patch = kind == Kind::Trainer;
    for side in [-1, 1] {
        if patch && side == -1 {
            continue;
        }
        let d = toward(side * fx::deg(spread), fx::deg(el));
        let size = v3(f.s(eye_w), f.s(eye_h), f.s(30));
        let eye = decal(&mut f.m, head, &hf, r, d, size, DECAL_LIFT, dark);
        let cs = v3(f.s(20), f.s(20), f.s(22));
        catch_light(&mut f.m, eye, cs, white);
    }
    let blush = !matches!(kind, Kind::Bandit | Kind::Lookout | Kind::Orc) || dress.worn & HOOD != 0;
    let scarfed = matches!(kind, Kind::Bandit | Kind::Lookout);
    if blush && !scarfed {
        let pink = f.c("blush");
        for side in [-1, 1] {
            let d = toward(side * fx::deg(36), -fx::deg(15));
            f.mark(head, &hf, r, d, (58, 28, 24), pink);
        }
    }
    if kind == Kind::Orc {
        // the orc's cheeks are darker, a bruise-green flush
        let flush = f.c("orc_flush");
        for side in [-1, 1] {
            let d = toward(side * fx::deg(44), -fx::deg(16));
            f.mark(head, &hf, r, d, (46, 22, 24), flush);
        }
    }
    if matches!(kind, Kind::Merchant | Kind::Villager) {
        // a small smile
        let d = toward(0, -fx::deg(34));
        f.mark(head, &hf, r, d, (36, 12, 22), dark);
    }
}

/// Each kind's own clothes and the one thing that says who they are.
fn outfit(
    f: &mut Fig,
    kind: Kind,
    dress: Dress,
    cl: &Clothes,
    head: usize,
    hr: (i32, i32, i32),
    torso: (usize, V3, V3),
) {
    let hooded = f.worn & HOOD != 0;
    let p = f.pose;
    let hrv = hv(f, hr);
    let none = Affine::IDENTITY;
    match kind {
        Kind::Player => {
            if !hooded {
                // the nightcap: a soft cap, a cone drooping back, a pompom
                let cap = f.c("player_cap");
                let hair = f.c("player_hair");
                f.head_part((0, 6, 54), (198, 186, 170), &none, Mesh::Facet, hair);
                f.head_part((0, 92, -118), (166, 48, 84), &none, Mesh::Facet, hair);
                f.head_part((0, 66, 14), (200, 152, 194), &none, Mesh::Facet, cap);
                let flop = Affine::rot_z(-fx::deg(96)).then(&Affine::rot_x(fx::deg(10)));
                f.head_part((96, 176, 26), (78, 170, 78), &flop, Mesh::Cone, cap);
                let pom = f.c("player_pompom");
                f.head_part((268, 148, 30), (48, 48, 48), &none, Mesh::Facet, pom);
                let _ = (hrv, head);
            }
            // the nightshirt's skirt, to mid-thigh
            skirt(f, 60, -260, (196, 168), cl.shirt);
        }
        Kind::Orc => orc(f, hooded, cl, torso),
        Kind::Bandit | Kind::Lookout => {
            let (hood_c, scarf_c) = if kind == Kind::Bandit {
                (f.c("bandit_hood"), f.c("bandit_scarf"))
            } else {
                (f.c("lookout_hood"), f.c("lookout_scarf"))
            };
            if !hooded {
                hood(f, hr, hood_c, true);
            }
            // the scarf over mouth and chin, and its tail down the back
            f.head_part(
                (0, -116, -6),
                (hr.0 + 14, 62, hr.2 + 16),
                &none,
                Mesh::Facet,
                scarf_c,
            );
            let tail = Affine::rot_x(-fx::deg(14));
            f.head_part((60, -190, 150), (40, 120, 24), &tail, Mesh::Taper, scarf_c);
            belt(f, torso, f.c("bandit_belt"), f.c("brass"));
            if kind == Kind::Lookout {
                // the spyglass on the right hip
                let a = f.off(p.pelvis, 190, -20, -30);
                let b = f.off(p.pelvis, 200, -170, -60);
                let brass = f.c("brass");
                let at = bar(a, b, (f.s(44), f.s(44)), f.right);
                f.m.add(Mesh::Cyl, at, brass, f.ink);
                let lens = f.c("glass");
                let at = bar(
                    b,
                    b.add(b.sub(a).norm().scale(f.s(30))),
                    (f.s(56), f.s(56)),
                    f.right,
                );
                f.m.add(Mesh::Cyl, at, lens, f.ink);
                // a feather in the hood
                let feather = f.c("lookout_feather");
                let tilt = Affine::rot_z(-fx::deg(30)).then(&Affine::rot_x(fx::deg(25)));
                if !hooded {
                    f.head_part((150, 170, 60), (22, 120, 10), &tilt, Mesh::Facet, feather);
                }
            } else {
                // the knife's sheath at the left hip
                let a = f.off(p.pelvis, -175, 10, -10);
                let b = f.off(p.pelvis, -195, -190, -40);
                let leather = f.c("bandit_belt");
                let at = bar(a, b, (f.s(56), f.s(30)), f.right);
                f.m.add(Mesh::Taper, at, leather, f.ink);
            }
        }
        Kind::Leader => leader(f, hooded, hrv, head, torso),
        Kind::Merchant => merchant(f, hooded, hrv, head, torso),
        Kind::Trainer => trainer(f, hooded, hrv, head, torso),
        Kind::Villager => villager(
            f,
            dress.variant % VILLAGER_VARIANTS,
            hooded,
            hr,
            hrv,
            head,
            torso,
            cl,
        ),
    }
}

/// Tufts of hair across the forehead, under a cap or hood.
fn hair_fringe(f: &mut Fig, r: V3, head: usize, hair: Rgb) {
    let hf = f.at(f.pose.head);
    for (az, el) in [(-30, 30), (-10, 36), (12, 34), (32, 28)] {
        let (p, n) = on_ellipsoid(r, toward(fx::deg(az), fx::deg(el)));
        let c = p.add(n.scale(-f.s(6)));
        let a = put(&hf, c, v3(f.s(84), f.s(56), f.s(64)));
        let k = f.m.add(Mesh::Facet, a, hair, f.ink);
        let _ = (k, head);
    }
}

/// A skirt from `top` to `bottom` (millimetres above the pelvis), its hem's
/// radii `hem` (x, z): the nightshirt's, a dress's, a coat's.
fn skirt(f: &mut Fig, top: i32, bottom: i32, hem: (i32, i32), rgb: Rgb) {
    let p = f.pose.pelvis;
    let a = f.off(p, 0, bottom, 0);
    let b = f.off(p, 0, top, 0);
    let at = bar(a, b, (f.w(2 * hem.0), f.w(2 * hem.1)), f.right);
    f.m.add(Mesh::Taper, at, rgb, f.ink);
}

/// A hood: a shell set back on the head so the face shows through its
/// front, a point at the back, and a cowl on the shoulders.
fn hood(f: &mut Fig, hr: (i32, i32, i32), rgb: Rgb, pointed: bool) {
    let none = Affine::IDENTITY;
    f.head_part(
        (0, 18, 42),
        (hr.0 + 22, hr.1 + 20, hr.2 + 10),
        &none,
        Mesh::Facet,
        rgb,
    );
    if pointed {
        let back = Affine::rot_x(-fx::deg(62));
        f.head_part((0, 120, 160), (70, 110, 70), &back, Mesh::Cone, rgb);
    }
    let p = f.pose;
    let cowl = f.off(p.chest, 0, 30, 10);
    f.ball(cowl, v3(f.w(200), f.s(84), f.w(150)), rgb);
}

/// A belt round the belly at the pelvis's height, standing off it, with a
/// buckle in front.
fn belt(f: &mut Fig, torso: (usize, V3, V3), strap: Rgb, buckle: Rgb) {
    let (_, c, r) = torso;
    let p = f.pose;
    let y = p.pelvis.add(f.up.scale(f.s(40)));
    // the belly's half-widths at that height, then out past its widest
    let lift = fx::div(DECAL_LIFT, ONE * 951 / 1000) + mm(2);
    let (rx, rz) = (r.x + lift, r.z + lift);
    let h = f.s(56);
    let a = y.sub(f.up.scale(h / 2));
    let b = y.add(f.up.scale(h / 2));
    let at = bar(a, b, (2 * rx, 2 * rz), f.right);
    f.m.add(Mesh::Cyl, at, strap, f.ink);
    let _ = c;
    // the buckle, standing off the belt's front face
    let front = y.add(f.fwd.scale(fx::mul(rz, ONE * 951 / 1000) + DECAL_LIFT));
    let bf = f.at(front);
    let size = v3(f.s(70), f.s(62), f.s(24));
    let at = put(&bf, V3::ZERO, size);
    f.m.add(Mesh::Cube, at, buckle, f.ink);
}

fn orc(f: &mut Fig, hooded: bool, cl: &Clothes, torso: (usize, V3, V3)) {
    let none = Affine::IDENTITY;
    let p = f.pose;
    let dark = f.c("orc_skin_dark");
    let tusk = f.c("orc_tusk");
    let iron = f.c("orc_iron");
    let iron_dark = f.c("orc_iron_dark");
    let leather = f.c("orc_leather");
    // a heavy jaw, a brow ridge over the eyes, pointed ears, tusks
    f.head_part((0, -96, -28), (196, 112, 170), &none, Mesh::Facet, cl.skin);
    f.head_part((0, 52, -132), (176, 44, 64), &none, Mesh::Facet, dark);
    for side in [-1, 1] {
        let ear = Affine::rot_z(-side * fx::deg(72)).then(&Affine::rot_x(fx::deg(18)));
        f.head_part((side * 214, 22, 30), (34, 92, 50), &ear, Mesh::Cone, cl.skin);
        let up = Affine::rot_z(-side * fx::deg(12));
        f.head_part((side * 88, -64, -178), (18, 46, 18), &up, Mesh::Cone, tusk);
    }
    if !hooded {
        // a black topknot
        let hair = f.c("orc_hair");
        f.head_part((0, 196, 30), (60, 50, 60), &none, Mesh::Facet, hair);
        let tie = Affine::rot_x(-fx::deg(40));
        f.head_part((0, 250, 76), (26, 70, 26), &tie, Mesh::Cone, hair);
    }
    // crude armour: one great pauldron with spikes, a plate on the chest, bracers
    let sh = p.shoulder[LEFT];
    let pad = f.off(sh, -18, 36, 0);
    f.ball(pad, v3(f.w(118), f.s(84), f.w(118)), iron);
    for (dx, dz) in [(-30, -40), (-40, 40)] {
        let spike = f.off(sh, dx, 116, dz);
        let a = put_turned(
            &f.at(spike),
            V3::ZERO,
            &Affine::rot_z(fx::deg(20)),
            v3(f.s(34), f.s(80), f.s(34)),
        );
        f.m.add(Mesh::Cone, a, iron_dark, f.ink);
    }
    let (_, bc, br) = torso;
    let plate_c = bc.add(f.up.scale(f.s(70))).add(f.fwd.scale(f.s(26)));
    f.ball(
        plate_c,
        v3(br.x * 92 / 100, br.y * 66 / 100, br.z * 96 / 100),
        iron,
    );
    // rivets on the plate, standing off it
    let pf = f.at(plate_c);
    let pr = v3(br.x * 92 / 100, br.y * 66 / 100, br.z * 96 / 100);
    let k = f.m.parts.len() - 1;
    let rivet = f.c("orc_rivet");
    for (az, el) in [(-26, 22), (26, 22), (-30, -14), (30, -14)] {
        f.mark(k, &pf, pr, toward(fx::deg(az), fx::deg(el)), (26, 26, 24), rivet);
    }
    // a strap from the pauldron across the chest
    belt(f, torso, leather, iron_dark);
    for s in [LEFT, RIGHT] {
        f.band(
            p.elbow[s],
            p.hand[s],
            f.w(FOREARM),
            ONE * 55 / 100,
            f.s(120),
            leather,
        );
    }
    // a hide kilt
    skirt(f, 30, -210, (176, 134), f.c("orc_hide"));
}

fn leader(f: &mut Fig, hooded: bool, hr: V3, head: usize, torso: (usize, V3, V3)) {
    let none = Affine::IDENTITY;
    let p = f.pose;
    let mantle = f.c("leader_mantle");
    let lining = f.c("leader_lining");
    let gold = f.c("leader_gold");
    if !hooded {
        // dark hair swept back, and a gold circlet with a gem
        let hair = f.c("leader_hair");
        f.head_part((0, 44, 40), (196, 172, 178), &none, Mesh::Facet, hair);
        let band_r = (
            hr.x + fx::div(DECAL_LIFT, ONE * 92 / 100),
            hr.z + fx::div(DECAL_LIFT, ONE * 92 / 100),
        );
        let hf = f.at(p.head);
        let y = f.s(70);
        // the head's half-widths at the circlet's height
        let k = fx::sqrt(ONE - fx::mul(fx::div(y, hr.y), fx::div(y, hr.y)));
        let (bx, bz) = (fx::mul(band_r.0, k) + mm(4), fx::mul(band_r.1, k) + mm(4));
        let at = put(&hf, v3(0, y, 0), v3(2 * bx, f.s(30), 2 * bz));
        f.m.add(Mesh::Tube, at, gold, f.ink);
        let gem = f.c("leader_gem");
        let gat = put(&hf, v3(0, y, -bz - f.s(8)), v3(f.s(34), f.s(40), f.s(24)));
        f.m.add_glow(Mesh::Dot, gat, gem, ONE / 2);
    }
    let _ = head;
    // a short dark beard
    let beard = f.c("leader_hair");
    f.head_part((0, -150, -96), (110, 64, 86), &none, Mesh::Facet, beard);
    // the mantle: a great cape round the shoulders and down the back, its
    // lining showing at a high collar
    let cape = f.off(p.chest, 0, -330, 110);
    f.ball(cape, v3(f.w(236), f.s(480), f.w(112)), mantle);
    let drape = f.off(p.chest, 0, -4, 16);
    f.ball(drape, v3(f.w(262), f.s(150), f.w(170)), mantle);
    let collar = f.off(p.chest, 0, 120, 70);
    f.ball(collar, v3(f.w(170), f.s(120), f.w(70)), lining);
    // a gold clasp at the throat
    let (_, bc, br) = torso;
    let _ = (bc, br);
    let clasp = f.off(p.chest, 0, 10, -170);
    let at = put(&f.at(clasp), V3::ZERO, v3(f.s(64), f.s(64), f.s(40)));
    f.m.add(Mesh::Facet, at, gold, f.ink);
    belt(f, torso, f.c("leader_belt"), gold);
    // the caster's sign: three orbs of the leader's pattern by the free hand
    let hand = p.hand[LEFT];
    for (k, name) in ["orb_rose", "orb_sky", "orb_gold"].iter().enumerate() {
        let a = fx::TURN * k as i32 / 3 + fx::deg(20);
        let o = f
            .right
            .scale(fx::mul(fx::cos(a), f.s(150)))
            .add(f.fwd.scale(fx::mul(fx::sin(a), f.s(150))))
            .add(f.up.scale(f.s(80 + 40 * k as i32)));
        let at = put(&f.at(hand.add(o)), V3::ZERO, v3(f.s(64), f.s(64), f.s(64)));
        let c = f.c(name);
        f.m.add_glow(Mesh::Dot, at, c, ONE * 3 / 4);
    }
}

fn merchant(f: &mut Fig, hooded: bool, hr: V3, head: usize, torso: (usize, V3, V3)) {
    let none = Affine::IDENTITY;
    let p = f.pose;
    let hf = f.at(p.head);
    let skin = f.c("skin_light");
    // a round nose and a moustache
    f.head_part((0, -38, -196), (36, 32, 36), &none, Mesh::Facet, skin);
    let tache = f.c("merchant_moustache");
    for side in [-1, 1] {
        let turn = Affine::rot_z(side * fx::deg(16));
        f.head_part((side * 46, -74, -180), (52, 20, 30), &turn, Mesh::Facet, tache);
    }
    let _ = (head, hr, hf);
    if !hooded {
        // the big hat: a wide brim, a crown, a band
        let straw = f.c("merchant_hat");
        let band = f.c("merchant_hat_band");
        f.head_part((0, 120, 0), (340, 14, 330), &none, Mesh::Cyl, straw);
        f.head_part((0, 210, 0), (170, 80, 165), &none, Mesh::Taper, straw);
        // the band round the crown's foot, off its faces (eight sides:
        // the faces sit at cos 22.5° = 0.924 of the radius)
        let lift = fx::div(DECAL_LIFT, ONE * 924 / 1000) / (ONE / 1000) + 2;
        let br = (170 * 96 / 100 + lift, 165 * 96 / 100 + lift);
        f.head_part((0, 160, 0), (br.0, 22, br.1), &none, Mesh::Taper, band);
    }
    // the apron, from the chest to the knee, standing off the belly
    let (_, bc, br) = torso;
    let apron_c = f.c("merchant_apron");
    let front = bc
        .sub(f.up.scale(f.s(150)))
        .add(f.fwd.scale(br.z + DECAL_LIFT + f.s(8)));
    let at = put(&f.at(front), V3::ZERO, v3(f.w(240), f.s(480), f.s(16)));
    f.m.add(Mesh::Cube, at, apron_c, f.ink);
    // its bib over the chest, and the strap round the neck
    let bib = bc
        .add(f.up.scale(f.s(170)))
        .add(f.fwd.scale(br.z + DECAL_LIFT + f.s(8)));
    let at = put(&f.at(bib), V3::ZERO, v3(f.w(170), f.s(170), f.s(16)));
    f.m.add(Mesh::Cube, at, apron_c, f.ink);
    // a pocket on it
    let pocket = front
        .add(f.fwd.scale(f.s(8) + DECAL_LIFT))
        .sub(f.up.scale(f.s(80)));
    let at = put(&f.at(pocket), V3::ZERO, v3(f.w(130), f.s(80), f.s(16)));
    let pc = f.c("merchant_pocket");
    f.m.add(Mesh::Cube, at, pc, f.ink);
    belt(f, torso, f.c("merchant_belt"), f.c("brass"));
    // a fat purse on the right hip
    let purse = f.off(p.pelvis, 205, -60, -60);
    let pc = f.c("merchant_purse");
    f.ball(purse, v3(f.s(62), f.s(72), f.s(56)), pc);
}

fn trainer(f: &mut Fig, hooded: bool, hr: V3, head: usize, torso: (usize, V3, V3)) {
    let none = Affine::IDENTITY;
    let p = f.pose;
    let grey = f.c("trainer_hair");
    let brass = f.c("brass");
    let hf = f.at(p.head);
    if !hooded {
        // grey hair and a topknot
        f.head_part((0, 40, 44), (196, 172, 176), &none, Mesh::Facet, grey);
        f.head_part((0, 214, 40), (58, 52, 58), &none, Mesh::Facet, grey);
    }
    // bushy brows, a long grey beard, a patch over the left eye
    for side in [-1, 1] {
        let tilt = Affine::rot_z(side * fx::deg(12));
        f.head_part((side * 66, 44, -170), (44, 14, 22), &tilt, Mesh::Cube, grey);
    }
    f.head_part((0, -150, -110), (124, 112, 92), &none, Mesh::Facet, grey);
    let patch = f.c("trainer_patch");
    f.mark(
        head,
        &hf,
        hr,
        toward(-fx::deg(24), -fx::deg(5)),
        (66, 62, 24),
        patch,
    );
    // the patch's strap, a thin ring round the head
    let y = f.s(10);
    let k = fx::sqrt(ONE - fx::mul(fx::div(y, hr.y), fx::div(y, hr.y)));
    let lift = fx::div(DECAL_LIFT, ONE * 92 / 100) + mm(2);
    let (bx, bz) = (fx::mul(hr.x, k) + lift, fx::mul(hr.z, k) + lift);
    let at = put_turned(
        &hf,
        v3(0, y, 0),
        &Affine::rot_z(fx::deg(10)),
        v3(2 * bx, f.s(12), 2 * bz),
    );
    f.m.add(Mesh::Tube, at, patch, 0);
    // the duelist's coat: long skirts, a high collar, brass buttons, a sash
    let coat = f.c("trainer_coat");
    let trim = f.c("trainer_trim");
    skirt(f, 40, -400, (190, 168), coat);
    let collar = f.off(p.chest, 0, 70, 30);
    f.ball(collar, v3(f.w(140), f.s(70), f.w(120)), trim);
    let (k, bc, br) = torso;
    let bf = f.at(bc);
    for (az, el) in [(-16, 30), (16, 30), (-18, 10), (18, 10), (-19, -10), (19, -10)] {
        f.mark(k, &bf, br, toward(fx::deg(az), fx::deg(el)), (26, 26, 24), brass);
    }
    belt(f, torso, f.c("trainer_sash"), brass);
    for s in [LEFT, RIGHT] {
        f.band(p.elbow[s], p.hand[s], f.w(FOREARM), ONE * 82 / 100, f.s(56), trim);
    }
}

#[allow(clippy::too_many_arguments)]
fn villager(
    f: &mut Fig,
    v: u8,
    hooded: bool,
    hr: (i32, i32, i32),
    hrv: V3,
    head: usize,
    torso: (usize, V3, V3),
    cl: &Clothes,
) {
    let none = Affine::IDENTITY;
    let p = f.pose;
    match v {
        0 => {
            // the farmer: a straw hat and a rope belt
            if !hooded {
                let straw = f.c("villager_straw");
                f.head_part((0, 122, 6), (290, 14, 282), &none, Mesh::Cyl, straw);
                f.head_part((0, 186, 6), (150, 66, 146), &none, Mesh::Facet, straw);
                hair_fringe(f, hrv, head, f.c("villager_hair_0"));
            }
            belt(f, torso, f.c("villager_rope"), f.c("villager_rope"));
            skirt(f, 40, -170, (172, 140), cl.shirt);
        }
        1 => {
            // a headscarf knotted at the back, a long skirt, a shawl
            if !hooded {
                let scarf = f.c("villager_kerchief_1");
                f.head_part(
                    (0, 34, 36),
                    (hr.0 + 14, hr.1 + 8, hr.2 + 6),
                    &none,
                    Mesh::Facet,
                    scarf,
                );
                f.head_part((0, -40, 206), (54, 50, 46), &none, Mesh::Facet, scarf);
                let tail = Affine::rot_x(-fx::deg(20));
                f.head_part((0, -130, 210), (40, 80, 20), &tail, Mesh::Taper, scarf);
            }
            skirt(f, 40, -660, (200, 176), cl.shirt);
            let shawl = f.c("villager_shawl_1");
            let at = f.off(p.chest, 0, 10, 14);
            f.ball(at, v3(f.w(218), f.s(90), f.w(150)), shawl);
        }
        2 => {
            // the old man: bald on top, white hair at the sides, a big beard, a cane
            let white = f.c("villager_white_2");
            for side in [-1, 1] {
                f.head_part((side * 150, -10, 60), (64, 90, 100), &none, Mesh::Facet, white);
            }
            f.head_part((0, -148, -104), (140, 126, 96), &none, Mesh::Facet, white);
            let brow = Affine::IDENTITY;
            for side in [-1, 1] {
                f.head_part((side * 66, 46, -168), (40, 14, 22), &brow, Mesh::Cube, white);
            }
            let ground = p.foot[LEFT].y.min(p.foot[RIGHT].y);
            let hand = p.hand[RIGHT];
            let foot = v3(hand.x, ground, hand.z).add(f.fwd.scale(f.s(60)));
            let cane = f.c("villager_cane");
            let at = bar(foot, hand.add(V3::UP.scale(f.s(60))), (f.s(30), f.s(30)), f.right);
            f.m.add(Mesh::Cyl, at, cane, f.ink);
            let knob = put(
                &f.at(hand.add(V3::UP.scale(f.s(70)))),
                V3::ZERO,
                v3(f.s(50), f.s(44), f.s(50)),
            );
            f.m.add(Mesh::Facet, knob, cane, f.ink);
            skirt(f, 40, -150, (170, 136), cl.shirt);
        }
        _ => {
            // a bun of hair, a dress to the shin, a basket on the arm
            if !hooded {
                let hair = f.c("villager_hair_3");
                f.head_part((0, 40, 40), (200, 178, 180), &none, Mesh::Facet, hair);
                f.head_part((0, 160, 150), (78, 74, 74), &none, Mesh::Facet, hair);
            }
            skirt(f, 40, -540, (196, 170), cl.shirt);
            let hand = p.hand[LEFT];
            let wicker = f.c("villager_wicker");
            let base = hand.sub(V3::UP.scale(f.s(120)));
            let at = put(&f.at(base), V3::ZERO, v3(f.s(200), f.s(130), f.s(150)));
            f.m.add(Mesh::Taper, at, wicker, f.ink);
            let handle = put_turned(
                &f.at(hand.sub(V3::UP.scale(f.s(30)))),
                V3::ZERO,
                &Affine::rot_z(fx::QUARTER_TURN),
                v3(f.s(150), f.s(16), f.s(140)),
            );
            f.m.add(Mesh::Tube, handle, wicker, f.ink);
            let bread = f.c("villager_bread");
            let at = put(
                &f.at(base.add(V3::UP.scale(f.s(70)))),
                V3::ZERO,
                v3(f.s(120), f.s(60), f.s(80)),
            );
            f.m.add(Mesh::Facet, at, bread, f.ink);
        }
    }
}

/// The speedster pieces worn: a sleek hood with swept fins, a stripe across
/// the jacket, bands on the trousers, fins on the boots. The plain colours
/// are already on the body (`model`).
fn speedster(f: &mut Fig, head: usize, hr: (i32, i32, i32), torso: (usize, V3, V3)) {
    let p = f.pose;
    let base = f.c("speed_base");
    let stripe = f.c("speed_stripe");
    let wing = f.c("speed_wing");
    let none = Affine::IDENTITY;
    let _ = head;
    if f.worn & HOOD != 0 {
        f.head_part(
            (0, 22, 34),
            (hr.0 + 16, hr.1 + 16, hr.2 + 8),
            &none,
            Mesh::Facet,
            base,
        );
        for side in [-1, 1] {
            let sweep = Affine::rot_y(side * fx::deg(8)).then(&Affine::rot_x(-fx::deg(70)));
            f.head_part((side * 196, 60, 90), (12, 120, 54), &sweep, Mesh::Wedge, wing);
        }
        // a stripe over the crown, front to back
        let hrv = hv(f, (hr.0 + 16, hr.1 + 16, hr.2 + 8));
        let hf = f.at(p.head).then(&Affine::translate(v3(0, f.s(22), f.s(34))));
        let k = f.m.parts.len() - 3;
        f.mark(k, &hf, hrv, toward(0, fx::deg(60)), (60, 190, 24), stripe);
    }
    if f.worn & JACKET != 0 {
        // a lightning stripe across the chest, and a collar
        let (k, bc, br) = torso;
        let bf = f.at(bc);
        for (az, el, len) in [(-20, 26, 120), (0, 14, 110), (20, 2, 120)] {
            f.mark(
                k,
                &bf,
                br,
                toward(fx::deg(az), fx::deg(el)),
                (len, 34, 24),
                stripe,
            );
        }
        let collar = f.off(p.chest, 0, 60, 20);
        f.ball(collar, v3(f.w(140), f.s(64), f.w(118)), stripe);
    }
    if f.worn & TROUSERS != 0 {
        for s in [LEFT, RIGHT] {
            f.band(p.hip[s], p.knee[s], f.w(THIGH), ONE * 70 / 100, f.s(40), stripe);
            f.band(p.knee[s], p.foot[s], f.w(SHIN), ONE * 30 / 100, f.s(40), stripe);
        }
    }
    if f.worn & BOOTS != 0 {
        for s in [LEFT, RIGHT] {
            let ankle = p.foot[s].add(V3::UP.scale(f.s(70)));
            f.band(p.knee[s], ankle, f.w(SHIN), ONE * 82 / 100, f.s(110), base);
            let sole = p.foot[s].add(V3::UP.scale(f.s(10))).add(f.fwd.scale(f.s(34)));
            let at = put(&f.at(sole), V3::ZERO, v3(f.w(130), f.s(28), f.s(250)));
            f.m.add(Mesh::Facet, at, stripe, f.ink);
            let heel = f.off(p.foot[s], if s == LEFT { -72 } else { 72 }, 80, 60);
            let fin = put_turned(
                &f.at(heel),
                V3::ZERO,
                &Affine::rot_x(-fx::deg(60)),
                v3(f.s(20), f.s(110), f.s(70)),
            );
            f.m.add(Mesh::Wedge, fin, wing, f.ink);
        }
    }
}

/// A soft round shadow on the ground under the feet.
pub(super) fn shadow(kind: Kind, pose: &Pose, look: &Look, out: &mut Shapes) {
    let b = build(kind);
    let ground = pose.foot[LEFT].y.min(pose.foot[RIGHT].y);
    let mid = pose.foot[LEFT].lerp(pose.foot[RIGHT], ONE / 2);
    let width = fx::mul(ONE * 110 / 100, b.height) / 170 * 100 * b.girth_pct / 10_000;
    out.shadow(
        v3(mid.x, ground + crate::player::SHADOW_LIFT, mid.z),
        width,
        rgb(look.palette.shadow),
        ONE,
    );
}

// ---- the rest pose ----

/// Where the elbow (or knee) goes for a limb of lengths `l1` and `l2` from
/// `a` to `c`, bent toward `pole`. A target out of reach is brought in.
fn bend(a: V3, c: V3, l1: Fx, l2: Fx, pole: V3) -> V3 {
    let d = c.sub(a);
    let len = d.len().clamp((l1 - l2).abs() + 1, l1 + l2 - 1);
    let u = d.norm();
    let x = fx::div(fx::mul(l1, l1) - fx::mul(l2, l2) + fx::mul(len, len), 2 * len);
    let h = fx::sqrt(fx::mul(l1, l1) - fx::mul(x, x));
    let side = pole.sub(u.scale(pole.dot(u))).norm();
    a.add(u.scale(x)).add(side.scale(h))
}

/// A kind standing at its guard: feet apart, knees soft, the kind's own
/// item (`kind_held`) at guard in both hands, or in the right hand for a
/// one-handed item; a kind holding nothing stands with its arms down.
pub fn rest_pose(kind: Kind, feet: V3, yaw: i32) -> Pose {
    let b = build(kind);
    let fw = facing(yaw, 0);
    let r = right_of(yaw);
    let up = V3::UP;
    let sc = |v: i32| (v as i64 * ONE as i64 * b.height as i64 / (1000 * 170 * ONE as i64 / 100)) as Fx;
    let at = |o: V3, x: Fx, y: Fx, z: Fx| o.add(r.scale(x)).add(up.scale(y)).add(fw.scale(z));
    let sink = sc(30);
    let pelvis = at(feet, 0, b.hip_y - sink, 0);
    let chest = at(feet, 0, b.shoulder_y - sink, 0);
    let head = at(feet, 0, b.height - sink - b.head / 2, 0);
    let mut hip = [V3::ZERO; 2];
    let mut knee = [V3::ZERO; 2];
    let mut foot = [V3::ZERO; 2];
    for (s, side) in [(LEFT, -1), (RIGHT, 1)] {
        hip[s] = at(pelvis, side * b.hip_x, 0, 0);
        // the left foot a little forward, the right a little back
        let ahead = if s == LEFT { sc(90) } else { -sc(70) };
        foot[s] = at(feet, side * (b.hip_x + sc(40)), 0, ahead);
        knee[s] = bend(hip[s], foot[s], b.thigh, b.shin, fw);
    }
    let mut shoulder = [V3::ZERO; 2];
    for (s, side) in [(LEFT, -1), (RIGHT, 1)] {
        shoulder[s] = at(chest, side * b.shoulder_x, 0, 0);
    }
    let item = held::kind_held(kind);
    let reach = held::grip_to_tip(item);
    let two_hands = matches!(item, Held::ShortSword | Held::Sword | Held::Cleaver);
    let (grip, dir, hand) = if two_hands {
        let grip = at(chest, sc(40), -sc(330), sc(300));
        let dir = fw
            .scale(ONE * 56 / 100)
            .add(up.scale(ONE * 80 / 100))
            .sub(r.scale(ONE / 10))
            .norm();
        // the right hand by the guard, the left below it
        (
            grip,
            dir,
            [grip.sub(dir.scale(sc(46))), grip.add(dir.scale(sc(46)))],
        )
    } else if item == Held::Nothing {
        let hands = [
            at(shoulder[LEFT], -sc(40), -sc(560), sc(40)),
            at(shoulder[RIGHT], sc(40), -sc(560), sc(40)),
        ];
        (hands[RIGHT], fw, hands)
    } else {
        let grip = at(chest, sc(230), -sc(300), sc(280));
        let dir = fw.scale(ONE * 70 / 100).add(up.scale(ONE * 70 / 100)).norm();
        let left = at(shoulder[LEFT], -sc(60), -sc(540), sc(80));
        (grip, dir, [left, grip])
    };
    let tip = grip.add(dir.scale(reach));
    let mut elbow = [V3::ZERO; 2];
    for (s, side) in [(LEFT, -1), (RIGHT, 1)] {
        let pole = up.neg().add(r.scale(side * ONE / 4)).add(fw.scale(-ONE * 3 / 10));
        elbow[s] = bend(shoulder[s], hand[s], b.upper_arm, b.forearm, pole);
    }
    Pose {
        pelvis,
        chest,
        head,
        facing: yaw,
        shoulder,
        elbow,
        hand,
        hip,
        knee,
        foot,
        grip,
        tip,
    }
}
