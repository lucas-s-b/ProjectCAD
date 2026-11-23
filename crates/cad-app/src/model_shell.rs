//! Thickness field and the Shell face button; draft angle and the Draft faces button.
use crate::app::{CadApp, PickFace, ViewMode};
use crate::widgets as w;
use eframe::egui::{self, TextEdit};

pub fn controls(app: &mut CadApp, ui: &mut egui::Ui, has_body: bool) {
    ui.add_enabled_ui(has_body, |ui| {
        ui.horizontal(|ui| {
            ui.label("Thickness");
            w::add(ui, "Thickness", TextEdit::singleline(&mut app.shell_thickness).desired_width(80.0));
        });
        if w::button(ui, "Shell face").clicked() {
            app.pick_face = PickFace::Shell;
            app.pick_edges = None;
            app.view = ViewMode::ThreeD;
            app.status = "Click the planar face to open".into();
        }
        ui.horizontal(|ui| {
            ui.label("Draft angle");
            w::add(ui, "Draft angle", TextEdit::singleline(&mut app.draft_angle).desired_width(80.0));
        });
        if w::button(ui, "Draft faces").clicked() {
            app.pick_face = PickFace::Draft(None);
            app.pick_edges = None;
            app.view = ViewMode::ThreeD;
            app.status = format!("Click faces of one body to draft about sketch {}; Esc to finish", app.sketch);
        }
    });
}
