use super::*;

fn tol() -> Tolerance {
    Tolerance::default()
}
fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn v3(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

#[test]
fn accepts_good_shapes() {
    let good = [
        Shape::Line { a: v2(0.0, 0.0), b: v2(1.0, 0.0) },
        Shape::Circle { center: v2(0.0, 0.0), radius: 1.0 },
        Shape::rect(v2(0.0, 0.0), v2(2.0, 3.0)),
        Shape::Box { center: v3(0.0, 0.0, 0.0), size: v3(1.0, 1.0, 1.0) },
        Shape::Cylinder { center: v3(0.0, 0.0, 0.0), radius: 1.0, height: 2.0 },
        Shape::Sphere { center: v3(0.0, 0.0, 0.0), radius: 1.0 },
    ];
    for s in good {
        assert_eq!(s.validate(&tol()), Ok(()), "{s:?}");
    }
}

#[test]
fn rejects_bad_shapes() {
    let nan = Shape::Circle { center: v2(f64::NAN, 0.0), radius: 1.0 };
    assert_eq!(nan.validate(&tol()), Err(GeomError::NonFinite));
    let bad = [
        Shape::Line { a: v2(1.0, 1.0), b: v2(1.0, 1.0) },
        Shape::Circle { center: v2(0.0, 0.0), radius: 0.0 },
        Shape::Circle { center: v2(0.0, 0.0), radius: -1.0 },
        Shape::Sphere { center: v3(0.0, 0.0, 0.0), radius: -2.0 },
        Shape::Cylinder { center: v3(0.0, 0.0, 0.0), radius: 1.0, height: 0.0 },
        Shape::Rect { min: v2(1.0, 1.0), max: v2(0.0, 2.0) },
        Shape::Box { center: v3(0.0, 0.0, 0.0), size: v3(1.0, 0.0, 1.0) },
    ];
    for s in bad {
        assert!(matches!(s.validate(&tol()), Err(GeomError::Degenerate(_))), "{s:?}");
    }
}

#[test]
fn arc_through_quarter_and_full() {
    let Shape::Arc { radius, start, sweep, .. } = Shape::arc_through(v2(0.0, 0.0), v2(2.0, 0.0), v2(0.0, 5.0)) else {
        panic!()
    };
    assert!((radius - 2.0).abs() < 1e-12 && start.abs() < 1e-12 && (sweep - FRAC_PI_2).abs() < 1e-12);
    let Shape::Arc { sweep, .. } = Shape::arc_through(v2(0.0, 0.0), v2(2.0, 0.0), v2(2.0, 0.0)) else { panic!() };
    assert_eq!(sweep, TAU);
}

#[test]
fn validate_arc_and_polyline() {
    let arc = |sweep| Shape::Arc { center: v2(0.0, 0.0), radius: 1.0, start: 0.0, sweep };
    assert_eq!(arc(1.0).validate(&tol()), Ok(()));
    assert!(arc(0.0).validate(&tol()).is_err());
    assert!(arc(TAU + 0.1).validate(&tol()).is_err());
    let pl = |pts: &[(f64, f64)], closed| Shape::Polyline { points: pts.iter().map(|p| v2(p.0, p.1)).collect(), closed };
    assert_eq!(pl(&[(0.0, 0.0), (1.0, 0.0)], false).validate(&tol()), Ok(()));
    assert!(pl(&[(0.0, 0.0)], false).validate(&tol()).is_err());
    assert!(pl(&[(0.0, 0.0), (1.0, 0.0)], true).validate(&tol()).is_err());
    assert!(pl(&[(0.0, 0.0), (0.0, 0.0), (1.0, 1.0)], false).validate(&tol()).is_err());
    assert!(pl(&[(0.0, 0.0), (1.0, 0.0), (0.0, 0.0)], true).validate(&tol()).is_err());
}

#[test]
fn arc_bbox() {
    let q = Shape::Arc { center: v2(0.0, 0.0), radius: 1.0, start: 0.0, sweep: FRAC_PI_2 };
    let (lo, hi) = q.bbox();
    assert!(lo.distance(v3(0.0, 0.0, 0.0)) < 1e-9 && hi.distance(v3(1.0, 1.0, 0.0)) < 1e-9);
    let c = Shape::Arc { center: v2(0.0, 0.0), radius: 1.0, start: TAU - 0.5, sweep: 1.0 };
    let (lo, hi) = c.bbox();
    assert!((hi.x - 1.0).abs() < 1e-9 && (lo.y + 0.5f64.sin()).abs() < 1e-9 && (hi.y - 0.5f64.sin()).abs() < 1e-9);
}

#[test]
fn rect_sorts_corners() {
    assert_eq!(
        Shape::rect(v2(3.0, -1.0), v2(1.0, 2.0)),
        Shape::Rect { min: v2(1.0, -1.0), max: v2(3.0, 2.0) }
    );
}

#[test]
fn translate_then_bbox() {
    let mut s = Shape::Circle { center: v2(1.0, 1.0), radius: 2.0 };
    s.translate(v3(1.0, 2.0, 5.0));
    assert_eq!(s.bbox(), (v3(0.0, 1.0, 0.0), v3(4.0, 5.0, 0.0)));
    let mut b = Shape::Box { center: v3(0.0, 0.0, 0.0), size: v3(2.0, 2.0, 2.0) };
    b.translate(v3(1.0, 2.0, 3.0));
    assert_eq!(b.bbox(), (v3(0.0, 1.0, 2.0), v3(2.0, 3.0, 4.0)));
}

#[test]
fn bbox_of_each_shape() {
    let l = Shape::Line { a: v2(2.0, -1.0), b: v2(-1.0, 3.0) };
    assert_eq!(l.bbox(), (v3(-1.0, -1.0, 0.0), v3(2.0, 3.0, 0.0)));
    let r = Shape::rect(v2(0.0, 0.0), v2(2.0, 1.0));
    assert_eq!(r.bbox(), (v3(0.0, 0.0, 0.0), v3(2.0, 1.0, 0.0)));
    let c = Shape::Cylinder { center: v3(0.0, 0.0, 5.0), radius: 2.0, height: 4.0 };
    assert_eq!(c.bbox(), (v3(-2.0, -2.0, 3.0), v3(2.0, 2.0, 7.0)));
    let s = Shape::Sphere { center: v3(1.0, 1.0, 1.0), radius: 1.0 };
    assert_eq!(s.bbox(), (v3(0.0, 0.0, 0.0), v3(2.0, 2.0, 2.0)));
    assert!(s.is_solid() && !l.is_solid());
    assert_eq!(c.kind(), "Cylinder");
}


#[test]
fn tiny_negative_angle_wraps_to_zero() {
    assert_eq!(norm_angle(-1e-17), 0.0);
    assert!((norm_angle(-FRAC_PI_2) - 3.0 * FRAC_PI_2).abs() < 1e-12);
}


fn text(s: &str, height: f64) -> Shape {
    Shape::Text { pos: v2(0.0, 0.0), height, angle: 0.0, text: s.into() }
}

#[test]
fn validate_text_and_dimension() {
    let t = Tolerance::default();
    assert_eq!(text("a", 1.0).validate(&t), Ok(()));
    assert_eq!(text("  ", 1.0).validate(&t), Err(GeomError::Degenerate("empty text")));
    assert!(text("a", 0.0).validate(&t).is_err());
    let dim = |b, th| Shape::Dimension { a: v2(0.0, 0.0), b, offset: 1.0, text_height: th };
    assert_eq!(dim(v2(5.0, 0.0), 1.0).validate(&t), Ok(()));
    assert!(dim(v2(0.0, 0.0), 1.0).validate(&t).is_err());
    assert!(dim(v2(5.0, 0.0), 0.0).validate(&t).is_err());
    assert!(!text("a", 1.0).is_solid());
    assert_eq!(text("a", 1.0).kind(), "Text");
    assert_eq!(dim(v2(5.0, 0.0), 1.0).kind(), "Dimension");
}

#[test]
fn bbox_contains_layout() {
    let d = Shape::Dimension { a: v2(0.0, 0.0), b: v2(100.0, 0.0), offset: 20.0, text_height: 4.0 };
    let (lo, hi) = d.bbox();
    let l = dim_layout(v2(0.0, 0.0), v2(100.0, 0.0), 20.0, 4.0);
    let pts = l.ext.iter().flatten().chain(&l.line).chain(l.arrows.iter().flatten());
    for p in pts.chain([&l.text_center]) {
        assert!(p.x >= lo.x - 1e-9 && p.x <= hi.x + 1e-9 && p.y >= lo.y - 1e-9 && p.y <= hi.y + 1e-9, "{p:?}");
    }
    let (lo, hi) = text("abc", 10.0).bbox();
    assert_eq!((lo, hi), (DVec3::ZERO, DVec3::new(18.0, 10.0, 0.0)));
}

#[test]
fn translate_moves_anchors() {
    let mut d = Shape::Dimension { a: v2(0.0, 0.0), b: v2(1.0, 0.0), offset: 1.0, text_height: 1.0 };
    d.translate(DVec3::new(1.0, 2.0, 9.0));
    assert_eq!(d, Shape::Dimension { a: v2(1.0, 2.0), b: v2(2.0, 2.0), offset: 1.0, text_height: 1.0 });
    let mut t = text("a", 1.0);
    t.translate(DVec3::new(1.0, 2.0, 9.0));
    assert!(matches!(t, Shape::Text { pos, .. } if pos == v2(1.0, 2.0)));
}
