//! The test field M0 stands the player on: flat faceted grass to the
//! horizon, a few trees, rocks, a fence and a lantern near the start so
//! depth and motion read, and low hills kilometres off so the haze shows.
//! Laid out from a fixed seed (`SEED`), so every visit sees the same field
//! (CW-4).
//!
//! The ground is triangles in square rings round the start, each ring's
//! squares four times as wide as the ring inside it (4 m, 16 m, 64 m,
//! 256 m), then four wide bands out to 16 km. The colour varies in soft
//! patches near the start and evens out with distance, because a colour that
//! changes from one far, tiny triangle to the next shimmers when the view
//! moves (CW-18). Under it all, 2 m down, a plain floor fills any pinhole
//! where a small square's corner meets a larger square's edge; 2 m is wider
//! than the depth buffer needs at the horizon (`render::depth_step`).
//!
//! M2 (`terrain.rs`, `lod.rs`, `scatter.rs`, `places.rs`) replaces this file:
//! when the world has terrain, `World::scene` stops calling `field::groups`
//! and this file is deleted.

use crate::fx::{self, Fx, ONE};
use crate::geom::{v3, Affine, V3};
use crate::look::{mix, rgb, Look, Rgb};
use crate::prim::Mesh;
use crate::render::Group;
use crate::rng::Rng;

/// The field's layout seed.
pub const SEED: u64 = 0x00c0_ffee;
/// The ground's half-width: 16 km, inside the 32 km a Q16.16 length can hold.
pub const HALF: Fx = fx::int(16_000);
/// The floor under the ground.
pub const UNDER: Fx = -fx::int(2);
/// The rings of squares: (square width, half-width of the ring's outer edge).
pub const RINGS: [(i32, i32); 4] = [(4, 96), (16, 384), (64, 1536), (256, 6144)];
/// How much of a ring's colour is its own patchwork, percent: full near the
/// start, none by the outer ring.
const PATCHWORK_PCT: [i32; 4] = [100, 30, 8, 0];
/// The width of the patches of lighter and darker grass.
const PATCH: i32 = 24;

/// A square's half-diagonal from its half-width (× 1.42, a little over √2).
fn diag(half: Fx) -> Fx {
    fx::mul(half, ONE * 142 / 100)
}

/// A whole-number hash of a lattice point and the seed, 0 to `ONE`.
fn lattice(x: i32, z: i32) -> Fx {
    let mut r = Rng::new(SEED ^ ((x as u32 as u64) << 32) ^ (z as u32 as u64));
    r.below(ONE as u32 + 1) as Fx
}

/// Smooth value noise at (x, z) metres: lattice values every `PATCH` metres,
/// blended with a smoothstep, 0 to `ONE`.
fn patches(x: i32, z: i32) -> Fx {
    let (cx, cz) = (x.div_euclid(PATCH), z.div_euclid(PATCH));
    let fxp = x.rem_euclid(PATCH) * ONE / PATCH;
    let fzp = z.rem_euclid(PATCH) * ONE / PATCH;
    let smooth = |t: Fx| fx::mul(fx::mul(t, t), 3 * ONE - 2 * t);
    let (sx, sz) = (smooth(fxp), smooth(fzp));
    let top = fx::lerp(lattice(cx, cz), lattice(cx + 1, cz), sx);
    let bottom = fx::lerp(lattice(cx, cz + 1), lattice(cx + 1, cz + 1), sx);
    fx::lerp(top, bottom, sz)
}

/// The grass colour of the triangle whose square has its south-west corner
/// at (x, z) metres from the start, in ring `ring`.
fn grass(look: &Look, x: i32, z: i32, ring: usize, half: bool, rng: &mut Rng) -> Rgb {
    let p = &look.palette;
    let far = rgb(p.grass_far);
    // the patch decides between the palette's tones; each facet then varies
    // a little on its own, as faceted ground catches the light
    let n = patches(x, z);
    let tones = &p.grass;
    let k = (fx::mul(n, (tones.len() as i32 - 1) * ONE) / ONE) as usize;
    let t = fx::mul(n, (tones.len() as i32 - 1) * ONE) % ONE;
    let base = mix(rgb(tones[k]), rgb(tones[(k + 1).min(tones.len() - 1)]), t);
    let facet = 97 + rng.between(0, 5) + if half { 1 } else { 0 };
    let own = crate::look::shade(base, facet);
    mix(far, own, PATCHWORK_PCT[ring] * ONE / 100)
}

