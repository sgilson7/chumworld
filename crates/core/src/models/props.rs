//! The props: every `Prop`, built from parts in millimetres with the base
//! at the origin and the front toward −z (heading 0, `geom` conventions),
//! then baked once into a mesh with a colour per vertex (`baked`), as the
//! extension contract asks (R2.16). Parts that touch overlap rather than
//! meet in a plane, and a detail laid on a surface stands `PROP_GAP` proud
//! of it, so no two faces of one prop share a plane (CW-18).
//!
//! Also the donkey, which moves, and `prop_shapes`, a prop as separate
//! shapes for one that moves (the merchant's cart).

use super::*;

/// How far ahead of the cart's base the donkey's middle stands, along the
/// cart's heading: M5 places the donkey `DONKEY_TO_CART` in front of a cart.
pub const DONKEY_TO_CART: Fx = mm(3200);

/// A prop's mesh, its outline as a second mesh, and the parts that glow (a
/// flame is drawn unlit, so it is a shape of its own).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baked {
    pub body: Vec<i32>,
    pub hull: Vec<i32>,
    pub loose: Vec<Part>,
}

static BAKED: OnceLock<Vec<Baked>> = OnceLock::new();

/// Every prop's bake, made on first use.
pub fn baked(kind: Prop) -> &'static Baked {
    let all = BAKED.get_or_init(|| Prop::ALL.iter().map(|&k| bake(&model(k))).collect());
    &all[Prop::ALL.iter().position(|&k| k == kind).unwrap_or(0)]
}

/// One mesh of the solid parts, coloured per vertex, and one of their
/// outlines (each part grown by its ink width, white, for the ink pass).
pub fn bake(m: &Model) -> Baked {
    let mut body = Vec::new();
    let mut hull = Vec::new();
    let mut loose = Vec::new();
    for p in &m.parts {
        if p.glow != 0 || p.halo != 0 {
            loose.push(*p);
            continue;
        }
        push_placed(&mut body, p.mesh, &p.at, p.rgb);
        if p.ink > 0 {
            push_placed(
                &mut hull,
                p.mesh,
                &render::inflate_by(&p.at, ink_width(&p.at, p.ink)),
                [ONE; 3],
            );
        }
    }
    Baked { body, hull, loose }
}

/// A primitive's triangles placed by `at` (its columns at right angles), in
/// `rgb`: positions placed, normals turned and scaled by the inverse of the
/// columns' lengths, then made unit.
fn push_placed(out: &mut Vec<i32>, mesh: Mesh, at: &Affine, rgb: Rgb) {
    let lens = [at.c0.len().max(1), at.c1.len().max(1), at.c2.len().max(1)];
    let thin = lens[0].min(lens[1]).min(lens[2]);
    let units = [at.c0.norm(), at.c1.norm(), at.c2.norm()];
    for v in crate::prim::vertices(mesh).chunks(VERTEX_VALUES) {
        let p = at.apply(v3(v[0], v[1], v[2]));
        let k = |n: Fx, l: Fx| fx::div(fx::mul(n, thin), l);
        let n = units[0]
            .scale(k(v[3], lens[0]))
            .add(units[1].scale(k(v[4], lens[1])))
            .add(units[2].scale(k(v[5], lens[2])))
            .norm();
        out.extend_from_slice(&[p.x, p.y, p.z, n.x, n.y, n.z, rgb[0], rgb[1], rgb[2]]);
    }
}

/// Every prop's radius round its base: what M2's group must hold.
pub fn prop_radius(kind: Prop) -> Fx {
    let m = model(kind);
    m.parts
        .iter()
        .map(|p| p.at.t.len() + p.at.reach())
        .max()
        .unwrap_or(0)
}

/// A prop drawn as separate shapes (each part with its own outline): for a
/// prop that moves, and for the lineup, whose shapes move with it.
pub fn prop_shapes(kind: Prop, at: V3, yaw: i32, look: &Look, out: &mut Shapes) {
    let place = Affine::translate(at).then(&Affine::rot_y(yaw));
    model(kind).placed(&place).emit(look.ink(), out);
}

/// Builds a prop from parts in millimetres.
struct Kit {
    m: Model,
    ink: Fx,
    t: &'static Table,
}

impl Kit {
    fn new() -> Kit {
        let t = table();
        Kit {
            m: Model::default(),
            ink: t.ink_prop,
            t,
        }
    }

    fn c(&self, name: &str) -> Rgb {
        self.t.c(name)
    }

    /// A shape `size` across centred at `at`, turned by `turn`.
    fn shape(
        &mut self,
        mesh: Mesh,
        at: (i32, i32, i32),
        size: (i32, i32, i32),
        turn: Affine,
        rgb: Rgb,
    ) -> usize {
        let a = put_turned(
            &Affine::IDENTITY,
            v3(mm(at.0), mm(at.1), mm(at.2)),
            &turn,
            v3(mm(size.0), mm(size.1), mm(size.2)),
        );
        self.m.add(mesh, a, rgb, self.ink)
    }

    fn bx(&mut self, at: (i32, i32, i32), size: (i32, i32, i32), rgb: Rgb) -> usize {
        self.shape(Mesh::Cube, at, size, Affine::IDENTITY, rgb)
    }

