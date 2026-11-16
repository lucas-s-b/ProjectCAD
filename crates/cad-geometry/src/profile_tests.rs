use super::*;
use std::f64::consts::{FRAC_PI_2, TAU};

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn line(a: (f64, f64), b: (f64, f64)) -> Shape {
    Shape::Line { a: v(a.0, a.1), b: v(b.0, b.1) }
}
fn prof(shapes: &[Shape]) -> Result<Profile, GeomError> {
    let refs: Vec<&Shape> = shapes.iter().collect();
    Profile::from_shapes(&refs)
}
fn closed(l: &Loop) {
    for i in 0..l.len() {
        let d = l[i].end().distance(l[(i + 1) % l.len()].start());
        assert!(d < 1e-9, "gap {d} at {i}");
    }
}
fn circle(c: (f64, f64), r: f64) -> Shape {
    Shape::Circle { center: v(c.0, c.1), radius: r }
}

#[test]
fn shuffled_rectangle() {
    let p = prof(&[line((40.0, 20.0), (0.0, 20.0)), line((0.0, 0.0), (40.0, 0.0)), line((0.0, 20.0), (0.0, 0.0)), line((40.0, 0.0), (40.0, 20.0))])
        .unwrap();
    assert_eq!(p.outer.len(), 4);
    assert!(p.holes.is_empty());
    closed(&p.outer);
    assert!(loop_area(&p.outer) > 0.0);
    assert!((p.area() - 800.0).abs() < 1e-9);
    assert!(p.centroid().distance(v(20.0, 10.0)) < 1e-9);
}

#[test]
fn rect_and_circle() {
    let p = prof(&[Shape::rect(v(0.0, 0.0), v(3.0, 2.0))]).unwrap();
    assert_eq!(p.outer.len(), 4);
    assert!((p.area() - 6.0).abs() < 1e-12);
    let r = 7.5;
    let p = prof(&[circle((1.0, -2.0), r)]).unwrap();
    assert_eq!(p.outer.len(), 2);
    closed(&p.outer);
    assert!((p.area() - PI * r * r).abs() < 1e-9);
    assert!(p.centroid().distance(v(1.0, -2.0)) < 1e-9);
}

#[test]
fn rounded_rectangle() {
    let (w, h, r) = (40.0, 20.0, 3.0);
    let arc = |cx: f64, cy: f64, start: f64| Shape::Arc { center: v(cx, cy), radius: r, start, sweep: FRAC_PI_2 };
    let p = prof(&[
        line((r, 0.0), (w - r, 0.0)),
        arc(w - r, r, -FRAC_PI_2),
        line((w, r), (w, h - r)),
        arc(w - r, h - r, 0.0),
        line((w - r, h), (r, h)),
        arc(r, h - r, FRAC_PI_2),
        line((0.0, h - r), (0.0, r)),
        arc(r, r, PI),
    ])
    .unwrap();
    assert_eq!(p.outer.len(), 8);
    closed(&p.outer);
    assert!((p.area() - (w * h - (4.0 - PI) * r * r)).abs() < 1e-9);
    assert!(p.centroid().distance(v(20.0, 10.0)) < 1e-9);
}

#[test]
fn rectangle_with_circular_hole() {
    let p = prof(&[circle((20.0, 10.0), 5.0), Shape::rect(v(0.0, 0.0), v(40.0, 20.0))]).unwrap();
    assert_eq!(p.holes.len(), 1);
    assert!(loop_area(&p.outer) > 0.0);
    assert!(loop_area(&p.holes[0]) < 0.0);
    closed(&p.holes[0]);
    assert!((p.area() - (800.0 - PI * 25.0)).abs() < 1e-9);
    assert!(p.centroid().distance(v(20.0, 10.0)) < 1e-9);
}

