use crate::feature::{BODY_TOLERANCE, EDGE_PICK_TOLERANCE};
use crate::rebuild::{Bodies, hash_of};
use cad_geometry::DVec3;
use cad_modeler::shell;
use std::sync::Arc;

/// A shell with its thickness evaluated.
pub(crate) struct Shell {
    pub thickness: f64,
    pub open: Option<DVec3>,
    pub at: DVec3,
}

impl Shell {
    pub fn key(&self, prev: u64) -> u64 {
        let point = |p: DVec3| p.to_array().map(f64::to_bits);
        hash_of((prev, "shell", self.thickness.to_bits(), self.open.map(point), point(self.at)))
    }

    /// The body list after the shell; `bodies` is not touched, so an error leaves it as it was.
    pub fn apply(&self, bodies: &Bodies) -> Result<Bodies, String> {
        let nearest = bodies
            .iter()
            .enumerate()
            .map(|(i, (_, b))| (i, b.surface_distance(self.at)))
            .filter(|(_, d)| *d <= EDGE_PICK_TOLERANCE)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let (i, _) = nearest.ok_or("body not found; pick the face again")?;
        let mut out = bodies.clone();
        out[i].1 = Arc::new(shell(&bodies[i].1, self.thickness, self.open, BODY_TOLERANCE).map_err(|e| e.to_string())?);
        Ok(out)
    }
}
