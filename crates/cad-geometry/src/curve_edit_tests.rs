use crate::curve::{FLATTEN_TOL, ellipse_point, flatten, spline_point};
use crate::edit::{split, trim};
use crate::pick::point_segment_distance;
use crate::snap::intersections;
use crate::{DVec2, Shape};
use std::f64::consts::{FRAC_PI_2, PI};

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn ellipse(c: DVec2, rx: f64, ry: f64, angle: f64) -> Shape {
    Shape::Ellipse { center: c, rx, ry, angle }
}
fn spline(pts: &[(f64, f64)]) -> Shape {
    Shape::Spline { points: pts.iter().map(|p| v2(p.0, p.1)).collect(), closed: false }
}
fn line(a: (f64, f64), b: (f64, f64)) -> Shape {
    Shape::Line { a: v2(a.0, a.1), b: v2(b.0, b.1) }
}
/// Largest distance of 20000 points of the true curve from the flattening.
fn worst_deviation(flat: &[DVec2], at: impl Fn(f64) -> DVec2) -> f64 {
    (0..20_000)
        .map(|i| at(f64::from(i) / 20_000.0))
        .map(|p| flat.windows(2).map(|w| point_segment_distance(p, w[0], w[1])).fold(f64::INFINITY, f64::min))
        .fold(0.0, f64::max)
}

#[test]
fn flatten_stays_within_the_tolerance() {
    let (c, angle) = (v2(1.0, 2.0), 0.5);
    let e = ellipse(c, 30.0, 5.0, angle);
    let worst = worst_deviation(&flatten(&e, FLATTEN_TOL), |s| ellipse_point(c, 30.0, 5.0, angle, 2.0 * PI * s));
    assert!(worst <= FLATTEN_TOL, "{worst}");
    let pts = [(0.0, 0.0), (10.0, 8.0), (20.0, -3.0), (35.0, 4.0), (40.0, 15.0)];
    let Shape::Spline { points, .. } = spline(&pts) else { unreachable!() };
    let worst = worst_deviation(&flatten(&spline(&pts), FLATTEN_TOL), |s| {
        let u = s * 4.0;
        let i = (u.floor() as usize).min(3);
        spline_point(&points, false, i, u - i as f64)
    });
    assert!(worst <= FLATTEN_TOL, "{worst}");
    assert!(flatten(&line((0.0, 0.0), (1.0, 0.0)), FLATTEN_TOL).is_empty());
}

#[test]
fn a_line_through_the_ellipse_centre_hits_the_analytic_points() {
    let (c, rx, ry, angle) = (v2(1.0, 2.0), 5.0, 2.0, 30f64.to_radians());
    let e = ellipse(c, rx, ry, angle);
    let dir = DVec2::from_angle(1.0);
    let hits = intersections(&e, &Shape::Line { a: c - dir * 20.0, b: c + dir * 20.0 });
    assert_eq!(hits.len(), 2);
    // distance from the centre along `dir` in the ellipse frame
    let local = DVec2::from_angle(-angle).rotate(dir);
    let r = 1.0 / ((local.x / rx).powi(2) + (local.y / ry).powi(2)).sqrt();
    for want in [c + dir * r, c - dir * r] {
        assert!(hits.iter().any(|h| h.distance(want) < FLATTEN_TOL), "{hits:?} {want}");
    }
}

#[test]
fn two_ellipses_cross_four_times_and_a_spline_crosses_a_line() {
    let a = ellipse(DVec2::ZERO, 4.0, 2.0, 0.0);
    let b = ellipse(DVec2::ZERO, 4.0, 2.0, FRAC_PI_2);
    assert_eq!(intersections(&a, &b).len(), 4);
    let s = spline(&[(0.0, 0.0), (10.0, 5.0), (20.0, 0.0)]);
    assert_eq!(intersections(&s, &line((10.0, -9.0), (10.0, 9.0))).len(), 1);
}

#[test]
fn trimming_an_ellipse_between_two_lines_leaves_an_open_polyline() {
    let e = ellipse(DVec2::ZERO, 5.0, 3.0, 0.0);
    let (l, r) = (line((-2.0, -9.0), (-2.0, 9.0)), line((2.0, -9.0), (2.0, 9.0)));
    let out = trim(&e, v2(0.0, 3.0), &[&l, &r]).unwrap();
    let [Shape::Polyline { points, closed: false }] = &out[..] else { panic!("{out:?}") };
    let y = 3.0 * (1.0 - 0.16f64).sqrt();
    assert!(points[0].distance(v2(-2.0, y)) < FLATTEN_TOL, "{}", points[0]);
    assert!(points[points.len() - 1].distance(v2(2.0, y)) < FLATTEN_TOL);
    // the part under the cutters stays: the polyline reaches the bottom of the ellipse
    assert!(points.iter().any(|p| p.y < -3.0 + 1e-3));
    assert!(points.iter().all(|p| p.y < y + 1e-3 || p.x.abs() > 2.0 - 1e-3));
}

#[test]
fn trim_and_split_a_spline() {
    let s = spline(&[(0.0, 0.0), (10.0, 5.0), (20.0, 0.0), (30.0, 5.0)]);
    let cut = line((15.0, -9.0), (15.0, 9.0));
    let out = trim(&s, v2(25.0, 4.0), &[&cut]).unwrap();
    let [Shape::Spline { points, closed: false }] = &out[..] else { panic!("{out:?}") };
    assert_eq!(points[0], v2(0.0, 0.0));
    assert!((points[points.len() - 1].x - 15.0).abs() < 1e-3);
    assert!(points.contains(&v2(10.0, 5.0)) && !points.contains(&v2(20.0, 0.0)));

    let parts = split(&s, v2(10.0, 5.2)).unwrap();
    let [Shape::Spline { points: a, .. }, Shape::Spline { points: b, .. }] = &parts[..] else { panic!("{parts:?}") };
    assert!(a[a.len() - 1].distance(v2(10.0, 5.0)) < 1e-3 && b[0] == a[a.len() - 1]);
    assert_eq!((a.len(), b.len()), (2, 3));
    assert!(split(&s, v2(-5.0, -5.0)).is_err());
}

#[test]
fn split_an_ellipse_gives_one_polyline_from_the_point_around() {
    let e = ellipse(DVec2::ZERO, 5.0, 3.0, 0.0);
    let out = split(&e, v2(0.0, 3.0)).unwrap();
    let [Shape::Polyline { points, closed: false }] = &out[..] else { panic!("{out:?}") };
    let (first, last) = (points[0], points[points.len() - 1]);
    assert!(first.distance(v2(0.0, 3.0)) < FLATTEN_TOL && first.distance(last) < 1e-9);
    assert!(points.len() > 100);
}
