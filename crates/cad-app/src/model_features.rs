use crate::app::CadApp;
use cad_document::{BodyOp, Command, Feature, FeatureKind, FeatureState, PartDef, RevolveAxis, Target};
use crate::widgets as w;
use eframe::egui::{self, Color32, RichText, TextEdit};

fn op_name(op: BodyOp) -> String {
    if op == BodyOp::New { "New body".to_string() } else { format!("{op:?}") }
}

fn op_combo(ui: &mut egui::Ui, id: &str, op: &mut BodyOp) -> bool {
    let before = *op;
    w::combo(ui, "Operation", egui::ComboBox::from_id_salt(id).selected_text(op_name(*op)).width(70.0), |ui| {
        for o in [BodyOp::Add, BodyOp::Cut, BodyOp::Intersect, BodyOp::New] {
            w::selectable_value(ui, op, o, &op_name(o));
        }
    });
    *op != before
}

/// `bodies` are the ids of the bodies of the current rebuild.
fn target_combo(ui: &mut egui::Ui, id: &str, target: &mut Target, bodies: &[u64]) -> bool {
    let before = *target;
    let text = match target {
        Target::Auto => "Auto".to_string(),
        Target::All => "All bodies".to_string(),
        Target::Body(b) => format!("Body {b}"),
    };
    w::combo(ui, "Target", egui::ComboBox::from_id_salt(id).selected_text(text).width(70.0), |ui| {
        w::selectable_value(ui, target, Target::Auto, "Auto");
        w::selectable_value(ui, target, Target::All, "All bodies");
        for &b in bodies {
            w::selectable_value(ui, target, Target::Body(b), &format!("Body {b}"));
        }
    });
    *target != before
}

fn axis_combo(ui: &mut egui::Ui, id: &str, axis: &mut RevolveAxis) -> bool {
    let before = *axis;
    let text = match axis {
        RevolveAxis::X => "X axis".to_string(),
        RevolveAxis::Y => "Y axis".to_string(),
        RevolveAxis::Line(l) => format!("Line {l}"),
    };
    w::combo(ui, "Axis", egui::ComboBox::from_id_salt(id).selected_text(text).width(70.0), |ui| {
        w::selectable_value(ui, axis, RevolveAxis::X, "X axis");
        w::selectable_value(ui, axis, RevolveAxis::Y, "Y axis");
    });
    *axis != before
}

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    new_controls(app, ui);
    list(app, ui);
    editor(app, ui);
}

pub fn add_feature(app: &mut CadApp, label: &str, kind: FeatureKind) -> u64 {
    let id = app.doc.alloc_feature_id();
    let index = app.doc.features().len();
    let feature = Feature { id, name: format!("{label} {id}"), suppressed: false, kind };
    app.exec(vec![Command::AddFeature { index, feature }]);
    id
}

fn new_controls(app: &mut CadApp, ui: &mut egui::Ui) {
    let profile: Vec<_> = app.selection.iter().copied().collect();
    let bodies = app.body_ids();
    ui.add_enabled_ui(!profile.is_empty(), |ui| {
        ui.horizontal(|ui| {
            ui.label("Distance");
            w::add(ui, "Distance", TextEdit::singleline(&mut app.feat_distance).desired_width(80.0));
        });
        ui.horizontal(|ui| {
            ui.label("Taper");
            w::add(ui, "Taper", TextEdit::singleline(&mut app.feat_taper).desired_width(80.0));
        });
        ui.horizontal(|ui| {
            ui.label("Operation");
            op_combo(ui, "feat_op", &mut app.feat_op);
        });
        if w::button(ui, "Extrude").clicked() {
            let (distance, taper, op, flip, target) = (app.feat_distance.clone(), app.feat_taper.clone(), app.feat_op, app.feat_flip, app.feat_target);
            add_feature(app, "Extrude", FeatureKind::Extrude { profile: profile.clone(), distance, offset: "0".into(), taper, op, flip, target });
        }
        ui.horizontal(|ui| {
            ui.label("Angle");
            w::add(ui, "Angle", TextEdit::singleline(&mut app.feat_angle).desired_width(80.0));
        });
        ui.horizontal(|ui| {
            ui.label("Axis");
            axis_combo(ui, "feat_axis", &mut app.feat_axis);
        });
        if w::button(ui, "Revolve").clicked() {
            let (angle, axis, op, target) = (app.feat_angle.clone(), app.feat_axis, app.feat_op, app.feat_target);
            add_feature(app, "Revolve", FeatureKind::Revolve { profile, axis, angle, op, target });
        }
        w::checkbox(ui, &mut app.feat_flip, "Flip");
        ui.horizontal(|ui| {
            ui.label("Target");
            target_combo(ui, "feat_target", &mut app.feat_target, &bodies);
        });
    });
    blend_controls(app, ui, !bodies.is_empty());
    crate::model_pattern::controls(app, ui, !bodies.is_empty());
    crate::model_sweep::controls(app, ui);
    crate::model_shell::controls(app, ui, !bodies.is_empty());
    crate::model_parts::controls(app, ui);
}

