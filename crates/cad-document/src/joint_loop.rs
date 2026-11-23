//! Closed loops: a joint marked `closes_loop` is not walked; Levenberg-Marquardt (`cad_constraints::least_squares`)
//! sets the free joints' values so that its frames meet.
use crate::component::ComponentId;
use crate::document::Document;
use crate::feature::FeatureId;
use crate::joint::{Joint, JointId, JointKind};
use crate::rebuild::{RebuildResult, eval};
use cad_constraints::{SolveError, Triplet, least_squares};
use cad_geometry::{DAffine3, DVec3};
use cad_modeler::Body;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Residual rows of a closing joint of `kind` (see `closure`).
fn rows(kind: JointKind) -> usize {
    match kind {
        JointKind::Revolute => 6,
        JointKind::Slider => 8,
        JointKind::Fixed => 9,
    }
}

/// Pushes the loop-closure residual of a closing joint of `kind` whose child is off by `m` = Fp⁻¹ · Tp⁻¹ · Tc ·
/// mate(I, I, Fc)⁻¹ (the identity when the child sits exactly where the joint puts it at value 0): the translation
/// (a slider leaves its Z free), the turned Z axis minus Z (all three components, so a half turn is not taken for a
/// fit; a revolute leaves the turn about Z free), and for slider and fixed the turned X axis minus X. mm and unit vectors.
pub(crate) fn closure(kind: JointKind, m: DAffine3, out: &mut Vec<f64>) {
    let (t, x, z) = (m.translation, m.matrix3.x_axis - DVec3::X, m.matrix3.z_axis - DVec3::Z);
    match kind {
        JointKind::Revolute => out.extend([t.x, t.y, t.z, z.x, z.y, z.z]),
        JointKind::Slider => out.extend([t.x, t.y, x.x, x.y, x.z, z.x, z.y, z.z]),
        JointKind::Fixed => out.extend([t.x, t.y, t.z, x.x, x.y, x.z, z.x, z.y, z.z]),
    }
}

