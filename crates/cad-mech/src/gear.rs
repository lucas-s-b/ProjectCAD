//! Involute spur gears without profile shift or backlash: addendum one module, dedendum 1.25 modules.
use crate::{circle, num};
use cad_geometry::DVec2;
use cad_geometry::profile::{Profile, Seg};
use cad_modeler::{Body, ModelError, Plane3, extrude};
use std::f64::consts::PI;

/// Line segments per involute flank.
pub const FLANK_SEGMENTS: usize = 10;

/// An involute spur gear about the plane normal through the plane origin, from the plane along its normal by `width`.
/// Tooth 0 is centred on `plane.x`. Millimetres; `pressure` in radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gear {
    pub module: f64,
    pub teeth: u32,
    pub pressure: f64,
    pub width: f64,
    /// Diameter of the hole; 0 for none.
    pub bore: f64,
}

/// The involute function tan a - a.
fn inv(a: f64) -> f64 {
    a.tan() - a
}

fn bad(msg: &'static str) -> Result<Profile, ModelError> {
    Err(ModelError::Invalid(msg))
}

impl Gear {
    pub fn pitch_diameter(&self) -> f64 {
        self.module * f64::from(self.teeth)
    }

    pub fn tip_diameter(&self) -> f64 {
        self.pitch_diameter() + 2.0 * self.module
    }

    pub fn root_diameter(&self) -> f64 {
        self.pitch_diameter() - 2.5 * self.module
    }

    pub fn base_diameter(&self) -> f64 {
        self.pitch_diameter() * self.pressure.cos()
    }

    /// "Spur gear m2 z20 b10".
    pub fn designation(&self) -> String {
        format!("Spur gear m{} z{} b{}", num(self.module), self.teeth, num(self.width))
    }

    /// The outline in the plane. Per tooth, counter-clockwise from tooth 0: a radial line from the root circle to the
    /// base circle when the root lies inside it, the lower flank in `FLANK_SEGMENTS` lines, the tip arc, the upper flank,
    /// the radial line back down and the root arc to the next tooth. The bore is a clockwise circle.
    pub fn profile(&self) -> Result<Profile, ModelError> {
        if !(self.module > 0.0 && self.width > 0.0 && self.module.is_finite() && self.width.is_finite()) {
            return bad("gear module and face width must be positive");
        }
        if !(4..=1000).contains(&self.teeth) {
            return bad("gear needs 4 to 1000 teeth");
        }
        if !(self.pressure >= 10f64.to_radians() - 1e-12 && self.pressure <= 35f64.to_radians() + 1e-12) {
            return bad("gear pressure angle must be from 10 to 35 degrees");
        }
        let z = f64::from(self.teeth);
        let (rb, ra, rf) = (self.base_diameter() / 2.0, self.tip_diameter() / 2.0, self.root_diameter() / 2.0);
        if !(self.bore >= 0.0 && self.bore < 2.0 * rf) {
            return bad("gear bore must be smaller than the root diameter");
        }
        // The lower flank leaves the base circle at -beta and crosses the pitch circle at -pi / 2z.
        let beta = PI / (2.0 * z) + inv(self.pressure);
        let t_tip = ((ra / rb).powi(2) - 1.0).sqrt();
        let t_root = if rf > rb { ((rf / rb).powi(2) - 1.0).sqrt() } else { 0.0 };
        // Polar angles of the involute at the tip and at its lowest point.
        let (tip, root) = (t_tip - t_tip.atan(), t_root - t_root.atan());
        let tip_sweep = 2.0 * (beta - tip);
        let root_sweep = 2.0 * PI / z - 2.0 * beta + 2.0 * root;
        if tip_sweep <= 1e-9 {
            return bad("gear teeth are pointed");
        }
        if root_sweep <= 1e-9 {
            return bad("gear teeth overlap");
        }
        // The lower flank of tooth 0, root to tip.
        let flank: Vec<DVec2> = (0..=FLANK_SEGMENTS)
            .map(|i| {
                let t = t_root + (t_tip - t_root) * i as f64 / FLANK_SEGMENTS as f64;
                DVec2::from_angle(-beta).rotate(DVec2::new(t.cos() + t * t.sin(), t.sin() - t * t.cos()) * rb)
            })
            .collect();
        let line = |a: DVec2, b: DVec2| Seg::Line { a, b };
        let mut outer = Vec::new();
        for k in 0..self.teeth {
            let phi = 2.0 * PI * f64::from(k) / z;
            let turn = DVec2::from_angle(phi);
            let lower: Vec<DVec2> = flank.iter().map(|p| turn.rotate(*p)).collect();
            let upper: Vec<DVec2> = flank.iter().rev().map(|p| turn.rotate(DVec2::new(p.x, -p.y))).collect();
            if rf < rb {
                outer.push(line(DVec2::from_angle(phi - beta) * rf, lower[0]));
            }
            outer.extend(lower.windows(2).map(|w| line(w[0], w[1])));
            outer.push(Seg::Arc { center: DVec2::ZERO, radius: ra, start: phi - beta + tip, sweep: tip_sweep });
            outer.extend(upper.windows(2).map(|w| line(w[0], w[1])));
            if rf < rb {
                outer.push(line(upper[FLANK_SEGMENTS], DVec2::from_angle(phi + beta) * rf));
            }
            outer.push(Seg::Arc { center: DVec2::ZERO, radius: rf, start: phi + beta - root, sweep: root_sweep });
        }
        let holes = if self.bore > 0.0 { vec![circle(self.bore / 2.0, false)] } else { Vec::new() };
        Ok(Profile { outer, holes })
    }

    /// The profile extruded by `width` (B-Rep), tessellated within `tol`.
    pub fn body(&self, plane: &Plane3, tol: f64) -> Result<Body, ModelError> {
        Body::from_brep(&extrude(&self.profile()?, plane, self.width)?, tol)
    }
}

#[cfg(test)]
#[path = "gear_tests.rs"]
mod tests;