    fn up(&mut self, mesh: Mesh, at: (i32, i32, i32), size: (i32, i32, i32), rgb: Rgb) -> usize {
        self.shape(mesh, at, size, Affine::IDENTITY, rgb)
    }

    /// A bar of `mesh` from `a` to `b`, `thick` across.
    fn bar(&mut self, mesh: Mesh, a: (i32, i32, i32), b: (i32, i32, i32), thick: i32, rgb: Rgb) -> usize {
        let (a, b) = (v3(mm(a.0), mm(a.1), mm(a.2)), v3(mm(b.0), mm(b.1), mm(b.2)));
        let at = bar(a, b, (mm(thick), mm(thick)), v3(ONE, 0, 0));
        self.m.add(mesh, at, rgb, self.ink)
    }

    /// A glowing shape, unlit, with a halo.
    fn glow(&mut self, mesh: Mesh, at: (i32, i32, i32), size: (i32, i32, i32), rgb: Rgb, halo: Fx) -> usize {
        let a = put(
            &Affine::IDENTITY,
            v3(mm(at.0), mm(at.1), mm(at.2)),
            v3(mm(size.0), mm(size.1), mm(size.2)),
        );
        self.m.add_glow(mesh, a, rgb, halo)
    }
}

const fn deg(d: i32) -> i32 {
    fx::deg(d)
}

/// The model of a prop, base at the origin, front to −z.
pub fn model(kind: Prop) -> Model {
    let mut k = Kit::new();
    match kind {
        Prop::House => house(&mut k),
        Prop::Barn => barn(&mut k),
        Prop::Well => well(&mut k),
        Prop::Fence => fence(&mut k),
        Prop::Tent => tent(&mut k),
        Prop::Campfire => campfire(&mut k),
        Prop::Chest => chest(&mut k),
        Prop::Scarecrow => scarecrow(&mut k),
        Prop::Shrine => shrine(&mut k),
        Prop::Hut => hut(&mut k),
        Prop::Bridge => bridge(&mut k),
        Prop::Cart => cart(&mut k),
        Prop::Bed => bed(&mut k),
        Prop::Signpost => signpost(&mut k),
        Prop::HollowTree => hollow_tree(&mut k),
    }
    k.m
}

/// A gabled building: footing, walls `w` wide (x) and `d` deep (z), `h`
/// tall, a gable in the walls' colour, two roof slabs at 40° meeting at a
/// ridge along x, and a ridge cap.
#[allow(clippy::too_many_arguments)]
fn gabled(k: &mut Kit, w: i32, d: i32, h: i32, footing: i32, wall: Rgb, roof: Rgb, cap: Rgb) -> i32 {
    let stone = k.c("stone_dark");
    k.bx((0, footing / 2, 0), (w + 300, footing, d + 300), stone);
    let top = footing + h;
    k.bx((0, footing + h / 2 - 10, 0), (w, h + 20, d), wall);
    // tan 40° = 0.839: the gable rises 0.839 of the half depth
    let rise = d / 2 * 839 / 1000;
    let gable = Affine::rot_y(fx::QUARTER_TURN);
    k.shape(Mesh::Wedge, (0, top + rise / 2, 0), (d, rise, w), gable, wall);
    // the slabs: along the slope (half depth / cos 40° = 1.305), 220 thick,
    // their undersides 25 mm into the gable, past the eave by 450 mm and
    // the ridge by 120 mm
    let slope = d / 2 * 1305 / 1000;
    let (len, thick) = (slope + 450 + 120, 220);
    let shift = (450 - 120) / 2;
    for side in [-1, 1] {
        // the slope's middle, then out along its normal and down along it
        let (s40, c40) = (643, 766);
        let mid_y = top + rise / 2;
        let mid_z = side * d / 4;
        let out_n = thick / 2 - 25;
        let y = mid_y + out_n * c40 / 1000 - shift * s40 / 1000;
        let z = mid_z + side * (out_n * s40 / 1000 + shift * c40 / 1000);
        let turn = Affine::rot_x(side * deg(40));
        k.shape(Mesh::Cube, (0, y, z), (w + 700, thick, len), turn, roof);
    }
    let ridge = Affine::rot_z(fx::QUARTER_TURN);
    k.shape(
        Mesh::Cyl,
        (0, top + rise + 150, 0),
        (300, w + 760, 300),
        ridge,
        cap,
    );
    top
}

