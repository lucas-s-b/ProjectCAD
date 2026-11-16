use crate::fixtures::*;
use crate::{ModelError, Plane3, revolve, tessellate};
use cad_geometry::profile::Profile;
use cad_geometry::shape::Shape;
use cad_topology::{Brep, Surface};
use glam::DVec2;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

const Y_AXIS: (DVec2, DVec2) = (DVec2::ZERO, DVec2::Y);

fn pappus(p: &Profile, angle: f64) -> f64 {
    p.area() * angle * p.centroid().x.abs()
}

/// Revolves about the Y axis, checks validity, closed mesh, outward volume against Pappus. Returns the solid and the relative error.
fn solid(p: &Profile, plane: &Plane3, angle: f64) -> (Brep, f64) {
    solid_with(p, plane, angle, 0.01, 2e-3)
}

fn solid_with(p: &Profile, plane: &Plane3, angle: f64, tol: f64, bound: f64) -> (Brep, f64) {
    let b = revolve(p, plane, Y_AXIS.0, Y_AXIS.1, angle).unwrap();
    assert_eq!(b.validate(), Ok(()));
    let m = tessellate(&b, tol).unwrap();
    assert!(m.is_closed());
    assert_eq!(m.normals.len(), m.indices.len());
    assert!(m.normals.iter().all(|n| (n.length() - 1.0).abs() < 1e-9));
    let exact = pappus(p, angle);
    let err = (m.volume() - exact) / exact;
    println!("angle {angle:.3}: {} faces, volume {} vs Pappus {exact}, relative error {err:.3e}", b.faces.len(), m.volume());
    assert!(err.abs() < bound, "{err}");
    (b, err)
}

fn count(b: &Brep, f: impl Fn(&Surface) -> bool) -> usize {
    b.faces.iter().filter(|x| f(&x.surface)).count()
}

#[test]
fn rectangle_full_and_partial() {
    for x in [(10.0, 20.0), (-20.0, -10.0)] {
        let p = make(&[rect(x.0, 0.0, x.1, 5.0)]);
        for plane in [Plane3::XY, tilted()] {
            let (b, _) = solid(&p, &plane, TAU);
            assert_eq!(b.faces.len(), 8);
            assert_eq!(count(&b, |s| matches!(s, Surface::Cylinder { .. })), 4);
            assert_eq!(b.euler(), 0);
            let (b, _) = solid(&p, &plane, FRAC_PI_2);
            assert_eq!(b.faces.len(), 6);
            assert_eq!(b.euler(), 2);
            let (b, _) = solid(&p, &plane, 1.5 * PI);
            assert_eq!(b.faces.len(), 2 + 8);
            solid(&p, &plane, TAU + 1e-10);
        }
    }
}

#[test]
fn hollow_ring() {
    for x in [20.0, -20.0] {
        let p = make(&[rect(x - 10.0, 0.0, x + 10.0, 10.0), circle(x, 5.0, 2.0)]);
        let (b, _) = solid(&p, &Plane3::XY, TAU);
        assert_eq!(count(&b, |s| matches!(s, Surface::Torus { .. })), 4);
        let (b, _) = solid(&p, &Plane3::XY, 2.0);
        assert_eq!(b.faces.len(), 2 + 6);
        assert_eq!(b.faces[0].loops.len(), 2);
    }
}

#[test]
fn torus_from_a_disc() {
    for cx in [10.0, -10.0] {
        let p = make(&[circle(cx, 0.0, 3.0)]);
        // Facet error of a grid on the surface is (x^2 + y^2) / 6 with steps x, y <= d and d^2 ~ 8 tol / r_max,
        // so at most 8 tol / (3 r_max) = 0.205 % here (r_max = 13): the plan's 0.2 % is that figure rounded down.
        let (b, err) = solid_with(&p, &Plane3::XY, TAU, 0.01, 8.0 * 0.01 / (3.0 * 13.0));
        assert_eq!(b.faces.len(), 4);
        assert_eq!(count(&b, |s| matches!(s, Surface::Torus { .. })), 4);
        let exact = 2.0 * PI * PI * 10.0 * 9.0;
        assert!((pappus(&p, TAU) - exact).abs() < 1e-9);
        println!("torus relative volume error at tol 0.01: {err:.3e}");
    }
}

