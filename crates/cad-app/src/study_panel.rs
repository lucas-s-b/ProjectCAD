//! The Model tab's "Static study" section: linear-elastic studies stored in the document, run on demand, and the
//! colour map and legend the 3D view draws.
use crate::analysis_worker::{JobKind, JobOutput};
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::{Body, Command, FaceKey, FaceRef, FeatureId, Load, LoadKind, MATERIALS, Study, StudyReport};
use cad_geometry::DVec3;
use eframe::egui::{self, Color32, RichText, TextEdit};

/// "plane +X", "plane (0.600, 0.800, 0.000)", "cylinder Z", "surface": which way a face points.
pub(crate) fn key_text(key: &FaceKey) -> String {
    let dir = |v: DVec3| {
        for (name, a) in [("X", DVec3::X), ("Y", DVec3::Y), ("Z", DVec3::Z)] {
            if v.distance(a) < 1e-9 {
                return format!("+{name}");
            }
            if v.distance(-a) < 1e-9 {
                return format!("-{name}");
            }
        }
        format!("({:.3}, {:.3}, {:.3})", v.x, v.y, v.z)
    };
    match key {
        FaceKey::Plane(n) => format!("plane {}", dir(*n)),
        FaceKey::Cylinder(a) => format!("cylinder {}", dir(*a).trim_start_matches(['+', '-'])),
        FaceKey::Other => "surface".into(),
    }
}

/// Blue (0), cyan, green, yellow, red (1).
pub fn ramp(t: f32) -> [f32; 3] {
    const STOPS: [[f32; 3]; 5] = [[0.0, 0.0, 1.0], [0.0, 1.0, 1.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.0, 0.0]];
    let x = t.clamp(0.0, 1.0) * 4.0;
    let k = (x as usize).min(3);
    let f = x - k as f32;
    [0, 1, 2].map(|c| STOPS[k][c] + (STOPS[k + 1][c] - STOPS[k][c]) * f)
}

/// The coloured quantity per node: von Mises (MPa) or displacement length (mm).
pub fn values(app: &CadApp, rep: &StudyReport) -> Vec<f64> {
    if app.study_by_displacement { rep.result.displacement.iter().map(|u| u.length()).collect() } else { rep.result.von_mises.clone() }
}

/// (smallest, largest) of `v`.
pub fn range(v: &[f64]) -> (f64, f64) {
    v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| (lo.min(x), hi.max(x)))
}

/// The result the 3D view draws: the last run's while the document is unchanged and "Show result" is on.
pub fn shown(app: &CadApp) -> Option<&StudyReport> {
    if !app.show_study {
        return None;
    }
    app.study_result()?.as_ref().ok()
}

/// Study choice, "New static study" and "Delete static study", the editor of the chosen study, "Run static study" and
/// the result. A new study takes the body of the selected face, else the last body.
pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let labels = crate::assembly_panel::body_labels(app);
    if labels.is_empty() {
        return;
    }
    ui.separator();
    ui.heading(crate::i18n::tr("Static study"));
    let count = app.doc.studies().len();
    app.study_sel = app.study_sel.min(count.saturating_sub(1));
    let (mut new, mut delete) = (false, false);
    ui.horizontal_wrapped(|ui| {
        let current = app.doc.studies().get(app.study_sel).map_or_else(|| "No study".to_string(), |s| s.name.clone());
        w::combo(ui, "Static study", egui::ComboBox::from_id_salt("static_study").selected_text(current), |ui| {
            for i in 0..count {
                let name = app.doc.studies()[i].name.clone();
                w::selectable_value(ui, &mut app.study_sel, i, &name);
            }
        });
        new = w::button(ui, "New static study").clicked();
        delete = w::add_enabled(ui, count > 0, "Delete static study", egui::Button::new("Delete static study")).clicked();
    });
    if new {
        let body = app.face_sel.map(|(b, _)| b).filter(|b| labels.iter().any(|(id, _)| id == b)).unwrap_or(labels[labels.len() - 1].0);
        let mut all = app.doc.studies().to_vec();
        all.push(Study::new(format!("Study {}", count + 1), body, app.mesh_size.clone()));
        app.exec(vec![Command::SetStudies(all)]);
        app.study_sel = count;
        return;
    }
    if delete {
        let mut all = app.doc.studies().to_vec();
        all.remove(app.study_sel);
        app.exec(vec![Command::SetStudies(all)]);
        return;
    }
    if count == 0 {
        ui.small("A study holds faces of a body, loads others and shows displacements and stresses (linear elastic).");
        return;
    }
    editor(app, ui, &labels);
    let mut run_it = false;
    ui.horizontal_wrapped(|ui| {
        run_it = w::add_enabled(ui, app.model_current() && app.jobs.running().is_none(), "Run static study", egui::Button::new("Run static study")).clicked();
    });
    if run_it {
        run(app, ui.ctx());
    }
    rows(app, ui);
}