fn house(k: &mut Kit) {
    let (plaster, timber, roof, glass, shutter) = (
        k.c("plaster"),
        k.c("timber"),
        k.c("roof_tile"),
        k.c("glass"),
        k.c("shutter"),
    );
    let (w, d, h, f) = (6000, 5000, 2600, 400);
    let top = gabled(k, w, d, h, f, plaster, roof, timber);
    // the frame: corner posts, a beam round the top and one round the middle
    for x in [-1, 1] {
        for z in [-1, 1] {
            k.bx(
                (x * w / 2, f - 20 + (h - 20) / 2, z * d / 2),
                (260, h - 20, 260),
                timber,
            );
        }
    }
    for y in [top - 140, f + 300] {
        for x in [-1, 1] {
            k.bx((x * w / 2, y, 0), (200, 200, d - 200), timber);
        }
    }
    for z in [-1, 1] {
        k.bx((0, top - 140, z * d / 2), (w - 200, 200, 200), timber);
    }
    // the low front beam stops at the door's frame
    for x in [-1, 1] {
        let (a, b) = (710 - 20, w / 2 - 100);
        k.bx((x * (a + b) / 2, f + 300, -d / 2), (b - a, 200, 200), timber);
    }
    k.bx((0, f + 300, d / 2), (w - 200, 200, 200), timber);
    // braces on the side walls
    for x in [-1, 1] {
        for z in [-1, 1] {
            let turn = Affine::rot_x(z * deg(45));
            k.shape(
                Mesh::Cube,
                (x * (w / 2 + 30), f + 620, z * 1200),
                (140, 1300, 120),
                turn,
                timber,
            );
        }
    }
    // the door, its frame and its knob
    let door = k.c("door");
    let front = -d / 2;
    k.bx((0, f - 10 + 1000, front), (1100, 2000, 160), door);
    k.bx((0, f + 1300, front - 90), (1000, 110, 40), timber);
    for x in [-1, 1] {
        k.bx((x * 630, f - 10 + 1050, front), (160, 2100, 200), timber);
    }
    k.bx((0, f + 2080, front), (1380, 160, 200), timber);
    k.up(
        Mesh::Facet,
        (380, f + 1000, front - 110),
        (80, 80, 80),
        k.c("brass"),
    );
    // windows, with shutters, a sill and a box of flowers
    let flowers = [k.c("flower_red"), k.c("flower_yellow"), k.c("flower_red")];
    for x in [-1900, 1900] {
        let y = f + 1300;
        k.bx((x, y, front), (900, 800, 120), glass);
        k.bx((x, y, front - 70), (60, 800, 40), timber);
        k.bx((x, y, front - 70), (900, 60, 40), timber);
        for s in [-1, 1] {
            k.bx((x + s * 680, y, front - 50), (420, 860, 80), shutter);
        }
        k.bx((x, y - 460, front - 60), (1100, 100, 240), timber);
        k.bx((x, y - 640, front - 130), (1000, 220, 260), door);
        for (i, c) in flowers.iter().enumerate() {
            let fx_ = x - 300 + 300 * i as i32;
            k.up(Mesh::Facet, (fx_, y - 500, front - 130), (220, 160, 200), *c);
        }
    }
    // a window on each side wall
    for x in [-1, 1] {
        let turn = Affine::rot_y(fx::QUARTER_TURN);
        k.shape(Mesh::Cube, (x * w / 2, f + 1300, 0), (900, 800, 120), turn, glass);
        k.shape(
            Mesh::Cube,
            (x * (w / 2 + 70), f + 1300, 0),
            (60, 800, 40),
            turn,
            timber,
        );
    }
    // the chimney and its cap
    let stone = k.c("stone");
    k.bx((1800, top + 1200, 900), (600, 2800, 600), stone);
    k.bx((1800, top + 2680, 900), (760, 160, 760), k.c("stone_dark"));
}

fn barn(k: &mut Kit) {
    let (red, trim, roof, hay) = (k.c("barn_red"), k.c("barn_trim"), k.c("barn_roof"), k.c("hay"));
    let (w, d, h, f) = (8000, 6000, 3400, 300);
    let top = gabled(k, w, d, h, f, red, roof, k.c("timber"));
    let front = -d / 2;
    // the great doors, a white frame and the X on each leaf
    for s in [-1, 1] {
        k.bx((s * 760, f - 10 + 1400, front), (1500, 2800, 160), red);
        let x = s * 760;
        for turn in [deg(57), -deg(57)] {
            k.shape(
                Mesh::Cube,
                (x, f + 1390, front - 100),
                (120, 3200, 40),
                Affine::rot_z(turn),
                trim,
            );
        }
    }
    for x in [-1, 1] {
        k.bx((x * 1600, f - 10 + 1450, front), (180, 2900, 220), trim);
    }
    k.bx((0, f + 2900, front), (3380, 180, 220), trim);
    k.bx((0, f + 1390, front - 90), (3100, 140, 40), trim);
    // the hayloft door in the gable, open, hay showing
    k.bx((0, top + 700, front + 100), (1300, 1000, 120), k.c("barn_dark"));
    k.bx((0, top + 420, front + 40), (1000, 420, 120), hay);
    k.bx((0, top + 1260, front), (1500, 140, 220), trim);
    for x in [-1, 1] {
        k.bx((x * 690, top + 700, front), (140, 1260, 200), trim);
    }
    // corner trim
    for x in [-1, 1] {
        for z in [-1, 1] {
            k.bx(
                (x * w / 2, f - 20 + (h - 20) / 2, z * d / 2),
                (220, h - 20, 220),
                trim,
            );
        }
    }
    // bales stacked by the wall
    for (x, y, z) in [(4600, 400, -1600), (4600, 400, -500), (4600, 1190, -1050)] {
        k.bx((x, y, z), (900, 800, 1060), hay);
        k.bx((x, y, z), (960, 100, 1120), k.c("hay_band"));
    }
}

