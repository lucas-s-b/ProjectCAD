//! Fillet and chamfer of one edge, made with a boolean: a tool solid is built along the edge whose
//! cross-section is the corner region outside the fillet arc (or the chamfer triangle). It is subtracted
//! on a convex edge and added on a concave edge. Corners where several filleted edges meet are not blended.
use crate::edges::point_segment;
use crate::{BodyEdge, BoolOp, Body, Convexity, EdgeGeom, ModelError, Plane3, boolean, extrude, revolve};
use cad_geometry::profile::Profile;
use cad_geometry::shape::Shape;
use glam::{DQuat, DVec2, DVec3};
use std::f64::consts::{PI, TAU};

/// The tool is tessellated this fine relative to the blend size: the facet error must stay far below a few percent of the blend volume.
const TOOL_FINENESS: f64 = 1e-4;
const NOT_YET: ModelError = ModelError::Invalid("this edge cannot be blended yet");
const TOO_LARGE: ModelError = ModelError::Invalid("blend size too large for this edge");

#[derive(Clone, Copy)]
enum Kind {
    Fillet,
    Chamfer,
}

pub fn fillet_edge(body: &Body, edge: &BodyEdge, radius: f64, tol: f64) -> Result<Body, ModelError> {
    blend(body, edge, Kind::Fillet, radius, tol)
}

pub fn chamfer_edge(body: &Body, edge: &BodyEdge, distance: f64, tol: f64) -> Result<Body, ModelError> {
    blend(body, edge, Kind::Chamfer, distance, tol)
}

/// The third vertex of the triangle of `face` that has the segment `p`-`q` as a side.
fn third_vertex(body: &Body, face: u32, p: DVec3, q: DVec3) -> Option<DVec3> {
    let pos = |i: u32| body.mesh.positions[i as usize];
    let corners = body.mesh.indices.as_chunks::<3>().0.iter().zip(&body.tri_face).filter(|(_, f)| **f == face);
    corners.into_iter().find_map(|(t, _)| {
        let a = t.iter().position(|&i| pos(i).distance(p) < 1e-9)?;
        let b = t.iter().position(|&i| pos(i).distance(q) < 1e-9)?;
        (0..3).find(|&k| k != a && k != b).map(|k| pos(t[k]))
    })
}

/// A point of the edge, its direction there, and the unit directions into both faces, perpendicular to the edge.
fn cross_section(body: &Body, edge: &BodyEdge) -> Result<(DVec3, DVec3, DVec3, DVec3), ModelError> {
    let mid = edge.midpoint();
    let (m, t) = match edge.geom {
        EdgeGeom::Line { a, b } => (mid, (b - a).normalize()),
        EdgeGeom::Circle { center, axis, radius, .. } => {
            let r = (mid - center).reject_from(axis).normalize();
            (center + r * radius, axis.cross(r))
        }
        EdgeGeom::Other => return Err(NOT_YET),
    };
    let (p, q) = edge.segments().min_by(|x, y| point_segment(mid, x.0, x.1).total_cmp(&point_segment(mid, y.0, y.1))).ok_or(NOT_YET)?;
    let mut u = [DVec3::ZERO; 2];
    for (k, &f) in edge.faces.iter().enumerate() {
        let third = third_vertex(body, f, p, q).ok_or(NOT_YET)?;
        let dir = body.faces[f as usize].surface.normal_at(m).cross(t).normalize();
        u[k] = if dir.dot(third - m) < 0.0 { -dir } else { dir };
    }
    Ok((m, t, u[0], u[1]))
}

/// Section plane through `o` spanned by `ua` and `e2` (unit, perpendicular), the wedge between the faces.
struct Wedge {
    o: DVec3,
    ua: DVec3,
    e2: DVec3,
    theta: f64,
}

