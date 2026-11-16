use crate::boolean::to_manifold;
use crate::{Body, BodyFace, BoolOp, ModelError, TriMesh, boolean};
use cad_topology::Surface;
use glam::{DAffine3, DVec3};
use manifold_rust::manifold::Manifold;
use manifold_rust::types::{Error, MeshGL64};
use std::collections::BTreeSet;
use std::f64::consts::{PI, TAU};

const LON: usize = 16;
const LAT: usize = 8;
/// Largest distance from the picked point to the face it opens.
const PICK: f64 = 0.05;
const MIN_THICKNESS: f64 = 1e-6;

/// UV sphere of `LON` x `LAT` segments; the poles and the equator points lie on the coordinate axes, so a
/// flat face moves by exactly `r` when a solid is eroded by it.
fn tool(r: f64) -> Result<Manifold, ModelError> {
    let snap = |v: f64| if v.abs() < 1e-12 * r { 0.0 } else { v };
    let mut verts = vec![DVec3::new(0.0, 0.0, r), DVec3::new(0.0, 0.0, -r)];
    for k in 1..LAT {
        let (s, c) = (k as f64 * PI / LAT as f64).sin_cos();
        for j in 0..LON {
            let (sj, cj) = (j as f64 * TAU / LON as f64).sin_cos();
            verts.push(DVec3::new(snap(r * s * cj), snap(r * s * sj), snap(r * c)));
        }
    }
    let ring = |k: usize, j: usize| (2 + (k - 1) * LON + j % LON) as u64;
    let mut tris: Vec<u64> = Vec::new();
    for j in 0..LON {
        tris.extend([0, ring(1, j), ring(1, j + 1)]);
        tris.extend([1, ring(LAT - 1, j + 1), ring(LAT - 1, j)]);
        for k in 1..LAT - 1 {
            let (a, b, c, d) = (ring(k, j), ring(k, j + 1), ring(k + 1, j + 1), ring(k + 1, j));
            tris.extend([a, d, c, a, c, b]);
        }
    }
    let mesh = MeshGL64 { num_prop: 3, vert_properties: verts.iter().flat_map(|p| p.to_array()).collect(), tri_verts: tris, ..Default::default() };
    let m = Manifold::from_mesh_gl64(&mesh);
    if m.status() == Error::NoError { Ok(m) } else { Err(ModelError::Invalid("offset tool failed")) }
}

/// The body eroded by the tool (Minkowski difference). A face parallel to an original planar face, `t` inside it,
/// lies on a plane of the offset body; the rest is faceted.
fn inner_body(body: &Body, t: f64, tol: f64) -> Result<Body, ModelError> {
    let too_large = ModelError::Invalid("shell thickness is too large for this body");
    let eroded = to_manifold(body, 0)?.minkowski_difference(&tool(t)?);
    if eroded.status() != Error::NoError {
        return Err(ModelError::Invalid("offset operation failed"));
    }
    let g = eroded.get_mesh_gl64(-1);
    let np = g.num_prop as usize;
    if g.tri_verts.is_empty() || np < 3 {
        return Err(too_large);
    }
    let positions: Vec<DVec3> = g.vert_properties.chunks(np).map(|c| DVec3::new(c[0], c[1], c[2])).collect();
    let indices: Vec<u32> = g.tri_verts.iter().map(|&i| i as u32).collect();
    let tri_face = vec![0; indices.len() / 3];
    let faces = vec![BodyFace { surface: Surface::Faceted, outward: true }];
    let mut inner = Body { mesh: TriMesh { positions, indices, normals: Vec::new() }, tri_face, faces };
    if !inner.mesh.is_closed() || inner.volume() <= 1e-9 {
        return Err(too_large);
    }
    for face in &body.faces {
        let Surface::Plane { origin, normal } = face.surface else { continue };
        let out = if face.outward { normal } else { -normal };
        let moved = origin - out * t;
        let id = inner.faces.len() as u32;
        let mut used = false;
        for (k, tri) in inner.mesh.indices.as_chunks::<3>().0.iter().enumerate() {
            let [a, b, c] = tri.map(|i| inner.mesh.positions[i as usize]);
            let aligned = (b - a).cross(c - a).normalize_or_zero().dot(out) > 1.0 - 1e-9;
            if aligned && [a, b, c].iter().all(|p| (*p - moved).dot(out).abs() <= tol) {
                inner.tri_face[k] = id;
                used = true;
            }
        }
        if used {
            inner.faces.push(BodyFace { surface: Surface::Plane { origin: moved, normal }, outward: face.outward });
        }
    }
    inner.merge_faces();
    inner.recompute_normals();
    Ok(inner)
}

