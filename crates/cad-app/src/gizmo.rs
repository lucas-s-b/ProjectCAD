//! Translate gizmo in the 3D view: three axis arrows at the centre of the selected solids; drag one to move along it.
use crate::app::CadApp;
use cad_document::Command;
use cad_geometry::DVec3;
use eframe::egui::{self, Pos2};

pub const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];
const COLORS: [egui::Color32; 3] = [egui::Color32::from_rgb(230, 50, 50), egui::Color32::from_rgb(50, 200, 50), egui::Color32::from_rgb(80, 100, 255)];

/// A drag in progress: positions along `AXES[axis]` where it started and where the pointer is now.
pub struct GizmoDrag {
    pub axis: usize,
    pub start: f64,
    pub now: f64,
}

impl GizmoDrag {
    pub fn delta(&self) -> DVec3 {
        AXES[self.axis] * (self.now - self.start)
    }
}

/// Centre of the bounding box of the selection when it is non-empty and holds only solids (stored shapes, not the drag preview);
/// with no entity selected, the centre of the selected component's solid.
pub fn center(app: &CadApp) -> Option<DVec3> {
    if app.selection.is_empty() {
        let (lo, hi) = app.model_result().and_then(|r| r.component(app.component_sel?))?.bbox();
        return Some((lo + hi) / 2.0);
    }
    let mut bounds: Option<(DVec3, DVec3)> = None;
    for e in app.selection.iter().filter_map(|id| app.doc.get(*id)) {
        if !e.shape.is_solid() {
            return None;
        }
        let (lo, hi) = e.shape.bbox();
        bounds = Some(bounds.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
    }
    bounds.map(|(lo, hi)| (lo + hi) / 2.0)
}

/// Parameter `s` of the point on the line `c + axis * s` closest to the ray; None when line and ray are nearly parallel. `axis` and `dir` are unit.
pub fn axis_param(c: DVec3, axis: DVec3, origin: DVec3, dir: DVec3) -> Option<f64> {
    let (w, b) = (c - origin, axis.dot(dir));
    let den = 1.0 - b * b;
    (den > 1e-6).then(|| (b * dir.dot(w) - axis.dot(w)) / den)
}

/// The arrow (segment from `origin` to `tips[i]`) nearest to `p` within `tol` pixels.
pub fn hit(origin: Pos2, tips: [Pos2; 3], p: Pos2, tol: f32) -> Option<usize> {
    let ap = p - origin;
    let dist = |tip: Pos2| {
        let ab = tip - origin;
        let t = (ap.dot(ab) / ab.length_sq().max(1e-9)).clamp(0.0, 1.0);
        (ap - ab * t).length()
    };
    (0..3).map(|i| (i, dist(tips[i]))).filter(|&(_, d)| d <= tol).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i)
}

/// Screen points (origin, X tip, Y tip, Z tip) of the gizmo, moved by the drag in progress; arrows are 0.2 x the camera distance long.
/// None without a gizmo or when the centre is behind the camera.
pub fn screen(app: &CadApp, rect: egui::Rect) -> Option<[Pos2; 4]> {
    let vp = app.cam3d.view_proj(rect.width() / rect.height().max(1.0));
    let c = center(app)? + app.gizmo.as_ref().map_or(DVec3::ZERO, GizmoDrag::delta);
    let len = 0.2 * f64::from(app.cam3d.distance);
    let project = |q: DVec3| {
        let c = vp * q.as_vec3().extend(1.0);
        if c.w <= 1e-6 {
            return None;
        }
        let ndc = c.truncate() / c.w;
        Some(Pos2::new(rect.left() + (ndc.x + 1.0) / 2.0 * rect.width(), rect.top() + (1.0 - ndc.y) / 2.0 * rect.height()))
    };
    Some([project(c)?, project(c + AXES[0] * len)?, project(c + AXES[1] * len)?, project(c + AXES[2] * len)?])
}

/// Draws the arrows (width 3, `COLORS`, width 5 for the dragged axis) and a filled circle of radius 4 at each tip.
pub fn paint(painter: &egui::Painter, pts: [Pos2; 4], active: Option<usize>) {
    for i in 0..3 {
        let width = if active == Some(i) { 5.0 } else { 3.0 };
        painter.line_segment([pts[0], pts[i + 1]], egui::Stroke::new(width, COLORS[i]));
        painter.circle_filled(pts[i + 1], 4.0, COLORS[i]);
    }
}

/// Starts, updates and commits a gizmo drag with the primary button.
pub fn input(app: &mut CadApp, ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect) {
    let param = |app: &CadApp, axis: usize, pos: Pos2| {
        let (o, d) = crate::view3d::ray(app, rect, pos);
        axis_param(center(app)?, AXES[axis], o, d)
    };
    if resp.drag_started_by(egui::PointerButton::Primary)
        && let Some(pts) = app.gizmo_screen
        && let Some(press) = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos())
        && let Some(axis) = hit(pts[0], [pts[1], pts[2], pts[3]], press, 8.0)
        && let Some(s) = param(app, axis, press)
    {
        app.gizmo = Some(GizmoDrag { axis, start: s, now: s });
    }
    if resp.dragged_by(egui::PointerButton::Primary)
        && let Some(axis) = app.gizmo.as_ref().map(|g| g.axis)
        && let Some(pos) = resp.interact_pointer_pos()
        && let Some(s) = param(app, axis, pos)
        && let Some(g) = app.gizmo.as_mut()
    {
        g.now = s;
    }
    if resp.drag_stopped_by(egui::PointerButton::Primary)
        && let Some(g) = app.gizmo.take()
        && g.delta().length() > 1e-9
    {
        let delta = g.delta();
        if app.selection.is_empty() {
            // The gizmo of the selected component (see `center`).
            let moved = app.component_sel.and_then(|id| app.doc.component(id)).map(|c| app.doc.component_moved(c, delta));
            match moved {
                Some(Ok(c)) => {
                    app.component_edit = None;
                    app.exec(vec![Command::SetComponent(c)]);
                }
                Some(Err(e)) => app.status = e,
                None => {}
            }
            return;
        }
        let cmds: Vec<Command> = app
            .selection
            .iter()
            .filter_map(|id| app.doc.get(*id))
            .map(|e| {
                let mut shape = e.shape.clone();
                shape.translate(delta);
                Command::SetShape { id: e.id, shape }
            })
            .collect();
        let pinned = app.selection.clone();
        app.exec_pinned(cmds, &pinned);
    }
}

#[cfg(test)]
#[path = "gizmo_tests.rs"]
mod tests;