/// The tool's section in the 2D frame given by `to2`; `delta` pushes the two straight sides out of the wedge.
fn section(w: &Wedge, kind: Kind, size: f64, delta: f64, to2: &dyn Fn(DVec3) -> DVec2) -> Result<Profile, ModelError> {
    let (s, c, half) = (w.theta.sin(), w.theta.cos(), w.theta / 2.0);
    let cot = 1.0 / half.tan();
    let pt = |x: f64, y: f64| to2(w.o + w.ua * x + w.e2 * y);
    let setback = match kind {
        Kind::Fillet => size * cot,
        Kind::Chamfer => size,
    };
    let (corner, a1, ta) = (pt(-delta * cot, -delta), pt(setback, -delta), pt(setback, 0.0));
    let (tb, b1) = (pt(setback * c, setback * s), pt(setback * c - delta * s, setback * s + delta * c));
    let mut shapes = Vec::new();
    let line = |shapes: &mut Vec<Shape>, a: DVec2, b: DVec2| {
        if a.distance(b) > 1e-12 {
            shapes.push(Shape::Line { a, b });
        }
    };
    line(&mut shapes, corner, a1);
    line(&mut shapes, a1, ta);
    match kind {
        Kind::Chamfer => line(&mut shapes, ta, tb),
        Kind::Fillet => {
            let k = size / half.sin();
            let center = pt(k * half.cos(), k * half.sin());
            // The short arc, bulging toward the corner; `arc_through` runs counter-clockwise.
            let (from, to) = if (ta - center).perp_dot(tb - center) > 0.0 { (ta, tb) } else { (tb, ta) };
            shapes.push(Shape::arc_through(center, from, to));
        }
    }
    line(&mut shapes, tb, b1);
    line(&mut shapes, b1, corner);
    let refs: Vec<&Shape> = shapes.iter().collect();
    Ok(Profile::from_shapes(&refs)?)
}

fn blend(body: &Body, edge: &BodyEdge, kind: Kind, size: f64, tol: f64) -> Result<Body, ModelError> {
    if !(size.is_finite() && size > 1e-6) {
        return Err(ModelError::Invalid("blend size must be positive"));
    }
    if edge.convexity == Convexity::Smooth || edge.geom == EdgeGeom::Other {
        return Err(NOT_YET);
    }
    let (m, _, ua, ub) = cross_section(body, edge)?;
    let theta = ua.dot(ub).clamp(-1.0, 1.0).acos();
    if !(theta > 1e-3 && theta < PI - 1e-3) {
        return Err(NOT_YET);
    }
    let e2 = (ub - ua * ua.dot(ub)).normalize();
    let delta = (0.01 * size).clamp(1e-4, 0.05);
    // The section frame at the start of the edge; the faces are planes or surfaces of revolution, so it is the same all along.
    let (wedge, plane, sweep) = match edge.geom {
        EdgeGeom::Line { a, b } => {
            let d = (b - a).normalize();
            (Wedge { o: a, ua, e2, theta }, Plane3 { origin: a, x: ua, y: d.cross(ua) }, (b - a).length())
        }
        EdgeGeom::Circle { center, axis, radius, start, sweep } => {
            let r0 = (start - center).reject_from(axis).normalize();
            let phi = r0.cross((m - center).reject_from(axis)).dot(axis).atan2(r0.dot((m - center).reject_from(axis)));
            let rot = DQuat::from_axis_angle(axis, -phi);
            (Wedge { o: center + r0 * radius, ua: rot * ua, e2: rot * e2, theta }, Plane3 { origin: center, x: r0, y: axis }, sweep)
        }
        EdgeGeom::Other => return Err(NOT_YET),
    };
    let to2 = |p: DVec3| DVec2::new((p - plane.origin).dot(plane.x), (p - plane.origin).dot(plane.y));
    let profile = section(&wedge, kind, size, delta, &to2)?;
    // Area of the section without the margin and the length its centroid travels.
    let bare = section(&wedge, kind, size, 0.0, &to2)?;
    let (area, length) = match edge.geom {
        EdgeGeom::Line { .. } => (bare.area(), sweep),
        _ => (bare.area(), sweep.min(TAU) * bare.centroid().x),
    };
    let brep = match edge.geom {
        EdgeGeom::Line { .. } => extrude(&profile, &plane, sweep)?,
        _ => {
            if profile.outer.iter().any(|s| s.start().x <= 1e-6) {
                return Err(TOO_LARGE);
            }
            revolve(&profile, &plane, DVec2::ZERO, DVec2::Y, sweep.min(TAU)).map_err(|e| match e {
                ModelError::Invalid("profile must lie on one side of the axis without touching it") => TOO_LARGE,
                e => e,
            })?
        }
    };
    let tool = Body::from_brep(&brep, (size * TOOL_FINENESS).min(tol).max(1e-6))?;
    let convex = edge.convexity == Convexity::Convex;
    let result = boolean(body, &tool, if convex { BoolOp::Difference } else { BoolOp::Union })?;
    let change = if convex { body.volume() - result.volume() } else { result.volume() - body.volume() };
    // Mesh facets of the body edge add up to about `tol` times the blend size along the edge.
    if change < -1e-9 || change > 1.02 * area * length + tol * size * length + 1e-9 {
        return Err(TOO_LARGE);
    }
    Ok(result)
}

#[cfg(test)]
#[path = "blend_tests.rs"]
mod tests;
