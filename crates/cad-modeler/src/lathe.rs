//! Solids of revolution whose outline touches the axis (shafts, pins), built as a mesh directly: `revolve` makes
//! B-Reps and needs a profile off the axis.
use crate::{Body, BodyFace, ModelError, Plane3, Surface, TriMesh};
use glam::DVec2;
use std::f64::consts::TAU;

/// The solid swept when `outline` turns once about the normal of `plane` through its origin. Outline points are
/// (radius, height): radius along `plane.x` at angle 0, height along the normal. The outline runs counter-clockwise
/// from a point on the axis (radius 0) to a last point on the axis; the points between have a positive radius and the
/// closing edge lies on the axis. Each turn has `segments` facets: every ring is a regular polygon with vertices at the
/// angles 2 pi k / segments, so the volume is (segments / 2) sin(2 pi / segments) times the integral of radius squared
/// over height. Every outline edge is one face: a plane (same height), a cylinder (same radius) or a cone.
pub fn lathe(outline: &[DVec2], plane: &Plane3, segments: usize) -> Result<Body, ModelError> {
    let m = outline.len();
    let bad = ModelError::Invalid("lathe outline must start and end on the axis with positive radii between");
    if m < 3 || segments < 3 || outline[0].x.abs() > 1e-12 || outline[m - 1].x.abs() > 1e-12 {
        return Err(bad);
    }
    if outline.iter().any(|p| !p.is_finite()) || outline[1..m - 1].iter().any(|p| p.x <= 1e-9) {
        return Err(bad);
    }
    let axis = plane.normal();
    let dir = |k: usize| {
        let a = TAU * (k % segments) as f64 / segments as f64;
        plane.x * a.cos() + plane.y * a.sin()
    };
    // Vertex 0: the first axis point; then one ring per inner outline point; last: the closing axis point.
    let mut positions = vec![plane.origin + axis * outline[0].y];
    for p in &outline[1..m - 1] {
        positions.extend((0..segments).map(|k| plane.origin + dir(k) * p.x + axis * p.y));
    }
    let last = positions.len() as u32;
    positions.push(plane.origin + axis * outline[m - 1].y);
    // Vertex k of the ring of outline point j (1..m-1).
    let ring = |j: usize, k: usize| (1 + (j - 1) * segments + k % segments) as u32;
    let (mut indices, mut tri_face, mut faces) = (Vec::new(), Vec::new(), Vec::new());
    for i in 0..m - 1 {
        let f = faces.len() as u32;
        let first = indices.len();
        for k in 0..segments {
            let tris = if i == 0 {
                vec![[0, ring(1, k + 1), ring(1, k)]]
            } else if i == m - 2 {
                vec![[ring(i, k), ring(i, k + 1), last]]
            } else {
                let (p, q, r, s) = (ring(i, k), ring(i, k + 1), ring(i + 1, k + 1), ring(i + 1, k));
                vec![[p, q, r], [p, r, s]]
            };
            for t in tris {
                indices.extend(t);
                tri_face.push(f);
            }
        }
        let (a, b) = (outline[i], outline[i + 1]);
        let surface = if (a.y - b.y).abs() <= 1e-12 {
            Surface::Plane { origin: plane.origin + axis * a.y, normal: if b.x < a.x { axis } else { -axis } }
        } else if (a.x - b.x).abs() <= 1e-12 {
            Surface::Cylinder { origin: plane.origin, axis, radius: a.x }
        } else {
            // The line through a and b meets the axis at the apex; the cone axis points from there towards the edge.
            let h = a.y + (b.y - a.y) * a.x / (a.x - b.x);
            let towards = if (a.y + b.y) / 2.0 > h { axis } else { -axis };
            Surface::Cone { apex: plane.origin + axis * h, axis: towards, half_angle: ((a.x - b.x) / (a.y - b.y)).abs().atan() }
        };
        let [p, q, r] = [0, 1, 2].map(|j| positions[indices[first + j] as usize]);
        let outward = surface.normal_at((p + q + r) / 3.0).dot((q - p).cross(r - p)) > 0.0;
        faces.push(BodyFace { surface, outward });
    }
    let mesh = TriMesh { positions, indices, normals: Vec::new() };
    if !mesh.is_closed() || mesh.volume() <= 0.0 {
        return Err(ModelError::Invalid("lathe outline must run counter-clockwise without crossing itself"));
    }
    let mut body = Body { mesh, tri_face, faces };
    body.merge_faces();
    body.recompute_normals();
    Ok(body)
}

#[cfg(test)]
#[path = "lathe_tests.rs"]
mod tests;
