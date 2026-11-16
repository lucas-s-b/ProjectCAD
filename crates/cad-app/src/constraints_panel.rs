use crate::app::CadApp;
use crate::constraint_options::proposals;
use cad_document::{Command, Constraint, ConstraintId, Document, Status};
use crate::widgets as w;
use eframe::egui;

/// The status line text; None when the document has no constraints.
pub fn status_text(doc: &Document) -> Option<String> {
    Some(match doc.constraint_status()? {
        Ok(s) => match s.status() {
            Status::FullyConstrained => "Fully constrained".to_string(),
            Status::UnderConstrained => format!("Under-constrained ({} DOF)", s.dof),
            Status::OverConstrained => format!("Over-constrained ({} redundant)", s.redundant),
        },
        Err(e) => e.to_string(),
    })
}

pub fn show(app: &mut CadApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let options = proposals(&app.doc, &app.selection);
    let any = app.doc.constraints().next().is_some();
    if options.is_empty() && !any {
        app.constraint_edit = None;
        return;
    }
    ui.heading("Constraints");
    constrain_buttons(app, ui, options);
    if !any {
        app.constraint_edit = None;
        return;
    }
    let rev = app.doc.revision();
    if app.constraint_cache.as_ref().is_none_or(|(r, _)| *r != rev) {
        app.constraint_cache = Some((rev, status_text(&app.doc)));
    }
    if let Some((_, Some(text))) = &app.constraint_cache {
        ui.label(text);
    }
    constraint_list(app, ui);
    commit_edit(app, ctx);
}

fn constrain_buttons(app: &mut CadApp, ui: &mut egui::Ui, options: Vec<(&'static str, Vec<Constraint>)>) {
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        for (label, list) in options {
            if w::button(ui, label).clicked() {
                chosen = Some(list);
            }
        }
    });
    if let Some(list) = chosen {
        let cmds = list.into_iter().map(|c| Command::AddConstraint(app.doc.alloc_constraint_id(), c)).collect();
        app.exec(cmds);
    }
}

fn constraint_list(app: &mut CadApp, ui: &mut egui::Ui) {
    let rows: Vec<(ConstraintId, Constraint)> = app.doc.constraints().map(|(id, c)| (id, c.clone())).collect();
    let mut remove = None;
    for (id, c) in rows {
        let names: Vec<String> =
            c.entities().iter().map(|e| app.doc.get(*e).map_or_else(|| e.to_string(), |e| e.name.clone())).collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{}: {}", c.name(), names.join(", ")));
            if let Some(stored) = c.value() {
                let is_angle = matches!(c, Constraint::Angle(..));
                let value = app.constraint_edit.filter(|(i, _)| *i == id).map_or(stored, |(_, v)| v);
                let mut shown = if is_angle { value.to_degrees() } else { value };
                let before = shown;
                w::add(ui, "Constraint value", egui::DragValue::new(&mut shown).speed(0.1));
                if shown != before {
                    app.constraint_edit = Some((id, if is_angle { shown.to_radians() } else { shown }));
                }
            }
            let x = ui.small_button("x");
            if w::tag(ui, "x", x).clicked() {
                remove = Some(id);
            }
        });
    }
    if let Some(id) = remove {
        app.exec(vec![Command::RemoveConstraint(id)]);
    }
}

/// Same rule as the Properties inspector: commit when the buffer differs and nothing is being edited.
fn commit_edit(app: &mut CadApp, ctx: &egui::Context) {
    let Some((id, buffer)) = app.constraint_edit else { return };
    let new = app.doc.constraint(id).filter(|c| c.value() != Some(buffer)).and_then(|c| c.with_value(buffer));
    let Some(new) = new else {
        app.constraint_edit = None;
        return;
    };
    if ctx.egui_is_using_pointer() || ctx.memory(|m| m.focused().is_some()) {
        ctx.request_repaint();
    } else {
        app.constraint_edit = None;
        app.exec(vec![Command::SetConstraint(id, new)]);
    }
}
