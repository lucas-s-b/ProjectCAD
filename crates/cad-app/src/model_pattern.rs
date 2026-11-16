//! Controls for pattern and mirror features.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::{Axis3, BasePlane, Document, FeatureKind, MirrorSource};
use eframe::egui;

/// New pattern and mirror features at the end of the list; the selected feature is the source.
pub fn controls(app: &mut CadApp, ui: &mut egui::Ui, has_body: bool) {
    let selected = app.feature_sel.filter(|id| app.doc.feature(*id).is_some());
    let mirror = |source| FeatureKind::Mirror { source, plane: BasePlane::YZ, offset: "0".into() };
    ui.add_enabled_ui(selected.is_some(), |ui| {
        let source = selected.unwrap_or_default();
        if w::button(ui, "Linear pattern").clicked() {
            let kind = FeatureKind::LinearPattern { source, count: "3".into(), dx: "10 mm".into(), dy: "0".into(), dz: "0".into() };
            add(app, "Linear pattern", kind);
        }
        if w::button(ui, "Circular pattern").clicked() {
            let (z, count, angle) = ("0".to_string(), "4".to_string(), "360 deg".to_string());
            add(app, "Circular pattern", FeatureKind::CircularPattern { source, count, angle, axis: Axis3::Z, cx: z.clone(), cy: z.clone(), cz: z });
        }
        if w::button(ui, "Mirror feature").clicked() {
            add(app, "Mirror", mirror(MirrorSource::Feature(source)));
        }
    });
    ui.add_enabled_ui(has_body, |ui| {
        if w::button(ui, "Mirror bodies").clicked() {
            add(app, "Mirror", mirror(MirrorSource::Bodies));
        }
    });
}

fn add(app: &mut CadApp, label: &str, kind: FeatureKind) {
    let id = crate::model_features::add_feature(app, label, kind);
    if app.doc.feature(id).is_some() {
        app.feature_sel = Some(id);
    }
}

fn combo<T: PartialEq + Copy>(ui: &mut egui::Ui, name: &str, value: &mut T, options: &[(T, &str)]) -> bool {
    let before = *value;
    let text = options.iter().find(|(v, _)| v == value).map_or("", |(_, label)| *label);
    w::combo(ui, name, egui::ComboBox::from_id_salt(name).selected_text(text).width(70.0), |ui| {
        for (v, label) in options {
            w::selectable_value(ui, value, *v, label);
        }
    });
    *value != before
}

/// "Source: ..." for a pattern or mirror feature.
fn source_label(doc: &Document, kind: &FeatureKind) -> Option<String> {
    let name = |id| doc.feature(id).map_or_else(|| format!("feature {id}"), |f| f.name.clone());
    match kind {
        FeatureKind::LinearPattern { source, .. } | FeatureKind::CircularPattern { source, .. } => Some(name(*source)),
        FeatureKind::Mirror { source: MirrorSource::Feature(source), .. } => Some(name(*source)),
        FeatureKind::Mirror { source: MirrorSource::Bodies, .. } => Some("all bodies".to_string()),
        _ => None,
    }
}

/// Fields of the selected pattern or mirror; true when a combo changed (text fields commit through `field`).
pub fn fields(ui: &mut egui::Ui, doc: &Document, kind: &mut FeatureKind, field: &mut dyn FnMut(&mut egui::Ui, &str, &mut String)) -> bool {
    if let Some(source) = source_label(doc, kind) {
        ui.label(format!("Source: {source}"));
    }
    match kind {
        FeatureKind::LinearPattern { count, dx, dy, dz, .. } => {
            [("Count", count), ("dx", dx), ("dy", dy), ("dz", dz)].into_iter().for_each(|(l, t)| field(ui, l, t));
            false
        }
        FeatureKind::CircularPattern { count, angle, axis, cx, cy, cz, .. } => {
            [("Count", count), ("Angle", angle), ("cx", cx), ("cy", cy), ("cz", cz)].into_iter().for_each(|(l, t)| field(ui, l, t));
            combo(ui, "Pattern axis", axis, &[(Axis3::X, "About X"), (Axis3::Y, "About Y"), (Axis3::Z, "About Z")])
        }
        FeatureKind::Mirror { plane, offset, .. } => {
            field(ui, "Mirror offset", offset);
            combo(ui, "Mirror plane", plane, &[(BasePlane::XY, "Mirror XY"), (BasePlane::XZ, "Mirror XZ"), (BasePlane::YZ, "Mirror YZ")])
        }
        _ => false,
    }
}
