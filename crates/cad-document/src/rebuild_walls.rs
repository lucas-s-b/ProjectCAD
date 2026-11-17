//! Solids of the walls on every level, built by `cad_arch` on every rebuild.
use crate::document::{Document, EntityId};
use crate::feature::BODY_TOLERANCE;
use cad_arch::{footprints, wall_body};
use cad_geometry::{Shape, Wall};
use cad_modeler::Body;
use std::sync::Arc;

/// One solid per wall that is not construction, by level then entity id. A level whose plane does not resolve has
/// none; a wall whose joined outline is degenerate is left out.
// shortcut: no cache and no union of the walls; cache per level when large projects rebuild slowly
pub(crate) fn wall_bodies(doc: &Document) -> Vec<(EntityId, Arc<Body>)> {
    let mut out = Vec::new();
    for s in doc.sketches().filter(|s| s.level.is_some()) {
        let Ok(plane) = doc.resolve_plane(s.id) else { continue };
        let walls: Vec<(EntityId, Wall)> = doc
            .entities_in(s.id)
            .filter(|e| !e.construction)
            .filter_map(|e| match &e.shape {
                Shape::Wall(w) => Some((e.id, *w)),
                _ => None,
            })
            .collect();
        let outlines = footprints(&walls.iter().map(|(_, w)| *w).collect::<Vec<_>>());
        for ((id, w), outline) in walls.iter().zip(&outlines) {
            if let Ok(body) = wall_body(outline, &plane, w.height, BODY_TOLERANCE) {
                out.push((*id, Arc::new(body)));
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "rebuild_walls_tests.rs"]
mod tests;
