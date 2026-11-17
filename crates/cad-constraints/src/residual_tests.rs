use crate::residual::{layout, residuals, tangent_modes, write_back};
use crate::types::{Constraint, Id, PointKind, PointRef, SolveError};
use cad_geometry::{DVec2, Shape};
use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

pub(crate) fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
pub(crate) fn line(ax: f64, ay: f64, bx: f64, by: f64) -> Shape {
    Shape::Line { a: v(ax, ay), b: v(bx, by) }
}
pub(crate) fn circle(cx: f64, cy: f64, r: f64) -> Shape {
    Shape::Circle { center: v(cx, cy), radius: r }
}
pub(crate) fn pt(entity: Id, kind: PointKind) -> PointRef {
    PointRef { entity, kind }
}
pub(crate) fn map(shapes: Vec<(Id, Shape)>) -> BTreeMap<Id, Shape> {
    shapes.into_iter().collect()
}

fn res(shapes: Vec<(Id, Shape)>, c: Constraint) -> Vec<f64> {
    let cs = [c];
    let (l, x) = layout(&map(shapes), &cs).unwrap();
    let modes = tangent_modes(&l, &cs, &x);
    let mut out = Vec::new();
    residuals(&l, &modes, &cs, &x, &mut out);
    out
}

fn near(got: Vec<f64>, want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?} vs {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 1e-12, "{got:?} vs {want:?}");
    }
}

use PointKind::{Center, End, Start};

#[test]
fn coincident() {
    let s = || vec![(1, line(0.0, 0.0, 1.0, 0.0)), (2, line(1.0, 0.0, 2.0, 0.0))];
    near(res(s(), Constraint::Coincident(pt(1, End), pt(2, Start))), &[0.0, 0.0]);
    near(res(s(), Constraint::Coincident(pt(1, End), pt(2, End))), &[-1.0, 0.0]);
}

#[test]
fn horizontal_and_vertical() {
    near(res(vec![(1, line(0.0, 0.0, 1.0, 0.0))], Constraint::Horizontal(1)), &[0.0]);
    near(res(vec![(1, line(0.0, 0.0, 1.0, 1.0))], Constraint::Horizontal(1)), &[-1.0]);
    near(res(vec![(1, line(0.0, 0.0, 0.0, 1.0))], Constraint::Vertical(1)), &[0.0]);
    near(res(vec![(1, line(0.0, 0.0, 1.0, 1.0))], Constraint::Vertical(1)), &[-1.0]);
}

#[test]
fn parallel_and_perpendicular() {
    let par = |b: Shape| res(vec![(1, line(0.0, 0.0, 2.0, 0.0)), (2, b)], Constraint::Parallel(1, 2));
    near(par(line(0.0, 1.0, 4.0, 1.0)), &[0.0]);
    near(par(line(0.0, 0.0, 0.0, 3.0)), &[1.0]);
    let perp = |b: Shape| res(vec![(1, line(0.0, 0.0, 1.0, 0.0)), (2, b)], Constraint::Perpendicular(1, 2));
    near(perp(line(5.0, 0.0, 5.0, 2.0)), &[0.0]);
    near(perp(line(0.0, 1.0, 3.0, 1.0)), &[1.0]);
}

#[test]
fn tangent_line_circle_either_order() {
    let l = line(-5.0, 0.0, 5.0, 0.0);
    near(res(vec![(1, l.clone()), (2, circle(0.0, 3.0, 3.0))], Constraint::Tangent(1, 2)), &[0.0]);
    near(res(vec![(1, l.clone()), (2, circle(0.0, 3.0, 2.0))], Constraint::Tangent(1, 2)), &[1.0]);
    near(res(vec![(1, l), (2, circle(0.0, 3.0, 3.0))], Constraint::Tangent(2, 1)), &[0.0]);
}

#[test]
fn tangent_circles_external_and_internal() {
    let t = |a: Shape, b: Shape| res(vec![(1, a), (2, b)], Constraint::Tangent(1, 2));
    near(t(circle(0.0, 0.0, 2.0), circle(5.0, 0.0, 3.0)), &[0.0]);
    near(t(circle(0.0, 0.0, 2.0), circle(5.0, 0.0, 2.0)), &[1.0]);
    // one centre inside the other circle: internal mode
    near(t(circle(0.0, 0.0, 5.0), circle(1.0, 0.0, 4.0)), &[0.0]);
    near(t(circle(0.0, 0.0, 5.0), circle(2.0, 0.0, 4.0)), &[1.0]);
}

