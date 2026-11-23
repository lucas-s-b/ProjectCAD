//! Architectural elements on a level: wall joins, the plan, wall, slab, roof and stair solids, and room areas. Works on shapes, knows no document.
mod join;
mod opening;
mod plan;
mod roof;
mod room;
mod solid;
#[cfg(test)]
mod fixtures;
pub use join::{JOIN_TOL, footprints};
pub use opening::{cut_openings, gap, ifc_class, opening_box, opening_symbol};
pub use plan::{Plan, plan};
pub use roof::{roof_body, stair_body};
pub use room::{Region, regions};
pub use solid::{slab_body, wall_body};
