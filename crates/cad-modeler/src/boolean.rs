use crate::{Body, BodyFace, ModelError, TriMesh};
use glam::DVec3;
use manifold_rust::manifold::Manifold;
use manifold_rust::types::{Error, MeshGL64};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoolOp {
    Union,
    Difference,
    Intersection,
}

const FAILED: ModelError = ModelError::Invalid("boolean operation failed");

/// `face_offset` is added to every face id so the two operands never share one.
pub(crate) fn to_manifold(body: &Body, face_offset: u64) -> Result<Manifold, ModelError> {
    let mesh = MeshGL64 {
        num_prop: 3,
        vert_properties: body.mesh.positions.iter().flat_map(|p| p.to_array()).collect(),
        tri_verts: body.mesh.indices.iter().map(|&i| u64::from(i)).collect(),
        face_id: body.tri_face.iter().map(|&f| u64::from(f) + face_offset).collect(),
        ..Default::default()
    };
    let m = Manifold::from_mesh_gl64(&mesh);
    if m.status() == Error::NoError { Ok(m) } else { Err(FAILED) }
}

// shortcut: booleans run on the tessellation, so curved faces take part as facets and intersection curves are polylines; replace with analytic surface intersections later
pub fn boolean(a: &Body, b: &Body, op: BoolOp) -> Result<Body, ModelError> {
    let (ma, mb) = (to_manifold(a, 0)?, to_manifold(b, a.faces.len() as u64)?);
    let result = match op {
        BoolOp::Union => ma.union(&mb),
        BoolOp::Difference => ma.difference(&mb),
        BoolOp::Intersection => ma.intersection(&mb),
    };
    if result.status() != Error::NoError {
        return Err(FAILED);
    }
    let g = result.get_mesh_gl64(-1);
    let np = g.num_prop as usize;
    if g.tri_verts.is_empty() {
        return Err(ModelError::Invalid("boolean result is empty"));
    }
    if np < 3 || g.face_id.len() * 3 != g.tri_verts.len() {
        return Err(FAILED);
    }
    // A cut tool's surface bounds the remaining solid from the other side.
    let flip_b = op == BoolOp::Difference;
    let all: Vec<BodyFace> = a
        .faces
        .iter()
        .cloned()
        .chain(b.faces.iter().map(|f| BodyFace { surface: f.surface.clone(), outward: f.outward != flip_b }))
        .collect();
    let mut renumber = vec![u32::MAX; all.len()];
    for &id in &g.face_id {
        *renumber.get_mut(id as usize).ok_or(FAILED)? = 0;
    }
    let mut faces = Vec::new();
    for (old, slot) in renumber.iter_mut().enumerate() {
        if *slot == 0 {
            *slot = faces.len() as u32;
            faces.push(all[old].clone());
        }
    }
    let tri_face: Vec<u32> = g.face_id.iter().map(|&id| renumber[id as usize]).collect();
    let positions: Vec<DVec3> = g.vert_properties.chunks(np).map(|c| DVec3::new(c[0], c[1], c[2])).collect();
    let indices: Vec<u32> = g.tri_verts.iter().map(|&i| i as u32).collect();
    let mesh = TriMesh { positions, indices, normals: Vec::new() };
    if !mesh.is_closed() {
        return Err(ModelError::Invalid("boolean result is not a closed solid"));
    }
    let mut body = Body { mesh, tri_face, faces };
    body.merge_faces();
    body.recompute_normals();
    Ok(body)
}

#[cfg(test)]
#[path = "boolean_tests.rs"]
mod tests;