/// Fields of the selected study; text fields commit when they lose focus, the other widgets at once.
fn editor(app: &mut CadApp, ui: &mut egui::Ui, labels: &[(FeatureId, String)]) {
    let (rev, index) = (app.doc.revision(), app.study_sel);
    if app.study_edit.as_ref().is_none_or(|(r, i, _)| *r != rev || *i != index) {
        app.study_edit = app.doc.studies().get(index).cloned().map(|s| (rev, index, s));
    }
    let Some(body_id) = app.study_edit.as_ref().map(|(_, _, s)| s.body) else { return };
    // A face clicked in 3D on the study's body becomes the chosen face.
    if app.face_sel != app.study_face_seen {
        app.study_face_seen = app.face_sel;
        if let Some((b, f)) = app.face_sel
            && b == body_id
        {
            app.study_face = Some(f);
        }
    }
    let body: Option<std::sync::Arc<Body>> = app.model_result().and_then(|r| r.bodies.iter().find(|(id, _)| *id == body_id)).map(|(_, b)| b.clone());
    let faces: Vec<(u32, String)> = body.iter().flat_map(|b| (0..b.faces.len()).map(|f| (f as u32, format!("Face {f}: {}", key_text(&FaceKey::of(&b.faces[f])))))).collect();
    let picked: Option<FaceRef> = app.study_face.zip(body.as_ref()).and_then(|(f, b)| FaceRef::of(b, f as usize));
    let (mut commit, mut fix, mut force, mut pressure) = (false, false, false, false);
    let (mut drop_fixed, mut drop_load) = (None, None);
    let Some((_, _, buf)) = app.study_edit.as_mut() else { return };
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::i18n::tr("Body"));
        let current = labels.iter().find(|(id, _)| *id == buf.body).map_or_else(String::new, |(_, t)| t.clone());
        w::combo(ui, "Study body", egui::ComboBox::from_id_salt("study_body").selected_text(current).width(140.0), |ui| {
            for (id, text) in labels {
                commit |= w::selectable_value(ui, &mut buf.body, *id, text).clicked();
            }
        });
    });
    let field = |ui: &mut egui::Ui, label: &str, text: &mut String| -> bool { w::add(ui, label, TextEdit::singleline(text).id_salt(("study", index, label)).desired_width(70.0)).lost_focus() };
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::i18n::tr("Element size"));
        commit |= field(ui, "Study element size", &mut buf.h);
    });
    let mut material_clicked = None;
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::i18n::tr("Material"));
        w::combo(ui, "Study material", egui::ComboBox::from_id_salt("study_material").selected_text(buf.material.clone()), |ui| {
            for m in MATERIALS {
                if w::selectable(ui, buf.material == m.name, m.name).clicked() {
                    material_clicked = Some(Some(m.name));
                }
            }
            if w::selectable(ui, buf.material == "Custom", "Custom").clicked() {
                material_clicked = Some(None);
            }
        });
    });
    if let Some(choice) = material_clicked {
        match choice {
            Some(name) => {
                buf.set_material(name);
            }
            None => buf.material = "Custom".into(),
        }
        commit = true;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::i18n::tr("E (MPa)"));
        commit |= field(ui, "Young's modulus", &mut buf.young);
        ui.label(crate::i18n::tr("Poisson"));
        commit |= field(ui, "Poisson's ratio", &mut buf.poisson);
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::i18n::tr("Yield (MPa)"));
        commit |= field(ui, "Yield strength", &mut buf.yield_strength);
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Density (g/cm\u{b3})");
        commit |= field(ui, "Study density", &mut buf.density);
        commit |= w::checkbox(ui, &mut buf.gravity, "Gravity (-Z)").changed();
    });
    for (k, r) in buf.fixed.iter().enumerate() {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Fixed: {}", key_text(&r.key)));
            if w::button(ui, "Remove support").clicked() {
                drop_fixed = Some(k);
            }
        });
    }
    for (k, l) in buf.loads.iter().enumerate() {
        ui.horizontal_wrapped(|ui| {
            let text = match &l.kind {
                LoadKind::Force(c) => format!("Force ({}, {}, {}) N on {}", c[0], c[1], c[2], key_text(&l.face.key)),
                LoadKind::Pressure(p) => format!("Pressure {p} MPa on {}", key_text(&l.face.key)),
            };
            ui.label(text);
            if w::button(ui, "Remove load").clicked() {
                drop_load = Some(k);
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::i18n::tr("Face"));
        let current = app.study_face.and_then(|f| faces.iter().find(|(i, _)| *i == f)).map_or_else(|| "Pick a face".to_string(), |(_, t)| t.clone());
        w::combo(ui, "Study face", egui::ComboBox::from_id_salt("study_face").selected_text(current), |ui| {
            for (f, text) in &faces {
                w::selectable_value(ui, &mut app.study_face, Some(*f), text);
            }
        });
        fix = w::add_enabled(ui, picked.is_some(), "Fix face", egui::Button::new("Fix face")).clicked();
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::i18n::tr("Force (N)"));
        for (label, text) in ["Force X", "Force Y", "Force Z"].into_iter().zip(app.study_force.iter_mut()) {
            w::add(ui, label, TextEdit::singleline(text).desired_width(50.0));
        }
        force = w::add_enabled(ui, picked.is_some(), "Add force", egui::Button::new("Add force")).clicked();
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::i18n::tr("Pressure (MPa)"));
        w::add(ui, "Pressure", TextEdit::singleline(&mut app.study_pressure).desired_width(70.0));
        pressure = w::add_enabled(ui, picked.is_some(), "Add pressure", egui::Button::new("Add pressure")).clicked();
    });
    if let Some(k) = drop_fixed {
        buf.fixed.remove(k);
        commit = true;
    }
    if let Some(k) = drop_load {
        buf.loads.remove(k);
        commit = true;
    }
    if let Some(face) = picked {
        if fix {
            buf.fixed.push(face);
            commit = true;
        }
        if force {
            buf.loads.push(Load { face, kind: LoadKind::Force(app.study_force.clone()) });
            commit = true;
        }
        if pressure {
            buf.loads.push(Load { face, kind: LoadKind::Pressure(app.study_pressure.clone()) });
            commit = true;
        }
    }
    if commit {
        buf.custom_if_edited();
        let edited = buf.clone();
        let mut all = app.doc.studies().to_vec();
        // A text field that lost focus without a change must not drop the result.
        if all[index] != edited {
            all[index] = edited;
            app.exec(vec![Command::SetStudies(all)]);
        }
    }
}

