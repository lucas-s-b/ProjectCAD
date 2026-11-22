//! The part generators in the Model tab: kind buttons and "Add part" (a part feature on the active sketch), and the
//! fields of a selected part feature.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::{BodyOp, FeatureKind, PartDef, Target};
use eframe::egui;

/// Kind buttons and "Add part": a part with the default values at (0, 0) of the active sketch, as a new body; it
/// becomes the selected feature, so its fields show in the editor.
pub fn controls(app: &mut CadApp, ui: &mut egui::Ui) {
    let defaults = PartDef::defaults();
    ui.horizontal_wrapped(|ui| {
        for (i, d) in defaults.iter().enumerate() {
            w::selectable_value(ui, &mut app.part_choice, i, d.label());
        }
    });
    if w::button(ui, "Add part").clicked() {
        let def = defaults[app.part_choice].clone();
        let label = def.label();
        let kind = FeatureKind::Part { def, sketch: app.sketch, x: "0".into(), y: "0".into(), op: BodyOp::New, target: Target::Auto };
        let id = crate::model_features::add_feature(app, label, kind);
        if app.doc.feature(id).is_some() {
            app.feature_sel = Some(id);
        }
    }
}

/// The parameter fields of a part (text fields commit through `field`); true when "Add step" or "Remove step" changed
/// a shaft.
pub fn fields(ui: &mut egui::Ui, def: &mut PartDef, field: &mut dyn FnMut(&mut egui::Ui, &str, &mut String)) -> bool {
    match def {
        PartDef::Gear { module, teeth, pressure, width, bore } => {
            [("Gear module", module), ("Gear teeth", teeth), ("Pressure angle", pressure), ("Face width", width), ("Gear bore", bore)]
                .into_iter()
                .for_each(|(l, t)| field(ui, l, t));
            false
        }
        PartDef::Shaft { steps, chamfer } => {
            for (i, [d, l]) in steps.iter_mut().enumerate() {
                field(ui, &format!("Step {} diameter", i + 1), d);
                field(ui, &format!("Step {} length", i + 1), l);
            }
            field(ui, "Shaft chamfer", chamfer);
            let mut changed = false;
            ui.horizontal(|ui| {
                if w::button(ui, "Add step").clicked() {
                    let last = steps.last().cloned().unwrap_or_else(|| ["10 mm".into(), "10 mm".into()]);
                    steps.push(last);
                    changed = true;
                }
                if w::add_enabled(ui, steps.len() > 1, "Remove step", egui::Button::new("Remove step")).clicked() {
                    steps.pop();
                    changed = true;
                }
            });
            changed
        }
        PartDef::Spring { wire, mean, pitch, turns } => {
            [("Wire diameter", wire), ("Mean diameter", mean), ("Spring pitch", pitch), ("Spring turns", turns)]
                .into_iter()
                .for_each(|(l, t)| field(ui, l, t));
            false
        }
        PartDef::Bolt { size, length } => {
            field(ui, "Thread size", size);
            field(ui, "Bolt length", length);
            false
        }
        PartDef::Nut { size } => {
            field(ui, "Thread size", size);
            false
        }
    }
}