#[test]
fn invalid_profiles() {
    let closed_err = Err(GeomError::Degenerate("profile is not a set of closed loops"));
    let region_err = Err(GeomError::Degenerate("profile must be one region, with optional holes"));
    // open chain
    let open = [line((0.0, 0.0), (1.0, 0.0)), line((1.0, 0.0), (1.0, 1.0)), line((1.0, 1.0), (0.0, 1.0))];
    assert_eq!(prof(&open), closed_err);
    assert_eq!(prof(&[]), closed_err);
    // T junction: a spur ends on a corner
    let t = [Shape::rect(v(0.0, 0.0), v(2.0, 2.0)), line((0.0, 0.0), (-1.0, -1.0))];
    assert_eq!(prof(&t), closed_err);
    // two separate rectangles
    let two = [Shape::rect(v(0.0, 0.0), v(1.0, 1.0)), Shape::rect(v(5.0, 5.0), v(6.0, 6.0))];
    assert_eq!(prof(&two), region_err);
    // hole inside a hole
    let nested = [circle((0.0, 0.0), 10.0), circle((0.0, 0.0), 6.0), circle((0.0, 0.0), 3.0)];
    assert_eq!(prof(&nested), region_err);
    // unsupported shape
    let e = Shape::Ellipse { center: v(0.0, 0.0), rx: 2.0, ry: 1.0, angle: 0.0 };
    assert!(matches!(prof(&[e]), Err(GeomError::Unsupported(_))));
}

#[test]
fn long_arc_is_split() {
    let r = 10.0;
    let arc = Shape::Arc { center: v(0.0, 0.0), radius: r, start: 0.0, sweep: 1.5 * PI };
    let p = prof(&[arc, line((0.0, -r), (0.0, 0.0)), line((0.0, 0.0), (r, 0.0))]).unwrap();
    assert_eq!(p.outer.len(), 4);
    let sweeps: Vec<f64> = p.outer.iter().filter_map(|s| if let Seg::Arc { sweep, .. } = s { Some(*sweep) } else { None }).collect();
    assert_eq!(sweeps.len(), 2);
    assert!(sweeps.iter().all(|s| (s - 0.75 * PI).abs() < 1e-12));
    assert!((p.area() - 0.75 * PI * r * r).abs() < 1e-9);
    let full = Shape::Arc { center: v(0.0, 0.0), radius: 1.0, start: 0.3, sweep: TAU };
    assert_eq!(prof(&[full]).unwrap().outer.len(), 2);
}

#[test]
fn centroids() {
    let l = Shape::Polyline { points: vec![v(0.0, 0.0), v(2.0, 0.0), v(2.0, 1.0), v(1.0, 1.0), v(1.0, 2.0), v(0.0, 2.0)], closed: true };
    let p = prof(&[l]).unwrap();
    assert!((p.area() - 3.0).abs() < 1e-12);
    assert!(p.centroid().distance(v(5.0 / 6.0, 5.0 / 6.0)) < 1e-12);
    let r = 3.0;
    let half = Shape::Arc { center: v(0.0, 0.0), radius: r, start: 0.0, sweep: PI };
    let p = prof(&[half, line((-r, 0.0), (r, 0.0))]).unwrap();
    assert!((p.area() - PI * r * r / 2.0).abs() < 1e-12);
    assert!(p.centroid().distance(v(0.0, 4.0 * r / (3.0 * PI))) < 1e-12);
}

#[test]
fn point_in_loop_cases() {
    let disc = prof(&[circle((1.0, 1.0), 2.0)]).unwrap().outer;
    assert!(point_in_loop(&disc, v(1.0, 1.0)));
    assert!(point_in_loop(&disc, v(2.9, 1.0)));
    assert!(point_in_loop(&disc, v(1.0, 2.9)));
    assert!(point_in_loop(&disc, v(2.0, 2.0))); // inside the disc, outside its bounding polygon of chords
    assert!(!point_in_loop(&disc, v(3.1, 1.0)));
    assert!(!point_in_loop(&disc, v(2.8, 2.8)));
    assert!(!point_in_loop(&disc, v(-1.5, 1.0)));
    assert!(!point_in_loop(&disc, v(1.0, 3.1)));
    // C shape (concave)
    let c = Shape::Polyline {
        points: vec![v(0.0, 0.0), v(4.0, 0.0), v(4.0, 1.0), v(1.0, 1.0), v(1.0, 3.0), v(4.0, 3.0), v(4.0, 4.0), v(0.0, 4.0)],
        closed: true,
    };
    let c = prof(&[c]).unwrap().outer;
    assert!(point_in_loop(&c, v(0.5, 2.0)));
    assert!(point_in_loop(&c, v(3.0, 0.5)));
    assert!(point_in_loop(&c, v(3.0, 3.5)));
    assert!(!point_in_loop(&c, v(3.0, 2.0))); // in the notch
    assert!(!point_in_loop(&c, v(-1.0, 1.0)));
    assert!(!point_in_loop(&c, v(5.0, 1.0)));
    // ray through a vertex: y = 1 passes the notch corners
    assert!(!point_in_loop(&c, v(2.0, 1.0 + 1e-12)));
}