/// Starts the selected study on the current rebuild on the analysis worker (`MAX_DOFS` keeps it to seconds); `finish`
/// takes the result.
fn run(app: &mut CadApp, ctx: &egui::Context) {
    let Some(r) = app.model_result().cloned() else { return };
    let (doc, index) = (app.doc.clone(), app.study_sel);
    app.jobs.start(ctx, JobKind::Study, Some(app.doc.revision()), move || JobOutput::Study(doc.run_study(&r, index)));
}

/// Shows a finished study: the status line, a deformation scale so the largest displacement shows as about a tenth
/// of the mesh's size (a power of ten), and the result in 3D.
pub fn finish(app: &mut CadApp, result: Result<StudyReport, String>) {
    app.status = match &result {
        Ok(rep) => format!("Study: max displacement {:.6} mm, max von Mises {:.3} MPa ({:.0} ms)", rep.result.max_displacement().0, rep.result.max_von_mises().0, rep.mesh_ms + rep.solve_ms),
        Err(err) => format!("study: {err}"),
    };
    if let Ok(rep) = &result {
        let d = rep.result.max_displacement().0;
        let (lo, hi) = rep.mesh.nodes.iter().fold((DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        if d > 0.0 {
            app.study_scale = 10f64.powf((0.1 * (hi - lo).length() / d).log10().floor());
        }
    }
    app.fea_study = Some((app.doc.revision(), result));
    app.show_study = true;
    // The scene shows the result (see the stamp in `view3d::show`).
    app.sel_rev += 1;
}

/// Maxima with their places, reactions, counts, the accuracy label and the display controls; the error in red.
fn rows(app: &mut CadApp, ui: &mut egui::Ui) {
    let (lines, label) = match app.study_result() {
        None => {
            ui.small("Studies run on demand; any edit clears the result.");
            return;
        }
        Some(Err(err)) => {
            ui.label(RichText::new(err.as_str()).color(Color32::RED));
            return;
        }
        Some(Ok(rep)) => {
            let r = &rep.result;
            let ((d, dn), (s, sn)) = (r.max_displacement(), r.max_von_mises());
            let at = |p: DVec3| format!("({:.3}, {:.3}, {:.3})", p.x, p.y, p.z);
            let lines = vec![
                format!("Max displacement {d:.6} mm at {}", at(rep.mesh.nodes[dn])),
                format!("Max von Mises {s:.3} MPa at {}", at(rep.mesh.nodes[sn])),
                match (rep.yield_strength, rep.safety_factor()) {
                    (Some(y), Some(f)) => format!("Safety factor {f:.2} (yield strength {y} MPa / max von Mises {s:.3} MPa)"),
                    _ => "Safety factor: give a yield strength".into(),
                },
                format!(
                    "Reaction ({:.3}, {:.3}, {:.3}) N for loads ({:.3}, {:.3}, {:.3}) N, out of balance {:.1e}",
                    r.reaction.x, r.reaction.y, r.reaction.z, r.applied.x, r.applied.y, r.applied.z, r.residual
                ),
                format!("{} nodes, {} elements, {} unknowns; mesh {:.0} ms, solve {:.0} ms", rep.stats.nodes, rep.stats.elements, r.dofs, rep.mesh_ms, rep.solve_ms),
            ];
            (lines, rep.label())
        }
    };
    for l in &lines {
        ui.label(l);
    }
    let resp = ui.small(&label);
    w::tag(ui, &label, resp);
    let changed = ui
        .horizontal_wrapped(|ui| {
            let mut changed = w::checkbox(ui, &mut app.show_study, "Show result").changed();
            changed |= w::selectable_value(ui, &mut app.study_by_displacement, false, "Von Mises").changed();
            changed |= w::selectable_value(ui, &mut app.study_by_displacement, true, "Displacement").changed();
            changed | w::add(ui, "Deformation scale", egui::DragValue::new(&mut app.study_scale).speed(1.0).range(0.0..=1e9)).changed()
        })
        .inner;
    if changed {
        app.sel_rev += 1;
    }
}

/// The colour bar with the largest value at the top, its title and the accuracy label, at the right of the 3D view.
pub fn paint_legend(app: &CadApp, rep: &StudyReport, painter: &egui::Painter, rect: egui::Rect) {
    let (lo, hi) = range(&values(app, rep));
    let origin = rect.right_top() + egui::vec2(-110.0, 40.0);
    let (width, height, strips) = (18.0, 200.0, 32);
    let font = egui::FontId::proportional(12.0);
    let ink = Color32::from_gray(220);
    for k in 0..strips {
        let c = ramp(1.0 - (k as f32 + 0.5) / strips as f32).map(|v| (v * 255.0).round() as u8);
        let top = origin.y + height * k as f32 / strips as f32;
        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(origin.x, top), egui::pos2(origin.x + width, top + height / strips as f32 + 0.5)), 0.0, Color32::from_rgb(c[0], c[1], c[2]));
    }
    let x = origin.x + width + 4.0;
    painter.text(egui::pos2(x, origin.y), egui::Align2::LEFT_TOP, format!("{hi:.4}"), font.clone(), ink);
    painter.text(egui::pos2(x, origin.y + height), egui::Align2::LEFT_BOTTOM, format!("{lo:.4}"), font.clone(), ink);
    let right = rect.right() - 8.0;
    let title = if app.study_by_displacement { "Displacement (mm)" } else { "Von Mises (MPa)" };
    painter.text(egui::pos2(right, origin.y - 18.0), egui::Align2::RIGHT_TOP, title, font.clone(), ink);
    painter.text(egui::pos2(right, origin.y + height + 6.0), egui::Align2::RIGHT_TOP, rep.label(), font.clone(), ink);
    painter.text(egui::pos2(right, origin.y + height + 22.0), egui::Align2::RIGHT_TOP, format!("Deformation x{}", app.study_scale), font, ink);
}

#[cfg(test)]
#[path = "study_panel_tests.rs"]
mod tests;
