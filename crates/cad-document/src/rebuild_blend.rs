use crate::feature::{BODY_TOLERANCE, EDGE_PICK_TOLERANCE};
use crate::rebuild::{Bodies, hash_of};
use crate::refs::{FaceKey, find_edge};
use cad_geometry::DVec3;
use cad_modeler::{chamfer_edge, fillet_edge};
use std::sync::Arc;

pub(crate) const EDGE_ERROR: &str = "edge not found; pick it again";

/// A fillet or chamfer with its size evaluated.
pub(crate) struct Blend {
    pub edges: Vec<DVec3>,
    /// Empty, or one face pair per edge (see `FeatureKind::Fillet`).
    pub faces: Vec<[FaceKey; 2]>,
    pub size: f64,
    pub fillet: bool,
}

impl Blend {
    pub fn key(&self, prev: u64) -> u64 {
        let points: Vec<u64> = self.edges.iter().flat_map(|p| p.to_array()).map(f64::to_bits).collect();
        hash_of((prev, if self.fillet { "fillet" } else { "chamfer" }, self.size.to_bits(), points, serde_json::to_string(&self.faces).unwrap_or_default()))
    }

    /// The body list after the blend; `bodies` is not touched, so an error leaves it as it was.
    pub fn apply(&self, bodies: &Bodies) -> Result<Bodies, String> {
        let mut out = bodies.clone();
        for (k, &p) in self.edges.iter().enumerate() {
            let keys = self.faces.get(k);
            let closest = out
                .iter()
                .enumerate()
                .filter_map(|(i, (_, b))| {
                    let found = match keys {
                        Some(keys) => find_edge(b, keys, p),
                        None => b.nearest_edge(p, EDGE_PICK_TOLERANCE).map(|e| {
                            let d = e.distance(p);
                            (e, d)
                        }),
                    };
                    found.map(|(e, d)| (i, e, d))
                })
                .min_by(|x, y| x.2.total_cmp(&y.2));
            let (i, edge, _) = closest.ok_or(EDGE_ERROR)?;
            let body = &out[i].1;
            let blended = if self.fillet {
                fillet_edge(body, &edge, self.size, BODY_TOLERANCE)
            } else {
                chamfer_edge(body, &edge, self.size, BODY_TOLERANCE)
            };
            out[i].1 = Arc::new(blended.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }
}

#[cfg(test)]
#[path = "rebuild_blend_tests.rs"]
mod tests;
