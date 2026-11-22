//! Roof and stair solids.
use crate::solid::wall_body;
use cad_geometry::{DVec2, Roof, Stair};
use cad_modeler::{Body, BoolOp, ModelError, Plane3, TriMesh, boolean};

/// The solid of `r` on `plane` (its level), tessellated within `tol`: its underside meets the eave line `top` above
/// the plane (the storey height). Per roof plane, a tall prism over its piece of the plan cut by a sheet `thickness`
/// thick lying on the sloped plane through the eave line; a flat roof is its eaves outline extruded up from `top`.
pub fn roof_body(r: &Roof, plane: &Plane3, top: f64, tol: f64) -> Result<Body, ModelError> {
    let n = plane.normal();
    let pieces = r.pieces();
    let pts = || pieces.iter().flat_map(|p| p.outline.iter().copied());
    let lo = pts().fold(DVec2::INFINITY, DVec2::min);
    let hi = pts().fold(DVec2::NEG_INFINITY, DVec2::max);
    // Reaches across the whole roof from any of its points.
    let size = (hi - lo).length() + 1.0;
    let mut bodies = Vec::new();
    for piece in &pieces {
        let Some((a, d)) = piece.eave else {
            bodies.push(wall_body(&piece.outline, &Plane3 { origin: plane.origin + n * top, ..*plane }, r.thickness, tol)?);
            continue;
        };
        let pitch = r.pitch.to_radians();
        let (sin, cos) = pitch.sin_cos();
        let lift = size * pitch.tan() + r.thickness / cos + 1.0;
        let prism = wall_body(&piece.outline, &Plane3 { origin: plane.origin + n * (top - lift), ..*plane }, 2.0 * lift, tol)?;
        let up = d.perp();
        // x along the eave, y up the slope; its normal points up and out, so the sheet grows upward from the underside.
        let slope = Plane3 { origin: plane.point(a) + n * top, x: plane.x * d.x + plane.y * d.y, y: (plane.x * up.x + plane.y * up.y) * cos + n * sin };
        let s = size / cos + 1.0;
        let sheet = wall_body(&[DVec2::new(-s, -s), DVec2::new(s, -s), DVec2::new(s, s), DVec2::new(-s, s)], &slope, r.thickness, tol)?;
        bodies.push(boolean(&prism, &sheet, BoolOp::Intersection)?);
    }
    if bodies.is_empty() {
        return Err(ModelError::Invalid("roof has no planes"));
    }
    Ok(concat(bodies))
}

/// The solid of `s` on `plane` (its level), tessellated within `tol`: the stepped side profile (risers - 1 treads,
/// filled down to the floor; the last riser leads onto the floor above) extruded across the width.
pub fn stair_body(s: &Stair, plane: &Plane3, tol: f64) -> Result<Body, ModelError> {
    if s.risers < 2 {
        return Err(ModelError::Invalid("stair needs at least two risers"));
    }
    let (n, t, r) = (s.risers as usize, s.tread(), s.riser());
    let mut p = vec![DVec2::ZERO, DVec2::new(s.a.distance(s.b), 0.0)];
    for k in (0..n - 1).rev() {
        p.push(DVec2::new((k + 1) as f64 * t, (k + 1) as f64 * r));
        p.push(DVec2::new(k as f64 * t, (k + 1) as f64 * r));
    }
    let d = (s.b - s.a).normalize();
    // On the left side, x along the run, y up: its normal points right, across the width.
    let side = Plane3 { origin: plane.point(s.a + d.perp() * (s.width / 2.0)), x: plane.x * d.x + plane.y * d.y, y: plane.normal() };
    wall_body(&p, &side, s.width, tol)
}

/// The triangles and faces of `bodies` in one body. For solids that only touch: the volume is the sum.
// shortcut: no union, the touching faces stay inside; union them when IFC export needs one clean shell
fn concat(bodies: Vec<Body>) -> Body {
    let mut out = Body { mesh: TriMesh { positions: Vec::new(), indices: Vec::new(), normals: Vec::new() }, tri_face: Vec::new(), faces: Vec::new() };
    for b in bodies {
        let (first, face) = (out.mesh.positions.len() as u32, out.faces.len() as u32);
        out.mesh.indices.extend(b.mesh.indices.iter().map(|i| i + first));
        out.mesh.positions.extend(b.mesh.positions);
        out.mesh.normals.extend(b.mesh.normals);
        out.tri_face.extend(b.tri_face.iter().map(|f| f + face));
        out.faces.extend(b.faces);
    }
    out
}

#[cfg(test)]
#[path = "roof_tests.rs"]
mod tests;
