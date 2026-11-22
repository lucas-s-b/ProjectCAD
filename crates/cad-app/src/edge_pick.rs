use crate::app::{CadApp, PickFace, ViewMode};
use cad_document::{Body, BodyEdge, Command, Convexity, EdgeGeom, FaceKey, Feature, FeatureId, FeatureKind, edge_keys};
use cad_geometry::DVec3;
use std::sync::Arc;

pub struct EdgePick {
    pub chamfer: bool,
    pub replace: Option<FeatureId>,
    pub points: Vec<DVec3>,
    /// One face pair per point, from `edge_keys`.
    pub faces: Vec<[FaceKey; 2]>,
}

/// Starts picking; edge and face picking exclude each other.
pub fn start(app: &mut CadApp, chamfer: bool, replace: Option<FeatureId>) {
    app.pick_face = PickFace::Off;
    app.pick_edges = Some(EdgePick { chamfer, replace, points: Vec::new(), faces: Vec::new() });
    app.view = ViewMode::ThreeD;
    app.status = "Click the edges to blend".into();
}

/// Closest distance between the ray and the segment, and the ray parameter there.
fn ray_segment(o: DVec3, d: DVec3, a: DVec3, b: DVec3) -> (f64, f64) {
    let (v, w) = (b - a, o - a);
    let (uu, uv, vv, uw, vw) = (d.dot(d), d.dot(v), v.dot(v), d.dot(w), v.dot(w));
    let den = uu * vv - uv * uv;
    let mut s = if den > 1e-12 * uu * vv { (uu * vw - uv * uw) / den } else { 0.0 };
    s = s.clamp(0.0, 1.0);
    let t = ((uv * s - uw) / uu).max(0.0);
    s = if vv > 0.0 { ((vw + uv * t) / vv).clamp(0.0, 1.0) } else { 0.0 };
    ((o + d * t).distance(a + v * s), t)
}

/// The body edge closest to the ray within 8 pixels and not hidden behind a body face: (body id, edge, ray distance).
fn nearest_edge(app: &mut CadApp, origin: DVec3, dir: DVec3, pixel: f64) -> Option<(FeatureId, BodyEdge, f64)> {
    let face_t = crate::face_pick::ray_body(app, origin, dir).map(|(.., t)| t);
    let mut best: Option<(FeatureId, BodyEdge, f64)> = None;
    let bodies: Vec<(FeatureId, Arc<Body>)> = app.shown_bodies().cloned().collect();
    for (id, body) in &bodies {
        for edge in body.edges() {
            let n = edge.points.len();
            for i in 0..if edge.closed { n } else { n - 1 } {
                let (a, b) = (edge.points[i], edge.points[(i + 1) % n]);
                let (dist, t) = ray_segment(origin, dir, a, b);
                if dist <= 8.0 * pixel * t
                    && face_t.is_none_or(|f| t <= f * (1.0 + 8.0 * pixel))
                    && best.as_ref().is_none_or(|(.., bt)| t < *bt)
                {
                    best = Some((*id, edge.clone(), t));
                }
            }
        }
    }
    best
}

/// Toggles the edge under the ray; `pixel` is the world size of one screen pixel per unit of ray distance.
pub fn click(app: &mut CadApp, origin: DVec3, dir: DVec3, pixel: f64) {
    let picked = nearest_edge(app, origin, dir, pixel).and_then(|(id, e, _)| {
        let body = app.model.iter().flat_map(|(_, m)| &m.bodies).find(|(i, _)| *i == id).map(|(_, b)| b)?;
        Some((e.midpoint(), edge_keys(body, &e)))
    });
    let (Some((mid, keys)), Some(pick)) = (picked, app.pick_edges.as_mut()) else {
        app.status = "No edge there".into();
        return;
    };
    match pick.points.iter().position(|p| p.distance(mid) < 1e-6) {
        Some(i) => {
            pick.points.remove(i);
            pick.faces.remove(i);
        }
        None => {
            pick.points.push(mid);
            pick.faces.push(keys);
        }
    }
}

/// Selects the edge under the ray; the status shows its kind and size. Returns false when no edge is there (edge_sel untouched).
pub fn select_edge(app: &mut CadApp, origin: DVec3, dir: DVec3, pixel: f64) -> bool {
    let Some((id, edge, _)) = nearest_edge(app, origin, dir, pixel) else { return false };
    let conv = match edge.convexity {
        Convexity::Convex => "convex",
        Convexity::Concave => "concave",
        Convexity::Smooth => "smooth",
    };
    app.status = match &edge.geom {
        EdgeGeom::Line { a, b } => format!("Edge: line, length {:.3} mm, {conv}", a.distance(*b)),
        EdgeGeom::Circle { radius, sweep, .. } => {
            format!("Edge: circle, radius {radius:.3} mm, arc {:.1}\u{b0}, {conv}", sweep.to_degrees())
        }
        EdgeGeom::Ellipse { major, minor, .. } => format!("Edge: ellipse, semi-axes {major:.3} x {minor:.3} mm, {conv}"),
        EdgeGeom::Other => {
            let n = edge.points.len();
            let len: f64 = (0..if edge.closed { n } else { n - 1 }).map(|i| edge.points[i].distance(edge.points[(i + 1) % n])).sum();
            format!("Edge: curve, length \u{2248} {len:.3} mm, {conv}")
        }
    };
    app.edge_sel = Some((id, edge));
    true
}

pub fn finish(app: &mut CadApp) {
    let Some(pick) = app.pick_edges.as_ref().filter(|p| !p.points.is_empty()) else {
        app.status = "No edges picked".into();
        return;
    };
    let (edges, faces, chamfer, replace) = (pick.points.clone(), pick.faces.clone(), pick.chamfer, pick.replace);
    let old = replace.and_then(|id| app.doc.feature(id).cloned());
    let size = match old.as_ref().map(|f| &f.kind) {
        Some(FeatureKind::Fillet { radius: s, .. } | FeatureKind::Chamfer { distance: s, .. }) => s.clone(),
        _ => app.blend_size.clone(),
    };
    let kind = if chamfer { FeatureKind::Chamfer { edges, faces, distance: size } } else { FeatureKind::Fillet { edges, faces, radius: size } };
    let cmd = match old {
        Some(f) => Command::SetFeature(Feature { kind, ..f }),
        None => {
            let id = app.doc.alloc_feature_id();
            let index = app.doc.features().len();
            let label = if chamfer { "Chamfer" } else { "Fillet" };
            Command::AddFeature { index, feature: Feature { id, name: format!("{label} {id}"), suppressed: false, kind } }
        }
    };
    app.feature_edit = None;
    app.exec(vec![cmd]);
    app.pick_edges = None;
}
