//! The Assembly tab's interference section: interference, contact and clearance between placed components, on demand.
use crate::analysis_worker::{JobKind, JobOutput};
use crate::app::CadApp;
use crate::assembly_joints::name;
use crate::widgets as w;
use cad_document::{Check, Overlap};
use eframe::egui::{self, Color32, RichText, TextEdit};

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.separator();
    ui.heading(crate::i18n::tr("Interference"));
    // The check reads the last rebuild, so it waits until that matches the document.
    let (current, mut run) = (app.model_current(), false);
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::i18n::tr("Clearance limit"));
        w::add(ui, "Clearance limit", TextEdit::singleline(&mut app.check_limit).desired_width(70.0));
        run = w::add_enabled(ui, current && app.jobs.running().is_none(), "Check", egui::Button::new("Check")).clicked();
    });
    if run {
        run_check(app, ui.ctx());
    }
    rows(app, ui);
}

/// Starts `Document::check` on the current rebuild on the analysis worker; `finish` takes the result.
fn run_check(app: &mut CadApp, ctx: &egui::Context) {
    let Some(r) = app.model_result().cloned() else { return };
    let (doc, limit) = (app.doc.clone(), app.check_limit.clone());
    app.jobs.start(ctx, JobKind::Check, Some(app.doc.revision()), move || JobOutput::Check(doc.check(&r, &limit)));
}

/// Shows a finished check; the status line gives the counts or the limit's error.
pub fn finish(app: &mut CadApp, result: Result<Check, String>) {
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
        ui.label(crate::i18n::tr("No interference, contact or clearance within the limit"));
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
