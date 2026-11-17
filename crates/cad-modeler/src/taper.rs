//! Extrusion with a draft angle. Every profile loop is moved to its left, into the material, by
//! `|distance| · tan(angle)` with mitred vertices; the moved loops are lifted by `distance` and every sample is joined
//! to its moved copy by a ruled quad. The profile outline comes from `sweep::outline`, so arcs are chords.
use crate::cap::triangulate;
use crate::extrude::flatten;
use crate::sweep::outline;
use crate::{Body, BodyFace, ModelError, Plane3, Surface, TriMesh};
use cad_geometry::profile::{Profile, Seg};
use glam::{DVec2, DVec3};
use std::f64::consts::FRAC_PI_2;

const TOO_STEEP: ModelError = ModelError::Invalid("draft angle too large for the profile");
const NOT_SOLID: ModelError = ModelError::Invalid("tapered extrusion did not produce a closed solid");

/// Extrudes `profile` from `plane` by `distance` along the plane normal (negative: against it) while every side wall
/// leans by `angle` (radians) about the plane: a positive angle narrows the solid away from the plane and widens its
/// holes, a negative one widens it. Walls of straight segments are `Surface::Plane`, walls of arcs `Surface::Faceted`.
// shortcut: arc walls are faceted, not cones; add `Surface::Cone` when STEP export of cones lands
pub fn extrude_tapered(profile: &Profile, plane: &Plane3, distance: f64, angle: f64, tol: f64) -> Result<Body, ModelError> {
    if !distance.is_finite() || distance.abs() <= 1e-9 {
        return Err(ModelError::Invalid("extrusion distance is zero"));
    }
    if !angle.is_finite() || angle.abs() >= FRAC_PI_2 - 1e-6 {
        return Err(ModelError::Invalid("draft angle must be between -90 and 90 degrees"));
    }
    if !tol.is_finite() || tol < 1e-6 {
        return Err(ModelError::Invalid("tessellation tolerance too small"));
    }
    // Outer loops run counter-clockwise and holes clockwise, so the material is on the left of every loop.
    let loops = outline(profile, tol, false);
    let bottom: Vec<Vec<DVec2>> = loops.iter().map(|l| l.iter().map(|s| s.0).collect()).collect();
    let top = bottom.iter().map(|l| offset_loop(l, distance.abs() * angle.tan())).collect::<Result<Vec<_>, _>>()?;
    check_top(&top)?;
    let lift = plane.normal() * distance;
    let m: usize = bottom.iter().map(Vec::len).sum();
    let mut positions: Vec<DVec3> = bottom.iter().flatten().map(|&p| plane.point(p)).collect();
    positions.extend(top.iter().flatten().map(|&p| plane.point(p) + lift));
    let seg: Vec<usize> = loops.iter().flatten().map(|s| s.1).collect();
    let mut next = Vec::with_capacity(m);
    let mut at = 0;
    for l in &bottom {
        next.extend((0..l.len()).map(|k| (at + (k + 1) % l.len()) as u32));
        at += l.len();
    }
    let segs = flatten(profile).segs;
    let mut faces = vec![BodyFace { surface: Surface::Faceted, outward: true }; segs.len()];
    let (mut indices, mut tri_face) = (Vec::new(), Vec::new());
    let up = distance > 0.0;
    for j in 0..m {
        let (a, b) = (j as u32, next[j]);
        let (c, d) = (b + m as u32, a + m as u32);
        // Material on the left of a loop and lifted along the normal: this winding faces out.
        let tris = if up { [[a, b, c], [a, c, d]] } else { [[a, c, b], [a, d, c]] };
        for t in tris {
            indices.extend(t);
            tri_face.push(seg[j] as u32);
        }
        if matches!(segs[seg[j]], Seg::Line { .. }) {
            let [p, q, r] = tris[0].map(|i| positions[i as usize]);
            faces[seg[j]].surface = Surface::Plane { origin: p, normal: (q - p).cross(r - p).normalize() };
        }
    }
    let out = if up { plane.normal() } else { -plane.normal() };
    for (rings, shift, out) in [(&bottom, 0, -out), (&top, m, out)] {
        let slot = faces.len() as u32;
        faces.push(BodyFace { surface: Surface::Plane { origin: positions[shift], normal: out }, outward: true });
        for t in triangulate(rings)? {
            let [a, b, c] = t.map(|k| (shift + k) as u32);
            let pos = |i: u32| positions[i as usize];
            let facing = (pos(b) - pos(a)).cross(pos(c) - pos(a)).dot(out);
            indices.extend(if facing < 0.0 { [a, c, b] } else { [a, b, c] });
            tri_face.push(slot);
        }
    }
    let mesh = TriMesh { positions, indices, normals: Vec::new() };
    if !mesh.is_closed() || mesh.volume() <= 0.0 {
        return Err(NOT_SOLID);
    }
    let mut body = Body { mesh, tri_face, faces };
    body.merge_faces();
    body.recompute_normals();
    Ok(body)
}

/// `pts`, a closed loop with the material on its left, moved left by `d`: every vertex onto the crossing of its two
/// moved edges. `TOO_STEEP` when an edge would turn around.
fn offset_loop(pts: &[DVec2], d: f64) -> Result<Vec<DVec2>, ModelError> {
    let m = pts.len();
    let left = |a: DVec2, b: DVec2| (b - a).normalize().perp();
    let mut out = Vec::with_capacity(m);
    for j in 0..m {
        let (n1, n2) = (left(pts[(j + m - 1) % m], pts[j]), left(pts[j], pts[(j + 1) % m]));
        let den = 1.0 + n1.dot(n2);
        // NaN from a zero-length edge fails every comparison, so test it explicitly.
        if den.is_nan() || den < 1e-9 {
            return Err(TOO_STEEP);
        }
        out.push(pts[j] + (n1 + n2) * (d / den));
    }
    if (0..m).any(|j| (out[(j + 1) % m] - out[j]).dot(pts[(j + 1) % m] - pts[j]) <= 0.0) {
        return Err(TOO_STEEP);
    }
    Ok(out)
}

/// `TOO_STEEP` when two edges of the moved loops cross, or the first point of a moved hole is not inside the moved
/// outer loop (`top[0]`).
fn check_top(top: &[Vec<DVec2>]) -> Result<(), ModelError> {
    let edges: Vec<(DVec2, DVec2)> = top.iter().flat_map(|r| (0..r.len()).map(move |j| (r[j], r[(j + 1) % r.len()]))).collect();
    let side = |o: DVec2, a: DVec2, b: DVec2| (a - o).perp_dot(b - o);
    for (i, &(a, b)) in edges.iter().enumerate() {
        for &(c, e) in &edges[i + 1..] {
            if side(a, b, c) * side(a, b, e) < 0.0 && side(c, e, a) * side(c, e, b) < 0.0 {
                return Err(TOO_STEEP);
            }
        }
    }
    for hole in &top[1..] {
        if !inside(hole[0], &top[0]) {
            return Err(TOO_STEEP);
        }
    }
    Ok(())
}

/// `p` lies inside the closed polygon `ring` (even-odd rule).
fn inside(p: DVec2, ring: &[DVec2]) -> bool {
    let mut odd = false;
    for j in 0..ring.len() {
        let (a, b) = (ring[j], ring[(j + 1) % ring.len()]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            odd = !odd;
        }
    }
    odd
}

#[cfg(test)]
#[path = "taper_tests.rs"]
mod tests;
