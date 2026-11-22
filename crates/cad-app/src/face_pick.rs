use crate::app::{CadApp, PickFace, ViewMode};
use cad_document::{Body, Bvh, Command, FaceKey, FaceRef, FeatureId, FeatureKind, PlaneDef, Sketch, Surface};
use cad_geometry::DVec3;
use std::sync::Arc;

/// Nearest triangle of `bodies` under the ray: (id, triangle index, distance). BVHs are cached per body and dropped
/// once no body or component solid of the model holds it.
pub fn nearest(app: &mut CadApp, bodies: &[(u64, Arc<Body>)], origin: DVec3, dir: DVec3) -> Option<(u64, usize, f64)> {
    let live: Vec<Arc<Body>> = app
        .model
        .iter()
        .flat_map(|(_, m)| m.bodies.iter().map(|(_, b)| b.clone()).chain(m.components.iter().filter_map(|(_, r)| r.as_ref().ok().cloned())))
        .collect();
    app.bvhs.retain(|(b, _)| live.iter().any(|l| Arc::ptr_eq(b, l)));
    let mut best: Option<(u64, usize, f64)> = None;
    for (id, body) in bodies {
        let bvh = match app.bvhs.iter().find(|(b, _)| Arc::ptr_eq(b, body)) {
            Some((_, bvh)) => bvh.clone(),
            None => {
                let bvh = Arc::new(Bvh::build(&body.mesh));
                app.bvhs.push((body.clone(), bvh.clone()));
                bvh
            }
        };
        if let Some((t, tri)) = bvh.ray(&body.mesh, origin, dir)
            && best.is_none_or(|(.., b)| t < b)
        {
            best = Some((*id, tri, t));
        }
    }
    best
}

/// Nearest shown body triangle under the ray: (body id, triangle index, distance).
pub fn ray_body(app: &mut CadApp, origin: DVec3, dir: DVec3) -> Option<(FeatureId, usize, f64)> {
    let bodies: Vec<(FeatureId, Arc<Body>)> = app.shown_bodies().cloned().collect();
    nearest(app, &bodies, origin, dir)
}

/// Normal of a planar surface; None for any other surface.
fn plane_normal(surface: &Surface) -> Option<DVec3> {
    match surface {
        Surface::Plane { normal, .. } => Some(*normal),
        _ => None,
    }
}

/// Handles a click while a face is being picked: a hit on a planar face creates and activates a sketch on it, adds a shell open at it, or adds it to a draft.
pub fn click(app: &mut CadApp, origin: DVec3, dir: DVec3) {
    let best = ray_body(app, origin, dir).and_then(|(id, tri, t)| {
        let body = app.model.iter().flat_map(|(_, m)| &m.bodies).find(|(i, _)| *i == id).map(|(_, b)| b)?;
        let f = &body.faces[body.tri_face[tri] as usize];
        Some((id, t, plane_normal(&f.surface), f.outward, FaceKey::of(f)))
    });
    let Some((id, t, normal, outward, key)) = best else {
        app.status = "No face there".into();
        return;
    };
    let Some(normal) = normal else {
        app.status = "Pick a planar face".into();
        return;
    };
    let hit = origin + dir * t;
    if let PickFace::Draft(session) = app.pick_face {
        let r = FaceRef { key, at: hit };
        match session.and_then(|s| app.doc.feature(s).cloned()) {
            Some(mut f) => {
                let FeatureKind::Draft { body, faces, .. } = &mut f.kind else { return };
                if *body != id {
                    app.status = format!("Pick faces of body {body}");
                    return;
                }
                faces.push(r);
                app.exec(vec![Command::SetFeature(f)]);
            }
            None => {
                let kind = FeatureKind::Draft { body: id, faces: vec![r], sketch: app.sketch, angle: app.draft_angle.clone() };
                let fid = crate::model_features::add_feature(app, "Draft", kind);
                if app.doc.feature(fid).is_none() {
                    return;
                }
                app.feature_sel = Some(fid);
                app.pick_face = PickFace::Draft(Some(fid));
            }
        }
        app.status = "Click more faces to draft; Esc to finish".into();
        return;
    }
    if app.pick_face == PickFace::Shell {
        let kind = FeatureKind::Shell { thickness: app.shell_thickness.clone(), open: Some(hit), at: hit };
        let id = crate::model_features::add_feature(app, "Shell", kind);
        if app.doc.feature(id).is_some() {
            app.feature_sel = Some(id);
            app.pick_face = PickFace::Off;
        }
        return;
    }
    let n = if outward { normal } else { -normal }.normalize();
    let x = DVec3::Z.cross(n);
    let x = if x.length() < 1e-9 { DVec3::X } else { x.normalize() };
    let sketch = app.doc.alloc_sketch_id();
    let plane = PlaneDef::Face { origin: n * n.dot(hit), x, y: n.cross(x), at: hit, body: id };
    app.exec(vec![Command::AddSketch(Sketch { id: sketch, name: format!("Sketch {sketch}"), plane, level: None })]);
    if app.doc.sketch(sketch).is_some() {
        app.set_sketch(sketch);
        app.view = ViewMode::TwoD;
        app.pick_face = PickFace::Off;
    }
}

/// Selects the body face hit by the ray, or clears the face selection; the status shows its surface kind and area.
pub fn select_face(app: &mut CadApp, origin: DVec3, dir: DVec3) {
    let before = app.face_sel;
    app.face_sel = None;
    if let Some((id, tri, _)) = ray_body(app, origin, dir)
        && let Some(body) = app.model.iter().flat_map(|(_, m)| &m.bodies).find(|(i, _)| *i == id).map(|(_, b)| b)
    {
        let face = body.tri_face[tri];
        let m = &body.mesh;
        let area: f64 = m
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .zip(&body.tri_face)
            .filter(|(_, f)| **f == face)
            .map(|(t, _)| {
                let [a, b, c] = t.map(|i| m.positions[i as usize]);
                0.5 * (b - a).cross(c - a).length()
            })
            .sum();
        let kind = match body.faces[face as usize].surface {
            Surface::Plane { .. } => "plane",
            Surface::Cylinder { .. } => "cylinder",
            Surface::Cone { .. } => "cone",
            Surface::Sphere { .. } => "sphere",
            Surface::Torus { .. } => "torus",
            _ => "free-form",
        };
        app.status = format!("Face: {kind}, area \u{2248} {area:.3} mm\u{b2}");
        app.face_sel = Some((id, face));
    }
    if app.face_sel != before {
        app.sel_rev += 1;
    }
}