fn blend_controls(app: &mut CadApp, ui: &mut egui::Ui, has_body: bool) {
    ui.add_enabled_ui(has_body, |ui| {
        ui.horizontal(|ui| {
            ui.label("Size");
            w::add(ui, "Size", TextEdit::singleline(&mut app.blend_size).desired_width(80.0));
        });
        if w::button(ui, "Fillet edges").clicked() {
            crate::edge_pick::start(app, false, None);
        }
        if w::button(ui, "Chamfer edges").clicked() {
            crate::edge_pick::start(app, true, None);
        }
    });
}

/// Every change from this panel drops the edit buffer so it is rebuilt from the stored feature.
fn run(app: &mut CadApp, cmds: Vec<Command>) {
    app.feature_edit = None;
    app.exec(cmds);
}

fn list(app: &mut CadApp, ui: &mut egui::Ui) {
    let states = app.model_result().map(|r| r.states.clone()).unwrap_or_default();
    let features: Vec<Feature> = app.doc.features().to_vec();
    let last = features.len().saturating_sub(1);
    for (i, f) in features.into_iter().enumerate() {
        // The name gets its own line: a long name plus the buttons is wider than the panel.
        if w::selectable(ui, app.feature_sel == Some(f.id), &f.name).clicked() {
            app.feature_sel = Some(f.id);
        }
        ui.horizontal(|ui| {
            let mut suppressed = f.suppressed;
            if w::checkbox(ui, &mut suppressed, "S").changed() {
                run(app, vec![Command::SetFeature(Feature { suppressed, ..f.clone() })]);
            }
            if w::add_enabled(ui, i > 0, "Up", egui::Button::new("Up")).clicked() {
                run(app, vec![Command::MoveFeature { id: f.id, index: i - 1 }]);
            }
            if w::add_enabled(ui, i < last, "Dn", egui::Button::new("Dn")).clicked() {
                run(app, vec![Command::MoveFeature { id: f.id, index: i + 1 }]);
            }
            if w::button(ui, "x").clicked() {
                run(app, vec![Command::RemoveFeature(f.id)]);
            }
        });
        match states.get(&f.id) {
            Some(FeatureState::Suppressed) => ui.small(RichText::new("suppressed").color(Color32::GRAY)),
            Some(FeatureState::Error(e)) => ui.small(RichText::new(e).color(Color32::RED)),
            _ => continue,
        };
    }
}

