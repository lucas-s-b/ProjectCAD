//! Stepped shafts: cylinders along the plane normal with a 45 degree chamfer at both ends, turned with `lathe`.
use crate::num;
use cad_geometry::DVec2;
use cad_modeler::{Body, ModelError, Plane3, lathe};
use std::f64::consts::PI;

const BAD_CHAMFER: &str = "shaft chamfer must be at least 0 and smaller than the end radius and step length";

/// Steps from the plane along its normal, each (diameter, length), about the normal through the plane origin;
/// `chamfer` is the 45 degree chamfer at both ends (0 for none). Millimetres.
#[derive(Debug, Clone, PartialEq)]
pub struct Shaft {
    pub steps: Vec<(f64, f64)>,
    pub chamfer: f64,
}

impl Shaft {
    pub fn length(&self) -> f64 {
        self.steps.iter().map(|s| s.1).sum()
    }

    pub fn max_diameter(&self) -> f64 {
        self.steps.iter().map(|s| s.0).fold(0.0, f64::max)
    }

    /// "Shaft Ø20x80": largest diameter and total length.
    pub fn designation(&self) -> String {
        format!("Shaft \u{d8}{}x{}", num(self.max_diameter()), num(self.length()))
    }

    /// Facets per turn so that the largest circle is at most `tol` from its polygon; at least 16.
    pub fn segments(&self, tol: f64) -> usize {
        let r = self.max_diameter() / 2.0;
        ((PI / (1.0 - tol / r).clamp(-1.0, 1.0).acos()).ceil() as usize).max(16)
    }

    fn check(&self) -> Result<(), ModelError> {
        let (Some(&(d0, l0)), Some(&(dn, ln))) = (self.steps.first(), self.steps.last()) else {
            return Err(ModelError::Invalid("shaft needs at least one step"));
        };
        if !self.steps.iter().all(|&(d, l)| d > 0.0 && l > 0.0 && d.is_finite() && l.is_finite()) {
            return Err(ModelError::Invalid("shaft diameters and lengths must be positive"));
        }
        let c = self.chamfer;
        if c.is_nan() || c < 0.0 || c >= d0 / 2.0 || c >= dn / 2.0 || c >= l0 || c >= ln || (self.steps.len() == 1 && 2.0 * c >= l0) {
            return Err(ModelError::Invalid(BAD_CHAMFER));
        }
        Ok(())
    }

    /// The half section as (radius, height), counter-clockwise from the axis at the plane to the axis at the far end.
    /// Neighbouring steps of equal diameter make one cylinder.
    pub fn outline(&self) -> Vec<DVec2> {
        let (c, n) = (self.chamfer, self.steps.len());
        let r = |i: usize| self.steps[i].0 / 2.0;
        let mut p = vec![DVec2::ZERO, DVec2::new(r(0) - c, 0.0)];
        if c > 0.0 {
            p.push(DVec2::new(r(0), c));
        }
        let mut z = 0.0;
        for (i, step) in self.steps.iter().enumerate().take(n) {
            z += step.1;
            if i + 1 < n {
                if (r(i + 1) - r(i)).abs() > 1e-9 {
                    p.push(DVec2::new(r(i), z));
                    p.push(DVec2::new(r(i + 1), z));
                }
            } else if c > 0.0 {
                p.push(DVec2::new(r(i), z - c));
                p.push(DVec2::new(r(i) - c, z));
            } else {
                p.push(DVec2::new(r(i), z));
            }
        }
        p.push(DVec2::new(0.0, z));
        p
    }

    /// The turned solid, `segments(tol)` facets per turn.
    pub fn body(&self, plane: &Plane3, tol: f64) -> Result<Body, ModelError> {
        self.check()?;
        lathe(&self.outline(), plane, self.segments(tol))
    }
}

#[cfg(test)]
#[path = "shaft_tests.rs"]
mod tests;
