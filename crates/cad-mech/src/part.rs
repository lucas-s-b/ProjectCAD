//! Every generated part behind one type, for the document's part features.
use crate::{Bearing, Bolt, Gear, Nut, Shaft, Spring};
use cad_modeler::{Body, ModelError, Plane3};

/// A generated part with its parameters evaluated.
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Gear(Gear),
    Shaft(Shaft),
    Spring(Spring),
    Bolt(Bolt),
    Nut(Nut),
    Bearing(Bearing),
}

impl Part {
    /// The solid on `plane`, its axis along the plane normal through the plane origin, tessellated within `tol`.
    /// Parameters out of range give `ModelError::Invalid` with the reason.
    pub fn body(&self, plane: &Plane3, tol: f64) -> Result<Body, ModelError> {
        match self {
            Part::Gear(g) => g.body(plane, tol),
            Part::Shaft(s) => s.body(plane, tol),
            Part::Spring(s) => s.body(plane, tol),
            Part::Bolt(b) => b.body(plane, tol),
            Part::Nut(n) => n.body(plane, tol),
            Part::Bearing(b) => b.body(plane, tol),
        }
    }

    /// Standard designation for the bill of materials, e.g. "ISO 4017 M8x30".
    pub fn designation(&self) -> String {
        match self {
            Part::Gear(g) => g.designation(),
            Part::Shaft(s) => s.designation(),
            Part::Spring(s) => s.designation(),
            Part::Bolt(b) => b.designation(),
            Part::Nut(n) => n.designation(),
            Part::Bearing(b) => b.designation(),
        }
    }

    /// The cosmetic thread, e.g. "M8x1.25"; None for parts without one.
    pub fn thread(&self) -> Option<String> {
        match self {
            Part::Bolt(b) => Some(b.size.thread()),
            Part::Nut(n) => Some(n.size.thread()),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "part_tests.rs"]
mod tests;