/// The ground: the rings of triangles, the bands beyond, the floor beneath.
fn ground(look: &Look, centre: V3) -> Vec<Group> {
    let mut groups = Vec::new();
    let mut rng = Rng::new(SEED);
    let mut inner = 0;
    for (ring, &(w, outer)) in RINGS.iter().enumerate() {
        // the ring's corner is √2 × its half-width from the centre
        let mut g = Group::new("ground ring", centre, diag(fx::int(outer)));
        let n = outer / w;
        for i in -n..n {
            for j in -n..n {
                let (x, z) = (i * w, j * w);
                let inside = x >= -inner && x + w <= inner && z >= -inner && z + w <= inner;
                if inside {
                    continue;
                }
                let mid = centre.add(v3(fx::int(x) + fx::int(w) / 2, 0, fx::int(z) + fx::int(w) / 2));
                let size = Affine::scale(v3(fx::int(w), ONE, fx::int(w)));
                let a = grass(look, x, z, ring, false, &mut rng);
                let b = grass(look, x, z, ring, true, &mut rng);
                g.shapes
                    .solid(Mesh::Tri, Affine::translate(mid).then(&size), a, 0);
                let turned = Affine::rot_y(fx::HALF_TURN).then(&size);
                g.shapes
                    .solid(Mesh::Tri, Affine::translate(mid).then(&turned), b, 0);
            }
        }
        groups.push(g);
        inner = outer;
    }
    // four bands from the last ring to the edge: north, south, west, east
    let far = rgb(look.palette.grass_far);
    let edge = fx::int(RINGS[RINGS.len() - 1].1);
    let mut bands = Group::new("ground bands", centre, diag(HALF));
    let depth = HALF - edge;
    for (mx, mz, sx, sz) in [
        (0, -(edge + depth / 2), 2 * HALF, depth),
        (0, edge + depth / 2, 2 * HALF, depth),
        (-(edge + depth / 2), 0, depth, 2 * edge),
        (edge + depth / 2, 0, depth, 2 * edge),
    ] {
        let at = Affine::translate(centre.add(v3(mx, 0, mz))).then(&Affine::scale(v3(sx, ONE, sz)));
        bands.shapes.solid(Mesh::Square, at, far, 0);
    }
    groups.push(bands);
    let mut floor = Group::new("floor", centre, diag(HALF));
    let at = Affine::translate(centre.add(v3(0, UNDER, 0))).then(&Affine::scale(v3(2 * HALF, ONE, 2 * HALF)));
    floor.shapes.solid(Mesh::Square, at, rgb(look.palette.under), 0);
    groups.push(floor);
    groups
}

/// A tree: a trunk and two or three cones, inked when near.
fn tree(g: &mut Group, look: &Look, at: V3, size_pct: i32, inked: bool, rng: &mut Rng) {
    let p = &look.palette;
    let ink = look.ink();
    let s = |m: Fx| m * size_pct / 100;
    let leaf = rgb(p.leaf[rng.below(p.leaf.len() as u32) as usize]);
    let trunk_h = s(ONE * 17 / 10);
    // the trunk's foot is 10 cm under the ground, so no face lies in its plane
    let trunk = Affine::translate(at.add(v3(0, trunk_h / 2 - ONE / 10, 0))).then(&Affine::scale(v3(
        s(ONE * 34 / 100),
        trunk_h + ONE / 10,
        s(ONE * 34 / 100),
    )));
    let mut put = |m: Mesh, a: Affine, c: Rgb| {
        if inked {
            g.shapes.inked(m, a, c, ink);
        } else {
            g.shapes.solid(m, a, c, 0);
        }
    };
    put(Mesh::Cyl, trunk, rgb(p.trunk));
    let tiers = 2 + rng.below(2) as i32;
    for k in 0..tiers {
        let w = s(ONE * (34 - 7 * k) / 10);
        let h = s(ONE * (30 - 3 * k) / 10);
        let y = s(ONE * 12 / 10) + s(ONE * 17 / 10) * k + h / 2;
        let turn = Affine::rot_y(rng.below(fx::TURN as u32) as i32);
        let cone = Affine::translate(at.add(v3(0, y, 0)))
            .then(&turn)
            .then(&Affine::scale(v3(w, h, w)));
        put(Mesh::Cone, cone, crate::look::shade(leaf, 100 + 6 * k));
    }
}

