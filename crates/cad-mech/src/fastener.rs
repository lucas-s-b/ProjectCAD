//! ISO metric hex head screws (ISO 4017) and hex nuts (ISO 4032), coarse thread. Threads are cosmetic: the shank and
//! the nut hole have the nominal diameter and the thread is only named.
use crate::{circle, hexagon, num, polygon};
use cad_geometry::profile::Profile;
use cad_modeler::{Body, BoolOp, ModelError, Plane3, boolean, extrude};

/// One size: nominal diameter `d`, coarse pitch, width across flats `s`, head height `k` (ISO 4017) and nut height `m`
/// (ISO 4032, maximum). Millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metric {
    pub name: &'static str,
    pub d: f64,
    pub pitch: f64,
    pub s: f64,
    pub k: f64,
    pub m: f64,
}

const fn row(name: &'static str, d: f64, pitch: f64, s: f64, k: f64, m: f64) -> Metric {
    Metric { name, d, pitch, s, k, m }
}

/// The supported sizes, smallest first.
pub const METRIC: [Metric; 10] = [
    row("M3", 3.0, 0.5, 5.5, 2.0, 2.4),
    row("M4", 4.0, 0.7, 7.0, 2.8, 3.2),
    row("M5", 5.0, 0.8, 8.0, 3.5, 4.7),
    row("M6", 6.0, 1.0, 10.0, 4.0, 5.2),
    row("M8", 8.0, 1.25, 13.0, 5.3, 6.8),
    row("M10", 10.0, 1.5, 16.0, 6.4, 8.4),
    row("M12", 12.0, 1.75, 18.0, 7.5, 10.8),
    row("M16", 16.0, 2.0, 24.0, 10.0, 14.8),
    row("M20", 20.0, 2.5, 30.0, 12.5, 18.0),
    row("M24", 24.0, 3.0, 36.0, 15.0, 21.5),
];

/// The size called `name` ("M8"; surrounding spaces and a lower-case m are accepted).
pub fn metric(name: &str) -> Option<Metric> {
    let name = name.trim();
    METRIC.iter().find(|m| m.name.eq_ignore_ascii_case(name)).copied()
}

impl Metric {
    /// "M8x1.25".
    pub fn thread(&self) -> String {
        format!("{}x{}", self.name, num(self.pitch))
    }
}

/// ISO 4017 hex head screw, threaded up to the head. The bearing face of the head lies on the plane: the head rises
/// `k` along the normal, the shank goes `length` against it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bolt {
    pub size: Metric,
    /// Under the head, mm.
    pub length: f64,
}

/// ISO 4032 hex nut from the plane along its normal, with a hole of the nominal diameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nut {
    pub size: Metric,
}

impl Bolt {
    /// "ISO 4017 M8x30".
    pub fn designation(&self) -> String {
        format!("ISO 4017 {}x{}", self.size.name, num(self.length))
    }

    /// The head prism united with the shank cylinder, tessellated within `tol`. The shank starts halfway up the head,
    /// so the union has no coplanar faces.
    pub fn body(&self, plane: &Plane3, tol: f64) -> Result<Body, ModelError> {
        if !(self.length > 0.0 && self.length.is_finite()) {
            return Err(ModelError::Invalid("bolt length must be positive"));
        }
        let Metric { d, s, k, .. } = self.size;
        let head = extrude(&Profile { outer: polygon(&hexagon(s)), holes: Vec::new() }, plane, k)?;
        let start = Plane3 { origin: plane.origin + plane.normal() * (k / 2.0), ..*plane };
        let shank = extrude(&Profile { outer: circle(d / 2.0, true), holes: Vec::new() }, &start, -(self.length + k / 2.0))?;
        boolean(&Body::from_brep(&head, tol)?, &Body::from_brep(&shank, tol)?, BoolOp::Union)
    }
}

impl Nut {
    /// "ISO 4032 M8".
    pub fn designation(&self) -> String {
        format!("ISO 4032 {}", self.size.name)
    }

    /// The hexagon with its hole extruded by `m`, tessellated within `tol`.
    pub fn body(&self, plane: &Plane3, tol: f64) -> Result<Body, ModelError> {
        let Metric { d, s, m, .. } = self.size;
        let profile = Profile { outer: polygon(&hexagon(s)), holes: vec![circle(d / 2.0, false)] };
        Body::from_brep(&extrude(&profile, plane, m)?, tol)
    }
}

#[cfg(test)]
#[path = "fastener_tests.rs"]
mod tests;