#[test]
fn tangent_modes_follow_start_values() {
    let cs = [Constraint::Tangent(1, 2), Constraint::Horizontal(3)];
    let shapes = map(vec![(1, circle(0.0, 0.0, 5.0)), (2, circle(1.0, 0.0, 4.0)), (3, line(0.0, 0.0, 1.0, 0.0))]);
    let (l, x) = layout(&shapes, &cs).unwrap();
    assert_eq!(tangent_modes(&l, &cs, &x), [true, false]);
}

#[test]
fn concentric_and_equal() {
    let two = |b: Shape| vec![(1, circle(0.0, 0.0, 2.0)), (2, b)];
    near(res(two(circle(0.0, 0.0, 9.0)), Constraint::Concentric(1, 2)), &[0.0, 0.0]);
    near(res(two(circle(3.0, 4.0, 9.0)), Constraint::Concentric(1, 2)), &[-3.0, -4.0]);
    near(res(two(circle(0.0, 0.0, 2.0)), Constraint::Equal(1, 2)), &[0.0]);
    near(res(two(circle(0.0, 0.0, 3.0)), Constraint::Equal(1, 2)), &[-1.0]);
    let lines = |b: Shape| vec![(1, line(0.0, 0.0, 3.0, 4.0)), (2, b)];
    near(res(lines(line(0.0, 0.0, 5.0, 0.0)), Constraint::Equal(1, 2)), &[0.0]);
    near(res(lines(line(0.0, 0.0, 2.0, 0.0)), Constraint::Equal(1, 2)), &[3.0]);
}

#[test]
fn collinear() {
    let c = |b: Shape| res(vec![(1, line(0.0, 0.0, 1.0, 0.0)), (2, b)], Constraint::Collinear(1, 2));
    near(c(line(2.0, 0.0, 5.0, 0.0)), &[0.0, 0.0]);
    near(c(line(2.0, 1.0, 5.0, 1.0)), &[0.0, -1.0]);
}

#[test]
fn symmetric_about_vertical_line() {
    let s = |q: Shape| {
        let shapes = vec![(1, circle(-2.0, 1.0, 1.0)), (2, q), (3, line(0.0, -5.0, 0.0, 5.0))];
        res(shapes, Constraint::Symmetric(pt(1, Center), pt(2, Center), 3))
    };
    near(s(circle(2.0, 1.0, 1.0)), &[0.0, 0.0]);
    near(s(circle(2.0, 3.0, 1.0)), &[0.0, 2.0]);
    near(s(circle(3.0, 1.0, 1.0)), &[0.5, 0.0]);
}

#[test]
fn fixed_and_distance() {
    let at = |x, y| Constraint::Fixed { point: pt(1, Center), at: v(x, y) };
    near(res(vec![(1, circle(3.0, 4.0, 1.0))], at(3.0, 4.0)), &[0.0, 0.0]);
    near(res(vec![(1, circle(3.0, 4.0, 1.0))], at(1.0, 1.0)), &[2.0, 3.0]);
    let two = vec![(1, circle(0.0, 0.0, 1.0)), (2, circle(3.0, 4.0, 1.0))];
    near(res(two.clone(), Constraint::Distance(pt(1, Center), pt(2, Center), 5.0)), &[0.0]);
    near(res(two, Constraint::Distance(pt(1, Center), pt(2, Center), 7.0)), &[-2.0]);
}

#[test]
fn angle_wraps_into_half_open_interval() {
    let two = || vec![(1, line(0.0, 0.0, 1.0, 0.0)), (2, line(0.0, 0.0, 0.0, 1.0))];
    near(res(two(), Constraint::Angle(1, 2, FRAC_PI_2)), &[0.0]);
    near(res(two(), Constraint::Angle(1, 2, 0.5)), &[FRAC_PI_2 - 0.5]);
    near(res(two(), Constraint::Angle(1, 2, -3.0 * FRAC_PI_2)), &[0.0]);
    let flat = vec![(1, line(0.0, 0.0, 1.0, 0.0)), (2, line(0.0, 0.0, -1.0, 0.0))];
    near(res(flat, Constraint::Angle(1, 2, -PI)), &[0.0]);
}

