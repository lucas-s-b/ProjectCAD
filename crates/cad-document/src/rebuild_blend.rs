use crate::feature::{BODY_TOLERANCE, EDGE_PICK_TOLERANCE};
use crate::rebuild::{Bodies, hash_of};
use cad_geometry::DVec3;
use cad_modeler::{chamfer_edge, fillet_edge};
use std::sync::Arc;

/// A fillet or chamfer with its size evaluated.
pub(crate) struct Blend {
    pub edges: Vec<DVec3>,
    pub size: f64,
    pub fillet: bool,
}

impl Blend {
    pub fn key(&self, prev: u64) -> u64 {
        let points: Vec<u64> = self.edges.iter().flat_map(|p| p.to_array()).map(f64::to_bits).collect();
        hash_of((prev, if self.fillet { "fillet" } else { "chamfer" }, self.size.to_bits(), points))
    }

    /// The body list after the blend; `bodies` is not touched, so an error leaves it as it was.
    pub fn apply(&self, bodies: &Bodies) -> Result<Bodies, String> {
        let mut out = bodies.clone();
        for &p in &self.edges {
            let closest = out
                .iter()
                .enumerate()
                .filter_map(|(i, (_, b))| b.nearest_edge(p, EDGE_PICK_TOLERANCE).map(|e| (i, e)))
                .min_by(|x, y| x.1.distance(p).total_cmp(&y.1.distance(p)));
            let (i, edge) = closest.ok_or("edge not found; pick it again")?;
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
