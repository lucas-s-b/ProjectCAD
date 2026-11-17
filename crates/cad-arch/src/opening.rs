//! Doors and windows in a wall: the box each one cuts from the wall solid, its gap in the plan, its plan symbol, and
//! the IFC class of architectural shapes.
use crate::solid::wall_body;
use cad_geometry::{DVec2, Opening, OpeningKind, Shape};
use cad_modeler::{Body, BoolOp, Plane3, boolean};
use std::f64::consts::FRAC_PI_2;

/// Segments of the quarter circle a door leaf sweeps in plan.
const ARC_SEGMENTS: usize = 16;

/// `Opening::span`, pushed one wall thickness past a wall end it reaches, so the cut goes through that end instead of
/// stopping in its plane (no coplanar faces in the boolean).
pub fn gap(o: &Opening) -> (f64, f64) {
    let (u0, u1) = o.span();
    let len = o.wall.a.distance(o.wall.b);
    let t = o.wall.thickness;
    (if u0 <= 1e-9 { u0 - t } else { u0 }, if u1 >= len - 1e-9 { u1 + t } else { u1 })
}

/// `body` (the solid of the wall the openings sit in, on `plane`) minus one box per opening: `gap` along the
/// baseline, one thickness past each face, from the sill to the head. An opening at the floor or reaching the wall
/// top goes one thickness past it. A cut that fails leaves the body as it was.
pub fn cut_openings(mut body: Body, openings: &[Opening], plane: &Plane3, tol: f64) -> Body {
    for o in openings {
        let w = &o.wall;
        let t = w.thickness;
        let d = (w.b - w.a).normalize();
        let (l, r) = w.faces();
        let (u0, u1) = gap(o);
        let at = |u: f64, s: f64| w.a + d * u + d.perp() * s;
        let rect = [at(u0, r - t), at(u1, r - t), at(u1, l + t), at(u0, l + t)];
        let z0 = if o.sill > 0.0 { o.sill } else { -t };
        let head = o.sill + o.height;
        let z1 = if head < w.height { head } else { w.height + t };
        let base = Plane3 { origin: plane.origin + plane.normal() * z0, ..*plane };
        if let Ok(cutter) = wall_body(&rect, &base, z1 - z0, tol)
            && let Ok(cut) = boolean(&body, &cutter, BoolOp::Difference)
        {
            body = cut;
        }
    }
    body
}

/// Plan symbol lines. Door: the leaf open at 90 degrees from the hinge corner on the swing face, then the quarter arc
/// its tip sweeps to the other jamb. Window: lines across the gap on both faces and in the middle (the glass).
pub fn opening_symbol(o: &Opening) -> Vec<[DVec2; 2]> {
    let w = &o.wall;
    let d = (w.b - w.a).normalize();
    let n = d.perp();
    let (l, r) = w.faces();
    let (u0, u1) = o.span();
    let p = |u: f64, s: f64| w.a + d * u + n * s;
    match o.kind {
        OpeningKind::Window => [l, r, (l + r) / 2.0].map(|s| [p(u0, s), p(u1, s)]).to_vec(),
        OpeningKind::Door => {
            let (hinge, other) = if o.hinge_b { (u1, u0) } else { (u0, u1) };
            let (face, out) = if o.swing_left { (l, n) } else { (r, -n) };
            let h = p(hinge, face);
            let width = u1 - u0;
            let shut = d * (other - hinge).signum();
            let arc = |k: usize| {
                let a = FRAC_PI_2 * k as f64 / ARC_SEGMENTS as f64;
                h + (out * a.cos() + shut * a.sin()) * width
            };
            let mut lines = vec![[h, h + out * width]];
            lines.extend((0..ARC_SEGMENTS).map(|k| [arc(k), arc(k + 1)]));
            lines
        }
    }
}

/// The IFC class an entity of this shape exports as; `None` for drafting shapes.
pub fn ifc_class(shape: &Shape) -> Option<&'static str> {
    match shape {
        Shape::Wall(_) => Some("IfcWall"),
        Shape::Opening(o) => Some(match o.kind {
            OpeningKind::Door => "IfcDoor",
            OpeningKind::Window => "IfcWindow",
        }),
        _ => None,
    }
}

#[cfg(test)]
#[path = "opening_tests.rs"]
mod tests;