fn well(k: &mut Kit) {
    let (stone, light, wood, water, roof) = (
        k.c("stone"),
        k.c("stone_light"),
        k.c("wood"),
        k.c("water"),
        k.c("roof_tile"),
    );
    k.up(Mesh::Tube, (0, 350, 0), (1700, 700, 1700), stone);
    k.up(Mesh::Tube, (0, 740, 0), (1800, 80, 1800), light);
    k.up(Mesh::Cyl, (0, 500, 0), (1250, 100, 1250), water);
    for x in [-1, 1] {
        k.bx((x * 760, 700 + 950 - 40, 0), (130, 1980, 130), wood);
    }
    let axle = Affine::rot_z(fx::QUARTER_TURN);
    k.shape(Mesh::Cyl, (0, 2150, 0), (150, 1700, 150), axle, wood);
    k.bx((930, 2050, 0), (60, 260, 60), wood);
    // a little roof on the posts
    let along = Affine::rot_y(fx::QUARTER_TURN);
    k.shape(Mesh::Wedge, (0, 2850, 0), (1400, 720, 2100), along, roof);
    k.bx((0, 2420, 0), (2100, 60, 1500), wood);
    // the rope and the bucket
    k.bar(Mesh::Cyl, (0, 2100, 0), (0, 1650, 0), 24, k.c("rope"));
    k.up(Mesh::Taper, (0, 1500, 0), (300, 300, 300), wood);
    k.up(Mesh::Tube, (0, 1500, 0), (300, 40, 300), k.c("iron"));
}

fn fence(k: &mut Kit) {
    let wood = k.c("fence");
    for x in [-2000, 0, 2000] {
        k.bx((x, 550, 0), (110, 1100, 110), wood);
        k.up(Mesh::Pyramid, (x, 1160, 0), (110, 120, 110), wood);
    }
    let rail = k.c("fence_rail");
    for y in [450, 850] {
        k.bx((0, y, -60), (4000, 90, 50), rail);
    }
}

fn tent(k: &mut Kit) {
    let (canvas, dark, pole, rope) = (k.c("canvas"), k.c("tent_dark"), k.c("wood_dark"), k.c("rope"));
    k.up(Mesh::Wedge, (0, 1050, 0), (2800, 2100, 3400), canvas);
    // the doorway, a dark triangle standing off the front
    k.up(Mesh::Wedge, (0, 630, -1700), (1500, 1300, 60), dark);
    // the flaps tied back either side of it
    for s in [-1, 1] {
        let turn = Affine::rot_z(-s * deg(34));
        k.shape(
            Mesh::Cube,
            (s * 760, 700, -1760),
            (200, 1300, 50),
            turn,
            k.c("canvas_patch"),
        );
    }
    for z in [-1760, 1760] {
        k.bx((0, 1250, z), (80, 2500, 80), pole);
    }
    // guy ropes to pegs
    for z in [-1, 1] {
        k.bar(Mesh::Cyl, (0, 2400, z * 1760), (0, 0, z * 2700), 22, rope);
        k.bx((0, 60, z * 2700), (60, 160, 60), pole);
    }
    // a pennant on the front pole
    let flag = Affine::rot_z(-fx::QUARTER_TURN);
    k.shape(
        Mesh::Wedge,
        (230, 2380, -1760),
        (220, 380, 20),
        flag,
        k.c("tent_flag"),
    );
    // a bedroll by the door
    let roll = Affine::rot_z(fx::QUARTER_TURN);
    k.shape(
        Mesh::Cyl,
        (-1000, 130, -2300),
        (260, 900, 260),
        roll,
        k.c("canvas_patch"),
    );
}

fn campfire(k: &mut Kit) {
    let (stone, wood, ember) = (k.c("stone"), k.c("wood_dark"), k.c("ember"));
    for i in 0..8 {
        let a = fx::TURN * i / 8;
        let (x, z) = (fx::mul(fx::cos(a), 430), fx::mul(fx::sin(a), 430));
        let turn = Affine::rot_y(a);
        k.shape(Mesh::Facet, (x, 70, z), (280, 170, 230), turn, stone);
    }
    k.up(Mesh::Facet, (0, 40, 0), (520, 120, 520), ember);
    for i in 0..3 {
        let a = fx::TURN * i / 3;
        let (x, z) = (fx::mul(fx::cos(a), 300), fx::mul(fx::sin(a), 300));
        k.bar(Mesh::Cyl, (x, 40, z), (-x / 6, 480, -z / 6), 110, wood);
    }
    k.glow(Mesh::Cone, (0, 380, 0), (320, 620, 320), k.c("flame"), ONE / 2);
    k.glow(Mesh::Cone, (0, 300, 0), (180, 420, 180), k.c("flame_core"), 0);
}

fn chest(k: &mut Kit) {
    let (wood, iron, gold) = (k.c("chest_wood"), k.c("iron"), k.c("brass"));
    k.bx((0, 250, 0), (900, 500, 560), wood);
    let along = Affine::rot_y(fx::QUARTER_TURN);
    k.shape(Mesh::HalfCyl, (0, 620, 0), (560, 240, 900), along, wood);
    for x in [-300, 300] {
        k.bx((x, 255, 0), (80, 530, 620), iron);
        k.shape(Mesh::HalfCyl, (x, 630, 0), (620, 300, 80), along, iron);
    }
    k.bx((0, 500, -300), (140, 160, 40), gold);
    k.up(Mesh::Dot, (0, 490, -325), (30, 50, 30), k.c("tent_dark"));
}

