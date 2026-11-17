//! Associative linear dimensions: which geometry a new dimension measures, and the Linked / Unlink part of the properties panel.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::{Command, PointKind, PointRef};
use cad_geometry::{DVec2, Shape};
use eframe::egui;

/// A dimension point that sits this close to an entity point is that point, mm.
const SAME_POINT: f64 = 1e-9;

impl CadApp {
    /// The Line, Circle or Arc point of the active sketch at `p`.
    pub fn point_ref_at(&self, p: DVec2) -> Option<PointRef> {
        self.doc
            .entities_in(self.sketch)
            .filter(|e| matches!(e.shape, Shape::Line { .. } | Shape::Circle { .. } | Shape::Arc { .. }))
            .flat_map(|e| [PointKind::Start, PointKind::End, PointKind::Center].map(|kind| PointRef { entity: e.id, kind }))
            .find(|r| self.doc.point_of(r).is_some_and(|q| q.distance(p) <= SAME_POINT))
    }
}

/// "Linked" or "Not linked" and an Unlink button for the one selected linear dimension.
pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some(id) = (app.selection.len() == 1).then(|| app.selection.iter().next().copied()).flatten() else { return };
    if !app.doc.get(id).is_some_and(|e| matches!(e.shape, Shape::Dimension { .. })) {
        return;
    }
    let linked = app.doc.dim_link(id).is_some();
    let text = if linked { "Linked" } else { "Not linked" };
    let label = ui.label(text);
    w::tag(ui, text, label);
    if linked && w::button(ui, "Unlink").clicked() {
        app.exec(vec![Command::SetDimLink { id, link: None }]);
    }
}