/// A rock: a faceted blob, sunk a third into the ground.
fn rock(g: &mut Group, look: &Look, at: V3, size: Fx, rng: &mut Rng) {
    let p = &look.palette;
    let c = rgb(p.rock[rng.below(p.rock.len() as u32) as usize]);
    let h = size * (55 + rng.between(0, 20)) / 100;
    let turn = Affine::rot_y(rng.below(fx::TURN as u32) as i32);
    let a = Affine::translate(at.add(v3(0, h / 6, 0)))
        .then(&turn)
        .then(&Affine::scale(v3(size, h, size * 8 / 10)));
    g.shapes.inked(Mesh::Blob, a, c, look.ink());
}

/// The things near the start and the hills far off.
fn props(look: &Look, centre: V3) -> Vec<Group> {
    let p = &look.palette;
    let ink = look.ink();
    let mut rng = Rng::new(SEED ^ 0x7ee5);
    let at = |x: i32, z: i32| centre.add(v3(x * ONE / 10, 0, z * ONE / 10));

    let mut near = Group::new("near props", centre, fx::int(100));
    // trees framing the meadow north of the start (decimetres east, south)
    for &(x, z, size) in &[
        (-230, -260, 110),
        (-300, -90, 125),
        (-150, -420, 95),
        (260, -330, 115),
        (360, -150, 100),
        (180, -560, 130),
        (-380, -520, 115),
        (420, 120, 105),
        (-330, 200, 100),
        (60, -720, 120),
        (-210, -800, 100),
        (520, -640, 115),
        (-520, -300, 120),
    ] {
        tree(&mut near, look, at(x, z), size, true, &mut rng);
    }
    for &(x, z, s) in &[
        (-45, -70, 9),
        (60, -120, 14),
        (-120, -260, 22),
        (210, -340, 18),
        (30, -900, 30),
        (-380, -150, 12),
    ] {
        rock(&mut near, look, at(x, z), s * ONE / 10, &mut rng);
    }
    // a fence running north-east from beside the start: posts every 3 m, two
    // rails through them
    let post = rgb(p.post);
    let posts = 9;
    for k in 0..posts {
        let base = at(120 + 21 * k, -60 - 21 * k);
        let turn = Affine::rot_y(-fx::deg(45));
        let a = Affine::translate(base.add(v3(0, ONE / 2, 0)))
            .then(&turn)
            .then(&Affine::scale(v3(ONE * 16 / 100, ONE * 11 / 10, ONE * 16 / 100)));
        near.shapes.inked(Mesh::Cube, a, post, ink);
    }
    let run = fx::int(3) * 141 / 100 * (posts - 1);
    for y in [ONE * 45 / 100, ONE * 85 / 100] {
        let mid = at(120 + 21 * (posts - 1) / 2, -60 - 21 * (posts - 1) / 2).add(v3(0, y, 0));
        // the rail's length (its z) along the fence, which runs north-east
        let a = Affine::translate(mid)
            .then(&Affine::rot_y(-fx::deg(45)))
            .then(&Affine::scale(v3(ONE * 6 / 100, ONE * 9 / 100, run)));
        near.shapes
            .inked(Mesh::Cube, a, crate::look::shade(post, 90), ink);
    }
    // a lantern on a post, ahead and left of the start
    let lamp_at = at(-22, -45);
    let pole = Affine::translate(lamp_at.add(v3(0, ONE * 9 / 10, 0))).then(&Affine::scale(v3(
        ONE / 10,
        ONE * 19 / 10,
        ONE / 10,
    )));
    near.shapes
        .inked(Mesh::Cyl, pole, crate::look::shade(post, 70), ink);
    let lamp_c = lamp_at.add(v3(0, ONE * 19 / 10, 0));
    let lamp =
        Affine::translate(lamp_c).then(&Affine::scale(v3(ONE * 26 / 100, ONE * 30 / 100, ONE * 26 / 100)));
    // the lamp shows its own colour, unlit, inside its halo
    near.shapes.solid(Mesh::Octa, lamp, rgb(p.lamp), ONE);
    near.shapes.outline(Mesh::Octa, &lamp, ink, ONE);
    let halo = Affine::translate(lamp_c).then(&Affine::scale(v3(ONE * 9 / 10, ONE * 9 / 10, ONE * 9 / 10)));
    near.shapes.glow(Mesh::Sphere, halo, rgb(p.lamp), ONE * 22 / 100);

    // clumps of trees further out, not inked: their lines would be thinner
    // than a pixel
    let mut mid = Group::new("tree clumps", centre, fx::int(420));
    for _ in 0..14 {
        let angle = rng.below(fx::TURN as u32) as i32;
        let dist = rng.between(120, 380);
        let c = centre.add(v3(fx::cos(angle) * dist, 0, fx::sin(angle) * dist));
        for _ in 0..rng.between(3, 7) {
            let off = v3(rng.between(-9, 9) * ONE, 0, rng.between(-9, 9) * ONE);
            tree(&mut mid, look, c.add(off), rng.between(100, 160), false, &mut rng);
        }
    }

    // low hills round the horizon, sunk to two thirds of their height
    let mut hills = Group::new("hills", centre, fx::int(13_000));
    for k in 0..18 {
        let angle = fx::TURN * k / 18 + rng.between(-1500, 1500);
        let dist = rng.between(2_200, 9_000);
        let c = centre.add(v3(
            fx::mul(fx::cos(angle), fx::int(dist)),
            0,
            fx::mul(fx::sin(angle), fx::int(dist)),
        ));
        let w = fx::int(rng.between(1_000, 2_800));
        let h = fx::int(rng.between(260, 680));
        let colour = rgb(p.hill[rng.below(p.hill.len() as u32) as usize]);
        let turn = Affine::rot_y(rng.below(fx::TURN as u32) as i32);
        let a = Affine::translate(c.add(v3(0, -h / 6, 0)))
            .then(&turn)
            .then(&Affine::scale(v3(w, h, w * 7 / 10)));
        hills.shapes.solid(Mesh::Blob, a, colour, 0);
    }
    vec![near, mid, hills]
}

