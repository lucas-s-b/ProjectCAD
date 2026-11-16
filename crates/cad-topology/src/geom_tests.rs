use crate::geom::{Curve, Surface};
use glam::DVec3;
use std::f64::consts::FRAC_PI_2;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}
fn close(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-12, "{a:?} != {b:?}");
}

#[test]
fn plane() {
    let s = Surface::Plane { origin: v(0.0, 0.0, 0.0), normal: DVec3::Z };
    close(s.project(v(1.0, 2.0, 3.0)), v(1.0, 2.0, 0.0));
    close(s.normal_at(v(1.0, 2.0, 0.0)), DVec3::Z);
    assert!((s.distance(v(1.0, 2.0, -3.0)) - 3.0).abs() < 1e-12);
}

#[test]
fn cylinder() {
    let s = Surface::Cylinder { origin: v(0.0, 0.0, 0.0), axis: DVec3::Z, radius: 2.0 };
    close(s.project(v(3.0, 4.0, 5.0)), v(1.2, 1.6, 5.0));
    assert!((s.distance(v(3.0, 4.0, 5.0)) - 3.0).abs() < 1e-12);
    close(s.normal_at(v(0.0, 2.0, 1.0)), DVec3::Y);
    assert!((s.distance(v(0.0, 0.0, 0.0)) - 2.0).abs() < 1e-12);
}

#[test]
fn cone() {
    let s = Surface::Cone { apex: v(0.0, 0.0, 0.0), axis: DVec3::Z, half_angle: std::f64::consts::FRAC_PI_4 };
    close(s.project(v(2.0, 0.0, 1.0)), v(1.5, 0.0, 1.5));
    assert!((s.distance(v(2.0, 0.0, 1.0)) - 0.5_f64.sqrt()).abs() < 1e-12);
    let h = 0.5_f64.sqrt();
    close(s.normal_at(v(1.0, 0.0, 1.0)), v(h, 0.0, -h));
    close(s.project(v(0.0, 0.0, -1.0)), DVec3::ZERO);
    close(s.project(v(1.0, 0.0, 1.0)), v(1.0, 0.0, 1.0));
}

#[test]
fn sphere() {
    let s = Surface::Sphere { center: v(1.0, 1.0, 1.0), radius: 2.0 };
    close(s.project(v(1.0, 1.0, 5.0)), v(1.0, 1.0, 3.0));
    assert!((s.distance(v(1.0, 1.0, 5.0)) - 2.0).abs() < 1e-12);
    close(s.normal_at(v(-1.0, 1.0, 1.0)), -DVec3::X);
}

#[test]
fn torus() {
    let s = Surface::Torus { center: v(0.0, 0.0, 0.0), axis: DVec3::Z, major: 5.0, minor: 1.0 };
    close(s.project(v(7.0, 0.0, 0.0)), v(6.0, 0.0, 0.0));
    assert!((s.distance(v(7.0, 0.0, 0.0)) - 1.0).abs() < 1e-12);
    close(s.project(v(5.0, 0.0, 3.0)), v(5.0, 0.0, 1.0));
    close(s.project(v(3.0, 0.0, 0.0)), v(4.0, 0.0, 0.0));
    close(s.normal_at(v(5.0, 0.0, 1.0)), DVec3::Z);
    close(s.normal_at(v(4.0, 0.0, 0.0)), -DVec3::X);
    close(s.normal_at(v(0.0, 6.0, 0.0)), DVec3::Y);
}

#[test]
fn faceted() {
    let s = Surface::Faceted;
    close(s.project(v(1.0, 2.0, 3.0)), v(1.0, 2.0, 3.0));
    assert_eq!(s.distance(v(1.0, 2.0, 3.0)), 0.0);
    assert_eq!(s.normal_at(v(1.0, 2.0, 3.0)), DVec3::ZERO);
    assert_eq!(s.transformed(&glam::DAffine3::from_translation(DVec3::X)), Surface::Faceted);
}

#[test]
fn curve_points() {
    let (a, b) = (v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0));
    close(Curve::Line.point(a, b, 0.0), a);
    close(Curve::Line.point(a, b, 0.5), v(1.0, 0.0, 0.0));
    close(Curve::Line.point(a, b, 1.0), b);
    let arc = Curve::Arc { center: DVec3::ZERO, normal: DVec3::Z, sweep: FRAC_PI_2 };
    let (a, b) = (v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0));
    close(arc.point(a, b, 0.0), a);
    let h = 0.5_f64.sqrt();
    close(arc.point(a, b, 0.5), v(h, h, 0.0));
    close(arc.point(a, b, 1.0), b);
}
