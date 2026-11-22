//! Draft of an existing planar face about a neutral plane, made by moving the face's vertices along its normal.
use crate::{Body, BodyFace, ModelError, Surface};
use glam::DVec3;

const NOT_FOUND: ModelError = ModelError::Invalid("face not found; pick it again");
const NOT_PLANE: ModelError = ModelError::Invalid("only a planar face can be drafted");
const BAD_ANGLE: ModelError = ModelError::Invalid("draft angle must be between -89 and 89 degrees");
const NOT_PARALLEL: ModelError = ModelError::Invalid("the face must be parallel to the pull direction");
const BAD_NEIGHBOUR: ModelError = ModelError::Invalid("faces next to a drafted face must be planes at right angles to it");
const TOO_STEEP: ModelError = ModelError::Invalid("draft angle too large for the face");
/// Unit vectors whose dot product is at most this are at right angles.
const RIGHT: f64 = 1e-6;
/// mm: vertices this close to the neutral plane stay.
const ON_PLANE: f64 = 1e-12;

/// `body` with face `face` (a plane parallel to `pull`) drafted by `angle` (radians, |angle| < 89°) about the neutral
/// plane through `origin` with normal `pull`: every vertex of the face at height h = (p - origin) · pull moves by
/// -h tan(angle) along the face's outward normal n, so a positive angle narrows the solid away from the neutral plane.
/// The face's new outward normal is n cos(angle) + pull sin(angle). Every other face sharing a moving vertex must be
/// a plane whose normal is at right angles to n (it contains the move, so it stays flat); anything else is refused.
/// Face indices do not change.
// shortcut: only flipped triangles are caught, not other self-intersections; check the whole mesh if thin walls get drafted
pub fn draft_face(body: &Body, face: usize, origin: DVec3, pull: DVec3, angle: f64) -> Result<Body, ModelError> {
    let f = body.faces.get(face).ok_or(NOT_FOUND)?;
    let Surface::Plane { origin: o, normal } = &f.surface else { return Err(NOT_PLANE) };
    let o = *o;
    if angle.is_nan() || angle.abs() >= 89f64.to_radians() {
        return Err(BAD_ANGLE);
    }
    let pull = pull.normalize();
    let n = if f.outward { *normal } else { -*normal }.normalize();
    if n.dot(pull).abs() > RIGHT {
        return Err(NOT_PARALLEL);
    }
    let (t, pos) = (angle.tan(), &body.mesh.positions);
    let tris = body.mesh.indices.as_chunks::<3>().0;
    // How far each vertex moves along n; only vertices of the face off the neutral plane move.
    let mut shift = vec![0.0; pos.len()];
    for (tri, &g) in tris.iter().zip(&body.tri_face) {
        if g as usize == face {
            for &i in tri {
                let h = (pos[i as usize] - origin).dot(pull);
                if h.abs() > ON_PLANE {
                    shift[i as usize] = -h * t;
                }
            }
        }
    }
    let moves = |tri: &[u32; 3]| tri.iter().any(|&i| shift[i as usize] != 0.0);
    for (tri, &g) in tris.iter().zip(&body.tri_face) {
        if g as usize == face || !moves(tri) {
            continue;
        }
        match &body.faces[g as usize].surface {
            Surface::Plane { normal, .. } if normal.normalize().dot(n).abs() <= RIGHT => {}
            _ => return Err(BAD_NEIGHBOUR),
        }
    }
    let mut out = body.clone();
    for (p, s) in out.mesh.positions.iter_mut().zip(&shift) {
        *p += n * *s;
    }
    let cross = |m: &[DVec3], tri: &[u32; 3]| {
        let [a, b, c] = tri.map(|i| m[i as usize]);
        (b - a).cross(c - a)
    };
    if tris.iter().any(|tri| moves(tri) && cross(pos, tri).dot(cross(&out.mesh.positions, tri)) <= 0.0) {
        return Err(TOO_STEEP);
    }
    // The plane through the face's line on the neutral plane: `o` moves like the face's points.
    let surface = Surface::Plane { origin: o - n * ((o - origin).dot(pull) * t), normal: n * angle.cos() + pull * angle.sin() };
    out.faces[face] = BodyFace { surface, outward: true };
    if out.mesh.volume() <= 0.0 {
        return Err(TOO_STEEP);
    }
    out.recompute_normals();
    Ok(out)
}

#[cfg(test)]
#[path = "draft_tests.rs"]
mod tests;
