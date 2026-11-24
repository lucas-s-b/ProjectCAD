//! The Hatch group of the properties panel and the pattern choice shared with the inspector.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::Command;
use cad_geometry::{DVec2, HatchPattern, Shape, outline_paths};
use eframe::egui;

/// Boundary of a closed curve as its flattened path without the repeated last point.
pub(crate) fn hatch_boundary(shape: &Shape) -> Option<Vec<DVec2>> {
    let closed = match shape {
        Shape::Rect { .. } | Shape::Circle { .. } | Shape::Ellipse { .. } => true,
        Shape::Polyline { closed, .. } | Shape::Spline { closed, .. } => *closed,
        _ => false,
    };
    if !closed {
        return None;
    }
    let mut path = outline_paths(shape).into_iter().next()?;
    path.pop();
    Some(path.iter().map(|p| p.truncate()).collect())
}

/// Combo "Hatch pattern". `salt` keeps the two places that show it apart.
pub fn pattern_combo(ui: &mut egui::Ui, salt: &str, pattern: &mut HatchPattern) {
    let text = format!("{pattern:?}");
    w::combo(ui, "Hatch pattern", egui::ComboBox::from_id_salt(salt).selected_text(text), |ui| {
        for p in [HatchPattern::Lines, HatchPattern::Cross, HatchPattern::Solid] {
            w::selectable_value(ui, pattern, p, &format!("{p:?}"));
        }
    });
}

pub fn hatch_group(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some(id) = (app.selection.len() == 1).then(|| app.selection.iter().next().copied()).flatten() else { return };
    let Some((boundary, layer)) = app.doc.get(id).filter(|e| !e.construction).and_then(|e| Some((hatch_boundary(&e.shape)?, e.layer))) else { return };
    ui.group(|ui| {
        ui.label(crate::i18n::tr("Hatch"));
        ui.horizontal(|ui| {
            w::add(ui, "Hatch angle", egui::DragValue::new(&mut app.hatch_angle_deg).speed(0.5));
            w::add(ui, "Hatch spacing", egui::DragValue::new(&mut app.hatch_spacing).speed(0.1).range(0.0..=f64::MAX));
        });
        pattern_combo(ui, "hatch_pattern_new", &mut app.hatch_pattern);
        if w::button(ui, "Hatch").clicked() {
            let shape = Shape::Hatch { boundary, angle: app.hatch_angle_deg.to_radians(), spacing: app.hatch_spacing, pattern: app.hatch_pattern };
            let e = app.new_entity(shape, layer);
            app.exec(vec![Command::Add(e)]);
        }
    });
}