/// Closest point of triangle `abc` to `p`.
fn closest_on_triangle(p: DVec3, [a, b, c]: [DVec3; 3]) -> DVec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

impl Body {
    /// The triangle nearest to `p` and its distance.
    fn nearest_triangle(&self, p: DVec3) -> Option<(usize, f64)> {
        let dist = |t: &[u32; 3]| closest_on_triangle(p, t.map(|i| self.mesh.positions[i as usize])).distance(p);
        self.mesh.indices.as_chunks::<3>().0.iter().enumerate().map(|(k, t)| (k, dist(t))).min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Distance from `p` to the surface of the body.
    pub fn surface_distance(&self, p: DVec3) -> f64 {
        self.nearest_triangle(p).map_or(f64::INFINITY, |t| t.1)
    }
}

/// Outward normal of the planar face nearest to `p`.
fn opened_normal(body: &Body, p: DVec3) -> Result<DVec3, ModelError> {
    let not_found = ModelError::Invalid("face not found; pick it again");
    let (best, d) = body.nearest_triangle(p).ok_or(not_found.clone())?;
    if d > PICK {
        return Err(not_found);
    }
    let face = &body.faces[body.tri_face[best] as usize];
    match face.surface {
        Surface::Plane { normal, .. } => Ok(if face.outward { normal } else { -normal }),
        _ => Err(ModelError::Invalid("only a planar face can be opened")),
    }
}

/// Number of pieces of the mesh, joined through shared vertices.
fn components(mesh: &TriMesh) -> usize {
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut parent: Vec<usize> = (0..mesh.positions.len()).collect();
    for t in mesh.indices.as_chunks::<3>().0 {
        let a = root(&mut parent, t[0] as usize);
        for &v in &t[1..] {
            let r = root(&mut parent, v as usize);
            parent[r] = a;
        }
    }
    mesh.indices.iter().map(|&i| root(&mut parent, i as usize)).collect::<BTreeSet<_>>().len()
}

/// Hollows the body, leaving walls of `thickness`. With `open = Some(p)`, the planar face nearest to `p` is removed so the cavity is open there.
/// `tol` is the distance within which an offset face counts as lying on an offset copy of an original plane.
// shortcut: the offset tool is a faceted sphere, so walls next to curved or slanted faces are thicker than asked by up to the facet error; replace with exact surface offsets when trimmed faces exist
pub fn shell(body: &Body, thickness: f64, open: Option<DVec3>, tol: f64) -> Result<Body, ModelError> {
    if !thickness.is_finite() || thickness <= MIN_THICKNESS {
        return Err(ModelError::Invalid("shell thickness must be positive"));
    }
    let normal = open.map(|p| opened_normal(body, p)).transpose()?;
    let inner = inner_body(body, thickness, tol)?;
    let cavity = match normal {
        None => inner,
        // The copy pokes out through the face; it only meets the inner solid while that is thicker than twice the wall.
        Some(n) => boolean(&inner, &inner.transformed(&DAffine3::from_translation(n * 2.0 * thickness))?, BoolOp::Union)?,
    };
    let result = boolean(body, &cavity, BoolOp::Difference)?;
    if normal.is_some() && components(&result.mesh) != 1 {
        return Err(ModelError::Invalid("shell thickness is too large to open this face"));
    }
    if result.volume() <= 0.0 {
        return Err(ModelError::Invalid("shell thickness is too large for this body"));
    }
    Ok(result)
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