#[test]
fn radius_and_diameter() {
    near(res(vec![(1, circle(0.0, 0.0, 2.0))], Constraint::Radius(1, 2.0)), &[0.0]);
    near(res(vec![(1, circle(0.0, 0.0, 2.0))], Constraint::Radius(1, 3.0)), &[-1.0]);
    near(res(vec![(1, circle(0.0, 0.0, 2.0))], Constraint::Diameter(1, 4.0)), &[0.0]);
    near(res(vec![(1, circle(0.0, 0.0, 2.0))], Constraint::Diameter(1, 6.0)), &[-2.0]);
}

#[test]
fn layout_orders_by_id_and_skips_unreferenced() {
    let arc = Shape::Arc { center: v(1.0, 2.0), radius: 3.0, start: 0.5, sweep: 1.5 };
    let shapes = map(vec![(9, arc), (5, line(1.0, 2.0, 3.0, 4.0)), (2, circle(7.0, 8.0, 9.0)), (7, circle(0.0, 0.0, 1.0))]);
    let cs = [Constraint::Horizontal(5), Constraint::Equal(9, 2)];
    let (l, x) = layout(&shapes, &cs).unwrap();
    assert_eq!(l.len(), 12);
    assert_eq!(x, [7.0, 8.0, 9.0, 1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 3.0, 0.5, 1.5]);
    assert_eq!(write_back(&l, &x, &shapes).keys().copied().collect::<Vec<_>>(), [2, 5, 9]);
}

#[test]
fn arc_points_and_write_back() {
    let arc = Shape::Arc { center: v(1.0, 1.0), radius: 2.0, start: 0.0, sweep: FRAC_PI_2 };
    let shapes = map(vec![(1, arc), (2, line(3.0, 1.0, 0.0, 0.0)), (3, line(1.0, 3.0, 0.0, 0.0))]);
    let cs = [Constraint::Coincident(pt(1, Start), pt(2, Start)), Constraint::Coincident(pt(1, End), pt(3, Start))];
    let (l, x) = layout(&shapes, &cs).unwrap();
    let mut out = Vec::new();
    residuals(&l, &[false, false], &cs, &x, &mut out);
    near(out, &[0.0, 0.0, 0.0, 0.0]);
    assert_eq!(write_back(&l, &x, &shapes), shapes);
    // a start angle outside [0, TAU) is normalised on the way out
    let mut x2 = x.clone();
    x2[3] = -0.5;
    let Shape::Arc { start, .. } = write_back(&l, &x2, &shapes)[&1] else { panic!() };
    assert!((start - (TAU - 0.5)).abs() < 1e-12);
}

fn reason(shapes: Vec<(Id, Shape)>, c: Constraint) -> &'static str {
    match layout(&map(shapes), &[c]) {
        Err(SolveError::BadReference { index: 0, reason }) => reason,
        other => panic!("expected BadReference, got {:?}", other.map(|(_, x)| x)),
    }
}