fn scarecrow(k: &mut Kit) {
    let (wood, coat, coat_dark, straw, sack, hat, dark) = (
        k.c("wood"),
        k.c("scarecrow_coat"),
        k.c("scarecrow_coat_dark"),
        k.c("straw"),
        k.c("sack"),
        k.c("scarecrow_hat"),
        k.c("tent_dark"),
    );
    k.bx((0, 1150, 0), (110, 2300, 110), wood);
    let across = Affine::rot_z(fx::QUARTER_TURN);
    k.shape(Mesh::Cyl, (0, 1520, 0), (90, 1700, 90), across, wood);
    k.up(Mesh::Facet, (0, 1300, 0), (560, 760, 380), coat);
    k.up(Mesh::Taper, (0, 820, 0), (580, 500, 420), coat_dark);
    // sleeves out along the arm, straw at the cuffs and the hem
    for s in [-1, 1] {
        let out = Affine::rot_z(s * fx::QUARTER_TURN);
        k.shape(Mesh::Taper, (s * 460, 1520, 0), (230, 560, 230), out, coat);
        k.shape(Mesh::Cone, (s * 800, 1520, 0), (130, 200, 130), out, straw);
    }
    for i in 0..5 {
        let x = -200 + 100 * i;
        k.shape(
            Mesh::Cone,
            (x, 520, 0),
            (90, 200, 90),
            Affine::rot_x(fx::HALF_TURN),
            straw,
        );
    }
    // a patch on the coat
    k.bx((130, 1420, -190), (150, 150, 40), k.c("scarecrow_patch"));
    // the sack head with stitched eyes and mouth, and a hat
    k.up(Mesh::Facet, (0, 2150, 0), (430, 450, 400), sack);
    for s in [-1, 1] {
        for t in [deg(45), -deg(45)] {
            k.shape(
                Mesh::Cube,
                (s * 90, 2210, -190),
                (90, 20, 30),
                Affine::rot_z(t),
                dark,
            );
        }
    }
    k.bx((0, 2080, -186), (150, 18, 30), dark);
    k.up(Mesh::Cyl, (0, 2370, 0), (640, 40, 640), hat);
    k.up(Mesh::Cone, (0, 2600, 0), (380, 440, 380), hat);
    // a crow on the right arm
    let crow = k.c("crow");
    k.up(Mesh::Facet, (720, 1660, 0), (160, 150, 250), crow);
    k.up(Mesh::Facet, (720, 1760, -100), (110, 110, 110), crow);
    let beak = Affine::rot_x(-fx::QUARTER_TURN);
    k.shape(
        Mesh::Cone,
        (720, 1760, -190),
        (40, 90, 40),
        beak,
        k.c("flower_yellow"),
    );
}

fn shrine(k: &mut Kit) {
    let (stone, light, moss, roof) = (
        k.c("shrine_stone"),
        k.c("stone_light"),
        k.c("moss"),
        k.c("roof_tile"),
    );
    k.bx((0, 150, 0), (2400, 300, 2400), stone);
    k.bx((0, 430, 0), (1900, 300, 1900), light);
    for x in [-700, 700] {
        for z in [-700, 700] {
            k.up(Mesh::Cyl, (x, 560 + 950 - 20, z), (260, 1940, 260), stone);
        }
    }
    k.bx((0, 2420, 0), (2500, 120, 2500), stone);
    k.up(Mesh::Pyramid, (0, 2900, 0), (2300, 900, 2300), roof);
    k.up(Mesh::Facet, (0, 3420, 0), (260, 260, 260), light);
    // the pedestal and its glowing sphere
    k.up(Mesh::Cyl, (0, 900, 0), (500, 700, 500), light);
    k.up(Mesh::Cyl, (0, 1280, 0), (620, 80, 620), stone);
    k.glow(
        Mesh::Dot,
        (0, 1520, 0),
        (380, 380, 380),
        k.c("shrine_orb"),
        ONE / 2,
    );
    // candles at the corners of the step
    for x in [-1, 1] {
        for z in [-1, 1] {
            k.up(Mesh::Cyl, (x * 520, 640, z * 520), (80, 200, 80), k.c("candle"));
            k.glow(
                Mesh::Cone,
                (x * 520, 790, z * 520),
                (50, 100, 50),
                k.c("flame"),
                ONE / 3,
            );
        }
    }
    // moss on the lower step's corners
    for (x, z) in [(-1150, -1150), (1150, 1150), (1150, -1150)] {
        k.up(Mesh::Facet, (x, 300, z), (420, 200, 360), moss);
    }
}

