use crate::residual::{layout, residuals, tangent_modes};
use crate::residual_tests::{circle, line, map, pt, v};
use crate::solve::{Solution, Status, solve};
use crate::types::{Constraint, Id, PointKind, SolveError};
use crate::{Triplet, least_squares};
use cad_geometry::{DVec2, Shape};
use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_3};

use PointKind::{Center, End, Start};

type Shapes = BTreeMap<Id, Shape>;

fn run(start: Vec<(Id, Shape)>, cs: &[Constraint]) -> (Solution, Shapes) {
    let start = map(start);
    let sol = solve(&start, cs).expect("solve");
    let mut now = start;
    now.extend(sol.shapes.clone());
    (sol, now)
}

/// Largest residual of `now`, with tangent modes decided from `start` like the solver does.
fn max_residual(start: &Shapes, now: &Shapes, cs: &[Constraint]) -> f64 {
    let (l0, x0) = layout(start, cs).unwrap();
    let modes = tangent_modes(&l0, cs, &x0);
    let (l, x) = layout(now, cs).unwrap();
    let mut out = Vec::new();
    residuals(&l, &modes, cs, &x, &mut out);
    out.iter().fold(0.0, |m, r| m.max(r.abs()))
}

fn ends(s: &Shape) -> (DVec2, DVec2) {
    match s {
        Shape::Line { a, b } => (*a, *b),
        other => panic!("not a line: {other:?}"),
    }
}

fn centre(s: &Shape) -> (DVec2, f64) {
    match s {
        Shape::Circle { center, radius } | Shape::Arc { center, radius, .. } => (*center, *radius),
        other => panic!("not a curve: {other:?}"),
    }
}

fn close(a: DVec2, b: DVec2) {
    assert!(a.distance(b) < 1e-6, "{a} vs {b}");
}

#[test]
fn every_constraint_alone_is_satisfied() {
    let lines = |b: Shape| vec![(1, line(0.0, 0.0, 10.0, 0.0)), (2, b)];
    let circles = |b: Shape| vec![(1, circle(0.0, 0.0, 5.0)), (2, b)];
    let cases = vec![
        ("Coincident", lines(line(10.5, 1.0, 20.0, 3.0)), Constraint::Coincident(pt(1, End), pt(2, Start))),
        ("Horizontal", vec![(1, line(0.0, 0.0, 10.0, 3.0))], Constraint::Horizontal(1)),
        ("Vertical", vec![(1, line(0.0, 0.0, 3.0, 10.0))], Constraint::Vertical(1)),
        ("Parallel", lines(line(0.0, 5.0, 10.0, 9.0)), Constraint::Parallel(1, 2)),
        ("Perpendicular", lines(line(5.0, 0.0, 7.0, 9.0)), Constraint::Perpendicular(1, 2)),
        ("Tangent line", vec![(1, line(-5.0, 0.0, 5.0, 0.0)), (2, circle(0.0, 4.0, 3.0))], Constraint::Tangent(1, 2)),
        ("Tangent circles", circles(circle(9.0, 1.0, 2.0)), Constraint::Tangent(1, 2)),
        ("Concentric", circles(circle(2.0, 3.0, 2.0)), Constraint::Concentric(1, 2)),
        ("Equal lines", lines(line(0.0, 5.0, 6.0, 5.0)), Constraint::Equal(1, 2)),
        ("Equal circles", circles(circle(9.0, 0.0, 2.0)), Constraint::Equal(1, 2)),
        ("Collinear", lines(line(12.0, 1.0, 20.0, 3.0)), Constraint::Collinear(1, 2)),
        (
            "Symmetric",
            vec![(1, circle(-2.0, 1.0, 1.0)), (2, circle(3.0, 2.0, 1.0)), (3, line(0.0, -5.0, 0.0, 5.0))],
            Constraint::Symmetric(pt(1, Center), pt(2, Center), 3),
        ),
        ("Fixed", vec![(1, circle(3.0, 4.0, 1.0))], Constraint::Fixed { point: pt(1, Center), at: v(5.0, -2.0) }),
        ("Distance", circles(circle(3.0, 4.0, 1.0)), Constraint::Distance(pt(1, Center), pt(2, Center), 12.0)),
        ("Angle", lines(line(0.0, 0.0, 5.0, 2.0)), Constraint::Angle(1, 2, FRAC_PI_3)),
        ("Radius", vec![(1, circle(0.0, 0.0, 5.0))], Constraint::Radius(1, 8.0)),
        ("Diameter", vec![(1, circle(0.0, 0.0, 5.0))], Constraint::Diameter(1, 3.0)),
    ];
    for (name, start, c) in cases {
        let cs = [c];
        let (sol, now) = run(start.clone(), &cs);
        assert!(sol.iterations > 0, "{name}");
        let r = max_residual(&map(start), &now, &cs);
        assert!(r < 1e-6, "{name}: residual {r}");
    }
}

