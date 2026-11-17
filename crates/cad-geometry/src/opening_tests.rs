use super::*;
use crate::shape::{GeomError, Shape};
use crate::wall::WallAlign;
use crate::xform::mirror_across;
use crate::{Tolerance, distance_2d};
use glam::DVec3;

/// A 2100 high door on the centred wall (0,0)-(4000,0), 200 thick, 2500 high.
fn door(at: f64, width: f64) -> Opening {
    let wall = Wall { a: DVec2::ZERO, b: DVec2::new(4000.0, 0.0), thickness: 200.0, height: 2500.0, align: WallAlign::Center };
    Opening { host: 1, wall, at, width, height: 2100.0, sill: 0.0, kind: OpeningKind::Door, hinge_b: false, swing_left: true }
}

fn close(a: DVec2, x: f64, y: f64) {
    assert!(a.distance(DVec2::new(x, y)) <= 1e-9, "{a} vs ({x}, {y})");
}

#[test]
fn span_centres_and_clamps() {
    assert_eq!(door(2000.0, 900.0).span(), (1550.0, 2450.0));
    assert_eq!(door(100.0, 900.0).span(), (0.0, 900.0));
    assert_eq!(door(5000.0, 900.0).span(), (3100.0, 4000.0));
    assert_eq!(door(2000.0, 5000.0).span(), (0.0, 4000.0));
}

#[test]
fn corners_follow_the_wall_faces() {
    let expect = |c: [DVec2; 4], e: [(f64, f64); 4]| c.iter().zip(e).for_each(|(p, (x, y))| close(*p, x, y));
    expect(door(2000.0, 900.0).corners(), [(1550.0, -100.0), (2450.0, -100.0), (2450.0, 100.0), (1550.0, 100.0)]);
    let mut o = door(2000.0, 900.0);
    o.wall.align = WallAlign::Left;
    expect(o.corners(), [(1550.0, 0.0), (2450.0, 0.0), (2450.0, 200.0), (1550.0, 200.0)]);
    let mut o = door(1000.0, 800.0);
    o.wall.b = DVec2::new(0.0, 4000.0);
    expect(o.corners(), [(100.0, 600.0), (100.0, 1400.0), (-100.0, 1400.0), (-100.0, 600.0)]);
}

#[test]
fn validate_and_kind() {
    let tol = Tolerance::default();
    let s = Shape::Opening(door(2000.0, 900.0));
    assert_eq!(s.validate(&tol), Ok(()));
    assert_eq!(s.kind(), "Door");
    let mut o = door(2000.0, 900.0);
    o.kind = OpeningKind::Window;
    assert_eq!(Shape::Opening(o).kind(), "Window");
    let msg = "opening width and height must be positive and its sill not negative";
    let mut o = door(2000.0, 900.0);
    o.width = 0.0;
    assert_eq!(Shape::Opening(o).validate(&tol), Err(GeomError::Degenerate(msg)));
    let mut o = door(2000.0, 900.0);
    o.sill = -1.0;
    assert_eq!(Shape::Opening(o).validate(&tol), Err(GeomError::Degenerate(msg)));
    let mut o = door(2000.0, 900.0);
    o.at = f64::NAN;
    assert_eq!(Shape::Opening(o).validate(&tol), Err(GeomError::NonFinite));
    let mut o = door(2000.0, 900.0);
    o.wall.b = o.wall.a;
    assert_eq!(Shape::Opening(o).validate(&tol), Err(GeomError::Degenerate("wall has zero length")));
}

#[test]
fn mirror_flips_the_swing() {
    let s = Shape::Opening(door(1000.0, 900.0)).transformed(&mirror_across(DVec2::ZERO, DVec2::Y)).unwrap();
    let Shape::Opening(o) = s else { panic!("not an opening") };
    assert!(!o.swing_left && !o.hinge_b);
    assert_eq!(o.at, 1000.0);
    assert!(o.wall.a.distance(DVec2::ZERO) <= 1e-12);
    assert!(o.wall.b.distance(DVec2::new(-4000.0, 0.0)) <= 1e-12);
    assert_eq!(o.wall.align, WallAlign::Center);
}

#[test]
fn translate_moves_the_wall_copy() {
    let mut s = Shape::Opening(door(1000.0, 900.0));
    s.translate(DVec3::new(0.0, 500.0, 0.0));
    let Shape::Opening(o) = s else { panic!("not an opening") };
    assert_eq!((o.wall.a, o.wall.b, o.at), (DVec2::new(0.0, 500.0), DVec2::new(4000.0, 500.0), 1000.0));
}

#[test]
fn picks_and_bounds_the_gap() {
    let s = Shape::Opening(door(2000.0, 900.0));
    assert!(distance_2d(&s, DVec2::new(2000.0, 50.0)).abs() <= 1e-9);
    assert!((distance_2d(&s, DVec2::new(2000.0, 300.0)) - 200.0).abs() <= 1e-9);
    assert_eq!(s.bbox(), (DVec3::new(1550.0, -100.0, 0.0), DVec3::new(2450.0, 100.0, 0.0)));
}
