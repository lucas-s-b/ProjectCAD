use super::*;
use crate::fixtures::*;
use crate::sweep::arc_steps;
use std::f64::consts::PI;

const TOL: f64 = 0.01;

fn square(half: f64) -> Profile {
    make(&[rect(-half, -half, half, half)])
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

/// Frustum between parallel similar squares of sides `a` and `b`, `h` apart.
fn frustum(h: f64, a: f64, b: f64) -> f64 {
    h / 3.0 * (a * a + b * b + a * b)
}

fn q() -> f64 {
    0.25_f64.atan() // tan = 0.25: 10 mm high walls move in by 2.5 mm
}

#[test]
fn zero_angle_is_a_prism() {
    let b = extrude_tapered(&square(10.0), &Plane3::XY, 10.0, 0.0, TOL).unwrap();
    close(b.volume(), 4000.0);
}

#[test]
fn square_becomes_a_frustum() {
    let b = extrude_tapered(&square(10.0), &Plane3::XY, 10.0, q(), TOL).unwrap();
    close(b.volume(), frustum(10.0, 20.0, 15.0));
    assert!(b.mesh.is_closed());
    assert_eq!(b.faces.len(), 6);
    assert!(b.faces.iter().all(|f| matches!(f.surface, Surface::Plane { .. })));
    let want = DVec3::new(0.0, -4.0, 1.0) / 17f64.sqrt();
    assert!(b.faces.iter().any(|f| match f.surface {
        Surface::Plane { normal, .. } => ((if f.outward { normal } else { -normal }) - want).length() < 1e-9,
        _ => false,
    }));
}

#[test]
fn negative_angle_widens() {
    let b = extrude_tapered(&square(10.0), &Plane3::XY, 10.0, -q(), TOL).unwrap();
    close(b.volume(), frustum(10.0, 20.0, 25.0));
}

#[test]
fn negative_distance_goes_below_the_plane() {
    let b = extrude_tapered(&square(10.0), &Plane3::XY, -10.0, q(), TOL).unwrap();
    close(b.volume(), frustum(10.0, 20.0, 15.0));
    let (lo, hi) = b.bbox();
    assert!((lo.z + 10.0).abs() < 1e-9 && hi.z.abs() < 1e-9);
    let x = b.mesh.positions.iter().filter(|p| (p.z + 10.0).abs() < 1e-9).map(|p| p.x).fold(f64::MIN, f64::max);
    assert!((x - 7.5).abs() < 1e-9, "{x}");
}

#[test]
fn holes_widen() {
    let p = make(&[rect(-10.0, -10.0, 10.0, 10.0), rect(-2.0, -2.0, 2.0, 2.0)]);
    let b = extrude_tapered(&p, &Plane3::XY, 10.0, q(), TOL).unwrap();
    close(b.volume(), frustum(10.0, 20.0, 15.0) - frustum(10.0, 4.0, 9.0));
}

#[test]
fn circle_becomes_a_faceted_cone_frustum() {
    let b = extrude_tapered(&make(&[circle(0.0, 0.0, 10.0)]), &Plane3::XY, 10.0, 0.5_f64.atan(), TOL).unwrap();
    let n = 2.0 * arc_steps(10.0, PI, TOL) as f64;
    let area = 0.5 * n * 100.0 * (2.0 * PI / n).sin();
    let s = 1.0 - 5.0 / (10.0 * (PI / n).cos()); // apothem shrinks by 5
    close(b.volume(), 10.0 / 3.0 * area * (1.0 + s + s * s));
    let cone = PI / 3.0 * 10.0 * (100.0 + 25.0 + 50.0);
    let deficit = 1.0 - b.volume() / cone;
    assert!(deficit > 0.0 && deficit < 0.01, "{deficit}");
}

#[test]
fn too_steep_is_refused() {
    let r = extrude_tapered(&square(10.0), &Plane3::XY, 10.0, 1.5_f64.atan(), TOL);
    assert_eq!(r.err(), Some(TOO_STEEP));
}

#[test]
fn colliding_loops_are_refused() {
    let a = 0.5_f64.atan();
    let crossing = make(&[rect(-10.0, -10.0, 10.0, 10.0), rect(2.0, -2.0, 6.0, 2.0)]);
    assert_eq!(extrude_tapered(&crossing, &Plane3::XY, 10.0, a, TOL).err(), Some(TOO_STEEP));
    let swallowed = make(&[rect(-10.0, -10.0, 10.0, 10.0), rect(-2.0, -2.0, 2.0, 2.0)]);
    assert_eq!(extrude_tapered(&swallowed, &Plane3::XY, 10.0, a, TOL).err(), Some(TOO_STEEP));
}

#[test]
fn bad_inputs_are_refused() {
    let r = extrude_tapered(&square(10.0), &Plane3::XY, 10.0, FRAC_PI_2, TOL);
    assert_eq!(r.err(), Some(ModelError::Invalid("draft angle must be between -90 and 90 degrees")));
    let r = extrude_tapered(&square(10.0), &Plane3::XY, 0.0, q(), TOL);
    assert_eq!(r.err(), Some(ModelError::Invalid("extrusion distance is zero")));
}
