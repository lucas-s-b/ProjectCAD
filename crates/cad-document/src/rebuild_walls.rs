//! Solids of the walls and floor slabs on every level, and the areas of the rooms, built by `cad_arch` on every rebuild.
use crate::document::{Document, EntityId};
use crate::feature::BODY_TOLERANCE;
use cad_arch::{Region, cut_openings, footprints, regions, slab_body, wall_body};
use cad_geometry::{DVec2, Opening, Shape, Wall};
use cad_modeler::Body;
use std::sync::Arc;

/// One solid per wall, then one per floor slab, that is not construction, by level then entity id. A level whose plane does not resolve has
/// none; a wall whose joined outline is degenerate is left out. Doors and windows that are not construction are cut
/// from their wall.
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
        let openings: Vec<Opening> = doc
            .entities_in(s.id)
            .filter(|x| !x.construction)
            .filter_map(|x| match &x.shape {
                Shape::Opening(o) => Some(*o),
                _ => None,
            })
            .collect();
        let outlines = footprints(&walls.iter().map(|(_, w)| *w).collect::<Vec<_>>());
        for ((id, w), outline) in walls.iter().zip(&outlines) {
            if let Ok(body) = wall_body(outline, &plane, w.height, BODY_TOLERANCE) {
                // Placed on the wall as it is now, never on the stored copy.
                let mine: Vec<Opening> = openings.iter().filter(|o| o.host == *id).map(|o| Opening { wall: *w, ..*o }).collect();
                out.push((*id, Arc::new(cut_openings(body, &mine, &plane, BODY_TOLERANCE))));
            }
        }
        for x in doc.entities_in(s.id).filter(|x| !x.construction) {
            if let Shape::Slab(slab) = &x.shape
                && let Ok(body) = slab_body(slab, &plane, BODY_TOLERANCE)
            {
                out.push((x.id, Arc::new(body)));
            }
        }
    }
    out
}

/// The area around each room that is not construction, from the walls of its level that are not construction (doors
/// play no part). Rooms whose seed is in no enclosed area are left out.
pub(crate) fn room_regions(doc: &Document) -> Vec<(EntityId, Region)> {
    let mut out = Vec::new();
    for s in doc.sketches().filter(|s| s.level.is_some()) {
        let live = || doc.entities_in(s.id).filter(|x| !x.construction);
        let walls: Vec<Wall> = live()
            .filter_map(|x| match &x.shape {
                Shape::Wall(w) => Some(*w),
                _ => None,
            })
            .collect();
        let rooms: Vec<(EntityId, DVec2)> = live()
            .filter_map(|x| match &x.shape {
                Shape::Room(r) => Some((x.id, r.seed)),
                _ => None,
            })
            .collect();
        let found = regions(&walls, &rooms.iter().map(|r| r.1).collect::<Vec<_>>());
        out.extend(rooms.iter().zip(found).filter_map(|((id, _), r)| Some((*id, r?))));
    }
    out
}

#[cfg(test)]
#[path = "rebuild_walls_tests.rs"]
mod tests;
