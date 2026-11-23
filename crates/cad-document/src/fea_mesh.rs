//! Tetrahedral meshes of bodies for analysis: made on demand from the rebuild result, never stored.
use crate::document::Document;
use crate::feature::FeatureId;
use crate::rebuild::{RebuildResult, eval};
use crate::refs::{FACE_ERROR, FaceRef, find_face};
use cad_analysis::{Accuracy, MESHED, MeshStats, TetMesh, tet_mesh};
use cad_modeler::{Body, TriMesh};

/// A body's tetrahedral mesh with its numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshReport {
    pub body: FeatureId,
    /// The element size asked for, mm.
    pub h: f64,
    pub mesh: TetMesh,
    pub stats: MeshStats,
    /// Volume of the body's triangle mesh, mm³: what the mesh volume is compared with.
    pub body_volume: f64,
    /// Always `Approximate(MESHED)`.
    pub accuracy: Accuracy,
}

impl MeshReport {
    /// (mesh volume − body volume) / body volume; usually slightly negative.
    pub fn volume_error(&self) -> f64 {
        (self.stats.volume - self.body_volume) / self.body_volume
    }
}

impl Document {
    /// The body `body` of `result`, or "body 9 not found".
    fn result_body(result: &RebuildResult, body: FeatureId) -> Result<&Body, String> {
        result.bodies.iter().find(|(id, _)| *id == body).map(|(_, b)| &**b).ok_or_else(|| format!("body {body} not found"))
    }

    /// The tetrahedral mesh (`cad_analysis::tet_mesh`) of body `body` of `result` with element size `size`, a length
    /// expression ("5 mm", "h / 2"). Errors: "body 9 not found", the expression's error, the mesher's ("element size
    /// must be positive", too many grid cells, ...).
    pub fn mesh_body(&self, result: &RebuildResult, body: FeatureId, size: &str) -> Result<MeshReport, String> {
        let b = Self::result_body(result, body)?;
        let h = eval(self, size, false)?;
        let mesh = tet_mesh(&b.mesh, h)?;
        Ok(MeshReport { body, h, stats: mesh.stats(), mesh, body_volume: b.volume(), accuracy: Accuracy::Approximate(MESHED) })
    }

    /// The nodes of `report`'s mesh on face `face` of its body: within h / 4 of the face's triangles (`find_face`
    /// finds the face again, so it follows when earlier features change), ascending. E fixes or loads these nodes.
    /// Errors: "body 9 not found", "face not found; pick it again".
    pub fn face_nodes(&self, result: &RebuildResult, report: &MeshReport, face: &FaceRef) -> Result<Vec<u32>, String> {
        let b = Self::result_body(result, report.body)?;
        let f = find_face(b, face).ok_or_else(|| FACE_ERROR.to_string())?;
        let mut tris = TriMesh { positions: b.mesh.positions.clone(), indices: Vec::new(), normals: Vec::new() };
        for (t, g) in b.mesh.indices.as_chunks::<3>().0.iter().zip(&b.tri_face) {
            if *g as usize == f {
                tris.indices.extend_from_slice(t);
            }
        }
        Ok(report.mesh.nodes_near(&tris, report.h / 4.0))
    }
}

#[cfg(test)]
#[path = "fea_mesh_tests.rs"]
mod tests;