impl Document {
    /// Components whose placement depends on a free joint: the subtrees of the free joints' children, in id order.
    pub(crate) fn loop_moving(&self) -> Vec<ComponentId> {
        let mut out: Vec<ComponentId> = self.joints().filter(|j| j.is_free()).flat_map(|j| self.subtree(j.child)).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Values (radians or mm) of the free joints (`Joint::is_free`, except the driven one) that close every loop:
    /// each closing joint's child must sit where that joint would put it, any turn about its axis allowed for a
    /// revolute, any move along it for a slider. Starts from `guess`, else the joint's value expression; one
    /// Levenberg-Marquardt solve for all loops over central differences; limits are not used here (the walk
    /// afterwards refuses values outside them). Returns those values with `drive`. Errors, named after the first
    /// closing joint (else the first free joint): "joint 4: under-constrained (degrees of freedom left: 1); mark fewer
    /// joints free", "joint 4: over-constrained: no free joint value can close the loop (residual 5.603e0)",
    /// "joint 4: the loop does not close (residual 1.234e1); check the free joints and their start values", a free
    /// joint's expression error ("joint 2: expected an angle"), or the walk's error at the start values (a frame).
    pub fn solve_joints(&self, bodies: &[(FeatureId, Arc<Body>)], drive: Option<(JointId, f64)>, guess: &BTreeMap<JointId, f64>) -> Result<BTreeMap<JointId, f64>, String> {
        let closing: Vec<&Joint> = self.joints().filter(|j| j.closes_loop).collect();
        let free: Vec<&Joint> = self.joints().filter(|j| j.is_free() && drive.is_none_or(|(id, _)| id != j.id)).collect();
        let named = closing.first().or(free.first()).map_or(0, |j| j.id);
        let fail = |what: String| format!("joint {named}: {what}");
        let mut x = Vec::new();
        for j in &free {
            x.push(match guess.get(&j.id) {
                Some(v) => *v,
                None => eval(self, &j.value, j.kind == JointKind::Revolute).map_err(|err| format!("joint {}: {err}", j.id))?,
            });
        }
        let base: BTreeMap<JointId, f64> = drive.into_iter().collect();
        let with = |z: &[f64]| {
            let mut s = base.clone();
            s.extend(free.iter().map(|j| j.id).zip(z.iter().copied()));
            s
        };
        // Frames once; the walk's error at the start values (a frame, a missing body) says more than a residual.
        let mut frames = Vec::new();
        let start = self.walk(bodies, &with(&x), false);
        for j in &closing {
            for c in [j.parent, j.child] {
                start.get(&c).cloned().unwrap_or_else(|| Err(format!("component {c} not found")))?;
            }
            frames.push((self.frame(&j.parent_frame, j.parent, bodies)?.inverse(), cad_mech::mate(DAffine3::IDENTITY, DAffine3::IDENTITY, self.frame(&j.child_frame, j.child, bodies)?).inverse()));
        }
        let residuals = |z: &[f64], out: &mut Vec<f64>| {
            out.clear();
            let placed = self.walk(bodies, &with(z), false);
            for (j, (fp_inv, fc_mate_inv)) in closing.iter().zip(&frames) {
                match (placed.get(&j.parent), placed.get(&j.child)) {
                    (Some(Ok(tp)), Some(Ok(tc))) => closure(j.kind, *fp_inv * tp.inverse() * *tc * *fc_mate_inv, out),
                    // A point where the walk fails is infeasible; the solver never accepts it.
                    _ => out.extend(std::iter::repeat_n(f64::NAN, rows(j.kind))),
                }
            }
        };
        // shortcut: central differences, two walks per free value; analytic screw-axis derivatives if loops get large.
        let jacobian = |z: &[f64]| {
            let (mut plus, mut minus, mut out, mut p) = (Vec::new(), Vec::new(), Vec::new(), z.to_vec());
            for k in 0..z.len() {
                let h = 1e-7 * z[k].abs().max(1.0);
                p[k] = z[k] + h;
                residuals(&p, &mut plus);
                p[k] = z[k] - h;
                residuals(&p, &mut minus);
                p[k] = z[k];
                for (r, (a, b)) in plus.iter().zip(&minus).enumerate() {
                    let v = (a - b) / (2.0 * h);
                    if v != 0.0 {
                        out.push(Triplet::new(r, k, v));
                    }
                }
            }
            out
        };
        // `residuals` is Copy (it captures only shared references), so the Jacobian closure can borrow it too.
        let fit = match least_squares(x, residuals, jacobian) {
            Ok(fit) => fit,
            Err(SolveError::NotConverged { residual }) if free.is_empty() => {
                return Err(fail(format!("over-constrained: no free joint value can close the loop (residual {residual:.3e})")));
            }
            Err(SolveError::NotConverged { residual }) => {
                return Err(fail(format!("the loop does not close (residual {residual:.3e}); check the free joints and their start values")));
            }
            Err(err) => return Err(fail(err.to_string())),
        };
        let dof = free.len() - fit.rank;
        if dof > 0 {
            return Err(fail(format!("under-constrained (degrees of freedom left: {dof}); mark fewer joints free")));
        }
        let mut out = base;
        out.extend(free.iter().map(|j| j.id).zip(fit.x));
        Ok(out)
    }

    /// `placements` with joint `drive.0` at `drive.1` (radians or mm) instead of its expression and the loops solved
    /// (`solve_joints`) from `guess`, which receives the solved values. Without closing or free joints this is the
    /// plain walk. A loop error fails every component that depends on a free joint and the closing joints' children
    /// with their subtrees; the others keep their walk placements.
    pub(crate) fn placements_driven(&self, bodies: &[(FeatureId, Arc<Body>)], drive: Option<(JointId, f64)>, guess: &mut BTreeMap<JointId, f64>) -> BTreeMap<ComponentId, Result<DAffine3, String>> {
        let mut set: BTreeMap<JointId, f64> = drive.into_iter().collect();
        if self.joints().any(|j| j.closes_loop || j.is_free()) {
            match self.solve_joints(bodies, drive, guess) {
                Ok(solved) => {
                    guess.extend(solved.iter().map(|(k, v)| (*k, *v)));
                    set = solved;
                }
                Err(err) => {
                    let mut out = self.walk(bodies, &set, true);
                    let closing = self.joints().filter(|j| j.closes_loop).flat_map(|j| self.subtree(j.child));
                    for c in self.loop_moving().into_iter().chain(closing) {
                        out.insert(c, Err(err.clone()));
                    }
                    return out;
                }
            }
        }
        self.walk(bodies, &set, true)
    }

    /// The value of `j` (radians or mm) in `result`: the solved value of a free joint, else its expression.
    pub fn joint_value(&self, result: &RebuildResult, j: &Joint) -> Result<f64, String> {
        match result.joint_values.get(&j.id) {
            Some(v) => Ok(*v),
            None => eval(self, &j.value, j.kind == JointKind::Revolute),
        }
    }
}

#[cfg(test)]
#[path = "joint_loop_tests.rs"]
pub(crate) mod tests;
