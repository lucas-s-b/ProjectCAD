use crate::app::{CadApp, PickFace, ViewMode};
use cad_document::{Command, Feature, FeatureId, FeatureKind};
use cad_geometry::DVec3;

pub struct EdgePick {
    pub chamfer: bool,
    pub replace: Option<FeatureId>,
    pub points: Vec<DVec3>,
}

/// Starts picking; edge and face picking exclude each other.
pub fn start(app: &mut CadApp, chamfer: bool, replace: Option<FeatureId>) {
    app.pick_face = PickFace::Off;
    app.pick_edges = Some(EdgePick { chamfer, replace, points: Vec::new() });
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

/// Toggles the edge under the ray; `pixel` is the world size of one screen pixel per unit of ray distance.
pub fn click(app: &mut CadApp, origin: DVec3, dir: DVec3, pixel: f64) {
    let mut best: Option<(f64, DVec3)> = None;
    for (_, body) in app.model.iter().flat_map(|(_, m)| &m.bodies) {
        for edge in body.edges() {
            let n = edge.points.len();
            for i in 0..if edge.closed { n } else { n - 1 } {
                let (a, b) = (edge.points[i], edge.points[(i + 1) % n]);
                let (dist, t) = ray_segment(origin, dir, a, b);
                // shortcut: edges hidden behind the body can be picked, add an occlusion test with the BVH
                if dist <= 8.0 * pixel * t && best.is_none_or(|(bt, _)| t < bt) {
                    best = Some((t, edge.midpoint()));
                }
            }
        }
    }
    let (Some((_, mid)), Some(pick)) = (best, app.pick_edges.as_mut()) else {
        app.status = "No edge there".into();
        return;
    };
    match pick.points.iter().position(|p| p.distance(mid) < 1e-6) {
        Some(i) => {
            pick.points.remove(i);
        }
        None => pick.points.push(mid),
    }
}

pub fn finish(app: &mut CadApp) {
    let Some(pick) = app.pick_edges.as_ref().filter(|p| !p.points.is_empty()) else {
        app.status = "No edges picked".into();
        return;
    };
    let (edges, chamfer, replace) = (pick.points.clone(), pick.chamfer, pick.replace);
    let old = replace.and_then(|id| app.doc.feature(id).cloned());
    let size = match old.as_ref().map(|f| &f.kind) {
        Some(FeatureKind::Fillet { radius: s, .. } | FeatureKind::Chamfer { distance: s, .. }) => s.clone(),
        _ => app.blend_size.clone(),
    };
    let kind = if chamfer { FeatureKind::Chamfer { edges, distance: size } } else { FeatureKind::Fillet { edges, radius: size } };
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
