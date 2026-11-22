//! The solid of one wall.
use cad_geometry::{DVec2, Slab};
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

/// The solid of a floor slab on `plane` (its level): the outline, either orientation, from `thickness` below the
/// plane up to it, tessellated within `tol`.
pub fn slab_body(s: &Slab, plane: &Plane3, tol: f64) -> Result<Body, ModelError> {
    let mut outline = s.outline.clone();
    // `wall_body` refuses clockwise outlines
    if s.area() < 0.0 {
        outline.reverse();
    }
    let base = Plane3 { origin: plane.origin - plane.normal() * s.thickness, ..*plane };
    wall_body(&outline, &base, s.thickness, tol)
}

#[cfg(test)]
#[path = "solid_tests.rs"]
mod tests;
