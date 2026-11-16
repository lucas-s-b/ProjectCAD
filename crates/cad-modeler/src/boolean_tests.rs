use crate::fixtures::*;
use crate::{BoolOp, Body, ModelError, Plane3, boolean, revolve};
use cad_topology::Surface;
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;

fn cube(min: f64, max: f64) -> Body {
    box_body([min; 3], [max; 3])
}

fn bool_ok(a: &Body, b: &Body, op: BoolOp) -> Body {
    let r = boolean(a, b, op).unwrap();
    assert!(r.mesh.is_closed());
    assert_eq!(r.tri_face.len(), r.mesh.triangle_count());
    assert_eq!(r.mesh.normals.len(), r.mesh.indices.len());
    // Every corner normal points to the outside of its triangle's winding.
    for (k, t) in r.mesh.indices.as_chunks::<3>().0.iter().enumerate() {
        let [a, b, c] = [0, 1, 2].map(|j| r.mesh.positions[t[j] as usize]);
        let n = (b - a).cross(c - a);
        assert!(r.mesh.normals[3 * k..3 * k + 3].iter().all(|m| (m.length() - 1.0).abs() < 1e-9 && n.dot(*m) > 0.0));
    }
    r
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs(), "{a} vs {b}");
}

#[test]
fn exact_box_cases() {
    let shifted = box_body([5.0; 3], [15.0; 3]);
    let cases = [
        (cube(0.0, 10.0), shifted, BoolOp::Union, 1875.0),
        (cube(0.0, 10.0), cube(0.0, 5.0), BoolOp::Difference, 875.0),
        (cube(0.0, 10.0), box_body([10.0, 0.0, 0.0], [20.0, 10.0, 10.0]), BoolOp::Union, 2000.0),
        (cube(0.0, 10.0), cube(0.0, 10.0), BoolOp::Union, 1000.0),
        (cube(0.0, 10.0), box_body([10.0, 10.0, 0.0], [20.0, 20.0, 10.0]), BoolOp::Difference, 1000.0),
    ];
    for (a, b, op, volume) in cases {
        close(bool_ok(&a, &b, op).volume(), volume, 1e-9);
    }
}

fn ring() -> Body {
    let p = make(&[rect(10.0, -2.0, 20.0, 2.0)]);
    Body::from_brep(&revolve(&p, &Plane3::XY, DVec2::ZERO, DVec2::Y, TAU).unwrap(), 0.01).unwrap()
}

#[test]
fn curved_pairs_are_consistent() {
    let cyl = cylinder_body(0.0, 0.0, 4.0, -30.0, 30.0);
    let pairs = [
        (box_body([-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]), cyl.clone()),
        (cylinder_body(15.0, 0.0, 3.0, -30.0, 30.0), ring()),
        (cylinder_body(0.0, 0.0, 5.0, 0.0, 10.0), cylinder_body(3.0, 1.0, 4.0, 2.0, 14.0)),
    ];
    for (a, b) in &pairs {
        let (u, i) = (bool_ok(a, b, BoolOp::Union), bool_ok(a, b, BoolOp::Intersection));
        let d = bool_ok(a, b, BoolOp::Difference);
        close(u.volume() + i.volume(), a.volume() + b.volume(), 1e-9);
        close(d.volume(), a.volume() - i.volume(), 1e-9);
    }
}

#[test]
fn box_minus_cylinder_keeps_surface_identity() {
    let r = bool_ok(&box_body([-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]), &cylinder_body(0.0, 0.0, 4.0, -5.0, 15.0), BoolOp::Difference);
    assert!(r.faces.iter().any(|f| matches!(f.surface, Surface::Cylinder { .. }) && !f.outward));
    assert!(!r.feature_edges().is_empty());
    for (k, &i) in r.mesh.indices.iter().enumerate() {
        let p = r.mesh.positions[i as usize];
        let f = &r.faces[r.tri_face[k / 3] as usize];
        assert!(f.surface.distance(p) < 0.02, "{p:?} is {} off {:?}", f.surface.distance(p), f.surface);
    }
}

