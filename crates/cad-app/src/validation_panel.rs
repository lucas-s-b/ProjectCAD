//! The Model tab's "Validation report": runs the analytic benchmarks (`cad_document::benchmarks`) on demand and lists
//! them. Only these fixed cases are ever labelled "validated against <case>"; studies keep their approximate label.
use crate::app::CadApp;
use crate::widgets as w;
use eframe::egui::{self, Color32, RichText};
use std::time::Instant;

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.separator();
    ui.heading("Validation report");
    ui.small("Studies you run stay approximate. These fixed cases run the same mesher, solver, mass properties and loop solver against analytic results; a case within its tolerance is validated against it.");
    if w::button(ui, "Run benchmarks").clicked() {
        let start = Instant::now();
        let rows = cad_document::benchmarks();
        app.validation = Some((start.elapsed().as_secs_f64() * 1000.0, rows));
    }
    let Some((ms, result)) = &app.validation else {
        ui.small("Runs on the UI thread in about a second; docs/VALIDATION.md is the same report (cargo run --release -p cad-bench -- validate).");
        return;
    };
    let rows = match result {
        Ok(rows) => rows,
        Err(err) => {
            ui.label(RichText::new(format!("benchmarks: {err}")).color(Color32::RED));
            return;
        }
    };
    let summary = format!("{} of {} cases within tolerance ({ms:.0} ms)", rows.iter().filter(|b| b.passed()).count(), rows.len());
    let resp = ui.label(&summary);
    w::tag(ui, &summary, resp);
    for b in rows {
        ui.label(format!(
            "{}, {}: {:.6} against {:.6} ({}); error {:+.2e}, tolerance {:.0e}; {}",
            b.case, b.quantity, b.result, b.reference, b.formula, b.error(), b.tolerance, b.mesh
        ));
        let label = b.accuracy().to_string();
        let color = if b.passed() { Color32::from_rgb(80, 180, 80) } else { Color32::RED };
        let resp = ui.small(RichText::new(&label).color(color));
        w::tag(ui, &label, resp);
    }
}