fn rectangle_start() -> Vec<(Id, Shape)> {
    vec![
        (1, line(0.5, 0.3, 39.0, -1.0)),
        (2, line(41.0, 1.0, 39.0, 18.0)),
        (3, line(40.5, 21.0, 1.0, 19.0)),
        (4, line(-1.0, 20.5, 0.5, 1.0)),
    ]
}

#[test]
fn rectangle_is_fully_constrained() {
    let cs = [
        Constraint::Coincident(pt(1, End), pt(2, Start)),
        Constraint::Coincident(pt(2, End), pt(3, Start)),
        Constraint::Coincident(pt(3, End), pt(4, Start)),
        Constraint::Coincident(pt(4, End), pt(1, Start)),
        Constraint::Horizontal(1),
        Constraint::Horizontal(3),
        Constraint::Vertical(2),
        Constraint::Vertical(4),
        Constraint::Fixed { point: pt(1, Start), at: v(0.0, 0.0) },
        Constraint::Distance(pt(1, Start), pt(1, End), 40.0),
        Constraint::Distance(pt(2, Start), pt(2, End), 20.0),
    ];
    let (sol, now) = run(rectangle_start(), &cs);
    assert_eq!((sol.dof, sol.redundant), (0, 0));
    assert_eq!(sol.status(), Status::FullyConstrained);
    close(ends(&now[&1]).0, v(0.0, 0.0));
    close(ends(&now[&1]).1, v(40.0, 0.0));
    close(ends(&now[&2]).1, v(40.0, 20.0));
    close(ends(&now[&3]).1, v(0.0, 20.0));
    close(ends(&now[&4]).1, v(0.0, 0.0));
}

#[test]
fn triangle_keeps_three_degrees_of_freedom() {
    let start = vec![(1, line(0.0, 0.0, 28.0, 2.0)), (2, line(29.0, 1.0, 31.0, 38.0)), (3, line(30.0, 41.0, 1.0, 1.0))];
    let cs = [
        Constraint::Coincident(pt(1, End), pt(2, Start)),
        Constraint::Coincident(pt(2, End), pt(3, Start)),
        Constraint::Coincident(pt(3, End), pt(1, Start)),
        Constraint::Distance(pt(1, Start), pt(1, End), 30.0),
        Constraint::Distance(pt(2, Start), pt(2, End), 40.0),
        Constraint::Distance(pt(3, Start), pt(3, End), 50.0),
    ];
    let (sol, now) = run(start, &cs);
    for (id, len) in [(1, 30.0), (2, 40.0), (3, 50.0)] {
        let (a, b) = ends(&now[&id]);
        assert!((a.distance(b) - len).abs() < 1e-6, "side {id}");
    }
    close(ends(&now[&1]).1, ends(&now[&2]).0);
    assert_eq!((sol.dof, sol.redundant), (3, 0));
    assert_eq!(sol.status(), Status::UnderConstrained);
}

#[test]
fn duplicate_constraint_is_redundant() {
    let (sol, now) = run(vec![(1, line(0.0, 0.0, 10.0, 4.0))], &[Constraint::Horizontal(1), Constraint::Horizontal(1)]);
    assert_eq!(sol.redundant, 1);
    assert_eq!(sol.status(), Status::OverConstrained);
    assert!((ends(&now[&1]).0.y - ends(&now[&1]).1.y).abs() < 1e-6);
}

