use super::*;
use crate::annot::{fills, labels, strokes};
use crate::tol::Tolerance;
use crate::xform::{mirror_across, rotation_about};

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn close(a: DVec2, b: DVec2) -> bool {
    a.distance(b) < 1e-9
}
fn square() -> Vec<DVec2> {
    vec![v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0), v2(0.0, 10.0)]
}
fn ok(s: &Shape) -> bool {
    s.validate(&Tolerance::default()).is_ok()
}

#[test]
fn validation() {
    let hatch = |boundary: Vec<DVec2>, spacing: f64| Shape::Hatch { boundary, angle: 0.0, spacing };
    assert!(ok(&hatch(square(), 2.0)));
    assert!(!ok(&hatch(square()[..2].to_vec(), 2.0)));
    assert!(!ok(&hatch(vec![v2(0.0, 0.0), v2(0.0, 0.0), v2(5.0, 5.0)], 2.0)));
    assert!(!ok(&hatch(square(), 0.0)));
    assert!(matches!(
        hatch(square(), 0.001).validate(&Tolerance::default()),
        Err(crate::GeomError::Degenerate("hatch spacing too small"))
    ));
    assert!(!ok(&hatch(vec![v2(f64::NAN, 0.0), v2(1.0, 0.0), v2(0.0, 1.0)], 1.0)));
    let leader = |points: Vec<DVec2>, text: &str, h: f64| Shape::Leader { points, text: text.into(), text_height: h };
    assert!(ok(&leader(vec![v2(0.0, 0.0), v2(5.0, 5.0)], "a", 2.0)));
    assert!(!ok(&leader(vec![v2(0.0, 0.0)], "a", 2.0)));
    assert!(!ok(&leader(vec![v2(0.0, 0.0), v2(0.0, 0.0)], "a", 2.0)));
    assert!(!ok(&leader(vec![v2(0.0, 0.0), v2(5.0, 5.0)], "  ", 2.0)));
    assert!(!ok(&leader(vec![v2(0.0, 0.0), v2(5.0, 5.0)], "a", 0.0)));
    let radial = |radius: f64, h: f64| Shape::RadialDim { center: DVec2::ZERO, radius, angle: 0.0, text_height: h, diameter: false };
    assert!(ok(&radial(5.0, 2.0)) && !ok(&radial(0.0, 2.0)) && !ok(&radial(5.0, 0.0)));
    let ang = |sweep: f64, radius: f64| Shape::AngularDim { center: DVec2::ZERO, start: 0.0, sweep, radius, text_height: 2.0 };
    assert!(ok(&ang(1.0, 5.0)) && ok(&ang(TAU, 5.0)));
    assert!(!ok(&ang(0.0, 5.0)) && !ok(&ang(TAU + 0.1, 5.0)) && !ok(&ang(1.0, 0.0)));
}

#[test]
fn square_hatch_line_count() {
    // y = 0, 2, 4, 6, 8, 10: the half-open rule `(p.y <= y) != (q.y <= y)` keeps the bottom edge (y = 0)
    // and drops the top edge (y = 10), so there are 5 lines
    let lines = hatch_lines(&square(), 0.0, 2.0);
    assert_eq!(lines.len(), 5);
    for (k, [a, b]) in lines.iter().enumerate() {
        assert!(close(*a, v2(0.0, 2.0 * k as f64)) && close(*b, v2(10.0, 2.0 * k as f64)), "{a:?} {b:?}");
    }
}

#[test]
fn l_shape_and_u_shape_segments() {
    let l = [v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 4.0), v2(4.0, 4.0), v2(4.0, 10.0), v2(0.0, 10.0)];
    let lines = hatch_lines(&l, 0.0, 3.0);
    // y = 6 and y = 9 cross only the tall arm: one segment each
    let at = |y: f64| lines.iter().filter(|[a, _]| (a.y - y).abs() < 1e-9).count();
    assert_eq!((at(0.0), at(3.0), at(6.0), at(9.0)), (1, 1, 1, 1));
    // a notch cut from the top: y = 6 crosses both legs, so two segments on that line
    let u = [v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0), v2(7.0, 10.0), v2(7.0, 4.0), v2(3.0, 4.0), v2(3.0, 10.0), v2(0.0, 10.0)];
    let lines = hatch_lines(&u, 0.0, 3.0);
    let row: Vec<_> = lines.iter().filter(|[a, _]| (a.y - 6.0).abs() < 1e-9).collect();
    assert_eq!(row.len(), 2);
    assert!(close(row[0][0], v2(0.0, 6.0)) && close(row[0][1], v2(3.0, 6.0)));
    assert!(close(row[1][0], v2(7.0, 6.0)) && close(row[1][1], v2(10.0, 6.0)));
}

