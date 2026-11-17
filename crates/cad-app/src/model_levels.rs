//! Levels: the list, Add level, and the elevation and height of the active level.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::{BasePlane, Command, Level, PlaneDef, Sketch, SketchId};
use eframe::egui::{self, TextEdit};

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.separator();
    ui.heading("Levels");
    let rows: Vec<(SketchId, String, String)> = app
        .doc
        .sketches()
        .filter_map(|s| match (&s.level, &s.plane) {
            (Some(l), PlaneDef::Principal { offset, .. }) => Some((s.id, s.name.clone(), format!("at {offset}, height {}", l.height))),
            _ => None,
        })
        .collect();
    for (id, name, text) in rows {
        ui.horizontal(|ui| {
            if w::selectable(ui, app.sketch == id, &name).clicked() {
                app.set_sketch(id);
            }
            ui.small(text);
        });
    }
    ui.horizontal(|ui| {
        ui.label("Elevation");
        w::add(ui, "Level elevation", TextEdit::singleline(&mut app.level_elevation).desired_width(70.0));
    });
    ui.horizontal(|ui| {
        ui.label("Height");
        w::add(ui, "Level height", TextEdit::singleline(&mut app.level_height).desired_width(70.0));
    });
    let plane = PlaneDef::Principal { base: BasePlane::XY, offset: app.level_elevation.clone() };
    let level = Some(Level { height: app.level_height.clone() });
    ui.horizontal(|ui| {
        if w::button(ui, "Add level").clicked() {
            let id = app.doc.alloc_sketch_id();
            let n = app.doc.sketches().filter(|s| s.level.is_some()).count() + 1;
            app.exec(vec![Command::AddSketch(Sketch { id, name: format!("Level {n}"), plane: plane.clone(), level: level.clone() })]);
            if app.doc.sketch(id).is_some() {
                app.set_sketch(id);
            }
        }
        let active = app.doc.sketch(app.sketch).filter(|s| s.level.is_some()).cloned();
        if w::add_enabled(ui, active.is_some(), "Apply to level", egui::Button::new("Apply to level")).clicked()
            && let Some(s) = active
        {
            app.exec(vec![Command::SetSketch(Sketch { plane, level, ..s })]);
        }
    });
}
