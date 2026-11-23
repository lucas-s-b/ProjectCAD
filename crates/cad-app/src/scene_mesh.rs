//! GPU meshes of bodies for instanced drawing: built once per body `Arc` (triangles and feature edges) and kept while
//! the rebuild result holds that body, so a 3D scene build costs one instance per shown body or component.
use crate::app::CadApp;
use cad_document::Body;
use cad_renderer::{MeshData, Vertex};
use std::sync::Arc;

/// Edge colour of bodies and components.
pub const EDGE: [f32; 3] = [0.12, 0.12, 0.14];

/// The triangles (white, alpha 1: coloured and clipped per instance) and feature edges of `body`, in its own coordinates.
pub fn mesh_data(body: &Body) -> MeshData {
    let m = &body.mesh;
    let tris = m.indices.iter().zip(&m.normals).map(|(&i, n)| Vertex { pos: m.positions[i as usize].as_vec3().to_array(), normal: n.as_vec3().to_array(), color: [1.0; 4] }).collect();
    let edge = [EDGE[0], EDGE[1], EDGE[2], 1.0];
    let lines = body.feature_edges().into_iter().flatten().map(|p| Vertex { pos: p.as_vec3().to_array(), normal: [0.0; 3], color: edge }).collect();
    MeshData { tris, lines }
}

/// Drops the meshes of bodies no longer in the model and builds those of new bodies.
pub fn refresh(app: &mut CadApp) {
    let live: Vec<Arc<Body>> = app.model.iter().flat_map(|(_, m)| &m.bodies).map(|(_, b)| b.clone()).collect();
    app.scene_meshes.retain(|(b, ..)| live.iter().any(|l| Arc::ptr_eq(b, l)));
    for body in live {
        if !app.scene_meshes.iter().any(|(b, ..)| Arc::ptr_eq(b, &body)) {
            app.mesh_key += 1;
            let data = Arc::new(mesh_data(&body));
            app.scene_meshes.push((body, app.mesh_key, data));
        }
    }
}

/// The key of `body`'s mesh; None for a body `refresh` has not seen.
pub fn key(app: &CadApp, body: &Arc<Body>) -> Option<u64> {
    app.scene_meshes.iter().find(|(b, ..)| Arc::ptr_eq(b, body)).map(|(_, k, _)| *k)
}

#[cfg(test)]
#[path = "scene_mesh_tests.rs"]
mod tests;