#[test]
fn rotated_hatch_is_vertical() {
    let lines = hatch_lines(&square(), FRAC_PI_2, 2.0);
    assert!(!lines.is_empty());
    for [a, b] in &lines {
        assert!((a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() > 1.0, "{a:?} {b:?}");
        assert!((a.x / 2.0 - (a.x / 2.0).round()).abs() < 1e-9, "aligned to the origin");
    }
}

#[test]
fn triangle_hatch_stays_inside() {
    let tri = [v2(0.0, 0.0), v2(10.0, 1.0), v2(3.0, 9.0)];
    let lines = hatch_lines(&tri, 0.6, 0.7);
    assert!(lines.len() > 5);
    let nudged = |p: DVec2, q: DVec2| p + (q - p) * 1e-6;
    for [a, b] in lines {
        assert!(inside_polygon(&tri, nudged(a, b)) && inside_polygon(&tri, nudged(b, a)));
        assert!(inside_polygon(&tri, (a + b) / 2.0));
    }
}

#[test]
fn leader_layout_both_directions() {
    let h = 2.0;
    let right = Shape::Leader { points: vec![v2(0.0, 0.0), v2(10.0, 10.0), v2(30.0, 10.0)], text: "N".into(), text_height: h };
    let f = fills(&right);
    assert_eq!(f.len(), 1);
    assert!(close(f[0][0], v2(0.0, 0.0)));
    let dir = v2(1.0, 1.0).normalize();
    assert!(close((f[0][1] + f[0][2]) / 2.0, dir * h));
    let l = &labels(&right)[0];
    assert!(l.align == Align::Left && close(l.anchor, v2(31.0, 10.0 - 0.7)) && l.angle == 0.0 && l.height == h);
    assert_eq!(l.text, LabelText::Literal("N".into()));
    assert_eq!(strokes(&right).len(), 1);
    let left = Shape::Leader { points: vec![v2(0.0, 0.0), v2(-10.0, 10.0), v2(-30.0, 10.0)], text: "N".into(), text_height: h };
    let l = &labels(&left)[0];
    assert!(l.align == Align::Right && close(l.anchor, v2(-31.0, 10.0 - 0.7)));
}

#[test]
fn radial_layout() {
    let mk = |angle: f64, diameter: bool| Shape::RadialDim { center: v2(1.0, 2.0), radius: 5.0, angle, text_height: 2.0, diameter };
    let r = mk(0.0, false);
    assert_eq!(strokes(&r), [vec![v2(1.0, 2.0), v2(6.0, 2.0)]]);
    assert!(close(fills(&r)[0][0], v2(6.0, 2.0)));
    let l = &labels(&r)[0];
    assert_eq!(l.text, LabelText::Radius(5.0));
    assert!(l.align == Align::Left && close(l.anchor, v2(8.0, 2.0 - 0.7)));
    let d = mk(PI, true);
    assert!(close(strokes(&d)[0][0], v2(6.0, 2.0)) && close(strokes(&d)[0][1], v2(-4.0, 2.0)));
    let l = &labels(&d)[0];
    assert_eq!(l.text, LabelText::Diameter(10.0));
    assert!(l.align == Align::Right && close(l.anchor, v2(-6.0, 2.0 - 0.7)));
}

#[test]
fn angular_layout() {
    let a = Shape::AngularDim { center: v2(1.0, 1.0), start: 0.0, sweep: FRAC_PI_2, radius: 10.0, text_height: 2.0 };
    let arc = &strokes(&a)[0];
    assert!(close(arc[0], v2(11.0, 1.0)) && close(*arc.last().unwrap(), v2(1.0, 11.0)));
    let f = fills(&a);
    assert!(close(f[0][0], v2(11.0, 1.0)) && close(f[1][0], v2(1.0, 11.0)));
    // the start arrow points along -Y (outward), its body lies towards +Y; the end arrow points along -X, its body lies towards +X
    assert!(((f[0][1] + f[0][2]) / 2.0 - v2(11.0, 1.0)).dot(DVec2::Y) > 1.9);
    assert!(((f[1][1] + f[1][2]) / 2.0 - v2(1.0, 11.0)).dot(DVec2::X) > 1.9);
    let l = &labels(&a)[0];
    let mid = FRAC_PI_4;
    assert!(close(l.anchor, v2(1.0, 1.0) + DVec2::from_angle(mid) * 11.8));
    assert_eq!(l.text, LabelText::Angle(FRAC_PI_2));
    assert!(l.align == Align::Center && (l.angle - (mid - FRAC_PI_2)).abs() < 1e-9);
    // on the left half the text is flipped to stay readable
    let b = Shape::AngularDim { center: DVec2::ZERO, start: FRAC_PI_2, sweep: FRAC_PI_2, radius: 10.0, text_height: 2.0 };
    let angle = labels(&b)[0].angle;
    assert!(angle > -FRAC_PI_2 && angle <= FRAC_PI_2 + 1e-9);
    assert!((angle - FRAC_PI_4).abs() < 1e-9, "{angle}");
}

fn samples() -> Vec<Shape> {
    vec![
        Shape::Hatch { boundary: square(), angle: 0.3, spacing: 2.0 },
        Shape::Leader { points: vec![v2(0.0, 0.0), v2(10.0, 10.0), v2(30.0, 10.0)], text: "Note".into(), text_height: 2.0 },
        Shape::RadialDim { center: v2(1.0, 2.0), radius: 5.0, angle: 1.0, text_height: 2.0, diameter: true },
        Shape::AngularDim { center: v2(1.0, 1.0), start: 0.5, sweep: 2.0, radius: 10.0, text_height: 2.0 },
    ]
}

#[test]
fn bbox_contains_everything_and_translate_moves_it() {
    for s in samples() {
        let (lo, hi) = s.bbox();
        let pts = strokes(&s).into_iter().flatten().chain(fills(&s).into_iter().flatten()).chain(labels(&s).into_iter().map(|l| l.anchor));
        for p in pts {
            assert!(p.x >= lo.x && p.y >= lo.y && p.x <= hi.x && p.y <= hi.y, "{} {p:?}", s.kind());
        }
        let mut t = s.clone();
        t.translate(glam::DVec3::new(3.0, -4.0, 0.0));
        let (lo2, _) = t.bbox();
        assert!(close(lo2.truncate(), lo.truncate() + v2(3.0, -4.0)));
        assert!(!s.is_solid());
    }
}

#[test]
fn kinds_and_snaps() {
    let kinds: Vec<_> = samples().iter().map(Shape::kind).collect();
    assert_eq!(kinds, ["Hatch", "Leader", "Radial dimension", "Angular dimension"]);
    let sn: Vec<_> = samples().iter().map(|s| crate::snap::snap_points(s).len()).collect();
    assert_eq!(sn, [4, 3, 1, 1]);
}

#[test]
fn distance_inside_hatch_is_zero() {
    let h = Shape::Hatch { boundary: square(), angle: 0.0, spacing: 2.5 };
    assert_eq!(crate::distance_2d(&h, v2(5.0, 5.1)), 0.0);
    assert!((crate::distance_2d(&h, v2(13.0, 5.0)) - 3.0).abs() < 1e-9);
}

#[test]
fn rotate_hatch_by_90() {
    let h = Shape::Hatch { boundary: square(), angle: 0.0, spacing: 2.0 };
    let Shape::Hatch { boundary, angle, spacing } = h.transformed(&rotation_about(DVec2::ZERO, FRAC_PI_2)).unwrap() else { panic!() };
    assert!(close(boundary[1], v2(0.0, 10.0)) && close(boundary[2], v2(-10.0, 10.0)));
    assert!((angle - FRAC_PI_2).abs() < 1e-9 && spacing == 2.0);
}

#[test]
fn mirrored_angular_dim_covers_the_mirrored_sector() {
    let a = Shape::AngularDim { center: DVec2::ZERO, start: 0.5, sweep: 1.0, radius: 5.0, text_height: 1.0 };
    let m = mirror_across(DVec2::ZERO, DVec2::X);
    let Shape::AngularDim { start, sweep, .. } = a.transformed(&m).unwrap() else { panic!() };
    assert_eq!(sweep, 1.0);
    // the sector 0.5..1.5 becomes -1.5..-0.5, that is 2*PI - 1.5 .. 2*PI - 0.5
    assert!((start - (TAU - 1.5)).abs() < 1e-9, "{start}");
    let Shape::AngularDim { start: s2, .. } = a.transformed(&rotation_about(DVec2::ZERO, 1.0)).unwrap() else { panic!() };
    assert!((s2 - 1.5).abs() < 1e-9);
}

#[test]
fn scale_and_radial_mirror() {
    let r = Shape::RadialDim { center: v2(1.0, 0.0), radius: 2.0, angle: 0.5, text_height: 1.0, diameter: false };
    let m = crate::xform::scale_about(DVec2::ZERO, 3.0) * mirror_across(DVec2::ZERO, DVec2::X);
    let Shape::RadialDim { center, radius, angle, text_height, .. } = r.transformed(&m).unwrap() else { panic!() };
    assert!(close(center, v2(3.0, 0.0)) && (radius - 6.0).abs() < 1e-9 && (angle + 0.5).abs() < 1e-9 && (text_height - 3.0).abs() < 1e-9);
}
