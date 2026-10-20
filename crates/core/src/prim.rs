//! The mesh table: the primitive shapes everything is drawn from. Each is a
//! triangle list of (position, normal) pairs in Q16, centred on the origin
//! and inside the unit cube, so an instance's affine carries its size.
//! Ported from Long Time Comin's `mesh.rs`, with a capsule, a ground
//! triangle and a flat square added, and the round shapes split into smooth
//! (`Sphere`, `Capsule`) and faceted (`Blob`, `Cone`, `Cyl`), the faceted
//! look of Protection of the Spheres' rocks and trees.
//!
//! The page uploads every mesh once (`numbers().meshes`, the shim's `mesh`),
//! in the order of `Mesh::ALL`, and an instance names its mesh by number.
//!
//! Extension (M3, `mesh.rs` and `models.rs`): a new shape is a new variant
//! here, appended to `Mesh::ALL` (ids are positions, so append, never
//! insert), with an arm in `vertices` that builds it, usually by calling a
//! builder in `crate::mesh`. Models are arrangements of these shapes, pushed
//! as instances; they are not meshes.

use crate::fx::{self, ONE};
use crate::geom::{v3, V3};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mesh {
    Cube = 0,
    /// half of a flat square, facing up: the ground's facets
    Tri = 1,
    /// a flat square facing up: shadows, decals
    Square = 2,
    /// a smooth globe: heads, the sky dome
    Sphere = 3,
    /// a faceted low globe: rocks, hills, crowns
    Blob = 4,
    /// a faceted upright cylinder: posts, trunks
    Cyl = 5,
    /// a faceted upright cone: pines, hats
    Cone = 6,
    /// a smooth capsule standing on end: bodies, limbs
    Capsule = 7,
    Octa = 8,
}

impl Mesh {
    pub const ALL: [Mesh; 9] = [
        Mesh::Cube,
        Mesh::Tri,
        Mesh::Square,
        Mesh::Sphere,
        Mesh::Blob,
        Mesh::Cyl,
        Mesh::Cone,
        Mesh::Capsule,
        Mesh::Octa,
    ];

    /// Shapes whose faces are flat: one of them can lie in another's plane,
    /// so the flicker scan (`flicker.rs`) reads their faces.
    pub fn flat_faced(self) -> bool {
        matches!(
            self,
            Mesh::Cube | Mesh::Tri | Mesh::Square | Mesh::Cyl | Mesh::Octa
        )
    }
}

/// Values per vertex: position xyz, normal xyz.
pub const VERTEX_VALUES: usize = 6;

/// The capsule's radius as a share of its height.
pub const CAPSULE_RADIUS: i32 = ONE * 18 / 100;

const H: i32 = ONE / 2;

fn push_flat(out: &mut Vec<i32>, a: V3, b: V3, c: V3) {
    let n = b.sub(a).cross(c.sub(a)).norm();
    for p in [a, b, c] {
        out.extend_from_slice(&[p.x, p.y, p.z, n.x, n.y, n.z]);
    }
}

fn push_quad(out: &mut Vec<i32>, a: V3, b: V3, c: V3, d: V3) {
    push_flat(out, a, b, c);
    push_flat(out, a, c, d);
}

/// A triangle whose normals are given per vertex (smooth shading).
fn push_smooth(out: &mut Vec<i32>, p: [(V3, V3); 3]) {
    for (q, n) in p {
        out.extend_from_slice(&[q.x, q.y, q.z, n.x, n.y, n.z]);
    }
}

/// A point on a globe of radius `H`: `i` of `lat` bands from the south pole,
/// `j` of `lon` round.
fn globe(i: i32, j: i32, lat: i32, lon: i32) -> V3 {
    let a = fx::HALF_TURN * i / lat - fx::QUARTER_TURN;
    let b = fx::TURN * j / lon;
    let r = fx::cos(a);
    v3(
        fx::mul(fx::mul(r, fx::cos(b)), H),
        fx::mul(fx::sin(a), H),
        fx::mul(fx::mul(r, -fx::sin(b)), H),
    )
}

