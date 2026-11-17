//! The solid of one wall.
use cad_geometry::DVec2;
use cad_geometry::profile::{Profile, Seg, loop_area};
use cad_modeler::{Body, ModelError, Plane3, extrude};

/// `footprint` (counter-clockwise, in the coordinates of the level's sketch) on `plane`, extruded by `height` along the
/// plane normal and tessellated within `tol`. Refuses outlines with fewer than three points or no positive area.
pub fn wall_body(footprint: &[DVec2], plane: &Plane3, height: f64, tol: f64) -> Result<Body, ModelError> {
    let n = footprint.len();
    let outer: Vec<Seg> = (0..n).map(|k| Seg::Line { a: footprint[k], b: footprint[(k + 1) % n] }).collect();
    if n < 3 || loop_area(&outer) <= 1e-9 {
        return Err(ModelError::Invalid("wall outline is degenerate"));
    }
    Body::from_brep(&extrude(&Profile { outer, holes: Vec::new() }, plane, height)?, tol)
}

#[cfg(test)]
#[path = "solid_tests.rs"]
mod tests;
