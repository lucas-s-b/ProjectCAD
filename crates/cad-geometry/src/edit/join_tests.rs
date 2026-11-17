use super::*;
use std::f64::consts::PI;

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn line(a: (f64, f64), b: (f64, f64)) -> Shape {
    Shape::Line { a: v2(a.0, a.1), b: v2(b.0, b.1) }
}
/// The chain runs from either free end, so both directions are right.
fn is_chain(s: &Shape, want: &[DVec2], closed: bool) -> bool {
    let Shape::Polyline { points, closed: c } = s else { return false };
    let mut back = want.to_vec();
    back.reverse();
    *c == closed && (points[..] == *want || *points == back)
}

#[test]
fn three_lines_given_out_of_order() {
    let (a, b, c) = (line((0.0, 0.0), (10.0, 0.0)), line((10.0, 10.0), (10.0, 0.0)), line((10.0, 10.0), (20.0, 10.0)));
    let r = join(&[&c, &a, &b]).unwrap();
    let want = [v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0), v2(20.0, 10.0)];
    assert!(is_chain(&r, &want, false), "{r:?}");
    assert!(is_chain(&join(&[&a, &b, &c]).unwrap(), &want, false));
}

#[test]
fn collinear_segments_are_kept() {
    let r = join(&[&line((0.0, 0.0), (1.0, 0.0)), &line((1.0, 0.0), (2.0, 0.0))]).unwrap();
    assert!(matches!(&r, Shape::Polyline { points, .. } if points.len() == 3));
}

#[test]
fn a_line_and_a_polyline() {
    let p = Shape::Polyline { points: vec![v2(5.0, 0.0), v2(5.0, 5.0), v2(0.0, 5.0)], closed: false };
    let r = join(&[&p, &line((0.0, 0.0), (5.0, 0.0))]).unwrap();
    assert!(is_chain(&r, &[v2(0.0, 0.0), v2(5.0, 0.0), v2(5.0, 5.0), v2(0.0, 5.0)], false), "{r:?}");
}

#[test]
fn a_chain_that_closes_gives_a_closed_polyline() {
    let r = join(&[&line((0.0, 0.0), (4.0, 0.0)), &line((4.0, 4.0), (4.0, 0.0)), &line((4.0, 4.0), (0.0, 0.0))]).unwrap();
    let Shape::Polyline { points, closed: true } = &r else { panic!("{r:?}") };
    assert_eq!(points.len(), 3);
}

#[test]
fn a_gap_within_tolerance_joins_and_a_larger_one_does_not() {
    assert!(join(&[&line((0.0, 0.0), (1.0, 0.0)), &line((1.0 + 5e-7, 0.0), (2.0, 0.0))]).is_ok());
    let r = join(&[&line((0.0, 0.0), (1.0, 0.0)), &line((1.001, 0.0), (2.0, 0.0))]);
    assert_eq!(r, Err(GeomError::Degenerate("shapes do not form one chain")));
}

#[test]
fn a_branch_is_an_error() {
    let r = join(&[&line((0.0, 0.0), (1.0, 0.0)), &line((1.0, 0.0), (2.0, 0.0)), &line((1.0, 0.0), (1.0, 1.0))]);
    assert_eq!(r, Err(GeomError::Degenerate("shapes do not form one chain")));
}

#[test]
fn other_shapes_are_unsupported() {
    let arc = Shape::Arc { center: DVec2::ZERO, radius: 1.0, start: 0.0, sweep: PI };
    let closed = Shape::Polyline { points: vec![v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, 1.0)], closed: true };
    for s in [arc, closed, Shape::rect(DVec2::ZERO, DVec2::ONE)] {
        let r = join(&[&line((0.0, 0.0), (1.0, 0.0)), &s]);
        assert_eq!(r, Err(GeomError::Unsupported("only lines and open polylines can be joined")));
    }
}
