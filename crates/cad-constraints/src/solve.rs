use crate::jacobian::Jacobian;
use crate::residual::{layout, residuals, tangent_modes, write_back};
use crate::types::{Constraint, Id, SolveError};
use cad_geometry::Shape;
use faer::linalg::solvers::Solve;
use faer::sparse::{SparseColMat, Triplet};
use faer::{Mat, Side};
use cad_geometry::shape::norm_angle;
use std::collections::{BTreeMap, BTreeSet};

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
    solve_pinned(shapes, constraints, &BTreeSet::new())
}

/// Like `solve`, but the pinned entities keep the values they have in `shapes`: their variables are not unknowns.
pub fn solve_pinned(shapes: &BTreeMap<Id, Shape>, constraints: &[Constraint], pinned: &BTreeSet<Id>) -> Result<Solution, SolveError> {
    let (lay, full) = layout(shapes, constraints)?;
    if constraints.is_empty() {
        return Ok(Solution { shapes: BTreeMap::new(), dof: 0, redundant: 0, iterations: 0 });
    }
    let modes = tangent_modes(&lay, constraints, &full);
    let free: Vec<usize> = lay.pinned(pinned).iter().enumerate().filter(|(_, p)| !**p).map(|(i, _)| i).collect();
    // The unknowns are the free variables; the others stay at their starting values.
    let expand = |z: &[f64]| {
        let mut all = full.clone();
        for (k, &i) in free.iter().enumerate() {
            all[i] = z[k];
        }
        all
    };
    let jacobian = Jacobian::new(&lay, &modes, constraints, &free, &full);
    let eval = |z: &[f64], out: &mut Vec<f64>| residuals(&lay, &modes, constraints, &expand(z), out);
    let mut x: Vec<f64> = free.iter().map(|&i| full[i]).collect();
    let n = x.len();
    let mut f = Vec::new();
    eval(&x, &mut f);
    let m = jacobian.rows();
    let (mut lambda, mut iterations) = (1e-3, 0);
    // J, Jᵀ and JᵀJ at the current point; rebuilt only after an accepted step.
    let mut jac: Option<[SparseColMat<usize, f64>; 3]> = None;
    let mut trial = Vec::new();
    loop {
        let err = max_abs(&f);
        if err <= TOL {
            break;
        }
        if n == 0 || iterations >= MAX_ITERATIONS || lambda > 1e10 {
            return Err(SolveError::NotConverged { residual: err });
        }
        iterations += 1;
        let [_, jt, jtj] = jac.get_or_insert_with(|| {
            let j = SparseColMat::try_new_from_triplets(m, n, &jacobian.triplets(&x)).expect("triplets are inside the matrix");
            let jt = j.transpose().to_col_major().expect("transpose fits in memory");
            let jtj = &jt * &j;
            [j, jt, jtj]
        });
        let fm = Mat::from_fn(m, 1, |i, _| f[i]);
        let g = &*jt * &fm;
        let mut t: Vec<Triplet<usize, usize, f64>> = jtj.triplet_iter().map(|e| Triplet::new(e.row, e.col, *e.val)).collect();
        t.extend((0..n).map(|i| Triplet::new(i, i, lambda)));
        let a = SparseColMat::try_new_from_triplets(n, n, &t).expect("triplets are inside the matrix");
        let Ok(llt) = a.sp_cholesky(Side::Lower) else {
            lambda *= 10.0;
            continue;
        };
        let delta = llt.solve(&g);
        let x_new: Vec<f64> = x.iter().enumerate().map(|(i, p)| p - delta[(i, 0)]).collect();
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
    let rank = if n == 0 {
        0
    } else {
        let j = match jac {
            Some([j, ..]) => j,
            None => SparseColMat::try_new_from_triplets(m, n, &jacobian.triplets(&x)).expect("triplets are inside the matrix"),
        };
        // shortcut: dense pivoted QR for the rank, about 0.1 s at 1600 unknowns; needs a sparse rank-revealing factorisation beyond a few thousand
        let qr = j.to_dense().col_piv_qr();
        let r = qr.thin_R();
        let diag: Vec<f64> = (0..m.min(n)).map(|i| r[(i, i)].abs()).collect();
        let limit = 1e-6 * diag.iter().fold(1.0_f64, |a, b| a.max(*b));
        diag.iter().filter(|d| **d > limit).count()
    };
    let changed = write_back(&lay, &expand(&x), shapes)
        .into_iter()
        .filter(|(id, s)| differs(s, &shapes[id]))
        .collect();
    Ok(Solution { shapes: changed, dof: n - rank, redundant: m - rank, iterations })
}
