//! Captured selections and the Sweep / Loft buttons.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::FeatureKind;
use eframe::egui::{self, Color32, RichText};

pub fn controls(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.label(format!("Captured: {}", app.captures.len()));
    for (k, c) in app.captures.iter().enumerate() {
        let sketch = c.first().and_then(|id| app.doc.get(*id)).and_then(|e| app.doc.sketch(e.sketch)).map_or("?", |s| s.name.as_str());
        ui.small(format!("{}: {} entities in {sketch}", k + 1, c.len()));
    }
    ui.add_enabled_ui(!app.selection.is_empty(), |ui| {
        if w::button(ui, "Capture selection").clicked() {
            app.captures.push(app.selection.iter().copied().collect());
            app.sel_rev += 1;
        }
    });
    ui.add_enabled_ui(!app.captures.is_empty(), |ui| {
        if w::button(ui, "Clear captures").clicked() {
            app.captures.clear();
            app.sel_rev += 1;
        }
    });
    let (op, target, n) = (app.feat_op, app.feat_target, app.captures.len());
    ui.add_enabled_ui(n == 2, |ui| {
        if w::button(ui, "Sweep").clicked() {
            let kind = FeatureKind::Sweep { profile: app.captures[0].clone(), path: app.captures[1].clone(), op, target };
            add(app, "Sweep", kind);
        }
    });
    ui.add_enabled_ui(n >= 2, |ui| {
        if w::button(ui, "Loft").clicked() {
            add(app, "Loft", FeatureKind::Loft { sections: app.captures.clone(), op, target });
        }
    });
    ui.small(RichText::new("Sweep: capture the profile, then the path. Loft: capture the sections in order.").color(Color32::GRAY));
}

fn add(app: &mut CadApp, label: &str, kind: FeatureKind) {
    let id = crate::model_features::add_feature(app, label, kind);
    if app.doc.feature(id).is_some() {
        app.captures.clear();
        app.sel_rev += 1;
        app.feature_sel = Some(id);
    }
}