fn hut(k: &mut Kit) {
    let (daub, thatch, stone, wood, dark) = (
        k.c("daub"),
        k.c("thatch"),
        k.c("stone"),
        k.c("wood"),
        k.c("tent_dark"),
    );
    k.up(Mesh::Cyl, (0, 150, 0), (5300, 300, 5300), stone);
    k.up(Mesh::Cyl, (0, 1400, 0), (5000, 2400, 5000), daub);
    k.up(Mesh::Cone, (0, 4000, 0), (6600, 2800, 6600), thatch);
    k.up(Mesh::Cyl, (0, 5600, 0), (120, 600, 120), wood);
    k.up(Mesh::Facet, (0, 5950, 0), (220, 220, 220), thatch);
    // the door, on the front face of the ten-sided wall (its face is at
    // cos 18° = 0.951 of the 2.5 m radius, 2.378 m out)
    let face = -2378;
    k.bx((0, 1240, face), (1000, 1900, 160), k.c("door"));
    for x in [-1, 1] {
        k.bx((x * 580, 1290, face), (160, 2000, 200), wood);
    }
    k.bx((0, 2250, face), (1300, 160, 200), wood);
    // a window on the faces either side
    for a in [18, 162] {
        let yaw = deg(a - 90);
        let f = Affine::rot_y(yaw);
        let at = f.apply(v3(0, mm(1500), mm(face)));
        let (x, y, z) = (at.x * 1000 / ONE, at.y * 1000 / ONE, at.z * 1000 / ONE);
        k.shape(Mesh::Cube, (x, y, z), (700, 600, 140), f, dark);
        k.shape(Mesh::Cube, (x, y - 360, z), (900, 100, 260), f, wood);
    }
    // the trainer's signs: a weapon rack and a straw dummy by the door
    let rack_x = -1700;
    for x in [rack_x - 500, rack_x + 500] {
        k.bx((x, 700, -2900), (100, 1400, 100), wood);
    }
    k.bx((rack_x, 1250, -2900), (1200, 90, 90), wood);
    let practice = k.c("practice_sword");
    for (i, x) in [rack_x - 260, rack_x, rack_x + 260].iter().enumerate() {
        let tilt = Affine::rot_x(deg(12 + 4 * i as i32));
        k.shape(Mesh::Cube, (*x, 700, -2990), (60, 1300, 24), tilt, practice);
    }
    let (sack, straw) = (k.c("sack"), k.c("straw"));
    k.bx((2300, 900, -3000), (110, 1800, 110), wood);
    k.up(Mesh::Facet, (2300, 1400, -3000), (520, 700, 400), sack);
    k.up(Mesh::Facet, (2300, 1940, -3000), (330, 330, 330), sack);
    let across = Affine::rot_z(fx::QUARTER_TURN);
    k.shape(Mesh::Cyl, (2300, 1550, -3000), (80, 1100, 80), across, wood);
    for s in [-1, 1] {
        k.shape(
            Mesh::Cone,
            (2300 + s * 600, 1550, -3000),
            (120, 180, 120),
            Affine::rot_z(s * fx::QUARTER_TURN),
            straw,
        );
    }
    k.up(Mesh::Dot, (2300, 1430, -3210), (140, 140, 60), k.c("barn_red"));
    // firewood against the wall
    let log = Affine::rot_x(fx::QUARTER_TURN);
    for (x, y) in [(-1200, 150), (-900, 150), (-1050, 400)] {
        k.shape(
            Mesh::Cyl,
            (x + 2800, y, 1400),
            (260, 900, 260),
            log,
            k.c("wood_dark"),
        );
    }
}

fn bridge(k: &mut Kit) {
    let (wood, dark) = (k.c("wood"), k.c("wood_dark"));
    let planks = [k.c("plank_0"), k.c("plank_1"), k.c("plank_2")];
    for i in 0..24 {
        let z = -5750 + 500 * i;
        k.bx((0, 560, z), (3000, 80, 460), planks[(i % 3) as usize]);
    }
    for x in [-1200, 1200] {
        k.bx((x, 400, 0), (250, 320, 12000), dark);
    }
    for z in [-6000, -3000, 0, 3000, 6000] {
        for x in [-1420, 1420] {
            k.bx((x, 1050, z), (160, 1100, 160), wood);
            k.up(Mesh::Pyramid, (x, 1660, z), (160, 120, 160), wood);
        }
    }
    for x in [-1420, 1420] {
        k.bx((x, 1500, 0), (100, 120, 12200), wood);
        k.bx((x, 1050, 0), (80, 90, 12100), wood);
    }
    for z in [-3000, 0, 3000] {
        for x in [-1200, 1200] {
            k.up(Mesh::Cyl, (x, -700, z), (300, 2400, 300), dark);
        }
    }
}

