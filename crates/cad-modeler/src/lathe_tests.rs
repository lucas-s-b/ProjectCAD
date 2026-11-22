use super::*;
use glam::DVec3;
use std::f64::consts::TAU;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs().max(1.0), "{a} vs {b}");
}

fn near3(a: DVec3, b: DVec3, tol: f64) {
    assert!(a.distance(b) <= tol, "{a} vs {b}");
}

#[test]
fn lathe_cylinder() {
    let b = lathe(&[v(0.0, 0.0), v(5.0, 0.0), v(5.0, 10.0), v(0.0, 10.0)], &Plane3::XY, 64).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), 32.0 * (TAU / 64.0).sin() * 25.0 * 10.0, 1e-9);
    assert_eq!(b.faces.len(), 3);
    assert!(b.faces.iter().any(|f| f.surface == Surface::Cylinder { origin: DVec3::ZERO, axis: DVec3::Z, radius: 5.0 }));
    let (lo, hi) = b.bbox();
    near3(lo, DVec3::new(-5.0, -5.0, 0.0), 1e-9);
    near3(hi, DVec3::new(5.0, 5.0, 10.0), 1e-9);
}

#[test]
fn lathe_cone_on_a_tilted_plane() {
    let plane = Plane3 { origin: DVec3::new(1.0, 2.0, 3.0), x: DVec3::Y, y: DVec3::Z };
    let b = lathe(&[v(0.0, 0.0), v(5.0, 0.0), v(0.0, 10.0)], &plane, 64).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), 32.0 * (TAU / 64.0).sin() * 25.0 * 10.0 / 3.0, 1e-9);
    assert_eq!(b.faces.len(), 2);
    let cone = b.faces.iter().find_map(|f| if let Surface::Cone { apex, axis, half_angle } = f.surface { Some((apex, axis, half_angle)) } else { None });
    let (apex, axis, half_angle) = cone.unwrap();
    near3(apex, DVec3::new(11.0, 2.0, 3.0), 1e-9);
    near3(axis, DVec3::new(-1.0, 0.0, 0.0), 1e-9);
    assert!((half_angle - 0.5f64.atan()).abs() <= 1e-9);
}

#[test]
fn lathe_refuses_bad_outlines() {
    let axis_err = Err(ModelError::Invalid("lathe outline must start and end on the axis with positive radii between"));
    assert_eq!(lathe(&[v(1.0, 0.0), v(5.0, 0.0), v(0.0, 10.0)], &Plane3::XY, 64), axis_err);
    assert_eq!(lathe(&[v(0.0, 0.0), v(5.0, 0.0), v(5.0, 10.0), v(0.0, 10.0)], &Plane3::XY, 2), axis_err);
    let cw = lathe(&[v(0.0, 10.0), v(5.0, 10.0), v(5.0, 0.0), v(0.0, 0.0)], &Plane3::XY, 64);
    assert_eq!(cw, Err(ModelError::Invalid("lathe outline must run counter-clockwise without crossing itself")));
}