/// Every static group of the test field round `centre`.
pub fn groups(look: &Look, centre: V3) -> Vec<Group> {
    let mut g = ground(look, centre);
    g.extend(props(look, centre));
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::tests::look;

    fn centre() -> V3 {
        v3(fx::int(2000), 0, fx::int(2800))
    }

    #[test]
    fn the_ground_rings_tile_without_gaps_or_overlaps() {
        // the area of every ground triangle adds up to the field's square:
        // 32 km × 32 km, the floor aside
        let gs = ground(&look(), centre());
        let mut area: i128 = 0;
        for g in gs.iter().filter(|g| g.name != "floor") {
            for s in &g.shapes.list {
                let a = s.at.c0.len() as i128 * s.at.c2.len() as i128;
                area += if s.mesh == Mesh::Tri { a / 2 } else { a };
            }
        }
        let want = (2 * HALF as i128) * (2 * HALF as i128);
        // within a millionth: the rounding of the triangles' columns
        assert!((area - want).abs() < want / 1_000_000, "{area} vs {want}");
    }

    #[test]
    fn the_same_seed_lays_out_the_same_field() {
        assert_eq!(groups(&look(), centre()), groups(&look(), centre()));
    }

    #[test]
    fn the_far_ground_is_one_even_colour() {
        let gs = ground(&look(), centre());
        let outer = &gs[RINGS.len() - 1];
        let first = outer.shapes.list[0].rgb;
        assert!(
            outer.shapes.list.iter().all(|s| s.rgb == first),
            "the outer ring shimmers if it varies"
        );
        assert_eq!(first, rgb(look().palette.grass_far));
    }
}
