//! Helical compression springs: a round wire (a polygon of `WIRE_SIDES` sides) swept along a helix polyline.
use crate::{num, polygon};
use cad_geometry::profile::Profile;
use cad_geometry::{DVec2, DVec3};
use cad_modeler::{Body, ModelError, PathPiece, Plane3, sweep};
use std::f64::consts::TAU;

/// Sides of the wire section.
pub const WIRE_SIDES: usize = 24;
/// Path segments per turn of the helix.
pub const SEGMENTS_PER_TURN: f64 = 48.0;

/// Coils about the plane normal through the plane origin, rising from the plane: wire diameter, mean coil diameter,
/// pitch (rise per turn) and turns (need not be whole). Plain ends: no closed or ground end coils. Millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub wire: f64,
    pub mean: f64,
    pub pitch: f64,
    pub turns: f64,
}

impl Spring {
    /// Unloaded length: the rise of the wire centre plus one wire diameter.
    pub fn free_length(&self) -> f64 {
        self.pitch * self.turns + self.wire
    }

    /// "Compression spring d2 D20 L32": wire diameter, mean diameter, free length.
    pub fn designation(&self) -> String {
        format!("Compression spring d{} D{} L{}", num(self.wire), num(self.mean), num(self.free_length()))
    }

    /// The wire centre line: from `plane.x` x mean / 2, turning from `plane.x` towards `plane.y` and rising along the
    /// normal; `ceil(SEGMENTS_PER_TURN x turns)` equal segments.
    pub fn helix(&self, plane: &Plane3) -> Vec<DVec3> {
        let n = (SEGMENTS_PER_TURN * self.turns - 1e-9).ceil().max(1.0) as usize;
        let (r, axis) = (self.mean / 2.0, plane.normal());
        (0..=n)
            .map(|k| {
                let t = self.turns * k as f64 / n as f64;
                let a = TAU * t;
                plane.origin + (plane.x * a.cos() + plane.y * a.sin()) * r + axis * (self.pitch * t)
            })
            .collect()
    }

    /// The swept wire. Its volume is exactly the section polygon's area times the length of the helix polyline.
    // shortcut: fixed 24 sides and 48 segments per turn whatever `tol`; derive both from tol when large springs need it
    pub fn body(&self, plane: &Plane3, tol: f64) -> Result<Body, ModelError> {
        if !(self.wire > 0.0 && self.turns > 0.0 && self.turns <= 200.0) {
            return Err(ModelError::Invalid("spring wire diameter and turns must be positive, at most 200 turns"));
        }
        if self.mean.is_nan() || self.mean < 2.0 * self.wire {
            return Err(ModelError::Invalid("spring mean diameter must be at least twice the wire diameter"));
        }
        if self.pitch.is_nan() || self.pitch <= self.wire {
            return Err(ModelError::Invalid("spring pitch must be larger than the wire diameter"));
        }
        let points = self.helix(plane);
        // The section lies at right angles to the first segment, so every mitred ring keeps its area.
        let d0 = (points[1] - points[0]).normalize();
        let x = (plane.x - d0 * plane.x.dot(d0)).normalize();
        let section = Plane3 { origin: points[0], x, y: d0.cross(x) };
        let corners: Vec<DVec2> =
            (0..WIRE_SIDES).map(|k| DVec2::from_angle(TAU * k as f64 / WIRE_SIDES as f64) * (self.wire / 2.0)).collect();
        let profile = Profile { outer: polygon(&corners), holes: Vec::new() };
        sweep(&profile, &section, &[PathPiece { points, straight: false }], false, tol)
    }
}

#[cfg(test)]
#[path = "spring_tests.rs"]
mod tests;
