//! Section plane controls: display only, the model is not changed.
use crate::app::CadApp;
use crate::widgets as w;
use eframe::egui::{self, TextEdit};

pub fn controls(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.heading("Section");
    w::checkbox(ui, &mut app.section.on, "Section view");
    ui.horizontal(|ui| {
        let axis = egui::ComboBox::from_id_salt("Section axis").selected_text(["X", "Y", "Z"][app.section.axis]).width(50.0);
        w::combo(ui, "Section axis", axis, |ui| {
            for (i, n) in ["X", "Y", "Z"].iter().enumerate() {
                w::selectable_value(ui, &mut app.section.axis, i, n);
            }
        });
        ui.label("at");
        w::add(ui, "Section offset", TextEdit::singleline(&mut app.section.offset).desired_width(60.0));
        ui.label("mm");
    });
    w::checkbox(ui, &mut app.section.flip, "Flip section");
    ui.small("display only; picks still hit hidden parts");
}
