//! Transform tools driven by picked points, as in AutoCAD: Move and Copy (base point, target), Rotate and Scale (base
//! point, reference point, target point), Mirror (two points on the mirror line) and Stretch (crossing window, base
//! point, target). Every point goes through the usual snaps, Shift polar steps and the coordinate field. Move, Copy,
//! Rotate, Scale and Mirror act on the selection; Stretch on what its window crosses.
use crate::app::{CadApp, Tool};
use cad_document::Command;
use cad_geometry::xform::{mirror_across, rotation_about, scale_about};
use cad_geometry::{DAffine2, DVec2, Shape};
use eframe::egui::{self, Color32, Stroke};

const GHOST: Color32 = Color32::from_rgb(120, 200, 255);

pub fn is_xform(t: Tool) -> bool {
    matches!(t, Tool::Move | Tool::Copy | Tool::RotateRef | Tool::ScaleRef | Tool::Mirror | Tool::Stretch)
}

/// The transform of a selection tool from its points (the last one may be the cursor); None while points are
/// missing or they give no transform (zero reference length, coincident mirror points).
pub fn affine(tool: Tool, pts: &[DVec2]) -> Option<DAffine2> {
    match (tool, pts) {
        (Tool::Move | Tool::Copy, [a, b, ..]) => Some(DAffine2::from_translation(*b - *a)),
        (Tool::RotateRef, [c, r, t, ..]) => {
            let (u, v) = (*r - *c, *t - *c);
            (u.length() > 1e-12 && v.length() > 1e-12).then(|| rotation_about(*c, v.to_angle() - u.to_angle()))
        }
        (Tool::ScaleRef, [c, r, t, ..]) => {
            let (lr, lt) = (r.distance(*c), t.distance(*c));
            (lr > 1e-12 && lt > 1e-12).then(|| scale_about(*c, lt / lr))
        }
        (Tool::Mirror, [a, b, ..]) => (a.distance(*b) > 1e-12).then(|| mirror_across(*a, *b - *a)),
        _ => None,
    }
}

/// The window of a Stretch from its two corners: a point is inside when it lies within the box.
fn window(a: DVec2, b: DVec2) -> impl Fn(DVec2) -> bool {
    let (lo, hi) = (a.min(b), a.max(b));
    move |p: DVec2| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y
}

/// The stretched entities of the active sketch: (id, new shape) for each pickable entity the window changes.
fn stretched(app: &CadApp, a: DVec2, b: DVec2, d: DVec2) -> Vec<(cad_document::EntityId, Shape)> {
    let inside = window(a, b);
    app.doc.entities_in(app.sketch).filter(|e| app.pickable(e)).filter_map(|e| cad_geometry::edit::stretch(&e.shape, &inside, d).map(|s| (e.id, s))).collect()
}

/// Points still to pick, for the hint.
fn hint(tool: Tool, n: usize) -> &'static str {
    match (tool, n) {
        (Tool::Move | Tool::Copy, 0) => "Pick the base point",
        (Tool::Move, _) => "Pick the target point",
        (Tool::Copy, _) => "Pick where the copy goes (Esc ends)",
        (Tool::RotateRef | Tool::ScaleRef, 0) => "Pick the base point",
        (Tool::RotateRef, 1) => "Pick a reference point (sets the start angle)",
        (Tool::RotateRef, _) => "Pick the target point (sets the new angle)",
        (Tool::ScaleRef, 1) => "Pick a reference point (sets the start length)",
        (Tool::ScaleRef, _) => "Pick the target point (sets the new length)",
        (Tool::Mirror, 0) => "Pick the first point of the mirror line",
        (Tool::Mirror, _) => "Pick the second point of the mirror line",
        (Tool::Stretch, 0) => "Pick the first corner of the crossing window",
        (Tool::Stretch, 1) => "Pick the opposite corner",
        (Tool::Stretch, 2) => "Pick the base point",
        _ => "Pick the target point",
    }
}