fn cart(k: &mut Kit) {
    let (wood, dark, iron, cloth, stripe) = (
        k.c("cart_wood"),
        k.c("wood_dark"),
        k.c("iron"),
        k.c("cart_cloth"),
        k.c("cart_stripe"),
    );
    k.bx((0, 700, 0), (1500, 100, 2400), wood);
    for x in [-1, 1] {
        k.bx((x * 710, 925, 0), (80, 450, 2400), wood);
    }
    for z in [-1, 1] {
        k.bx((0, 925, z * 1180), (1420, 450, 80), wood);
    }
    // the wheels: rim, spokes and hub, on an axle
    let axle = Affine::rot_z(fx::QUARTER_TURN);
    k.shape(Mesh::Cyl, (0, 575, 300), (100, 1860, 100), axle, iron);
    for x in [-830, 830] {
        k.shape(Mesh::Tube, (x, 575, 300), (1150, 100, 1150), axle, dark);
        k.shape(Mesh::Cyl, (x, 575, 300), (240, 170, 240), axle, iron);
        for s in 0..6 {
            let turn = Affine::rot_x(fx::TURN * s / 12);
            k.shape(Mesh::Cube, (x, 575, 300), (60, 1000, 50), turn, wood);
        }
    }
    // the shafts forward to the donkey
    for x in [-1, 1] {
        k.bar(Mesh::Cyl, (x * 600, 760, -1150), (x * 470, 820, -3700), 80, dark);
    }
    // the canopy over the back, striped
    k.up(Mesh::HalfCyl, (0, 1700, 300), (1560, 1100, 1800), cloth);
    for z in [-300, 300, 900] {
        k.up(Mesh::HalfCyl, (0, 1660, z), (1640, 1240, 220), stripe);
    }
    // the merchant's seat, crates and a barrel in front of the canopy
    k.bx((0, 1150, -900), (1300, 100, 400), wood);
    k.bx((-420, 950, -760), (440, 420, 380), k.c("crate"));
    k.up(Mesh::Cyl, (430, 1000, -720), (380, 520, 380), k.c("barrel"));
    k.up(Mesh::Tube, (430, 1000, -720), (400, 60, 400), iron);
    // a lantern on a hook at the canopy's front
    k.bar(Mesh::Cyl, (700, 2150, -620), (700, 1960, -620), 20, iron);
    k.up(Mesh::Cyl, (700, 1880, -620), (130, 160, 130), k.c("brass"));
    k.glow(
        Mesh::Dot,
        (700, 1880, -620),
        (90, 120, 90),
        k.c("lamp_glow"),
        ONE / 2,
    );
}

fn bed(k: &mut Kit) {
    let (wood, sheet, pillow, quilt, quilt2) = (
        k.c("bed_wood"),
        k.c("mattress"),
        k.c("pillow"),
        k.c("quilt"),
        k.c("quilt_patch"),
    );
    k.bx((0, 245, 0), (1100, 250, 2100), wood);
    for x in [-495, 495] {
        for z in [-995, 995] {
            k.bx((x, 70, z), (110, 160, 110), wood);
        }
    }
    k.bx((0, 450, 1050), (1180, 900, 100), wood);
    k.bx((0, 300, -1050), (1180, 600, 100), wood);
    k.up(Mesh::Facet, (0, 960, 1050), (300, 180, 160), wood);
    k.bx((0, 460, 0), (1000, 220, 1960), sheet);
    k.up(Mesh::Facet, (0, 610, 740), (700, 170, 380), pillow);
    k.bx((0, 580, -300), (1060, 120, 1380), quilt);
    for (x, z) in [(-250, -650), (250, -50)] {
        k.bx((x, 640, z), (360, 60, 360), quilt2);
    }
}

fn signpost(k: &mut Kit) {
    let (wood, board, paint) = (k.c("wood"), k.c("sign_board"), k.c("sign_paint"));
    k.bx((0, 1200, 0), (140, 2400, 140), wood);
    k.up(Mesh::Pyramid, (0, 2480, 0), (200, 160, 200), wood);
    for (y, yaw) in [(2050, 0), (1680, 160), (1320, 70)] {
        let f = Affine::rot_y(deg(yaw));
        let p = |x: i32| {
            let a = f.apply(v3(mm(x), mm(y), -mm(80)));
            (a.x * 1000 / ONE, a.y * 1000 / ONE, a.z * 1000 / ONE)
        };
        k.shape(Mesh::Cube, p(450), (900, 220, 50), f, board);
        let tip = f.then(&Affine::rot_z(-fx::QUARTER_TURN));
        k.shape(Mesh::Wedge, p(1000), (220, 200, 50), tip, board);
        // a painted band near the point, standing off the board
        k.shape(Mesh::Cube, p(780), (120, 220 + 2 * 30, 50 + 2 * 30), f, paint);
    }
    k.up(Mesh::Facet, (0, 60, 0), (360, 140, 360), k.c("stone"));
}

fn hollow_tree(k: &mut Kit) {
    let (bark, dark, leaf, leaf2, moss) = (
        k.c("bark"),
        k.c("hollow"),
        k.c("leaf"),
        k.c("leaf_dark"),
        k.c("moss"),
    );
    k.up(Mesh::Taper, (0, 1600, 0), (2200, 3200, 2200), bark);
    let lean = Affine::rot_z(deg(8));
    k.shape(Mesh::Taper, (-120, 3900, 0), (1400, 1800, 1400), lean, bark);
    k.bar(Mesh::Cyl, (300, 3800, 0), (1700, 5000, 300), 420, bark);
    k.bar(Mesh::Cyl, (-300, 4200, 100), (-1500, 5300, -400), 380, bark);
    // the crown
    k.up(Mesh::Facet, (0, 5900, 0), (3600, 2500, 3400), leaf);
    k.up(Mesh::Facet, (1700, 5400, 300), (2200, 1800, 2200), leaf2);
    k.up(Mesh::Facet, (-1600, 5600, -300), (2200, 1900, 2100), leaf2);
    k.up(Mesh::Facet, (300, 6900, 400), (2000, 1500, 1900), leaf);
    // the hollow: a dark oval just through the bark, at the front (the
    // eight-sided trunk's face there is 0.902 m out)
    k.up(Mesh::Facet, (0, 950, -760), (820, 1250, 500), dark);
    k.up(Mesh::Facet, (0, 1640, -820), (700, 200, 260), bark);
    // roots spreading at the foot
    for i in 0..5 {
        let a = fx::TURN * i / 5 + deg(20);
        let (x, z) = (fx::mul(fx::cos(a), 1050), fx::mul(-fx::sin(a), 1050));
        let out = Affine::rot_y(a - fx::QUARTER_TURN).then(&Affine::rot_x(-deg(70)));
        k.shape(Mesh::Cone, (x, 220, z), (420, 1000, 300), out, bark);
    }
    // moss and a cluster of red mushrooms
    k.up(Mesh::Facet, (900, 400, -500), (600, 300, 500), moss);
    for (x, z, s) in [(-900, -700, 220), (-700, -900, 160), (-1050, -500, 130)] {
        k.up(Mesh::Cyl, (x, s / 2, z), (s / 3, s, s / 3), k.c("mushroom_stem"));
        k.up(Mesh::Facet, (x, s, z), (s, s * 6 / 10, s), k.c("mushroom"));
    }
}

