use super::*;
use crate::fixtures::*;
use crate::sweep::arc_steps;
use cad_geometry::profile::Profile;
use cad_geometry::shape::Shape;
use std::f64::consts::PI;

const TOL: f64 = 0.01;

fn at(z: f64) -> Plane3 {
    Plane3 { origin: DVec3::new(0.0, 0.0, z), ..Plane3::XY }
}

fn square(half: f64) -> Profile {
    make(&[rect(-half, -half, half, half)])
}

fn build(parts: &[(&Profile, &Plane3)]) -> Result<Body, ModelError> {
    let sections: Vec<Section> = parts.iter().map(|&(profile, plane)| Section { profile, plane }).collect();
    loft(&sections, TOL)
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

fn frustum(h: f64, a: f64, b: f64) -> f64 {
    h / 3.0 * (a * a + b * b + a * b)
}

fn facet_circle(r: f64) -> f64 {
    let n = 2.0 * arc_steps(r, PI, TOL) as f64;
    0.5 * n * r * r * (2.0 * PI / n).sin()
}

#[test]
fn identical_squares_make_a_box() {
    let s = square(5.0);
    let b = build(&[(&s, &at(0.0)), (&s, &at(10.0))]).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), 1000.0);
}

#[test]
fn two_squares_make_a_frustum() {
    let (big, small) = (square(10.0), square(5.0));
    let b = build(&[(&big, &at(0.0)), (&small, &at(15.0))]).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), 15.0 / 3.0 * (400.0 + 100.0 + 200.0));
}

#[test]
fn opposite_directions_and_other_start_corners_give_the_same_frustum() {
    let big = square(10.0);
    // Given from another corner, in the other drawing direction, on a plane that is seen from below.
    let l = |a: (f64, f64), b: (f64, f64)| Shape::Line { a: v2(a.0, a.1), b: v2(b.0, b.1) };
    let small = make(&[l((2.5, 2.5), (-2.5, 2.5)), l((-2.5, 2.5), (-2.5, -2.5)), l((-2.5, -2.5), (2.5, -2.5)), l((2.5, -2.5), (2.5, 2.5))]);
    let flipped = Plane3 { origin: DVec3::new(0.0, 0.0, 15.0), x: DVec3::X, y: -DVec3::Y };
    let b = build(&[(&big, &at(0.0)), (&small, &flipped)]).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), frustum(15.0, 20.0, 5.0));
    let top_first = build(&[(&small, &flipped), (&big, &at(0.0))]).unwrap();
    close(top_first.volume(), b.volume());
}

#[test]
fn circles_make_a_prism_of_the_faceted_circle() {
    let c = make(&[circle(0.0, 0.0, 5.0)]);
    let b = build(&[(&c, &at(0.0)), (&c, &at(10.0))]).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), facet_circle(5.0) * 10.0);
    // The planned 0.2 % is below the faceting: 50 chords lose 1 - sin(a) / a = 0.26 % of the area (a = 2 pi / 50).
    let deficit = 1.0 - b.volume() / (PI * 25.0 * 10.0);
    assert!(deficit > 0.0 && deficit < 0.003, "{deficit}");
}

#[test]
fn square_to_circle_lies_between_the_prisms() {
    let (s, c) = (square(5.0), make(&[circle(0.0, 0.0, 5.0)]));
    let b = build(&[(&s, &at(0.0)), (&c, &at(10.0))]).unwrap();
    assert!(b.mesh.is_closed());
    let (lo, hi) = (facet_circle(5.0) * 10.0, 100.0 * 10.0);
    assert!(b.volume() > lo && b.volume() < hi, "{lo} < {} < {hi}", b.volume());
}

#[test]
fn three_sections_are_two_frusta() {
    let (a, big) = (square(5.0), square(10.0));
    let b = build(&[(&a, &at(0.0)), (&big, &at(10.0)), (&a, &at(20.0))]).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), 2.0 * 10.0 / 3.0 * (100.0 + 400.0 + 200.0));
}

#[test]
fn tilted_section_plane() {
    let s = square(5.0);
    let a = 30.0_f64.to_radians();
    let tilted = Plane3 { origin: DVec3::new(0.0, 0.0, 15.0), x: DVec3::X, y: DVec3::new(0.0, a.cos(), a.sin()) };
    let b = build(&[(&s, &at(0.0)), (&s, &tilted)]).unwrap();
    assert!(b.mesh.is_closed());
    assert!(b.volume() > 0.0);
}

#[test]
fn bad_sections_are_rejected() {
    let s = square(5.0);
    assert_eq!(build(&[(&s, &at(0.0))]).err(), Some(ModelError::Invalid("loft needs at least two sections")));
    let holed = make(&[rect(-5.0, -5.0, 5.0, 5.0), circle(0.0, 0.0, 1.0)]);
    assert_eq!(build(&[(&s, &at(0.0)), (&holed, &at(5.0))]).err(), Some(ModelError::Invalid("loft sections must be single loops without holes")));
    assert_eq!(build(&[(&s, &at(0.0)), (&s, &at(0.0))]).err(), Some(ModelError::Invalid("loft sections coincide")));
}