/// Feeds one picked point to the active transform tool and commits once it has enough.
pub fn point(app: &mut CadApp, p: DVec2) {
    let tool = app.tool;
    if tool != Tool::Stretch && app.selection.is_empty() {
        app.points.clear();
        app.status = "Select what to transform first".into();
        return;
    }
    app.points.push(p);
    let pts = app.points.clone();
    match (tool, pts.len()) {
        (Tool::Move, 2) => {
            app.points.clear();
            let d = (pts[1] - pts[0]).extend(0.0);
            let cmds: Vec<Command> = app
                .selection
                .iter()
                .filter_map(|id| app.doc.get(*id))
                .map(|e| {
                    let mut shape = e.shape.clone();
                    shape.translate(d);
                    Command::SetShape { id: e.id, shape }
                })
                .collect();
            let pinned = app.selection.clone();
            app.exec_pinned(cmds, &pinned);
        }
        (Tool::Copy, 2) => {
            // Several copies from one base point, until Escape.
            app.points.truncate(1);
            if let Some(m) = affine(tool, &pts) {
                app.copy_transformed(&m, false);
            }
        }
        (Tool::Mirror, 2) | (Tool::RotateRef | Tool::ScaleRef, 3) => {
            app.points.clear();
            let Some(m) = affine(tool, &pts) else {
                app.status = "The points give no transform (zero length)".into();
                return;
            };
            if tool == Tool::Mirror && app.mirror_keep {
                app.copy_transformed(&m, true);
            } else {
                app.transform_selection(m);
            }
        }
        (Tool::Stretch, 4) => {
            app.points.clear();
            let changes = stretched(app, pts[0], pts[1], pts[3] - pts[2]);
            if changes.is_empty() {
                app.status = "The window crosses nothing".into();
                return;
            }
            let pinned = changes.iter().map(|(id, _)| *id).collect();
            let cmds = changes.into_iter().map(|(id, shape)| Command::SetShape { id, shape }).collect();
            app.exec_pinned(cmds, &pinned);
        }
        _ => {}
    }
}

/// Draws the step hint, the rubber band, the crossing window and the transformed ghosts.
pub fn preview(app: &CadApp, painter: &egui::Painter, rect: egui::Rect, cursor: DVec2) {
    if !is_xform(app.tool) {
        return;
    }
    let to = |p: DVec2| app.cam2d.world_to_screen(rect, p);
    painter.text(rect.left_top() + egui::vec2(8.0, 8.0), egui::Align2::LEFT_TOP, hint(app.tool, app.points.len()), egui::FontId::proportional(14.0), Color32::from_gray(200));
    let mut pts = app.points.clone();
    pts.push(cursor);
    let dashed = |a: DVec2, b: DVec2| painter.extend(egui::Shape::dashed_line(&[to(a), to(b)], Stroke::new(1.0, Color32::from_gray(190)), 5.0, 4.0));
    if app.tool == Tool::Stretch {
        if pts.len() >= 2 {
            let r = egui::Rect::from_two_pos(to(pts[0]), to(pts[1]));
            painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(60, 200, 90, 30));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_rgb(60, 200, 90)), egui::StrokeKind::Middle);
        }
        if pts.len() == 4 {
            dashed(pts[2], pts[3]);
            for (_, s) in stretched(app, pts[0], pts[1], pts[3] - pts[2]) {
                crate::view2d::draw_shape(app, painter, rect, &s, Stroke::new(1.0, GHOST));
            }
        }
        return;
    }
    if pts.len() >= 2 {
        dashed(pts[0], *pts.last().unwrap_or(&pts[0]));
    }
    if app.tool == Tool::RotateRef && pts.len() >= 3 {
        dashed(pts[0], pts[1]);
    }
    let Some(m) = affine(app.tool, &pts) else { return };
    for e in app.selection.iter().filter_map(|id| app.doc.get(*id)) {
        if let Ok(s) = e.shape.transformed(&m) {
            crate::view2d::draw_shape(app, painter, rect, &s, Stroke::new(1.0, GHOST));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affines_from_points() {
        let p = |x: f64, y: f64| DVec2::new(x, y);
        let mv = affine(Tool::Move, &[p(1.0, 1.0), p(4.0, 5.0)]).unwrap();
        assert_eq!(mv.transform_point2(DVec2::ZERO), p(3.0, 4.0));
        // Reference along +X, target along +Y: a quarter turn about the base.
        let rot = affine(Tool::RotateRef, &[p(1.0, 0.0), p(2.0, 0.0), p(1.0, 5.0)]).unwrap();
        assert!(rot.transform_point2(p(3.0, 0.0)).distance(p(1.0, 2.0)) < 1e-12);
        let sc = affine(Tool::ScaleRef, &[DVec2::ZERO, p(2.0, 0.0), p(0.0, 6.0)]).unwrap();
        assert!(sc.transform_point2(p(1.0, 1.0)).distance(p(3.0, 3.0)) < 1e-12);
        let mi = affine(Tool::Mirror, &[DVec2::ZERO, p(0.0, 1.0)]).unwrap();
        assert!(mi.transform_point2(p(2.0, 3.0)).distance(p(-2.0, 3.0)) < 1e-12);
        assert!(affine(Tool::ScaleRef, &[DVec2::ZERO, DVec2::ZERO, p(1.0, 0.0)]).is_none());
        assert!(affine(Tool::Mirror, &[DVec2::ZERO]).is_none());
    }

    #[test]
    fn windows_work_from_either_corner() {
        let w = window(DVec2::new(5.0, 5.0), DVec2::new(-5.0, -5.0));
        assert!(w(DVec2::ZERO));
        assert!(w(DVec2::new(5.0, -5.0)));
        assert!(!w(DVec2::new(6.0, 0.0)));
    }
}