#[test]
fn sphere_and_cone_faces() {
    // Ring of a sphere of radius 10 around the origin: arc from 30 to 60 degrees, closed by lines.
    let (a, b) = (PI / 6.0, PI / 3.0);
    let (p1, p2) = (v2(10.0 * a.cos(), 10.0 * a.sin()), v2(10.0 * b.cos(), 10.0 * b.sin()));
    let line = |s: glam::DVec2, e: glam::DVec2| Shape::Line { a: s, b: e };
    let zone = make(&[
        Shape::Arc { center: v2(0.0, 0.0), radius: 10.0, start: a, sweep: b - a },
        line(p2, v2(3.0, p2.y)),
        line(v2(3.0, p2.y), v2(3.0, p1.y)),
        line(v2(3.0, p1.y), p1),
    ]);
    let (br, _) = solid_with(&zone, &Plane3::XY, TAU, 0.001, 2e-3);
    assert_eq!(count(&br, |s| matches!(s, Surface::Sphere { .. })), 2);
    assert_eq!(count(&br, |s| matches!(s, Surface::Cylinder { .. })), 2);
    assert_eq!(count(&br, |s| matches!(s, Surface::Plane { .. })), 4);
    let trapezoid = make(&[Shape::Polyline { points: vec![v2(10.0, 0.0), v2(20.0, 0.0), v2(15.0, 10.0), v2(10.0, 10.0)], closed: true }]);
    let (br, _) = solid(&trapezoid, &Plane3::XY, TAU);
    assert_eq!(count(&br, |s| matches!(s, Surface::Cone { .. })), 2);
    assert_eq!(count(&br, |s| matches!(s, Surface::Cylinder { .. })), 2);
    solid(&trapezoid, &tilted(), 2.5);
}

#[test]
fn invalid_requests() {
    let invalid = |m: &'static str| Err::<(), _>(ModelError::Invalid(m));
    let p = make(&[rect(10.0, 0.0, 20.0, 5.0)]);
    let go = |p: &Profile, ap: DVec2, ad: DVec2, angle: f64| revolve(p, &Plane3::XY, ap, ad, angle).map(|_| ());
    for angle in [0.0, 7.0, -1.0, 1e-10, f64::NAN] {
        assert_eq!(go(&p, DVec2::ZERO, DVec2::Y, angle), invalid("revolve angle out of range"));
    }
    assert_eq!(go(&p, DVec2::ZERO, DVec2::ZERO, 1.0), invalid("revolve axis is degenerate"));
    let touch = "profile must lie on one side of the axis without touching it";
    // Half disc whose straight side lies on the axis.
    let half = make(&[Shape::Arc { center: v2(0.0, 0.0), radius: 3.0, start: -FRAC_PI_2, sweep: PI }, Shape::Line { a: v2(0.0, 3.0), b: v2(0.0, -3.0) }]);
    assert_eq!(go(&half, DVec2::ZERO, DVec2::Y, TAU), invalid(touch));
    // Crossing, touching and an arc that reaches over the axis.
    assert_eq!(go(&make(&[rect(-5.0, 0.0, 5.0, 5.0)]), DVec2::ZERO, DVec2::Y, TAU), invalid(touch));
    assert_eq!(go(&make(&[rect(0.0, 0.0, 5.0, 5.0)]), DVec2::ZERO, DVec2::Y, TAU), invalid(touch));
    assert_eq!(go(&make(&[circle(2.0, 0.0, 3.0)]), DVec2::ZERO, DVec2::Y, TAU), invalid(touch));
    // Axis through the profile along x instead.
    assert_eq!(go(&p, DVec2::ZERO, DVec2::X, TAU), invalid(touch));
    assert!(go(&p, v2(0.0, -1.0), DVec2::X * 3.0, TAU).is_ok());
}

#[test]
fn off_origin_axis() {
    // Same ring revolved about the line x = 5 (direction -y): mirrored handling, same Pappus volume.
    let p = make(&[rect(10.0, 0.0, 20.0, 5.0)]);
    let b = revolve(&p, &Plane3::XY, v2(5.0, 0.0), -DVec2::Y, TAU).unwrap();
    let m = tessellate(&b, 0.01).unwrap();
    let exact = p.area() * TAU * 10.0;
    assert!(m.is_closed() && (m.volume() - exact).abs() / exact < 2e-3);
}
