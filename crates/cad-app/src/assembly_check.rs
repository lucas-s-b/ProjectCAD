//! The Assembly tab's interference section: interference, contact and clearance between placed components, on demand.
use crate::app::CadApp;
use crate::assembly_joints::name;
use crate::widgets as w;
use cad_document::{Check, Overlap};
use eframe::egui::{self, Color32, RichText, TextEdit};

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.separator();
    ui.heading("Interference");
    // The check reads the last rebuild, so it waits until that matches the document.
    let (current, mut run) = (app.model_current(), false);
    ui.horizontal_wrapped(|ui| {
        ui.label("Clearance limit");
        w::add(ui, "Clearance limit", TextEdit::singleline(&mut app.check_limit).desired_width(70.0));
        run = w::add_enabled(ui, current, "Check", egui::Button::new("Check")).clicked();
    });
    if run {
        run_check(app);
    }
    rows(app, ui);
}

/// Runs `Document::check` on the current rebuild; the status line gives the counts or the limit's error.
fn run_check(app: &mut CadApp) {
    let Some(result) = app.model_result().map(|r| app.doc.check(r, &app.check_limit)) else { return };
    match result {
        Ok(c) => {
            app.status = summary(&c);
            app.check = Some((app.doc.revision(), c));
        }
        Err(e) => {
            app.status = format!("clearance limit: {e}");
            app.check = None;
        }
    }
    // The scene shows the result (see the stamp in `view3d::show`).
    app.sel_rev += 1;
}

/// "Check: interference 1, contact 3, clearance 1".
fn summary(c: &Check) -> String {
    let interference = c.meetings.iter().filter(|m| matches!(m.kind, Ok(Overlap::Interference { .. }))).count();
    let contact = c.meetings.iter().filter(|m| matches!(m.kind, Ok(Overlap::Contact))).count();
    format!("Check: interference {interference}, contact {contact}, clearance {}", c.clearances.len())
}

/// One line per meeting (interference volume in red, contact, or the boolean's error in red) and per clearance.
fn rows(app: &CadApp, ui: &mut egui::Ui) {
    let Some(c) = app.check_result() else {
        ui.small("Check runs on demand; any edit clears the result.");
        return;
    };
    if c.meetings.is_empty() && c.clearances.is_empty() {
        ui.label("No interference, contact or clearance within the limit");
    }
    let s = app.doc.settings();
    for m in &c.meetings {
        let pair = format!("{} / {}", name(app, m.a), name(app, m.b));
        match &m.kind {
            Ok(Overlap::Interference { volume, .. }) => ui.label(RichText::new(format!("{pair}: interference {}", s.format_volume(*volume))).color(Color32::RED)),
            Ok(Overlap::Contact) => ui.label(format!("{pair}: contact")),
            Err(e) => ui.label(RichText::new(format!("{pair}: {e}")).color(Color32::RED)),
        };
    }
    for (a, b, k) in &c.clearances {
        ui.label(format!("{} / {}: clearance {} {}", name(app, *a), name(app, *b), s.format_length(k.distance), s.units.suffix()));
    }
}
