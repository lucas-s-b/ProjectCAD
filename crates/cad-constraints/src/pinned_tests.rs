use crate::residual_tests::{circle, line, map, pt, v};
use crate::solve::{Status, solve, solve_pinned};
use crate::types::{Constraint, PointKind, SolveError};
use cad_geometry::Shape;
use std::collections::BTreeSet;

use PointKind::{Center, End, Start};

fn pinned(ids: &[u64]) -> BTreeSet<u64> {
    ids.iter().copied().collect()
}

fn ends(s: &Shape) -> [f64; 4] {
    let Shape::Line { a, b } = s else { panic!("{s:?}") };
    [a.x, a.y, b.x, b.y]
}

#[test]
fn point_on_line_and_on_circle_alone() {
    let cases = [
        (vec![(1, line(0.0, 0.0, 10.0, 0.0)), (2, line(3.0, 4.0, 8.0, 6.0))], Constraint::PointOnLine(pt(2, Start), 1)),
        (vec![(1, circle(0.0, 0.0, 5.0)), (2, line(3.0, 5.0, 9.0, 9.0))], Constraint::PointOnCircle(pt(2, Start), 1)),
    ];
    for (start, c) in cases {
        let shapes = map(start);
        let sol = solve(&shapes, std::slice::from_ref(&c)).unwrap();
        let mut now = shapes.clone();
        now.extend(sol.shapes);
        // the other entity may move too: check the geometry of the result, not fixed numbers
        let (p, other) = (ends(&now[&2]), &now[&1]);
        let miss = match (c, other) {
            (Constraint::PointOnLine(..), Shape::Line { a, b }) => (b - a).perp_dot(v(p[0], p[1]) - a) / (b - a).length(),
            (_, Shape::Circle { center, radius }) => v(p[0], p[1]).distance(*center) - radius,
            _ => unreachable!(),
        };
        assert!(miss.abs() < 1e-6, "{miss}");
    }
}

#[test]
fn a_line_end_on_a_fixed_circle() {
    let cs = [
        Constraint::Fixed { point: pt(1, Center), at: v(0.0, 0.0) },
        Constraint::Radius(1, 5.0),
        Constraint::Fixed { point: pt(2, Start), at: v(10.0, 0.0) },
        Constraint::Horizontal(2),
        Constraint::PointOnCircle(pt(2, End), 1),
    ];
    let shapes = map(vec![(1, circle(0.2, -0.1, 4.0)), (2, line(10.2, 0.3, 5.5, 0.4))]);
    let sol = solve(&shapes, &cs).unwrap();
    assert_eq!((sol.dof, sol.redundant, sol.status()), (0, 0, Status::FullyConstrained));
    let p = ends(&sol.shapes[&2]);
    assert!((p[2] - 5.0).abs() < 1e-6 && p[3].abs() < 1e-6 && (p[0] - 10.0).abs() < 1e-6 && p[1].abs() < 1e-6, "{p:?}");
}

/// Two horizontal lines joined end to start; line 1 has been moved up by 5 and the constraints are broken.
fn moved() -> (std::collections::BTreeMap<u64, Shape>, [Constraint; 4]) {
    let cs = [
        Constraint::Coincident(pt(1, End), pt(2, Start)),
        Constraint::Horizontal(1),
        Constraint::Horizontal(2),
        Constraint::Distance(pt(2, Start), pt(2, End), 10.0),
    ];
    (map(vec![(1, line(0.0, 5.0, 10.0, 5.0)), (2, line(10.0, 0.0, 20.0, 0.0))]), cs)
}

#[test]
fn a_pinned_line_does_not_move_and_the_other_follows() {
    let (shapes, cs) = moved();
    let sol = solve_pinned(&shapes, &cs, &pinned(&[1])).unwrap();
    assert!(!sol.shapes.contains_key(&1));
    let p = ends(&sol.shapes[&2]);
    assert!((p[0] - 10.0).abs() < 1e-6 && (p[1] - 5.0).abs() < 1e-6 && (p[3] - 5.0).abs() < 1e-6 && (p[2] - 20.0).abs() < 1e-6, "{p:?}");
    // without pins both lines give way
    assert!(solve(&shapes, &cs).unwrap().shapes.contains_key(&1));
    assert_eq!(solve_pinned(&shapes, &cs, &BTreeSet::new()), solve(&shapes, &cs));
}

#[test]
fn degrees_of_freedom_count_only_free_variables() {
    let (shapes, cs) = moved();
    // pinned: the 4 variables of line 2 minus 3 independent equations (start and horizontal); free: 8 variables minus 4 equations
    let sol = solve_pinned(&shapes, &cs[..3], &pinned(&[1])).unwrap();
    assert_eq!(sol.dof, 1);
    assert_eq!(solve(&shapes, &cs[..3]).unwrap().dof, 4);
}

#[test]
fn pinning_everything_with_broken_constraints_does_not_converge() {
    let (shapes, cs) = moved();
    let r = solve_pinned(&shapes, &cs, &pinned(&[1, 2]));
    assert!(matches!(r, Err(SolveError::NotConverged { residual }) if residual > 1.0), "{r:?}");
}
