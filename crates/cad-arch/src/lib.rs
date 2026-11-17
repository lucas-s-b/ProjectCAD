//! Architectural elements on a level: wall joins, the plan and the wall solids. Works on shapes, knows no document.
mod join;
mod opening;
mod plan;
mod solid;
#[cfg(test)]
mod fixtures;
pub use join::{JOIN_TOL, footprints};
pub use opening::{cut_openings, gap, ifc_class, opening_symbol};
pub use plan::{Plan, plan};
pub use solid::wall_body;