/// Vertex data of a mesh, counter-clockwise from outside.
pub fn vertices(mesh: Mesh) -> Vec<i32> {
    let mut o = Vec::new();
    match mesh {
        Mesh::Cube => {
            let p = |x: i32, y: i32, z: i32| v3(x * H, y * H, z * H);
            push_quad(&mut o, p(1, -1, 1), p(1, -1, -1), p(1, 1, -1), p(1, 1, 1));
            push_quad(&mut o, p(-1, -1, -1), p(-1, -1, 1), p(-1, 1, 1), p(-1, 1, -1));
            push_quad(&mut o, p(-1, 1, 1), p(1, 1, 1), p(1, 1, -1), p(-1, 1, -1));
            push_quad(&mut o, p(-1, -1, -1), p(1, -1, -1), p(1, -1, 1), p(-1, -1, 1));
            push_quad(&mut o, p(-1, -1, 1), p(1, -1, 1), p(1, 1, 1), p(-1, 1, 1));
            push_quad(&mut o, p(1, -1, -1), p(-1, -1, -1), p(-1, 1, -1), p(1, 1, -1));
        }
        Mesh::Tri => {
            // the south-west, south-east and north-east corners; the other
            // half is this one turned half a turn
            push_flat(&mut o, v3(-H, 0, H), v3(H, 0, H), v3(H, 0, -H));
        }
        Mesh::Square => {
            push_quad(&mut o, v3(-H, 0, H), v3(H, 0, H), v3(H, 0, -H), v3(-H, 0, -H));
        }
        Mesh::Sphere => {
            let (lat, lon) = (16, 36);
            for i in 0..lat {
                for j in 0..lon {
                    let (a, b, c, d) = (
                        globe(i, j, lat, lon),
                        globe(i, j + 1, lat, lon),
                        globe(i + 1, j + 1, lat, lon),
                        globe(i + 1, j, lat, lon),
                    );
                    if i > 0 {
                        push_smooth(&mut o, [(a, a.norm()), (b, b.norm()), (c, c.norm())]);
                    }
                    if i < lat - 1 {
                        push_smooth(&mut o, [(a, a.norm()), (c, c.norm()), (d, d.norm())]);
                    }
                }
            }
        }
        Mesh::Blob => {
            let (lat, lon) = (5, 8);
            for i in 0..lat {
                for j in 0..lon {
                    let (a, b, c, d) = (
                        globe(i, j, lat, lon),
                        globe(i, j + 1, lat, lon),
                        globe(i + 1, j + 1, lat, lon),
                        globe(i + 1, j, lat, lon),
                    );
                    if i > 0 {
                        push_flat(&mut o, a, b, c);
                    }
                    if i < lat - 1 {
                        push_flat(&mut o, a, c, d);
                    }
                }
            }
        }
        Mesh::Cyl | Mesh::Cone => {
            let cone = mesh == Mesh::Cone;
            let sides = if cone { 8 } else { 10 };
            let rim = |j: i32, y: i32| {
                let a = fx::TURN * j / sides;
                v3(fx::mul(fx::cos(a), H), y, fx::mul(-fx::sin(a), H))
            };
            let apex = v3(0, H, 0);
            for j in 0..sides {
                let (a, b) = (rim(j, -H), rim(j + 1, -H));
                if cone {
                    push_flat(&mut o, a, b, apex);
                } else {
                    let (c, d) = (rim(j + 1, H), rim(j, H));
                    push_quad(&mut o, a, b, c, d);
                    push_flat(&mut o, d, c, apex);
                }
                push_flat(&mut o, b, a, v3(0, -H, 0));
            }
        }
        Mesh::Capsule => {
            // two half globes of radius r joined by a tube; normals point
            // out from the nearest point on the spine. 48 sides, so the
            // outline seen from 2.6 m stays round (with 20 its radius changed
            // by 1.2% as the camera went round, about a pixel)
            let r = CAPSULE_RADIUS;
            let spine = H - r;
            let (bands, lon) = (8, 48);
            // ring k of 2·bands + 1 from the bottom pole: its height and
            // radius, and the centre its normal points from
            let ring = |k: i32, j: i32| -> (V3, V3) {
                let (lat, centre) = if k <= bands {
                    (fx::QUARTER_TURN * k / bands - fx::QUARTER_TURN, -spine)
                } else {
                    (fx::QUARTER_TURN * (k - bands - 1) / bands, spine)
                };
                let lat = if k == bands + 1 { 0 } else { lat };
                let b = fx::TURN * j / lon;
                let n = v3(
                    fx::mul(fx::cos(lat), fx::cos(b)),
                    fx::sin(lat),
                    fx::mul(fx::cos(lat), -fx::sin(b)),
                );
                (v3(fx::mul(n.x, r), centre + fx::mul(n.y, r), fx::mul(n.z, r)), n)
            };
            let rings = 2 * bands + 2;
            for k in 0..rings - 1 {
                for j in 0..lon {
                    let (a, b, c, d) = (ring(k, j), ring(k, j + 1), ring(k + 1, j + 1), ring(k + 1, j));
                    if k > 0 {
                        push_smooth(&mut o, [a, b, c]);
                    }
                    if k < rings - 2 {
                        push_smooth(&mut o, [a, c, d]);
                    }
                }
            }
        }
        Mesh::Octa => {
            let (px, nx, py, ny, pz, nz) = (
                v3(H, 0, 0),
                v3(-H, 0, 0),
                v3(0, H, 0),
                v3(0, -H, 0),
                v3(0, 0, H),
                v3(0, 0, -H),
            );
            for (a, b) in [(px, pz), (pz, nx), (nx, nz), (nz, px)] {
                push_flat(&mut o, a, py, b);
                push_flat(&mut o, b, ny, a);
            }
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tris(m: Mesh) -> Vec<[V3; 4]> {
        vertices(m)
            .chunks(3 * VERTEX_VALUES)
            .map(|t| {
                let v = |i: usize| v3(t[i], t[i + 1], t[i + 2]);
                // the winding's own normal, not the stored one
                let n = v(6).sub(v(0)).cross(v(12).sub(v(0)));
                [v(0), v(6), v(12), n]
            })
            .collect()
    }

    #[test]
    fn every_face_of_a_closed_mesh_winds_away_from_its_centre() {
        for (m, centre) in [
            (Mesh::Cube, V3::ZERO),
            (Mesh::Octa, V3::ZERO),
            (Mesh::Sphere, V3::ZERO),
            (Mesh::Blob, V3::ZERO),
            (Mesh::Cyl, V3::ZERO),
            (Mesh::Cone, v3(0, -H / 2, 0)),
            (Mesh::Capsule, V3::ZERO),
        ] {
            for [a, b, c, n] in tris(m) {
                let centroid = v3(
                    (a.x + b.x + c.x) / 3,
                    (a.y + b.y + c.y) / 3,
                    (a.z + b.z + c.z) / 3,
                );
                assert!(
                    centroid.sub(centre).dot(n) > 0,
                    "{m:?}: face at {centroid:?} winds inward"
                );
            }
        }
    }

    #[test]
    fn every_mesh_fits_in_the_unit_cube() {
        for m in Mesh::ALL {
            for p in vertices(m).chunks(VERTEX_VALUES) {
                assert!(p[..3].iter().all(|c| c.abs() <= H), "{m:?}: {p:?}");
            }
        }
    }

    #[test]
    fn the_ground_shapes_face_up_and_the_cube_has_twelve_triangles() {
        assert_eq!(tris(Mesh::Cube).len(), 12);
        assert_eq!(tris(Mesh::Tri).len(), 1);
        for m in [Mesh::Tri, Mesh::Square] {
            for p in vertices(m).chunks(VERTEX_VALUES) {
                assert_eq!(&p[3..], &[0, ONE, 0], "{m:?} faces up");
            }
        }
    }

    #[test]
    fn the_capsule_is_round_ended_and_its_normals_point_out() {
        let vs = vertices(Mesh::Capsule);
        let top = vs.chunks(VERTEX_VALUES).map(|p| p[1]).max().unwrap();
        let widest = vs
            .chunks(VERTEX_VALUES)
            .map(|p| v3(p[0], 0, p[2]).len())
            .max()
            .unwrap();
        assert!((top - H).abs() <= 4, "the top pole is at half a unit: {top}");
        assert!((widest - CAPSULE_RADIUS).abs() <= 8, "{widest}");
        for p in vs.chunks(VERTEX_VALUES) {
            let n = v3(p[3], p[4], p[5]);
            assert!((n.len() - ONE).abs() < ONE / 100, "normal not unit: {n:?}");
            let out = v3(p[0], 0, p[2]);
            assert!(out.dot(n) >= -4, "a normal points into the capsule at {p:?}");
        }
    }
}