#[test]
fn chained_difference() {
    let first = bool_ok(&box_body([-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]), &cylinder_body(0.0, 0.0, 4.0, -5.0, 15.0), BoolOp::Difference);
    let second = cylinder_body(6.0, 6.0, 2.0, -5.0, 15.0);
    let both = bool_ok(&first, &second, BoolOp::Difference);
    let hole = bool_ok(&box_body([-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]), &second, BoolOp::Difference);
    // The two holes do not touch, so their volume losses add up.
    close(both.volume(), first.volume() + hole.volume() - 4000.0, 1e-9);
}

struct XorShift(u64);

impl XorShift {
    fn f(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Exact union volume of the 10-cubes at `offs` by coordinate compression.
fn cube_union_volume(offs: &[[f64; 3]]) -> f64 {
    let axis = |d: usize| {
        let mut v: Vec<f64> = offs.iter().flat_map(|o| [o[d], o[d] + 10.0]).collect();
        v.sort_by(f64::total_cmp);
        v.dedup();
        v
    };
    let ax = [axis(0), axis(1), axis(2)];
    let mut vol = 0.0;
    for i in 0..ax[0].len() - 1 {
        for j in 0..ax[1].len() - 1 {
            for k in 0..ax[2].len() - 1 {
                let mid = [(ax[0][i] + ax[0][i + 1]) / 2.0, (ax[1][j] + ax[1][j + 1]) / 2.0, (ax[2][k] + ax[2][k + 1]) / 2.0];
                if offs.iter().any(|o| (0..3).all(|d| mid[d] > o[d] && mid[d] < o[d] + 10.0)) {
                    vol += (ax[0][i + 1] - ax[0][i]) * (ax[1][j + 1] - ax[1][j]) * (ax[2][k + 1] - ax[2][k]);
                }
            }
        }
    }
    vol
}

#[test]
fn fold_of_twelve_cubes_stays_closed_and_exact() {
    let mut rng = XorShift(0x2545F4914F6CDD1D);
    let offs: Vec<[f64; 3]> = (0..12).map(|_| [rng.f() * 30.0, rng.f() * 30.0, rng.f() * 30.0]).collect();
    let cube_at = |o: &[f64; 3]| box_body(*o, [o[0] + 10.0, o[1] + 10.0, o[2] + 10.0]);
    let mut acc = cube_at(&offs[0]);
    for (n, o) in offs.iter().enumerate().skip(1) {
        acc = bool_ok(&acc, &cube_at(o), BoolOp::Union);
        close(acc.volume(), cube_union_volume(&offs[..=n]), 1e-9);
    }
}

#[test]
fn disjoint_intersection_is_empty() {
    let r = boolean(&cube(0.0, 10.0), &cube(20.0, 30.0), BoolOp::Intersection);
    assert_eq!(r, Err(ModelError::Invalid("boolean result is empty")));
}

#[test]
fn scaled_overlap_case() {
    for s in [1e-3, 1e4] {
        let (a, b) = (box_body([0.0; 3], [10.0 * s; 3]), box_body([5.0 * s; 3], [15.0 * s; 3]));
        close(bool_ok(&a, &b, BoolOp::Union).volume(), 1875.0 * s * s * s, 1e-9);
    }
}

#[test]
fn difference_flips_the_tool_faces() {
    let r = bool_ok(&cube(0.0, 10.0), &cube(0.0, 5.0), BoolOp::Difference);
    assert_eq!(r.bbox(), (DVec3::ZERO, DVec3::splat(10.0)));
    // The tool's faces on x, y, z = 5 bound the remaining solid from the other side.
    let at5: Vec<_> = r.faces.iter().filter(|f| matches!(&f.surface, Surface::Plane { origin, normal } if origin.dot(*normal) == 5.0)).collect();
    assert_eq!(at5.len(), 3);
    assert!(at5.iter().all(|f| !f.outward));
}
