pub mod brep;
pub mod geom;
mod validate;

pub use brep::{Brep, CoEdge, Edge, Face, TopoError};
pub use geom::{Curve, Surface};

/// Distance (mm) below which two points are the same point.
pub const TOL: f64 = 1e-7;

#[cfg(test)]
mod brep_tests;
#[cfg(test)]
mod geom_tests;