#[test]
fn contradicting_constraints_do_not_converge() {
    let shapes = map(vec![(1, circle(0.0, 0.0, 1.0)), (2, circle(15.0, 0.0, 1.0))]);
    let cs = [
        Constraint::Distance(pt(1, Center), pt(2, Center), 10.0),
        Constraint::Distance(pt(1, Center), pt(2, Center), 20.0),
    ];
    assert!(matches!(solve(&shapes, &cs), Err(SolveError::NotConverged { .. })));
}

#[test]
fn bad_reference_is_reported() {
    let shapes = map(vec![(1, circle(0.0, 0.0, 1.0))]);
    let r = solve(&shapes, &[Constraint::Horizontal(1)]);
    assert_eq!(r, Err(SolveError::BadReference { index: 0, reason: "wrong shape kind" }));
}

#[test]
fn line_tangent_to_circle() {
    let (_, now) = run(vec![(1, line(-5.0, 0.0, 5.0, 0.0)), (2, circle(0.0, 4.0, 3.0))], &[Constraint::Tangent(1, 2)]);
    let (a, b) = ends(&now[&1]);
    let (c, r) = centre(&now[&2]);
    let d = b - a;
    assert!(((c - a).perp_dot(d).abs() / d.length() - r).abs() < 1e-6);
}

#[test]
fn circles_tangent_externally_and_internally() {
    let cs = [Constraint::Tangent(1, 2)];
    let (_, now) = run(vec![(1, circle(0.0, 0.0, 2.0)), (2, circle(6.0, 0.0, 3.0))], &cs);
    let ((c1, r1), (c2, r2)) = (centre(&now[&1]), centre(&now[&2]));
    assert!((c1.distance(c2) - (r1 + r2)).abs() < 1e-6);
    let (_, now) = run(vec![(1, circle(0.0, 0.0, 5.0)), (2, circle(1.0, 0.0, 2.0))], &cs);
    let ((c1, r1), (c2, r2)) = (centre(&now[&1]), centre(&now[&2]));
    assert!((c1.distance(c2) - (r1 - r2).abs()).abs() < 1e-6);
    assert!(c1.distance(c2) < r1.max(r2), "still internal");
}

#[test]
fn angle_of_sixty_degrees() {
    let (_, now) = run(vec![(1, line(0.0, 0.0, 10.0, 0.0)), (2, line(0.0, 0.0, 5.0, 2.0))], &[Constraint::Angle(1, 2, FRAC_PI_3)]);
    let (d1, d2) = (ends(&now[&1]), ends(&now[&2]));
    let (u, w) = (d1.1 - d1.0, d2.1 - d2.0);
    assert!((u.perp_dot(w).atan2(u.dot(w)) - FRAC_PI_3).abs() < 1e-6);
}

#[test]
fn symmetric_points_about_vertical_line() {
    let start = vec![(1, circle(-2.0, 1.0, 1.0)), (2, circle(3.0, 2.0, 1.0)), (3, line(0.0, -5.0, 0.0, 5.0))];
    // the mirror line is pinned, otherwise the solver may move it instead of the points
    let cs = [
        Constraint::Symmetric(pt(1, Center), pt(2, Center), 3),
        Constraint::Fixed { point: pt(3, Start), at: v(0.0, -5.0) },
        Constraint::Fixed { point: pt(3, End), at: v(0.0, 5.0) },
    ];
    let (_, now) = run(start, &cs);
    let (p, q) = (centre(&now[&1]).0, centre(&now[&2]).0);
    assert!((p.x + q.x).abs() < 1e-6 && (p.y - q.y).abs() < 1e-6);
}

#[test]
fn arc_end_meets_line_start() {
    let arc = Shape::Arc { center: v(0.0, 0.0), radius: 5.0, start: 0.0, sweep: FRAC_PI_2 };
    let (_, now) = run(vec![(1, arc), (2, line(0.5, 5.5, 4.0, 6.0))], &[Constraint::Coincident(pt(1, End), pt(2, Start))]);
    let Shape::Arc { center, radius, start, sweep } = now[&1] else { panic!() };
    let end = center + radius * DVec2::from_angle(start + sweep);
    close(end, ends(&now[&2]).0);
}