/// The donkey, mid-stride at `step_phase` (an angle: a whole turn is one
/// stride), standing at `at` facing `yaw`. Its legs swing; the rest holds
/// still. M5 draws it in front of the merchant's cart (`DONKEY_TO_CART`).
pub fn donkey(at: V3, yaw: i32, step_phase: i32, look: &Look, out: &mut Shapes) {
    let place = Affine::translate(at).then(&Affine::rot_y(yaw));
    donkey_model(step_phase).placed(&place).emit(look.ink(), out);
    out.shadow(
        at.add(v3(0, crate::player::SHADOW_LIFT, 0)),
        mm(1500),
        rgb(look.palette.shadow),
        ONE,
    );
}

pub fn donkey_model(step_phase: i32) -> Model {
    let mut k = Kit::new();
    k.ink = table().ink_figure;
    let (coat, pale, mane, hoof, red, bag) = (
        k.c("donkey_coat"),
        k.c("donkey_pale"),
        k.c("donkey_mane"),
        k.c("donkey_hoof"),
        k.c("harness"),
        k.c("saddle_bag"),
    );
    k.up(Mesh::Facet, (0, 960, 0), (700, 560, 1300), coat);
    k.up(Mesh::Facet, (0, 820, 0), (560, 300, 1000), pale);
    // the neck up and forward, the head, a pale muzzle
    k.bar(Mesh::Taper, (0, 1000, -450), (0, 1380, -780), 380, coat);
    k.up(Mesh::Facet, (0, 1420, -880), (320, 330, 520), coat);
    k.up(Mesh::Facet, (0, 1340, -1120), (260, 230, 260), pale);
    for s in [-1, 1] {
        let ear = Affine::rot_z(-s * deg(14)).then(&Affine::rot_x(deg(14)));
        k.shape(Mesh::Facet, (s * 90, 1720, -790), (100, 400, 70), ear, coat);
        k.shape(Mesh::Facet, (s * 92, 1720, -805), (50, 300, 30), ear, pale);
    }
    // eyes, a catch-light each
    let head = Affine::translate(v3(0, mm(1420), -mm(880)));
    let hr = v3(mm(160), mm(165), mm(260));
    let hk = k.m.parts.len() - 6;
    let (eye, white) = (table().c("eye"), table().c("catch_light"));
    for s in [-1, 1] {
        let d = v3(s * ONE * 80 / 100, ONE * 20 / 100, -ONE * 55 / 100).norm();
        let e = decal(
            &mut k.m,
            hk,
            &head,
            hr,
            d,
            v3(mm(40), mm(46), mm(26)),
            DECAL_LIFT,
            eye,
        );
        catch_light(&mut k.m, e, v3(mm(15), mm(15), mm(22)), white);
    }
    // the mane down the neck, the tail
    k.bar(Mesh::Facet, (0, 1180, -440), (0, 1600, -780), 120, mane);
    k.bar(Mesh::Cyl, (0, 1080, 640), (0, 700, 820), 60, coat);
    k.up(Mesh::Facet, (0, 640, 840), (120, 200, 120), mane);
    // the harness and saddle bags
    k.bar(Mesh::Tube, (0, 1080, -520), (0, 1180, -610), 520, red);
    k.bx((0, 1240, 0), (760, 60, 700), red);
    for s in [-1, 1] {
        k.bx((s * 400, 1000, 0), (140, 380, 460), bag);
    }
    // the legs, swinging about the hip and the shoulder
    for (x, z, offset) in [
        (-200, -420, 0),
        (200, -420, fx::HALF_TURN),
        (-200, 420, fx::HALF_TURN),
        (200, 420, 0),
    ] {
        let swing = fx::mul(fx::sin(step_phase + offset), deg(22));
        let top = v3(mm(x), mm(860), mm(z));
        let len = mm(820);
        let foot = top.add(v3(0, -fx::mul(fx::cos(swing), len), fx::mul(fx::sin(swing), len)));
        let a = bar(top, foot, (mm(110), mm(110)), v3(ONE, 0, 0));
        k.m.add(Mesh::Cyl, a, coat, k.ink);
        let h = put(
            &Affine::IDENTITY,
            foot.add(v3(0, mm(40), 0)),
            v3(mm(140), mm(100), mm(160)),
        );
        k.m.add(Mesh::Facet, h, hoof, k.ink);
    }
    k.m
}
