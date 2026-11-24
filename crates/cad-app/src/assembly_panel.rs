//! The Assembly tab: components (placed instances of bodies) and the bill of materials.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::{Body, Command, Component, ComponentId, mm};
use cad_geometry::DVec3;
use eframe::egui::{self, Color32, RichText, TextEdit};
use std::sync::Arc;

/// Makes `id` the selected component (None clears it); selecting a component clears the entity, face and edge selections.
pub fn select(app: &mut CadApp, id: Option<ComponentId>) {
    if id.is_some() {
        app.set_selection(Vec::new());
        app.face_sel = None;
        app.edge_sel = None;
    }
    if app.component_sel != id {
        app.component_sel = id;
        app.component_edit = None;
        app.sel_rev += 1;
    }
}

/// The component and body face whose (exploded) solid the ray hits first, when no shown body is nearer.
pub fn pick(app: &mut CadApp, origin: DVec3, dir: DVec3) -> Option<(ComponentId, u32)> {
    let offsets = app.doc.explode_offsets(app.explode_factor());
    let placed: Vec<(u64, Arc<Body>)> = app.placed_components().map(|(id, b)| (id, b.clone())).collect();
    let mut best: Option<(ComponentId, usize, f64)> = None;
    for (id, body) in placed {
        // The solid is drawn moved by its offset: move the ray back instead.
        let shift = offsets.get(&id).copied().unwrap_or(DVec3::ZERO);
        if let Some((_, tri, t)) = crate::face_pick::nearest(app, &[(id, body)], origin - shift, dir)
            && best.is_none_or(|(.., b)| t < b)
        {
            best = Some((id, tri, t));
        }
    }
    let (id, tri, t) = best?;
    let nearer = crate::face_pick::ray_body(app, origin, dir).is_some_and(|(.., b)| b <= t);
    let face = *app.model_result()?.component(id)?.tri_face.get(tri)?;
    (!nearer).then_some((id, face))
}

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.heading(crate::i18n::tr("Components"));
    add_controls(app, ui);
    list(app, ui);
    editor(app, ui);
    crate::assembly_joints::joints(app, ui);
    crate::motion_panel::show(app, ui);
    crate::assembly_joints::explode(app, ui);
    crate::assembly_check::show(app, ui);
    crate::mass_panel::assembly(app, ui);
    ui.separator();
    ui.heading(crate::i18n::tr("Bill of materials"));
    bom(app, ui);
}

/// "Body 2: Shaft 2" for every body of the latest rebuild.
pub(crate) fn body_labels(app: &CadApp) -> Vec<(u64, String)> {
    app.body_ids().into_iter().map(|id| (id, format!("Body {id}: {}", app.doc.feature(id).map_or("", |f| f.name.as_str())))).collect()
}

/// Body choice and "Add component". The choice defaults to the body of the selected face, else the last body.
fn add_controls(app: &mut CadApp, ui: &mut egui::Ui) {
    let bodies = app.body_ids();
    if !bodies.contains(&app.component_body) {
        app.component_body = app.face_sel.map(|(b, _)| b).filter(|b| bodies.contains(b)).or(bodies.last().copied()).unwrap_or(0);
    }
    let labels = body_labels(app);
    let current = labels.iter().find(|(id, _)| *id == app.component_body).map_or_else(|| "No body".to_string(), |(_, t)| t.clone());
    ui.horizontal_wrapped(|ui| {
        w::combo(ui, "Component body", egui::ComboBox::from_id_salt("component_body").selected_text(current).width(140.0), |ui| {
            for (id, text) in &labels {
                w::selectable_value(ui, &mut app.component_body, *id, text);
            }
        });
        if w::add_enabled(ui, !labels.is_empty(), "Add component", egui::Button::new("Add component")).clicked() {
            add(app);
        }
    });
}

/// A component of `app.component_body`, named after its feature. When the body already has n components the new one
/// sits n x (body width along X + 10 mm) along X and copies the metadata of the last of them. It becomes the selected component.
fn add(app: &mut CadApp) {
    let body = app.component_body;
    let Some(feature) = app.doc.feature(body).map(|f| f.name.clone()) else { return };
    let (n, meta) = {
        let siblings: Vec<&Component> = app.doc.components().filter(|c| c.body == body).collect();
        (siblings.len(), siblings.last().map(|c| c.meta.clone()).unwrap_or_default())
    };
    let width = app.model_result().and_then(|r| r.body(body)).map_or(0.0, |b| {
        let (lo, hi) = b.bbox();
        hi.x - lo.x
    });
    let id = app.doc.alloc_component_id();
    let zero = || "0".to_string();
    let c = Component { id, name: format!("{feature} #{}", n + 1), body, at: [mm(n as f64 * (width + 10.0)), zero(), zero()], angles: [zero(), zero(), zero()], meta, grounded: false };
    app.exec(vec![Command::AddComponent(c)]);
    if app.doc.component(id).is_some() {
        select(app, Some(id));
    }
}

