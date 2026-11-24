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

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let options = proposals(&app.doc, &app.selection);
    let any = app.doc.constraints().next().is_some();
    if options.is_empty() && !any {
        app.constraint_edit = None;
        return;
    }
    ui.heading(crate::i18n::tr("Constraints"));
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
            if c.value().is_some() {
                value_field(app, ui, id, &c);
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

/// The expression when there is one, else the number (angles in degrees).
fn shown_text(doc: &Document, id: ConstraintId, c: &Constraint) -> String {
    if let Some(expr) = doc.constraint_expr(id) {
        return expr.to_string();
    }
    let v = c.value().unwrap_or_default();
    let n = if matches!(c, Constraint::Angle(..)) { v.to_degrees() } else { v };
    format!("{n:.6}").trim_end_matches('0').trim_end_matches('.').to_string()
}

fn value_field(app: &mut CadApp, ui: &mut egui::Ui, id: ConstraintId, c: &Constraint) {
    let shown = shown_text(&app.doc, id, c);
    let mut text = match &app.constraint_edit {
        Some((i, t)) if *i == id => t.clone(),
        _ => shown.clone(),
    };
    let r = w::add(ui, "Constraint value", egui::TextEdit::singleline(&mut text).id_salt(("constraint", id)).desired_width(80.0));
    if r.changed() {
        app.constraint_edit = Some((id, text));
    }
    if r.lost_focus()
        && let Some((_, t)) = app.constraint_edit.take().filter(|(i, t)| *i == id && *t != shown)
    {
        commit(app, id, c, t.trim());
    }
}

/// A plain number sets the value and drops any expression; anything else becomes the expression.
fn commit(app: &mut CadApp, id: ConstraintId, c: &Constraint, text: &str) {
    let cmds = match text.parse::<f64>() {
        Ok(n) if n.is_finite() => {
            let v = if matches!(c, Constraint::Angle(..)) { n.to_radians() } else { n };
            let drop_expr = app.doc.constraint_expr(id).map(|_| Command::SetConstraintExpr { id, expr: None });
            drop_expr.into_iter().chain(c.with_value(v).map(|new| Command::SetConstraint(id, new))).collect()
        }
        _ => vec![Command::SetConstraintExpr { id, expr: Some(text.to_string()) }],
    };
    app.exec(cmds);
}
