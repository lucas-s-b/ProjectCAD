use crate::app::{CadApp, PickFace, ViewMode};
use cad_document::{Command, FeatureKind, PlaneDef, Sketch, Surface};
use cad_geometry::DVec3;

/// Distance along the ray to the triangle, Moller-Trumbore.
fn ray_triangle(origin: DVec3, dir: DVec3, [a, b, c]: [DVec3; 3]) -> Option<f64> {
    let (e1, e2) = (b - a, c - a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let s = origin - a;
    let u = s.dot(p) / det;
    let q = s.cross(e1);
    let v = dir.dot(q) / det;
    let t = e2.dot(q) / det;
    (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0).then_some(t)
}

/// Normal of a planar surface; None for any other surface.
fn plane_normal(surface: &Surface) -> Option<DVec3> {
    match surface {
        Surface::Plane { normal, .. } => Some(*normal),
        _ => None,
    }
}

/// Handles a click while a face is being picked: a hit on a planar face creates and activates a sketch on it, or adds a shell open at it.
pub fn click(app: &mut CadApp, origin: DVec3, dir: DVec3) {
    let mut best: Option<(f64, Option<DVec3>, bool)> = None;
    // shortcut: scans all triangles, add a BVH when picking shows up in a profile
    for (_, body) in app.model.iter().flat_map(|(_, m)| &m.bodies) {
        let tris = body.mesh.indices.as_chunks::<3>().0;
        for (tri, &face) in tris.iter().zip(&body.tri_face) {
            let corners = tri.map(|i| body.mesh.positions[i as usize]);
            if let Some(t) = ray_triangle(origin, dir, corners)
                && best.is_none_or(|(b, ..)| t < b)
            {
                let f = &body.faces[face as usize];
                best = Some((t, plane_normal(&f.surface), f.outward));
            }
        }
    }
    let Some((t, normal, outward)) = best else {
        app.status = "No face there".into();
        return;
    };
    let Some(normal) = normal else {
        app.status = "Pick a planar face".into();
        return;
    };
    if app.pick_face == PickFace::Shell {
        let hit = origin + dir * t;
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
    let id = app.doc.alloc_sketch_id();
    let plane = PlaneDef::Fixed { origin: n * n.dot(origin + dir * t), x, y: n.cross(x) };
    app.exec(vec![Command::AddSketch(Sketch { id, name: format!("Sketch {id}"), plane })]);
    if app.doc.sketch(id).is_some() {
        app.set_sketch(id);
        app.view = ViewMode::TwoD;
        app.pick_face = PickFace::Off;
    }
}
