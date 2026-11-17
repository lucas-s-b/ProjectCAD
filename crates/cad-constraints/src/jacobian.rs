use crate::residual::{Layout, constraint_residuals, rows};
use crate::types::{Constraint, Id};
use faer::sparse::Triplet;
#[cfg(test)]
use nalgebra::DMatrix;
use std::collections::BTreeMap;

/// Central-difference Jacobian over the free variables; each variable only re-evaluates the constraints that reference its entity.
pub(crate) struct Jacobian<'a> {
    layout: &'a Layout,
    modes: &'a [bool],
    constraints: &'a [Constraint],
    /// Indices of the free variables in the full vector; `full` holds the others.
    free: &'a [usize],
    full: &'a [f64],
    owners: Vec<Id>,
    /// Entity -> (constraint index, first row) of the constraints that reference it.
    touching: BTreeMap<Id, Vec<(usize, usize)>>,
    rows: usize,
}

impl<'a> Jacobian<'a> {
    pub(crate) fn new(layout: &'a Layout, modes: &'a [bool], constraints: &'a [Constraint], free: &'a [usize], full: &'a [f64]) -> Self {
        let mut touching: BTreeMap<Id, Vec<(usize, usize)>> = BTreeMap::new();
        let mut first = 0;
        for (i, c) in constraints.iter().enumerate() {
            for id in c.entities() {
                let list = touching.entry(id).or_default();
                if !list.contains(&(i, first)) {
                    list.push((i, first));
                }
            }
            first += rows(c);
        }
        Jacobian { layout, modes, constraints, free, full, owners: layout.owners(), touching, rows: first }
    }

    /// Number of equations.
    pub(crate) fn rows(&self) -> usize {
        self.rows
    }

    // shortcut: numeric central differences; analytic derivatives would halve the residual evaluations
    #[cfg(test)]
    pub(crate) fn at(&self, z: &[f64]) -> DMatrix<f64> {
        let mut j = DMatrix::zeros(self.rows, z.len());
        for t in self.triplets(z) {
            j[(t.row, t.col)] = t.val;
        }
        j
    }

    /// The nonzero entries of the Jacobian at `z`.
    pub(crate) fn triplets(&self, z: &[f64]) -> Vec<Triplet<usize, usize, f64>> {
        let mut out = Vec::new();
        let mut all = self.full.to_vec();
        for (k, &i) in self.free.iter().enumerate() {
            all[i] = z[k];
        }
        let (mut plus, mut minus) = (Vec::new(), Vec::new());
        for (k, &i) in self.free.iter().enumerate() {
            let h = 1e-7 * z[k].abs().max(1.0);
            for &(ci, first) in self.touching.get(&self.owners[i]).map_or(&[][..], Vec::as_slice) {
                let c = &self.constraints[ci];
                all[i] = z[k] + h;
                plus.clear();
                constraint_residuals(self.layout, self.modes, ci, c, &all, &mut plus);
                all[i] = z[k] - h;
                minus.clear();
                constraint_residuals(self.layout, self.modes, ci, c, &all, &mut minus);
                for r in 0..plus.len() {
                    out.push(Triplet::new(first + r, k, (plus[r] - minus[r]) / (2.0 * h)));
                }
            }
            all[i] = z[k];
        }
        out
    }
}