fn path(shapes: &[Shape]) -> Result<(Vec<Seg>, bool), GeomError> {
    let refs: Vec<&Shape> = shapes.iter().collect();
    chain(&refs)
}

fn joined(segs: &[Seg]) {
    for w in segs.windows(2) {
        assert!(w[0].end().distance(w[1].start()) < 1e-9, "gap between {:?} and {:?}", w[0], w[1]);
    }
}

#[test]
fn chain_joins_shuffled_and_reversed_lines() {
    let (segs, closed) = path(&[line((20.0, 10.0), (10.0, 10.0)), line((0.0, 0.0), (10.0, 0.0)), line((10.0, 10.0), (10.0, 0.0))]).unwrap();
    assert!(!closed);
    assert_eq!(segs.len(), 3);
    joined(&segs);
    let ends = [segs[0].start(), segs[2].end()];
    assert!(ends.contains(&v(0.0, 0.0)) && ends.contains(&v(20.0, 10.0)), "{ends:?}");
}

#[test]
fn chain_line_arc_line() {
    let arc = Shape::Arc { center: v(10.0, 5.0), radius: 5.0, start: -FRAC_PI_2, sweep: FRAC_PI_2 };
    let (segs, closed) = path(&[line((15.0, 15.0), (15.0, 5.0)), arc, line((0.0, 0.0), (10.0, 0.0))]).unwrap();
    assert!(!closed);
    joined(&segs);
    assert!(matches!(segs[1], Seg::Arc { .. }));
    assert!(segs[0].start().distance(v(15.0, 15.0)) < 1e-9 || segs[0].start().distance(v(0.0, 0.0)) < 1e-9);
}

#[test]
fn chain_closed_shapes_and_rectangle_of_lines() {
    let sq = [line((0.0, 0.0), (4.0, 0.0)), line((4.0, 4.0), (4.0, 0.0)), line((4.0, 4.0), (0.0, 4.0)), line((0.0, 0.0), (0.0, 4.0))];
    let (segs, closed) = path(&sq).unwrap();
    assert!(closed);
    assert_eq!(segs.len(), 4);
    joined(&segs);
    assert!(segs[3].end().distance(segs[0].start()) < 1e-9);
    let (segs, closed) = path(&[circle((0.0, 0.0), 2.0)]).unwrap();
    assert!(closed && segs.len() == 2);
    joined(&segs);
    assert!(path(&[Shape::rect(v(0.0, 0.0), v(1.0, 1.0))]).unwrap().1);
}

#[test]
fn chain_errors() {
    let two = path(&[line((0.0, 0.0), (1.0, 0.0)), line((5.0, 5.0), (6.0, 5.0))]);
    assert_eq!(two, Err(GeomError::Degenerate("path must be one connected chain")));
    // The spur ends on the middle point of a two-line path: three ends meet there.
    let t = path(&[line((0.0, 0.0), (1.0, 0.0)), line((1.0, 0.0), (2.0, 0.0)), line((1.0, 0.0), (1.0, 1.0))]);
    assert_eq!(t, Err(GeomError::Degenerate("path branches: more than two ends meet at a point")));
    assert!(matches!(path(&[]), Err(GeomError::Degenerate(_))));
    let e = Shape::Ellipse { center: v(0.0, 0.0), rx: 2.0, ry: 1.0, angle: 0.0 };
    assert!(matches!(path(&[e]), Err(GeomError::Unsupported(_))));
}
