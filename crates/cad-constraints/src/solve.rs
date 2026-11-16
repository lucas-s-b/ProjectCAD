use crate::residual::{layout, residuals, tangent_modes, write_back};
use crate::types::{Constraint, Id, SolveError};
use cad_geometry::Shape;
use cad_geometry::shape::norm_angle;
use nalgebra::{DMatrix, DVector};
use std::collections::BTreeMap;

const TOL: f64 = 1e-9;
const MAX_ITERATIONS: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct Solution {
    /// Only shapes whose values changed.
    pub shapes: BTreeMap<Id, Shape>,
    /// Remaining degrees of freedom of the constrained entities.
    pub dof: usize,
    /// Equations that add nothing (over-constrained but consistent).
    pub redundant: usize,
    pub iterations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    UnderConstrained,
    FullyConstrained,
    OverConstrained,
}

impl Solution {
    pub fn status(&self) -> Status {
        if self.redundant > 0 {
            Status::OverConstrained
        } else if self.dof == 0 {
            Status::FullyConstrained
        } else {
            Status::UnderConstrained
        }
    }
}

fn max_abs(f: &[f64]) -> f64 {
    if f.iter().any(|v| !v.is_finite()) {
        return f64::INFINITY;
    }
    f.iter().fold(0.0, |m, v| m.max(v.abs()))
}

fn sum_sq(f: &[f64]) -> f64 {
    f.iter().map(|v| v * v).sum()
}

// shortcut: dense numeric Jacobian, O(n*m) evaluations per iteration; go analytic and sparse when the parametric-update benchmark needs it
fn jacobian(eval: &dyn Fn(&[f64], &mut Vec<f64>), x: &[f64], m: usize) -> DMatrix<f64> {
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

fn numbers(s: &Shape) -> Vec<f64> {
    match s {
        Shape::Line { a, b } => vec![a.x, a.y, b.x, b.y],
        Shape::Circle { center, radius } => vec![center.x, center.y, *radius],
        Shape::Arc { center, radius, start, sweep } => vec![center.x, center.y, *radius, norm_angle(*start), *sweep],
        _ => Vec::new(),
    }
}

fn differs(a: &Shape, b: &Shape) -> bool {
    let (na, nb) = (numbers(a), numbers(b));
    na.len() != nb.len() || na.iter().zip(&nb).any(|(p, q)| (p - q).abs() > 1e-12)
}

/// Levenberg-Marquardt on the constraint equations. Only entities that a constraint references can move.
pub fn solve(shapes: &BTreeMap<Id, Shape>, constraints: &[Constraint]) -> Result<Solution, SolveError> {
    let (lay, mut x) = layout(shapes, constraints)?;
    if constraints.is_empty() {
        return Ok(Solution { shapes: BTreeMap::new(), dof: 0, redundant: 0, iterations: 0 });
    }
    let modes = tangent_modes(&lay, constraints, &x);
    let eval = |x: &[f64], out: &mut Vec<f64>| residuals(&lay, &modes, constraints, x, out);
    let n = lay.len();
    let mut f = Vec::new();
    eval(&x, &mut f);
    let m = f.len();
    let (mut lambda, mut iterations) = (1e-3, 0);
    let mut jac: Option<DMatrix<f64>> = None;
    let mut trial = Vec::new();
    loop {
        let err = max_abs(&f);
        if err <= TOL {
            break;
        }
        if iterations >= MAX_ITERATIONS || lambda > 1e10 {
            return Err(SolveError::NotConverged { residual: err });
        }
        iterations += 1;
        let j = jac.get_or_insert_with(|| jacobian(&eval, &x, m));
        let jt = j.transpose();
        let mut a = &jt * &*j;
        for i in 0..n {
            a[(i, i)] += lambda;
        }
        let Some(chol) = a.cholesky() else {
            lambda *= 10.0;
            continue;
        };
        let delta = chol.solve(&(-(&jt * DVector::from_column_slice(&f))));
        let x_new: Vec<f64> = x.iter().zip(delta.iter()).map(|(p, d)| p + d).collect();
        eval(&x_new, &mut trial);
        if x_new.iter().all(|v| v.is_finite()) && trial.iter().all(|v| v.is_finite()) && sum_sq(&trial) < sum_sq(&f) {
            x = x_new;
            std::mem::swap(&mut f, &mut trial);
            jac = None;
            lambda = (lambda / 10.0).max(1e-12);
        } else {
            lambda *= 10.0;
        }
    }
    let j = jac.unwrap_or_else(|| jacobian(&eval, &x, m));
    let sv = j.svd(false, false).singular_values;
    let limit = 1e-6 * sv.iter().fold(1.0_f64, |a, b| a.max(*b));
    let rank = sv.iter().filter(|s| **s > limit).count();
    let changed = write_back(&lay, &x, shapes)
        .into_iter()
        .filter(|(id, s)| differs(s, &shapes[id]))
        .collect();
    Ok(Solution { shapes: changed, dof: n - rank, redundant: m - rank, iterations })
}
