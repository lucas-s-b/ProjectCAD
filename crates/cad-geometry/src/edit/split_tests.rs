use super::*;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn near(a: DVec2, b: DVec2) -> bool {
    a.distance(b) < 1e-9
}
fn pts(s: &Shape) -> &[DVec2] {
    let Shape::Polyline { points, .. } = s else { panic!("{s:?}") };
    points
}

#[test]
fn split_a_line() {
    let l = Shape::Line { a: v2(0.0, 0.0), b: v2(40.0, 0.0) };
    let r = split(&l, v2(10.0, 3.0)).unwrap();
    assert_eq!(r, [Shape::Line { a: v2(0.0, 0.0), b: v2(10.0, 0.0) }, Shape::Line { a: v2(10.0, 0.0), b: v2(40.0, 0.0) }]);
}

#[test]
fn split_an_arc() {
    let a = Shape::Arc { center: v2(0.0, 0.0), radius: 2.0, start: 0.0, sweep: PI };
    let r = split(&a, v2(0.0, 2.0)).unwrap();
    let [Shape::Arc { start: s0, sweep: w0, .. }, Shape::Arc { start: s1, sweep: w1, .. }] = r[..] else { panic!("{r:?}") };
    assert!(s0.abs() < 1e-12 && (w0 - FRAC_PI_2).abs() < 1e-9);
    assert!((s1 - FRAC_PI_2).abs() < 1e-9 && (w1 - FRAC_PI_2).abs() < 1e-9);
}

#[test]
fn a_circle_becomes_one_full_arc_starting_at_the_point() {
    let c = Shape::Circle { center: v2(1.0, 1.0), radius: 3.0 };
    let r = split(&c, v2(1.0, 10.0)).unwrap();
    let [Shape::Arc { start, sweep, radius, .. }] = r[..] else { panic!("{r:?}") };
    assert!((start - FRAC_PI_2).abs() < 1e-9 && (sweep - TAU).abs() < 1e-12 && radius == 3.0);
}

#[test]
fn split_an_open_polyline_keeps_the_corner_in_the_right_piece() {
    let p = Shape::Polyline { points: vec![v2(0.0, 0.0), v2(4.0, 0.0), v2(4.0, 4.0)], closed: false };
    let r = split(&p, v2(4.0, 1.0)).unwrap();
    assert_eq!(r.len(), 2);
    assert!(pts(&r[0]).len() == 3 && near(pts(&r[0])[1], v2(4.0, 0.0)) && near(pts(&r[0])[2], v2(4.0, 1.0)));
    assert!(pts(&r[1]).len() == 2 && near(pts(&r[1])[0], v2(4.0, 1.0)) && near(pts(&r[1])[1], v2(4.0, 4.0)));
}

#[test]
fn closed_polyline_and_rect_open_at_the_point() {
    let p = Shape::Polyline { points: vec![v2(0.0, 0.0), v2(4.0, 0.0), v2(4.0, 4.0), v2(0.0, 4.0)], closed: true };
    let r = split(&p, v2(4.0, 2.0)).unwrap();
    assert_eq!(r.len(), 1);
    let q = pts(&r[0]);
    assert!(matches!(r[0], Shape::Polyline { closed: false, .. }) && q.len() == 6);
    assert!(near(q[0], v2(4.0, 2.0)) && near(q[5], v2(4.0, 2.0)) && near(q[1], v2(4.0, 4.0)) && near(q[3], v2(0.0, 0.0)));
    let rect = Shape::rect(v2(0.0, 0.0), v2(2.0, 1.0));
    let r = split(&rect, v2(1.0, 0.0)).unwrap();
    let q = pts(&r[0]);
    assert!(near(q[0], v2(1.0, 0.0)) && near(*q.last().unwrap(), v2(1.0, 0.0)) && q.len() == 6);
}

#[test]
fn errors() {
    let l = Shape::Line { a: v2(0.0, 0.0), b: v2(4.0, 0.0) };
    assert_eq!(split(&l, v2(-5.0, 0.0)), Err(GeomError::Degenerate("split point is at an end")));
    assert_eq!(split(&l, v2(4.0, 1.0)), Err(GeomError::Degenerate("split point is at an end")));
    let t = Shape::Text { pos: DVec2::ZERO, height: 1.0, angle: 0.0, text: "a".into() };
    assert!(matches!(split(&t, DVec2::ZERO), Err(GeomError::Unsupported(_))));
}
