use crate::document::{Document, EntityId};
use crate::feature::{BODY_TOLERANCE, Feature, FeatureKind};
use crate::rebuild::{Inputs, Tool};
use cad_geometry::profile::{Profile, chain};
use cad_geometry::{DVec3, Shape};
use cad_modeler::{PathPiece, Plane3, path_pieces};

/// The shapes of one group of entities (all in one sketch, checked when the feature is added) and that sketch's plane.
fn group<'a>(doc: &'a Document, ids: &[EntityId]) -> Result<(Vec<&'a Shape>, Plane3), String> {
    let shapes = ids
        .iter()
        .map(|&id| doc.get(id).map(|e| &e.shape).ok_or_else(|| format!("entity {id} not found")))
        .collect::<Result<Vec<_>, _>>()?;
    let sketch = ids.first().and_then(|id| doc.get(*id)).map_or(0, |e| e.sketch);
    Ok((shapes, doc.resolve_plane(sketch)?))
}

fn plane_key(p: &Plane3) -> [[f64; 3]; 3] {
    [p.origin, p.x, p.y].map(|v| v.to_array())
}

/// The path starts at the end that is nearer to the profile plane.
fn nearest_end_first(pieces: &mut [PathPiece], profile_plane: &Plane3) {
    let away = |p: Option<&DVec3>| p.map_or(0.0, |p| (*p - profile_plane.origin).dot(profile_plane.normal()).abs());
    let ends = (pieces.first().and_then(|p| p.points.first()), pieces.last().and_then(|p| p.points.last()));
    if away(ends.1) < away(ends.0) {
        pieces.reverse();
        pieces.iter_mut().for_each(|p| p.points.reverse());
    }
}

/// Inputs of a sweep or loft feature.
pub(crate) fn resolve_sweep(doc: &Document, f: &Feature) -> Result<Inputs, String> {
    let from_shapes = |shapes: &[&Shape]| Profile::from_shapes(shapes).map_err(|e| e.to_string());
    match &f.kind {
        FeatureKind::Sweep { profile, path, op, target } => {
            let ((shapes, plane), (path_shapes, path_plane)) = (group(doc, profile)?, group(doc, path)?);
            let (segs, closed) = chain(&path_shapes).map_err(|e| e.to_string())?;
            let mut pieces = path_pieces(&segs, &path_plane, BODY_TOLERANCE);
            if !closed {
                nearest_end_first(&mut pieces, &plane);
            }
            let shapes_json = serde_json::to_string(&(&shapes, &path_shapes, plane_key(&path_plane))).map_err(|e| e.to_string())?;
            let profile = from_shapes(&shapes)?;
            Ok(Inputs { profile, shapes_json, plane, tool: Tool::Sweep { path: pieces, closed }, op: *op, target: *target })
        }
        FeatureKind::Loft { sections, op, target } => {
            let mut built = Vec::new();
            let mut json = Vec::new();
            for ids in sections {
                let (shapes, plane) = group(doc, ids)?;
                built.push((from_shapes(&shapes)?, plane));
                json.push((shapes, plane_key(&plane)));
            }
            let shapes_json = serde_json::to_string(&json).map_err(|e| e.to_string())?;
            let (profile, plane) = built.first().cloned().ok_or("loft needs at least two sections")?;
            Ok(Inputs { profile, shapes_json, plane, tool: Tool::Loft { sections: built }, op: *op, target: *target })
        }
        _ => Err("not a solid feature".to_string()),
    }
}
