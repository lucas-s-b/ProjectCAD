use crate::curve::ellipse_point;
use crate::edit::{extend, offset, trim};
use crate::snap::{snap_points, SnapKind};
use crate::xform::{rotation_about, scale_about};
use crate::{DVec2, GeomError, Shape, Tolerance, outline_paths};
use std::f64::consts::{FRAC_PI_2, PI};

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn ellipse(rx: f64, ry: f64, angle: f64) -> Shape {
    Shape::Ellipse { center: v2(1.0, 2.0), rx, ry, angle }
}
fn spline(pts: &[(f64, f64)], closed: bool) -> Shape {
    Shape::Spline { points: pts.iter().map(|p| v2(p.0, p.1)).collect(), closed }
}

#[test]
fn validation() {
    let t = Tolerance::default();
    assert_eq!(ellipse(3.0, 1.0, 0.5).validate(&t), Ok(()));
    assert!(matches!(ellipse(0.0, 1.0, 0.0).validate(&t), Err(GeomError::Degenerate(_))));
    assert!(matches!(ellipse(1.0, -1.0, 0.0).validate(&t), Err(GeomError::Degenerate(_))));
    assert_eq!(ellipse(1.0, 1.0, f64::NAN).validate(&t), Err(GeomError::NonFinite));
    assert_eq!(spline(&[(0.0, 0.0), (1.0, 0.0)], false).validate(&t), Ok(()));
    assert!(spline(&[(0.0, 0.0)], false).validate(&t).is_err());
    assert!(spline(&[(0.0, 0.0), (1.0, 0.0)], true).validate(&t).is_err());
    assert_eq!(spline(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)], true).validate(&t), Ok(()));
    assert!(spline(&[(0.0, 0.0), (0.0, 0.0), (1.0, 1.0)], false).validate(&t).is_err());
    assert!(spline(&[(0.0, 0.0), (1.0, 0.0), (0.0, 0.0)], true).validate(&t).is_err());
    assert_eq!(spline(&[(0.0, 0.0), (f64::INFINITY, 0.0)], false).validate(&t), Err(GeomError::NonFinite));
    assert_eq!((ellipse(1.0, 1.0, 0.0).kind(), spline(&[], false).kind()), ("Ellipse", "Spline"));
    assert!(!ellipse(1.0, 1.0, 0.0).is_solid());
}

#[test]
fn ellipse_bbox_is_exact_when_rotated() {
    let angle = 30f64.to_radians();
    let e = ellipse(5.0, 2.0, angle);
    let (lo, hi) = e.bbox();
    let (mut slo, mut shi) = (DVec2::INFINITY, DVec2::NEG_INFINITY);
    for i in 0..100_000 {
        let p = ellipse_point(v2(1.0, 2.0), 5.0, 2.0, angle, PI * 2.0 * f64::from(i) / 100_000.0);
        slo = slo.min(p);
        shi = shi.max(p);
    }
    assert!(lo.truncate().distance(slo) < 1e-6 && hi.truncate().distance(shi) < 1e-6, "{lo} {hi} {slo} {shi}");
}

#[test]
fn spline_bbox_contains_points_and_translate_moves_them() {
    let mut s = spline(&[(0.0, 0.0), (10.0, 5.0), (20.0, 0.0)], false);
    let (lo, hi) = s.bbox();
    assert!(lo.x <= 0.0 && hi.x >= 20.0 && hi.y >= 5.0);
    s.translate(glam::DVec3::new(1.0, 1.0, 0.0));
    assert_eq!(s, spline(&[(1.0, 1.0), (11.0, 6.0), (21.0, 1.0)], false));
}

#[test]
fn outline_segment_counts() {
    assert_eq!(outline_paths(&ellipse(2.0, 1.0, 0.0))[0].len(), 97);
    assert_eq!(outline_paths(&spline(&[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)], false))[0].len(), 2 * 16 + 1);
    let closed = &outline_paths(&spline(&[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)], true))[0];
    assert_eq!((closed.len(), closed[0]), (3 * 16 + 1, closed[3 * 16]));
}

#[test]
fn transformed_ellipse() {
    let e = Shape::Ellipse { center: v2(2.0, 0.0), rx: 3.0, ry: 1.0, angle: 0.0 };
    let m = scale_about(DVec2::ZERO, 2.0) * rotation_about(DVec2::ZERO, FRAC_PI_2);
    let Shape::Ellipse { center, rx, ry, angle } = e.transformed(&m).unwrap() else { panic!() };
    assert!(center.distance(v2(0.0, 4.0)) < 1e-9);
    assert!((rx - 6.0).abs() < 1e-9 && (ry - 2.0).abs() < 1e-9 && (angle - FRAC_PI_2).abs() < 1e-9);
    let sp = spline(&[(1.0, 0.0), (2.0, 0.0)], false).transformed(&rotation_about(DVec2::ZERO, FRAC_PI_2)).unwrap();
    let Shape::Spline { points, .. } = sp else { panic!() };
    assert!(points[0].distance(v2(0.0, 1.0)) < 1e-9 && points[1].distance(v2(0.0, 2.0)) < 1e-9);
}

#[test]
fn snap_point_counts() {
    let e = snap_points(&ellipse(2.0, 1.0, 0.0));
    assert_eq!((e.len(), e.iter().filter(|s| s.kind == SnapKind::Center).count()), (5, 1));
    assert!(e.iter().any(|s| s.point.distance(v2(3.0, 2.0)) < 1e-9));
    assert_eq!(snap_points(&spline(&[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)], false)).len(), 3);
}

#[test]
fn edits_are_unsupported_for_ellipse_and_spline() {
    let cutter = Shape::Line { a: v2(-9.0, -9.0), b: v2(9.0, 9.0) };
    for s in [ellipse(2.0, 1.0, 0.0), spline(&[(0.0, 0.0), (1.0, 1.0)], false)] {
        let want = GeomError::Unsupported("not supported for this shape yet");
        assert_eq!(trim(&s, v2(0.0, 0.0), &[&cutter]), Err(want.clone()));
        assert_eq!(extend(&s, v2(0.0, 0.0), &[&cutter]), Err(want.clone()));
        assert_eq!(offset(&s, 1.0, v2(0.0, 0.0)), Err(want));
    }
    let text = Shape::Text { pos: DVec2::ZERO, height: 1.0, angle: 0.0, text: "a".into() };
    assert_eq!(trim(&text, DVec2::ZERO, &[]), Err(GeomError::Unsupported("this shape is not a curve")));
}