/// One selectable line per component; a placement error in red below it.
fn list(app: &mut CadApp, ui: &mut egui::Ui) {
    let error = |id: ComponentId| {
        app.model_result().and_then(|r| r.components.iter().find(|(i, _)| *i == id)).and_then(|(_, r)| r.as_ref().err().cloned())
    };
    let rows: Vec<(ComponentId, String, Option<String>)> = app.doc.components().map(|c| (c.id, c.name.clone(), error(c.id))).collect();
    for (id, name, err) in rows {
        if w::selectable(ui, app.component_sel == Some(id), &name).clicked() {
            select(app, Some(id));
        }
        if let Some(e) = err {
            ui.small(RichText::new(e).color(Color32::RED));
        }
    }
}

/// Fields of the selected component; text fields commit when they lose focus, like the feature editor.
fn editor(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some(id) = app.component_sel.filter(|id| app.doc.component(*id).is_some()) else {
        app.component_edit = None;
        return;
    };
    if app.component_edit.as_ref().is_none_or(|c| c.id != id) {
        app.component_edit = app.doc.component(id).cloned();
    }
    let jointed = app.doc.parent_joint(id).map(|j| j.id);
    let Some(buffer) = app.component_edit.as_mut() else { return };
    ui.separator();
    ui.label(crate::i18n::tr("Selected component"));
    if let Some(j) = jointed {
        ui.small(format!("Placed by joint {j}: position and rotation are ignored"));
    }
    let mut commit = false;
    let mut field = |ui: &mut egui::Ui, label: &str, text: &mut String| {
        ui.horizontal(|ui| {
            ui.label(label);
            commit |= w::add(ui, label, TextEdit::singleline(text).id_salt(("component", id, label)).desired_width(110.0)).lost_focus();
        });
    };
    field(ui, "Component name", &mut buffer.name);
    for (label, text) in ["Component X", "Component Y", "Component Z"].into_iter().zip(buffer.at.iter_mut()) {
        field(ui, label, text);
    }
    for (label, text) in ["Rotation X", "Rotation Y", "Rotation Z"].into_iter().zip(buffer.angles.iter_mut()) {
        field(ui, label, text);
    }
    for (label, key) in [("Material", "material"), ("Part number", "part_number")] {
        let mut text = buffer.meta.get(key).cloned().unwrap_or_default();
        field(ui, label, &mut text);
        if text.trim().is_empty() {
            buffer.meta.remove(key);
        } else {
            buffer.meta.insert(key.to_string(), text);
        }
    }
    commit |= w::checkbox(ui, &mut buffer.grounded, "Grounded").changed();
    // `field` and `buffer` are not used below, so `app` is free again.
    if w::button(ui, "Delete component").clicked() {
        app.component_edit = None;
        app.joint_edit = None;
        app.explode_edit = None;
        let mut cmds: Vec<Command> = app.doc.joints().filter(|j| j.parent == id || j.child == id).map(|j| Command::RemoveJoint(j.id)).collect();
        if app.doc.explode_steps().iter().any(|s| s.component == id) {
            cmds.push(Command::SetExplode(app.doc.explode_steps().iter().filter(|s| s.component != id).cloned().collect()));
        }
        cmds.push(Command::RemoveComponent(id));
        app.exec(cmds);
        return;
    }
    if commit {
        let stored = app.doc.component(id).cloned();
        let edited = app.component_edit.take();
        if let Some(c) = edited.filter(|c| Some(c) != stored.as_ref()) {
            app.exec(vec![Command::SetComponent(c)]);
        }
    }
}

/// The rows of `Document::bom`: "item. name ×quantity", then designation, material, part number and mass.
fn bom(app: &CadApp, ui: &mut egui::Ui) {
    let Some(r) = app.model_result() else { return };
    let rows = app.doc.bom(r);
    if rows.is_empty() {
        ui.label(crate::i18n::tr("No components"));
        return;
    }
    for row in rows {
        ui.label(format!("{}. {} \u{d7}{}", row.item, row.name, row.quantity));
        let mut details: Vec<String> = [row.designation, row.material, row.part_number].into_iter().filter(|s| !s.is_empty()).collect();
        if let Some(m) = row.mass {
            details.push(format!("{m:.1} g"));
        }
        if !details.is_empty() {
            ui.small(details.join(", "));
        }
    }
    ui.small("CSV: File > Export BOM CSV...");
}
