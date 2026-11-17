use crate::feature::FeatureId;
use crate::rebuild::Bodies;
use cad_geometry::DVec3;
use cad_modeler::{Body, BodyEdge, BodyFace, Plane3, Surface};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Which way a body face points, without where it lies. Picks keep it so the face can be found again after
/// earlier features move or resize it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FaceKey {
    /// Unit normal pointing out of the solid.
    Plane(DVec3),
    /// Unit axis; its sign does not matter.
    Cylinder(DVec3),
    /// Any other surface.
    Other,
}

/// Unit vectors closer than this are the same direction.
const KEY_TOL: f64 = 1e-6;
/// mm: a point this close outside a triangle still counts as inside.
const CONTAIN_TOL: f64 = 1e-6;
pub(crate) const FACE_ERROR: &str = "face not found; pick it again";

impl FaceKey {
    /// The key of `face`; plane normals are flipped to point out of the solid.
    pub fn of(face: &BodyFace) -> FaceKey {
        match &face.surface {
            Surface::Plane { normal, .. } => FaceKey::Plane(if face.outward { *normal } else { -*normal }.normalize()),
            Surface::Cylinder { axis, .. } => FaceKey::Cylinder(axis.normalize()),
            _ => FaceKey::Other,
        }
    }

    /// Same kind and the same direction within `KEY_TOL` (cylinder axes in either sign).
    pub fn matches(&self, other: &FaceKey) -> bool {
        match (self, other) {
            (FaceKey::Plane(a), FaceKey::Plane(b)) => a.distance(*b) <= KEY_TOL,
            (FaceKey::Cylinder(a), FaceKey::Cylinder(b)) => a.cross(*b).length() <= KEY_TOL,
            (FaceKey::Other, FaceKey::Other) => true,
            _ => false,
        }
    }
}

/// The keys of the two faces of `edge`, an edge of `body`, in the order of `edge.faces`.
pub fn edge_keys(body: &Body, edge: &BodyEdge) -> [FaceKey; 2] {
    edge.faces.map(|f| FaceKey::of(&body.faces[f as usize]))
}

/// The edge of `body` between faces that match `keys` (in either order) nearest to `at`, and its distance to `at`.
pub(crate) fn find_edge(body: &Body, keys: &[FaceKey; 2], at: DVec3) -> Option<(BodyEdge, f64)> {
    let [k0, k1] = keys;
    body.edges()
        .into_iter()
        .filter(|e| {
            let [f, g] = edge_keys(body, e);
            (k0.matches(&f) && k1.matches(&g)) || (k0.matches(&g) && k1.matches(&f))
        })
        .map(|e| {
            let d = e.distance(at);
            (e, d)
        })
        .min_by(|x, y| x.1.total_cmp(&y.1))
}

/// The plane of a sketch on a face, moved along its normal onto that face as it is in `bodies` now. The face is
/// the planar face pointing the same way on which `at`, moved along the normal, falls; the nearest to `at` when
/// several do. Body `body` is searched; every body when it no longer exists. `Err(FACE_ERROR)` when none fits.
pub(crate) fn face_plane(plane: Plane3, at: DVec3, body: FeatureId, bodies: &Bodies) -> Result<Plane3, String> {
    let n = plane.normal();
    let own: Vec<&Arc<Body>> = bodies.iter().filter(|(id, _)| *id == body).map(|(_, b)| b).collect();
    let search = if own.is_empty() { bodies.iter().map(|(_, b)| b).collect() } else { own };
    let d = search
        .iter()
        .filter_map(|b| plane_face_offset(b, n, at))
        .min_by(|x, y| (x - n.dot(at)).abs().total_cmp(&(y - n.dot(at)).abs()))
        .ok_or(FACE_ERROR)?;
    Ok(Plane3 { origin: plane.origin + n * (d - n.dot(at)), ..plane })
}

/// Offset `n · p` of the planar face of `body` with outward normal `n` that contains `at` moved along `n`;
/// the nearest to `at` when several do.
fn plane_face_offset(body: &Body, n: DVec3, at: DVec3) -> Option<f64> {
    let key = FaceKey::Plane(n);
    let tris = body.mesh.indices.as_chunks::<3>().0;
    let mut best: Option<f64> = None;
    for (fi, face) in body.faces.iter().enumerate() {
        let Surface::Plane { origin, .. } = &face.surface else { continue };
        if !key.matches(&FaceKey::of(face)) {
            continue;
        }
        let d = n.dot(*origin);
        let q = at + n * (d - n.dot(at));
        let inside = tris
            .iter()
            .zip(&body.tri_face)
            .any(|(t, &f)| f as usize == fi && in_triangle(q, t.map(|i| body.mesh.positions[i as usize]), n));
        if inside && best.is_none_or(|b| (d - n.dot(at)).abs() < (b - n.dot(at)).abs()) {
            best = Some(d);
        }
    }
    best
}

/// `q`, in the plane of triangle `t`, lies inside it or within `CONTAIN_TOL` of it.
fn in_triangle(q: DVec3, t: [DVec3; 3], n: DVec3) -> bool {
    let [a, b, c] = t;
    let s = (b - a).cross(c - a).dot(n).signum();
    s != 0.0 && [(a, b), (b, c), (c, a)].iter().all(|&(p, r)| s * (r - p).cross(q - p).dot(n) >= -CONTAIN_TOL * (r - p).length())
}

#[cfg(test)]
#[path = "refs_tests.rs"]
mod tests;
