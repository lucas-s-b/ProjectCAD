use crate::feature::FeatureId;
use crate::rebuild::{Bodies, hash_of};
use crate::refs::{FACE_ERROR, FaceRef, find_face};
use cad_modeler::{Plane3, draft_face};
use std::sync::Arc;

/// A draft with its neutral plane and angle evaluated.
pub(crate) struct Draft {
    pub body: FeatureId,
    pub faces: Vec<FaceRef>,
    pub plane: Plane3,
    pub angle: f64,
}

impl Draft {
    pub fn key(&self, prev: u64) -> u64 {
        let Plane3 { origin, x, y } = self.plane;
        let numbers: Vec<u64> = [origin, x, y].iter().flat_map(|v| v.to_array()).chain([self.angle]).map(f64::to_bits).collect();
        hash_of((prev, "draft", self.body, numbers, serde_json::to_string(&self.faces).unwrap_or_default()))
    }

    /// The body list after the draft; `bodies` is not touched, so an error leaves it as it was.
    pub fn apply(&self, bodies: &Bodies) -> Result<Bodies, String> {
        let i = bodies.iter().position(|(id, _)| *id == self.body).ok_or("body not found")?;
        // Every face is found on the input body: a drafted face no longer points the way its reference says.
        let mut found = self.faces.iter().map(|r| find_face(&bodies[i].1, r).ok_or(FACE_ERROR)).collect::<Result<Vec<_>, _>>()?;
        found.sort_unstable();
        found.dedup();
        let mut body = (*bodies[i].1).clone();
        for face in found {
            body = draft_face(&body, face, self.plane.origin, self.plane.normal(), self.angle).map_err(|e| e.to_string())?;
        }
        let mut out = bodies.clone();
        out[i].1 = Arc::new(body);
        Ok(out)
    }
}

#[cfg(test)]
#[path = "rebuild_draft_tests.rs"]
mod tests;