#[test]
fn bad_references_name_their_reason() {
    let l = || (1, line(0.0, 0.0, 1.0, 0.0));
    let l2 = || (2, line(0.0, 1.0, 1.0, 1.0));
    let c = || (3, circle(0.0, 0.0, 1.0));
    let c4 = || (4, circle(1.0, 0.0, 1.0));
    assert_eq!(reason(vec![], Constraint::Horizontal(1)), "entity not found");
    assert_eq!(reason(vec![(1, Shape::rect(v(0.0, 0.0), v(1.0, 1.0)))], Constraint::Horizontal(1)), "shape cannot be constrained");
    assert_eq!(reason(vec![c()], Constraint::Horizontal(3)), "wrong shape kind");
    assert_eq!(reason(vec![l()], Constraint::Radius(1, 1.0)), "wrong shape kind");
    assert_eq!(reason(vec![l(), l2()], Constraint::Tangent(1, 2)), "wrong shape kind");
    assert_eq!(reason(vec![l(), c()], Constraint::Concentric(1, 3)), "wrong shape kind");
    assert_eq!(reason(vec![l(), c()], Constraint::Equal(1, 3)), "wrong shape kind");
    assert_eq!(reason(vec![l()], Constraint::Fixed { point: pt(1, Center), at: v(0.0, 0.0) }), "no such point");
    assert_eq!(reason(vec![c(), l()], Constraint::Coincident(pt(3, Start), pt(1, Start))), "no such point");
    assert_eq!(reason(vec![l()], Constraint::Parallel(1, 1)), "same entity twice");
    assert_eq!(reason(vec![c()], Constraint::Concentric(3, 3)), "same entity twice");
    assert_eq!(reason(vec![l(), l2()], Constraint::Angle(2, 2, 1.0)), "same entity twice");
    assert_eq!(reason(vec![l()], Constraint::Coincident(pt(1, Start), pt(1, Start))), "same entity twice");
    assert_eq!(reason(vec![c(), c4()], Constraint::Distance(pt(3, Center), pt(3, Center), 1.0)), "same entity twice");
    assert_eq!(reason(vec![c(), c4()], Constraint::Distance(pt(3, Center), pt(4, Center), 0.0)), "bad value");
    assert_eq!(reason(vec![c(), c4()], Constraint::Distance(pt(3, Center), pt(4, Center), -2.0)), "bad value");
    assert_eq!(reason(vec![c()], Constraint::Radius(3, f64::NAN)), "bad value");
    assert_eq!(reason(vec![c()], Constraint::Diameter(3, 0.0)), "bad value");
    assert_eq!(reason(vec![l(), l2()], Constraint::Angle(1, 2, f64::INFINITY)), "bad value");
    assert_eq!(reason(vec![c()], Constraint::Fixed { point: pt(3, Center), at: v(f64::NAN, 0.0) }), "bad value");
}

#[test]
fn bad_reference_reports_constraint_index() {
    let shapes = map(vec![(1, line(0.0, 0.0, 1.0, 0.0))]);
    let r = layout(&shapes, &[Constraint::Horizontal(1), Constraint::Vertical(8)]);
    assert!(matches!(r, Err(SolveError::BadReference { index: 1, reason: "entity not found" })));
}

#[test]
fn point_on_line_and_circle() {
    let on_line = |p: (f64, f64)| {
        let shapes = vec![(1, line(0.0, 0.0, 2.0, 0.0)), (2, circle(p.0, p.1, 1.0))];
        res(shapes, Constraint::PointOnLine(pt(2, Center), 1))
    };
    near(on_line((1.0, 0.0)), &[0.0]);
    near(on_line((1.0, 3.0)), &[-3.0]);
    let on_circle = |kind| res(vec![(1, circle(0.0, 0.0, 5.0)), (2, line(3.0, 4.0, 6.0, 8.0))], Constraint::PointOnCircle(pt(2, kind), 1));
    near(on_circle(Start), &[0.0]);
    near(on_circle(End), &[5.0]);
}

#[test]
fn point_on_curve_references_are_checked() {
    let l = || (1, line(0.0, 0.0, 1.0, 0.0));
    let c = || (3, circle(0.0, 0.0, 1.0));
    assert_eq!(reason(vec![l()], Constraint::PointOnLine(pt(1, Start), 1)), "same entity twice");
    assert_eq!(reason(vec![c()], Constraint::PointOnLine(pt(3, Center), 3)), "wrong shape kind");
    assert_eq!(reason(vec![c()], Constraint::PointOnCircle(pt(3, Center), 3)), "same entity twice");
    assert_eq!(reason(vec![l(), c()], Constraint::PointOnCircle(pt(3, Center), 1)), "wrong shape kind");
    assert_eq!(reason(vec![l(), c()], Constraint::PointOnLine(pt(1, Center), 1)), "no such point");
    assert_eq!(Constraint::PointOnLine(pt(2, Start), 1).entities(), [2, 1]);
    assert_eq!(Constraint::PointOnCircle(pt(2, Start), 1).name(), "Point on circle");
}
