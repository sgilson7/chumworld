//! The three orb shapes the primitives lacked (M6): a tetrahedron, a puffy
//! five-pointed star and a torus, appended to `prim::Mesh` as `Tetra`,
//! `Star` and `Torus`. Like every primitive, each is centred on the origin,
//! inside the unit cube and white, so an instance's affine and colour say
//! the rest. Faces wind outward (counter-clockwise seen from outside).

use crate::fx::{self, ONE};
use crate::geom::{v3, V3};
use crate::prim::VERTEX_VALUES;

const H: i32 = ONE / 2;
const WHITE: [i32; 3] = [ONE; 3];

/// A flat triangle, its winding turned outward from the origin (each shape
/// here is star-shaped about its centre, so a face's centroid and its
/// outward normal point the same way).
fn face(out: &mut Vec<i32>, a: V3, b: V3, c: V3) {
    let n = b.sub(a).cross(c.sub(a));
    let centroid = v3(
        (a.x + b.x + c.x) / 3,
        (a.y + b.y + c.y) / 3,
        (a.z + b.z + c.z) / 3,
    );
    let (b, c) = if dot64(n, centroid) < 0 { (c, b) } else { (b, c) };
    crate::prim::push_coloured(out, a, b, c, WHITE);
}

fn dot64(a: V3, b: V3) -> i64 {
    a.x as i64 * b.x as i64 + a.y as i64 * b.y as i64 + a.z as i64 * b.z as i64
}

/// Four corners of the cube, every other one: a regular tetrahedron.
pub fn tetra() -> Vec<i32> {
    let p = [v3(H, H, H), v3(H, -H, -H), v3(-H, H, -H), v3(-H, -H, H)];
    let mut o = Vec::new();
    for [a, b, c] in [[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]] {
        face(&mut o, p[a], p[b], p[c]);
    }
    o
}

/// The inner notches of the star, as a share of its points' reach, and its
/// depth front to back, as a share of half a unit.
const STAR_INNER_PCT: i32 = 46;
const STAR_DEPTH_PCT: i32 = 42;

/// A five-pointed star in the xy plane, its middle raised front and back to
/// a point: ten faces a side, read as a star from the front and as a gem
/// edge-on. One point is up.
pub fn star() -> Vec<i32> {
    let rim: Vec<V3> = (0..10)
        .map(|k| {
            let r = if k % 2 == 0 { H } else { H * STAR_INNER_PCT / 100 };
            let a = fx::TURN * k / 10;
            v3(-fx::mul(fx::sin(a), r), fx::mul(fx::cos(a), r), 0)
        })
        .collect();
    let front = v3(0, 0, H * STAR_DEPTH_PCT / 100);
    let back = front.neg();
    let mut o = Vec::new();
    for k in 0..10 {
        let (a, b) = (rim[k], rim[(k + 1) % 10]);
        face(&mut o, front, a, b);
        face(&mut o, back, a, b);
    }
    o
}

/// The torus's ring and tube radii, as shares of half a unit: together under
/// one, so it fits in the cube.
const TORUS_RING_PCT: i32 = 68;
const TORUS_TUBE_PCT: i32 = 30;

/// A smooth ring in the xy plane (its hole along z): 28 steps round, 10
/// round the tube.
pub fn torus() -> Vec<i32> {
    let (round, tube) = (28, 10);
    let ring = H * TORUS_RING_PCT / 100;
    let thick = H * TORUS_TUBE_PCT / 100;
    // the point and its normal at step i round the ring, j round the tube
    let at = |i: i32, j: i32| -> (V3, V3) {
        let (a, b) = (fx::TURN * i / round, fx::TURN * j / tube);
        let out = v3(fx::cos(a), fx::sin(a), 0);
        let n = out.scale(fx::cos(b)).add(v3(0, 0, fx::sin(b)));
        (out.scale(ring).add(n.scale(thick)), n)
    };
    let mut o = Vec::new();
    let mut tri = |p: [(V3, V3); 3]| {
        // wound so its face normal agrees with the vertices' normals
        let n = p[1].0.sub(p[0].0).cross(p[2].0.sub(p[0].0));
        let p = if dot64(n, p[0].1) < 0 {
            [p[0], p[2], p[1]]
        } else {
            p
        };
        for (q, n) in p {
            o.extend_from_slice(&[q.x, q.y, q.z, n.x, n.y, n.z, WHITE[0], WHITE[1], WHITE[2]]);
        }
    };
    for i in 0..round {
        for j in 0..tube {
            let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
            tri([a, b, c]);
            tri([a, c, d]);
        }
    }
    debug_assert_eq!(o.len() % VERTEX_VALUES, 0);
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tris(vs: &[i32]) -> Vec<[V3; 3]> {
        vs.chunks(VERTEX_VALUES * 3)
            .map(|t| {
                let p = |k: usize| v3(t[k * 9], t[k * 9 + 1], t[k * 9 + 2]);
                [p(0), p(1), p(2)]
            })
            .collect()
    }

    /// Every face of the three shapes faces away from the middle, and each
    /// fits in the unit cube: a tetrahedron has 4 faces, the star 20, the
    /// torus 28 × 10 × 2 = 560.
    #[test]
    fn the_orb_shapes_wind_outward_and_fit_the_unit_cube() {
        for (name, vs, faces) in [
            ("tetra", tetra(), 4),
            ("star", star(), 20),
            ("torus", torus(), 560),
        ] {
            let t = tris(&vs);
            assert_eq!(t.len(), faces, "{name}");
            for [a, b, c] in t {
                for p in [a, b, c] {
                    assert!(
                        p.x.abs() <= H && p.y.abs() <= H && p.z.abs() <= H,
                        "{name}: {p:?}"
                    );
                }
                let n = b.sub(a).cross(c.sub(a));
                let mid = v3(
                    (a.x + b.x + c.x) / 3,
                    (a.y + b.y + c.y) / 3,
                    (a.z + b.z + c.z) / 3,
                );
                // the torus's inner faces face the axis, not away from the
                // centre: test those against the ring's middle instead
                let from = if name == "torus" {
                    let r = v3(mid.x, mid.y, 0).norm().scale(H * TORUS_RING_PCT / 100);
                    mid.sub(r)
                } else {
                    mid
                };
                assert!(dot64(n, from) > 0, "{name}: a face at {mid:?} winds inward");
            }
        }
    }
}
