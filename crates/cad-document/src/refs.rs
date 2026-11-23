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

/// A face named by which way it points and a point that lay on it when it was picked. Found again on every rebuild
/// (`find_face`), so it follows when earlier features move or resize the face.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FaceRef {
    pub key: FaceKey,
    pub at: DVec3,
}

impl FaceRef {
    /// The reference of face `face` of `body`: its key and the centroid of its largest triangle (a point on the face
    /// even when the face is not convex). None for an index out of range or a face without triangles.
    pub fn of(body: &Body, face: usize) -> Option<FaceRef> {
        let f = body.faces.get(face)?;
        let largest = body
            .mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .zip(&body.tri_face)
            .filter(|(_, g)| **g as usize == face)
            .map(|(t, _)| t.map(|i| body.mesh.positions[i as usize]))
            .max_by(|a, b| twice_area(a).total_cmp(&twice_area(b)))?;
        Some(FaceRef { key: FaceKey::of(f), at: (largest[0] + largest[1] + largest[2]) / 3.0 })
    }
}

fn twice_area([a, b, c]: &[DVec3; 3]) -> f64 {
    (*b - *a).cross(*c - *a).length()
}

/// Area centroid of the triangles of face `face`; None when it has none.
fn face_centroid(body: &Body, face: usize) -> Option<DVec3> {
    let (mut area, mut sum) = (0.0, DVec3::ZERO);
    for (t, &g) in body.mesh.indices.as_chunks::<3>().0.iter().zip(&body.tri_face) {
        if g as usize == face {
            let p = t.map(|i| body.mesh.positions[i as usize]);
            let w = twice_area(&p);
            area += w;
            sum += (p[0] + p[1] + p[2]) * (w / 3.0);
        }
    }
    (area > 0.0).then(|| sum / area)
}

/// The face of `body` that `r` names. A plane: the planar face pointing the same way on which `r.at`, moved along
/// the normal, falls (the nearest to `at` along the normal when several do), as for sketches on faces. A cylinder:
/// the cylindrical face with the same axis whose area centroid is nearest to `at`. None when nothing matches and
/// for `FaceKey::Other`.
// shortcut: cylinders are matched by the nearest centroid; track coaxial cylinders of one body by more than that if they get mixed up
pub(crate) fn find_face(body: &Body, r: &FaceRef) -> Option<usize> {
    match r.key {
        FaceKey::Plane(n) => plane_face(body, n, r.at).map(|(i, _)| i),
        FaceKey::Cylinder(_) => (0..body.faces.len())
            .filter(|&i| r.key.matches(&FaceKey::of(&body.faces[i])))
            .filter_map(|i| face_centroid(body, i).map(|c| (i, c.distance(r.at))))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i),
        FaceKey::Other => None,
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
        .filter_map(|b| plane_face(b, n, at).map(|(_, d)| d))
        .min_by(|x, y| (x - n.dot(at)).abs().total_cmp(&(y - n.dot(at)).abs()))
        .ok_or(FACE_ERROR)?;
    Ok(Plane3 { origin: plane.origin + n * (d - n.dot(at)), ..plane })
}

/// Index and offset `n · p` of the planar face of `body` with outward normal `n` that contains `at` moved along `n`;
/// the nearest to `at` when several do.
fn plane_face(body: &Body, n: DVec3, at: DVec3) -> Option<(usize, f64)> {
    let key = FaceKey::Plane(n);
    let tris = body.mesh.indices.as_chunks::<3>().0;
    let mut best: Option<(usize, f64)> = None;
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
        if inside && best.is_none_or(|b| (d - n.dot(at)).abs() < (b.1 - n.dot(at)).abs()) {
            best = Some((fi, d));
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
