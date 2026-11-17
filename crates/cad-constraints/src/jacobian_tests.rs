use crate::jacobian::Jacobian;
use crate::residual::{layout, residuals, tangent_modes};
use crate::residual_tests::{circle, line, map, pt, v};
use crate::types::{Constraint, PointKind};
use cad_geometry::Shape;
use nalgebra::DMatrix;
use std::collections::BTreeSet;

use PointKind::{Center, End, Start};

/// The old way: perturb every free variable and re-evaluate every equation.
fn full_numeric(eval: &dyn Fn(&[f64], &mut Vec<f64>), x: &[f64], m: usize) -> DMatrix<f64> {
    let mut j = DMatrix::zeros(m, x.len());
    let mut xs = x.to_vec();
    let (mut plus, mut minus) = (Vec::new(), Vec::new());
    for i in 0..x.len() {
        let h = 1e-7 * x[i].abs().max(1.0);
        xs[i] = x[i] + h;
        eval(&xs, &mut plus);
        xs[i] = x[i] - h;
        eval(&xs, &mut minus);
        xs[i] = x[i];
        for r in 0..m {
            j[(r, i)] = (plus[r] - minus[r]) / (2.0 * h);
        }
    }
    j
}

#[test]
fn restricted_jacobian_equals_full_numeric() {
    let arc = Shape::Arc { center: v(5.0, 5.0), radius: 3.0, start: 0.4, sweep: 1.2 };
    let shapes = map(vec![
        (1, line(0.0, 0.1, 4.0, 0.7)),
        (2, line(4.2, 0.5, 4.1, 5.0)),
        (3, line(-1.0, 6.0, 3.0, 7.5)),
        (4, circle(6.0, 2.0, 2.5)),
        (5, circle(9.0, 2.5, 1.5)),
        (6, arc),
    ]);
    let cs = vec![
        Constraint::Coincident(pt(1, End), pt(2, Start)),
        Constraint::Horizontal(1),
        Constraint::Vertical(2),
        Constraint::Perpendicular(2, 3),
        Constraint::Parallel(1, 3),
        Constraint::Tangent(3, 4),
        Constraint::Tangent(4, 5),
        Constraint::Distance(pt(2, Start), pt(3, End), 5.0),
        Constraint::Radius(5, 1.4),
        Constraint::Diameter(4, 5.0),
        Constraint::Equal(1, 3),
        Constraint::Equal(4, 6),
        Constraint::Symmetric(pt(1, Start), pt(3, End), 2),
        Constraint::PointOnCircle(pt(6, End), 4),
        Constraint::PointOnLine(pt(6, Start), 1),
        Constraint::Concentric(4, 6),
        Constraint::Collinear(1, 3),
        Constraint::Angle(1, 3, 0.3),
        Constraint::Fixed { point: pt(4, Center), at: v(6.0, 2.0) },
    ];
    let (l, full) = layout(&shapes, &cs).unwrap();
    let modes = tangent_modes(&l, &cs, &full);
    let pinned = l.pinned(&BTreeSet::from([5]));
    let free: Vec<usize> = pinned.iter().enumerate().filter(|(_, p)| !**p).map(|(i, _)| i).collect();
    assert!(free.len() < full.len());
    let expand = |z: &[f64]| {
        let mut all = full.clone();
        for (k, &i) in free.iter().enumerate() {
            all[i] = z[k];
        }
        all
    };
    let eval = |z: &[f64], out: &mut Vec<f64>| residuals(&l, &modes, &cs, &expand(z), out);
    let z: Vec<f64> = free.iter().map(|&i| full[i]).collect();
    let mut f = Vec::new();
    eval(&z, &mut f);
    let restricted = Jacobian::new(&l, &modes, &cs, &free, &full);
    assert_eq!(restricted.rows(), f.len());
    let (got, want) = (restricted.at(&z), full_numeric(&eval, &z, f.len()));
    assert_eq!(got.shape(), want.shape());
    assert!(want.iter().any(|v| v.abs() > 1e-3), "the reference must not be trivially zero");
    let worst = (&got - &want).iter().fold(0.0_f64, |m, d| m.max(d.abs()));
    assert!(worst <= 1e-12, "largest difference {worst:e}");
}
