use crate::app::CadApp;
use crate::model_features;
use crate::widgets as w;
use cad_document::{Command, RebuildResult, Value};
use cad_geometry::DVec3;
use eframe::egui::{self, Color32, RichText, TextEdit};

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.heading("Parameters");
    params(app, ui);
    ui.separator();
    ui.heading("Features");
    model_features::show(app, ui);
    ui.separator();
    body(app, ui);
    ui.separator();
    crate::model_section::controls(app, ui);
    crate::model_sketches::show(app, ui);
}

fn value_text(app: &CadApp, v: Value) -> String {
    let s = app.doc.settings();
    let p = s.precision as usize;
    match (v.dim.length, v.dim.angle) {
        (1, 0) => format!("{} {}", s.format_length(v.v), s.units.suffix()),
        (0, 1) => format!("{:.p$}\u{b0}", v.v.to_degrees()),
        (0, 0) => format!("{:.p$}", v.v),
        (l, a) => format!("{:.p$} (mm^{l} rad^{a})", v.v),
    }
}

fn params(app: &mut CadApp, ui: &mut egui::Ui) {
    let values = app.doc.params().evaluate();
    let rows: Vec<(String, String)> = app.doc.params().iter().map(|(n, e)| (n.to_string(), e.to_string())).collect();
    for (name, expr) in rows {
        let mut text = match &app.param_edit {
            Some((n, t)) if *n == name => t.clone(),
            _ => expr.clone(),
        };
        ui.horizontal(|ui| {
            ui.label(&name);
            let r = w::add(ui, &format!("Param {name}"), TextEdit::singleline(&mut text).id_salt(("param", &name)).desired_width(110.0));
            if r.changed() {
                app.param_edit = Some((name.clone(), text.clone()));
            }
            if r.lost_focus()
                && let Some((_, t)) = app.param_edit.take().filter(|(n, t)| *n == name && *t != expr)
            {
                app.exec(vec![Command::SetParam { name: name.clone(), expr: Some(t) }]);
            }
            if w::button(ui, "x").clicked() {
                app.param_edit = None;
                app.exec(vec![Command::SetParam { name: name.clone(), expr: None }]);
            }
        });
        match values.get(&name) {
            Some(Ok(v)) => ui.small(value_text(app, *v)),
            Some(Err(e)) => ui.small(RichText::new(e.to_string()).color(Color32::RED)),
            None => continue,
        };
    }
    ui.horizontal(|ui| {
        w::add(ui, "Param name", TextEdit::singleline(&mut app.param_new.0).hint_text("name").desired_width(60.0));
        w::add(ui, "Param expression", TextEdit::singleline(&mut app.param_new.1).hint_text("expression").desired_width(90.0));
        if w::button(ui, "Add").clicked() {
            let (name, expr) = app.param_new.clone();
            app.exec(vec![Command::SetParam { name, expr: Some(expr) }]);
            if app.status.is_empty() {
                app.param_new = Default::default();
            }
        }
    });
}

/// Sums over every body; the centroid is weighted by volume.
pub struct Totals {
    pub volume: f64,
    pub area: f64,
    pub centroid: DVec3,
    pub triangles: usize,
    pub faces: usize,
}

pub fn totals(r: &RebuildResult) -> Option<Totals> {
    if r.bodies.is_empty() {
        return None;
    }
    let volume = r.total_volume();
    let weighted: DVec3 = r.bodies.iter().map(|(_, b)| b.centroid() * b.volume()).sum();
    Some(Totals {
        volume,
        area: r.bodies.iter().map(|(_, b)| b.area()).sum(),
        centroid: weighted / volume,
        triangles: r.bodies.iter().map(|(_, b)| b.mesh.triangle_count()).sum(),
        faces: r.bodies.iter().map(|(_, b)| b.faces.len()).sum(),
    })
}

fn body(app: &CadApp, ui: &mut egui::Ui) {
    let has_features = !app.doc.features().is_empty();
    if app.rebuilding() {
        ui.label("Rebuilding...");
    }
    let Some(r) = app.model_result() else { return };
    let Some(t) = totals(r) else {
        if has_features {
            ui.label("No body");
        }
        return;
    };
    ui.heading("Body");
    let c = t.centroid;
    ui.label(format!("Volume \u{2248} {:.3} mm\u{b3}", t.volume));
    ui.label(format!("Area \u{2248} {:.3} mm\u{b2}", t.area));
    ui.label(format!("Centroid \u{2248} ({:.3}, {:.3}, {:.3})", c.x, c.y, c.z));
    ui.label(format!("Triangles: {}", t.triangles));
    if r.bodies.len() > 1 {
        for (id, b) in &r.bodies {
            ui.label(format!("Body {id}: \u{2248} {:.3} mm\u{b3}", b.volume()));
        }
    }
    ui.small("approximate: computed from the tessellation at 0.02 mm");
}