fn editor(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some(id) = app.feature_sel.filter(|id| app.doc.feature(*id).is_some()) else {
        app.feature_edit = None;
        return;
    };
    if app.feature_edit.as_ref().is_none_or(|(i, _)| *i != id) {
        app.feature_edit = app.doc.feature(id).map(|f| (id, f.clone()));
    }
    let bodies = app.body_ids();
    // Designation and thread of a part, from the stored feature (the buffer may hold an unfinished expression).
    let part = app.doc.feature(id).and_then(|f| match &f.kind {
        FeatureKind::Part { def, .. } => Some(app.doc.part(def)),
        _ => None,
    });
    // What the feature read in the last rebuild (`RebuildResult::depends`), by feature name.
    let uses = app.model_result().and_then(|r| r.depends.get(&id)).map(|ids| {
        let names: Vec<String> = ids.iter().map(|i| app.doc.feature(*i).map_or_else(|| format!("feature {i}"), |f| f.name.clone())).collect();
        format!("Uses: {}", if names.is_empty() { "none".to_string() } else { names.join(", ") })
    });
    let Some((_, buffer)) = app.feature_edit.as_mut() else { return };
    ui.separator();
    ui.label("Selected feature");
    let blend = match buffer.kind {
        FeatureKind::Fillet { .. } => Some(false),
        FeatureKind::Chamfer { .. } => Some(true),
        _ => None,
    };
    let mut commit = false;
    let mut field = |ui: &mut egui::Ui, label: &str, text: &mut String| {
        ui.horizontal(|ui| {
            ui.label(label);
            commit |= w::add(ui, label, TextEdit::singleline(text).id_salt((id, label)).desired_width(110.0)).lost_focus();
        });
    };
    field(ui, "Name", &mut buffer.name);
    if let Some(text) = &uses {
        let r = ui.label(text);
        w::tag(ui, text, r);
    }
    match &part {
        Some(Ok(p)) => {
            let text = format!("Designation: {}", p.designation());
            let r = ui.label(&text);
            // Tagged so MCP clients can read it from `widgets`.
            w::tag(ui, &text, r);
            if let Some(t) = p.thread() {
                let modelled = matches!(&buffer.kind, FeatureKind::Part { def: PartDef::Bolt { modelled: true, .. } | PartDef::Nut { modelled: true, .. }, .. });
                ui.label(format!("Thread {t} ({})", if modelled { "modelled" } else { "cosmetic" }));
            }
        }
        Some(Err(e)) => {
            ui.label(RichText::new(e).color(Color32::RED));
        }
        None => {}
    }
    match &mut buffer.kind {
        FeatureKind::Extrude { distance, offset, taper, op, flip, target, .. } => {
            field(ui, "Distance", distance);
            field(ui, "Offset", offset);
            field(ui, "Taper", taper);
            commit |= op_combo(ui, "edit_op", op);
            commit |= w::checkbox(ui, flip, "Flip").changed();
            commit |= target_combo(ui, "edit_target", target, &bodies);
        }
        FeatureKind::Revolve { angle, axis, op, target, .. } => {
            field(ui, "Angle", angle);
            commit |= axis_combo(ui, "edit_axis", axis);
            commit |= op_combo(ui, "edit_op", op);
            commit |= target_combo(ui, "edit_target", target, &bodies);
        }
        FeatureKind::Sweep { op, target, .. } => {
            commit |= op_combo(ui, "edit_op", op);
            commit |= target_combo(ui, "edit_target", target, &bodies);
        }
        FeatureKind::Loft { op, target, smooth, .. } => {
            commit |= w::checkbox(ui, smooth, "Smooth").changed();
            commit |= op_combo(ui, "edit_op", op);
            commit |= target_combo(ui, "edit_target", target, &bodies);
        }
        FeatureKind::Fillet { radius, .. } => field(ui, "Fillet radius", radius),
        FeatureKind::Chamfer { distance, .. } => field(ui, "Chamfer distance", distance),
        FeatureKind::Shell { thickness, .. } => field(ui, "Thickness", thickness),
        FeatureKind::Part { def, x, y, op, target, .. } => {
            let mut changed = crate::model_parts::fields(ui, def, &mut field);
            field(ui, "Part X", x);
            field(ui, "Part Y", y);
            changed |= op_combo(ui, "edit_op", op);
            changed |= target_combo(ui, "edit_target", target, &bodies);
            commit |= changed;
        }
        FeatureKind::Draft { angle, faces, sketch, .. } => {
            field(ui, "Draft angle", angle);
            ui.label(format!("{} faces, neutral plane: sketch {sketch}", faces.len()));
        }
        FeatureKind::Mesh { positions, triangles } => {
            ui.label(format!("Imported mesh: {} points, {} triangles", positions.len(), triangles.len()));
        }
        kind => {
            let changed = crate::model_pattern::fields(ui, &app.doc, kind, &mut field);
            commit |= changed;
        }
    }
    if let Some(chamfer) = blend {
        let broken = matches!(app.model_result().and_then(|r| r.states.get(&id)), Some(FeatureState::Error(_) | FeatureState::Suppressed));
        if w::add_enabled(ui, broken, "Re-pick edges", egui::Button::new("Re-pick edges")).clicked() {
            crate::edge_pick::start(app, chamfer, Some(id));
        }
    }
    if commit {
        let stored = app.doc.feature(id).cloned();
        let edited = app.feature_edit.take().map(|(_, f)| f);
        if let Some(f) = edited.filter(|f| Some(f) != stored.as_ref()) {
            app.exec(vec![Command::SetFeature(f)]);
        }
    }
}
