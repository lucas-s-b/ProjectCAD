//! How far an analysis result can be trusted; printed beside every result.
use cad_modeler::{Body, Surface};
use std::fmt;

/// Reason of `Accuracy::Approximate` for results computed on the facets of curved faces.
pub const TESSELLATION: &str = "tessellation";

/// Reason of `Accuracy::Approximate` for checks made only at the samples of a motion study (a collision between two
/// samples is not seen).
pub const SAMPLED: &str = "sampled at the steps";

/// The label of an analysis result. Nothing is ever presented as certified: results are exact (the computation is
/// exact for the geometry, e.g. mass properties of a body with only planar faces), approximate (with the reason), or
/// validated against a named analytic benchmark case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accuracy {
    Exact,
    Approximate(&'static str),
    Validated(&'static str),
}

impl Accuracy {
    /// `Exact` when every face of `body` is a plane (its triangle mesh is the solid itself), else
    /// `Approximate(TESSELLATION)`.
    pub fn of_body(body: &Body) -> Accuracy {
        if body.faces.iter().all(|f| matches!(f.surface, Surface::Plane { .. })) { Accuracy::Exact } else { Accuracy::Approximate(TESSELLATION) }
    }
}

/// "exact", "approximate (tessellation)", "validated against cantilever beam".
impl fmt::Display for Accuracy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Accuracy::Exact => write!(f, "exact"),
            Accuracy::Approximate(why) => write!(f, "approximate ({why})"),
            Accuracy::Validated(case) => write!(f, "validated against {case}"),
        }
    }
}

#[cfg(test)]
#[path = "accuracy_tests.rs"]
mod tests;
