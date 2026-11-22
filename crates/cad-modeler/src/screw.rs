//! Screw-threaded rods built as a mesh: rings of vertices whose diagonals follow the helix, so a thread profile sampled
//! once per facet keeps its corners exactly.
use crate::{Body, BodyFace, ModelError, Plane3, Surface, TriMesh};
use std::f64::consts::TAU;

/// The rod of a right-hand screw thread about the normal of `plane` through its origin, from height `z0` to `z1`
/// (z0 < z1) along the normal, closed by flat ends. `radii` (n >= 3 values, all positive) is the thread profile over
/// one pitch: the radius at the axial positions k pitch / n (k = 0..n), linear between them and repeating every pitch.
/// The surface point at angle θ (from `plane.x` towards `plane.y`) and height z has radius R((z - pitch θ / 2π) mod
/// pitch): a crest line rises one pitch per turn and the profile starts at the plane origin on `plane.x`. Every ring
/// has n vertices at the angles 2πk/n; rings lie at z0, at z1 and at every height i pitch / n between them (a lattice
/// height closer than a quarter step to an end is skipped). Each quad is split along its diagonal from (k, i) to
/// (k + 1, i + 1), which lies on a helix, so every vertex of a lattice ring takes a profile sample exactly. Faces: the
/// flat end at z0, the threaded face (`Surface::Faceted`), the flat end at z1.
// shortcut: the threaded face is one Faceted face, so crest corners shade soft; give each profile run its own face if sharp shading is wanted
pub fn screw(radii: &[f64], pitch: f64, plane: &Plane3, z0: f64, z1: f64) -> Result<Body, ModelError> {
    let n = radii.len();
    let finite = pitch.is_finite() && z0.is_finite() && z1.is_finite();
    if n < 3 || !finite || pitch <= 0.0 || z1 - z0 <= 1e-9 || radii.iter().any(|r| !(r.is_finite() && *r > 1e-9)) {
        return Err(ModelError::Invalid("screw needs at least 3 positive radii, a positive pitch and z0 < z1"));
    }
    let (nf, dz, axis) = (n as f64, pitch / n as f64, plane.normal());
    // Ring heights in steps of dz: the two ends and the whole steps between them.
    let (a, b) = ((z0 / dz + 0.25).floor() as i64 + 1, (z1 / dz - 0.25).ceil() as i64 - 1);
    let mut rows = vec![z0 / dz];
    rows.extend((a..=b).map(|i| i as f64));
    rows.push(z1 / dz);
    // Radius at height s (in steps) on the generator line at angle step k.
    let radius = |s: f64, k: usize| {
        let u = (s - k as f64).rem_euclid(nf);
        let (i, f) = (u.floor() as usize % n, u - u.floor());
        radii[i] * (1.0 - f) + radii[(i + 1) % n] * f
    };
    let mut positions = vec![plane.origin + axis * z0];
    for &s in &rows {
        positions.extend((0..n).map(|k| {
            let t = TAU * k as f64 / nf;
            plane.origin + (plane.x * t.cos() + plane.y * t.sin()) * radius(s, k) + axis * (s * dz)
        }));
    }
    let top = positions.len() as u32;
    positions.push(plane.origin + axis * z1);
    let m = rows.len();
    let v = |r: usize, k: usize| (1 + r * n + k % n) as u32;
    let (mut indices, mut tri_face) = (Vec::new(), Vec::new());
    for k in 0..n {
        indices.extend([0, v(0, k + 1), v(0, k)]);
        tri_face.push(0);
        for r in 0..m - 1 {
            indices.extend([v(r, k), v(r, k + 1), v(r + 1, k + 1), v(r, k), v(r + 1, k + 1), v(r + 1, k)]);
            tri_face.extend([1, 1]);
        }
        indices.extend([v(m - 1, k), v(m - 1, k + 1), top]);
        tri_face.push(2);
    }
    let faces = vec![
        BodyFace { surface: Surface::Plane { origin: plane.origin + axis * z0, normal: -axis }, outward: true },
        BodyFace { surface: Surface::Faceted, outward: true },
        BodyFace { surface: Surface::Plane { origin: plane.origin + axis * z1, normal: axis }, outward: true },
    ];
    let mut body = Body { mesh: TriMesh { positions, indices, normals: Vec::new() }, tri_face, faces };
    if !body.mesh.is_closed() || body.mesh.volume() <= 0.0 {
        return Err(ModelError::Invalid("screw did not produce a closed solid"));
    }
    body.recompute_normals();
    Ok(body)
}

#[cfg(test)]
#[path = "screw_tests.rs"]
mod tests;