#[test]
fn unreferenced_shapes_are_left_alone() {
    let start = vec![(1, line(0.0, 0.0, 10.0, 3.0)), (7, circle(1.0, 1.0, 1.0))];
    let (sol, _) = run(start, &[Constraint::Horizontal(1)]);
    assert_eq!(sol.shapes.keys().copied().collect::<Vec<_>>(), [1]);
}

#[test]
fn satisfied_system_returns_immediately() {
    let shapes = map(vec![(1, line(0.0, 0.0, 10.0, 0.0))]);
    let sol = solve(&shapes, &[Constraint::Horizontal(1), Constraint::Fixed { point: pt(1, Start), at: v(0.0, 0.0) }]).unwrap();
    assert_eq!(sol.iterations, 0);
    assert!(sol.shapes.is_empty());
    assert_eq!(sol.dof, 1);
}

#[test]
fn no_constraints_gives_empty_solution() {
    let sol = solve(&map(vec![(1, circle(0.0, 0.0, 1.0))]), &[]).unwrap();
    assert_eq!((sol.dof, sol.redundant, sol.iterations, sol.shapes.len()), (0, 0, 0, 0));
}

#[test]
fn fixed_point_stays_put() {
    let start = vec![(1, line(1.0, 2.0, 9.0, 7.0))];
    let cs = [
        Constraint::Fixed { point: pt(1, Start), at: v(1.0, 2.0) },
        Constraint::Horizontal(1),
        Constraint::Distance(pt(1, Start), pt(1, End), 10.0),
    ];
    let (sol, now) = run(start, &cs);
    assert!(sol.iterations > 0);
    assert!(ends(&now[&1]).0.distance(v(1.0, 2.0)) < 1e-9);
    close(ends(&now[&1]).1, v(11.0, 2.0));
    assert_eq!(sol.status(), Status::FullyConstrained);
}

#[test]
fn least_squares_meets_two_equations() {
    let residuals = |x: &[f64], out: &mut Vec<f64>| {
        out.clear();
        out.extend([x[0] * x[0] + x[1] * x[1] - 25.0, x[0] - x[1] - 1.0]);
    };
    let jacobian = |x: &[f64]| vec![Triplet::new(0, 0, 2.0 * x[0]), Triplet::new(0, 1, 2.0 * x[1]), Triplet::new(1, 0, 1.0), Triplet::new(1, 1, -1.0)];
    let fit = least_squares(vec![4.5, 3.2], residuals, jacobian).unwrap();
    assert!((fit.x[0] - 4.0).abs() <= 1e-9 && (fit.x[1] - 3.0).abs() <= 1e-9);
    assert_eq!(fit.rank, 2);
    assert!(fit.iterations >= 1);
}

#[test]
fn least_squares_rank_and_failure() {
    let one = |x: &[f64], out: &mut Vec<f64>| {
        out.clear();
        out.push(x[0] + x[1] - 2.0);
    };
    let fit = least_squares(vec![0.0, 0.0], one, |_| vec![Triplet::new(0, 0, 1.0), Triplet::new(0, 1, 1.0)]).unwrap();
    assert!((fit.x[0] + fit.x[1] - 2.0).abs() <= 1e-9);
    assert_eq!(fit.rank, 1);
    let two = |x: &[f64], out: &mut Vec<f64>| {
        out.clear();
        out.extend([x[0] - 1.0, x[0] - 2.0]);
    };
    let r = least_squares(vec![0.0], two, |_| vec![Triplet::new(0, 0, 1.0), Triplet::new(1, 0, 1.0)]);
    assert!(matches!(r, Err(SolveError::NotConverged { residual }) if (residual - 0.5).abs() < 1e-6));
    let fit = least_squares(vec![1.0], |_, out: &mut Vec<f64>| out.clear(), |_| Vec::new()).unwrap();
    assert_eq!(fit.x, vec![1.0]);
    assert_eq!(fit.rank, 0);
    assert_eq!(fit.iterations, 0);
}
