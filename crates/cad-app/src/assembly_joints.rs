//! The Assembly tab's joints and exploded-view sections.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::{Accuracy, Command, ComponentId, ExplodeStep, Frame, Joint, JointId, JointKind, SOLVED};
use cad_geometry::{DAffine3, DVec3};
use eframe::egui::{self, Color32, RichText, TextEdit};
use std::collections::BTreeMap;

const KINDS: [(JointKind, &str); 3] = [(JointKind::Fixed, "Type: Fixed"), (JointKind::Revolute, "Type: Revolute"), (JointKind::Slider, "Type: Slider")];

pub(crate) fn name(app: &CadApp, id: ComponentId) -> String {
    app.doc.component(id).map_or_else(|| format!("component {id}"), |c| c.name.clone())
}

pub fn joints(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.separator();
    ui.heading("Joints");
    add_controls(app, ui);
    list(app, ui);
    editor(app, ui);
    drag(app, ui);
}

/// Parent choice and "Add joint", which makes the selected component a child of that parent. The parent defaults to
/// the first grounded component, else the first component, other than the selected one.
fn add_controls(app: &mut CadApp, ui: &mut egui::Ui) {
    let child = app.component_sel;
    let ids: Vec<ComponentId> = app.doc.components().map(|c| c.id).collect();
    if !ids.contains(&app.joint_parent) || Some(app.joint_parent) == child {
        let grounded = app.doc.components().find(|c| c.grounded && Some(c.id) != child).map(|c| c.id);
        app.joint_parent = grounded.or_else(|| ids.iter().copied().find(|id| Some(*id) != child)).unwrap_or(0);
    }
    let labels: Vec<(ComponentId, String)> = ids.iter().map(|&id| (id, format!("Parent: {}", name(app, id)))).collect();
    let current = labels.iter().find(|(id, _)| *id == app.joint_parent).map_or_else(|| "No parent".to_string(), |(_, t)| t.clone());
    ui.horizontal_wrapped(|ui| {
        w::combo(ui, "Joint parent", egui::ComboBox::from_id_salt("joint_parent").selected_text(current).width(140.0), |ui| {
            for (id, text) in &labels {
                w::selectable_value(ui, &mut app.joint_parent, *id, text);
            }
        });
        let ready = child.is_some_and(|c| c != app.joint_parent) && app.joint_parent != 0;
        if w::add_enabled(ui, ready, "Add joint", egui::Button::new("Add joint")).clicked()
            && let Some(child) = child
        {
            add(app, child);
        }
    });
}

/// A fixed joint with both frames at the component origins (parent axis +Z, child axis -Z: the child sits exactly on
/// the parent). It becomes the selected joint. It closes a loop when the child already has a parent joint (a grounded child is still refused, as before; tick "Closes loop" to close a loop onto it).
fn add(app: &mut CadApp, child: ComponentId) {
    let id = app.doc.alloc_joint_id();
    let j = Joint {
        id,
        kind: JointKind::Fixed,
        parent: app.joint_parent,
        child,
        parent_frame: Frame::from_vectors(DVec3::ZERO, DVec3::Z),
        child_frame: Frame::from_vectors(DVec3::ZERO, -DVec3::Z),
        value: "0".into(),
        min: None,
        max: None,
        closes_loop: app.doc.parent_joint(child).is_some(),
        free: false,
    };
    app.exec(vec![Command::AddJoint(j)]);
    if app.doc.joint(id).is_some() {
        app.joint_sel = Some(id);
        app.joint_edit = None;
    }
}

/// "Joint N" per joint, its kind and components below, and the child's placement error in red.
fn list(app: &mut CadApp, ui: &mut egui::Ui) {
    let error = |child: ComponentId| {
        app.model_result().and_then(|r| r.components.iter().find(|(i, _)| *i == child)).and_then(|(_, r)| r.as_ref().err().cloned())
    };
    let solved = |j: &Joint| app.model_result().and_then(|r| r.joint_values.get(&j.id)).map(|v| j.value_text(*v));
    let rows: Vec<(JointId, String, Option<String>, Option<String>)> = app
        .doc
        .joints()
        .map(|j| {
            let flag = if j.closes_loop { " (closes loop)" } else if j.is_free() { " (free)" } else { "" };
            (j.id, format!("{:?}: {} > {}{flag}", j.kind, name(app, j.parent), name(app, j.child)), error(j.child), solved(j))
        })
        .collect();
    for (id, text, err, solved) in rows {
        if w::selectable(ui, app.joint_sel == Some(id), &format!("Joint {id}")).clicked() {
            app.joint_sel = Some(id);
            app.joint_edit = None;
        }
        ui.small(text);
        if let Some(e) = err {
            ui.small(RichText::new(e).color(Color32::RED));
        }
        if let Some(v) = solved {
            ui.small(format!("Solved: {v}"));
            crate::motion_panel::accuracy(ui, Accuracy::Approximate(SOLVED));
        }
    }
}

/// Fields of the selected joint; text fields commit when they lose focus, the type and "frame from face" at once.
fn editor(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some(id) = app.joint_sel.filter(|id| app.doc.joint(*id).is_some()) else {
        app.joint_edit = None;
        return;
    };
    if app.joint_edit.as_ref().is_none_or(|j| j.id != id) {
        app.joint_edit = app.doc.joint(id).cloned();
    }
    let (mut commit, mut from_face) = (false, None);
    let delete;
    {
        let Some(buffer) = app.joint_edit.as_mut() else { return };
        let current = KINDS.iter().find(|(k, _)| *k == buffer.kind).map_or("", |(_, t)| *t);
        w::combo(ui, "Joint type", egui::ComboBox::from_id_salt("joint_type").selected_text(current), |ui| {
            for (k, text) in KINDS {
                commit |= w::selectable_value(ui, &mut buffer.kind, k, text).clicked();
            }
        });
        // A closing joint has no value of its own; a free joint's value is only the solver's start guess.
        commit |= w::checkbox(ui, &mut buffer.closes_loop, "Closes loop").changed();
        if buffer.kind != JointKind::Fixed && !buffer.closes_loop {
            commit |= w::checkbox(ui, &mut buffer.free, "Free joint").changed();
        }
        let mut field = |ui: &mut egui::Ui, label: &str, text: &mut String| {
            commit |= w::add(ui, label, TextEdit::singleline(text).id_salt(("joint", id, label)).desired_width(70.0)).lost_focus();
        };
        let unit = match (buffer.kind, buffer.closes_loop, buffer.free) {
            (JointKind::Fixed, ..) | (_, true, _) => "Value (unused)",
            (JointKind::Revolute, _, true) => "Start angle",
            (JointKind::Slider, _, true) => "Start distance",
            (JointKind::Revolute, ..) => "Angle",
            (JointKind::Slider, ..) => "Distance",
        };
        ui.horizontal(|ui| {
            ui.label(unit);
            field(ui, "Joint value", &mut buffer.value);
        });
        if buffer.kind != JointKind::Fixed && !buffer.closes_loop {
            // An empty field means no limit.
            let (mut min, mut max) = (buffer.min.clone().unwrap_or_default(), buffer.max.clone().unwrap_or_default());
            ui.horizontal_wrapped(|ui| {
                ui.label("Limits");
                field(ui, "Joint min", &mut min);
                ui.label("to");
                field(ui, "Joint max", &mut max);
            });
            let some = |s: String| (!s.trim().is_empty()).then_some(s);
            (buffer.min, buffer.max) = (some(min), some(max));
        }
        for (side, frame) in [("Parent", &mut buffer.parent_frame), ("Child", &mut buffer.child_frame)] {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{side} origin"));
                for (axis, text) in ["X", "Y", "Z"].into_iter().zip(frame.origin.iter_mut()) {
                    field(ui, &format!("{side} origin {axis}"), text);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{side} axis"));
                for (axis, text) in ["X", "Y", "Z"].into_iter().zip(frame.axis.iter_mut()) {
                    field(ui, &format!("{side} axis {axis}"), text);
                }
                if w::button(ui, &format!("{side} frame from face")).clicked() {
                    from_face = Some(side == "Parent");
                }
            });
            if frame.face.is_some() {
                ui.small(format!("{side} frame follows its face"));
            }
        }
        delete = w::button(ui, "Delete joint").clicked();
    }
    if delete {
        app.joint_edit = None;
        app.joint_sel = None;
        app.exec(vec![Command::RemoveJoint(id)]);
        return;
    }
    if let Some(parent) = from_face {
        match frame_from_face(app, id, parent) {
            Ok(f) => {
                if let Some(b) = app.joint_edit.as_mut() {
                    *(if parent { &mut b.parent_frame } else { &mut b.child_frame }) = f;
                }
                commit = true;
            }
            Err(e) => app.status = e,
        }
    }
    if commit {
        // Typed origin or axis numbers replace a frame taken from a face.
        if from_face.is_none()
            && let (Some(b), Some(s)) = (app.joint_edit.as_mut(), app.doc.joint(id))
        {
            for (f, old) in [(&mut b.parent_frame, &s.parent_frame), (&mut b.child_frame, &s.child_frame)] {
                if f.origin != old.origin || f.axis != old.axis {
                    f.face = None;
                }
            }
        }
        let stored = app.doc.joint(id).cloned();
        if let Some(j) = app.joint_edit.take().filter(|j| Some(j) != stored.as_ref()) {
            app.exec(vec![Command::SetJoint(j)]);
        }
    }
}

/// A joint value being dragged with the editor's slider: shown in 3D (`Document::pose`, display only) until released.
pub struct JointDrag {
    pub joint: JointId,
    /// The document revision it was computed on.
    pub rev: u64,
    /// Radians or mm.
    pub value: f64,
    /// The loops' solved values, where the next solve starts (keeps a linkage on its branch while dragging).
    pub guess: BTreeMap<JointId, f64>,
    pub moves: Result<BTreeMap<ComponentId, DAffine3>, String>,
}

/// "Drag" slider and value of the selected revolute or slider joint (not a closing one). Dragging the slider shows
/// the pose in 3D, loops solved per frame; releasing it, clicking it or typing a value writes the value and the free
/// joints' solved values (as their new start values) in one undo step. A pose that fails is shown in red, not written.
fn drag(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some(j) = app.joint_sel.and_then(|id| app.doc.joint(id)).filter(|j| j.kind != JointKind::Fixed && !j.closes_loop).cloned() else { return };
    if !app.model_current() {
        return;
    }
    let (rev, angle) = (app.doc.revision(), j.kind == JointKind::Revolute);
    let held = app.joint_drag.as_ref().filter(|d| d.joint == j.id && d.rev == rev).map(|d| (d.value, d.guess.clone(), d.moves.as_ref().err().cloned()));
    let Some(result) = app.model_result() else { return };
    // Without a value (an expression error) there is nothing to drag; the joint list shows the error.
    let Ok(v) = held.as_ref().map_or_else(|| app.doc.joint_value(result, &j), |(v, ..)| Ok(*v)) else { return };
    let mut shown = if angle { v.to_degrees() } else { v };
    // shortcut: a fixed slider range, not the joint's limits; a typed value may lie outside it.
    let range = if angle { -180.0..=180.0 } else { -100.0..=100.0 };
    let (slider, typed) = ui
        .horizontal_wrapped(|ui| {
            ui.label("Drag");
            let s = w::add(ui, "Joint drag", egui::Slider::new(&mut shown, range).show_value(false));
            let t = w::add(ui, "Joint drag value", egui::DragValue::new(&mut shown).speed(if angle { 1.0 } else { 0.5 }));
            ui.label(if angle { "deg" } else { "mm" });
            (s, t)
        })
        .inner;
    if let Some((_, _, Some(err))) = &held {
        ui.small(RichText::new(err.as_str()).color(Color32::RED));
    }
    let changed = slider.changed() || typed.changed();
    if !changed && !slider.drag_stopped() {
        return;
    }
    let value = if angle { shown.to_radians() } else { shown };
    let mut guess = held.map_or_else(|| result.joint_values.clone(), |(_, g, _)| g);
    let moves = app.doc.pose(result, j.id, value, &mut guess);
    // The scene shows the new pose (see the stamp in `view3d::show`).
    app.sel_rev += 1;
    if (slider.drag_stopped() || !slider.dragged()) && moves.is_ok() {
        let mut cmds = vec![Command::SetJoint(Joint { value: j.value_text(value), ..j.clone() })];
        for f in app.doc.joints().filter(|f| f.is_free() && f.id != j.id) {
            if let Some(g) = guess.get(&f.id) {
                cmds.push(Command::SetJoint(Joint { value: f.value_text(*g), ..f.clone() }));
            }
        }
        app.joint_drag = None;
        app.joint_edit = None;
        app.exec(cmds);
    } else {
        app.joint_drag = Some(JointDrag { joint: j.id, rev, value, guess, moves });
    }
}

/// The frame of the face last clicked in 3D, which must lie on the joint's parent (or child) component.
fn frame_from_face(app: &CadApp, joint: JointId, parent: bool) -> Result<Frame, String> {
    let j = app.doc.joint(joint).ok_or("no such joint")?;
    let comp = if parent { j.parent } else { j.child };
    let face = app.component_face.filter(|(c, _)| *c == comp).map(|(_, f)| f).ok_or_else(|| format!("click a face of {} in the 3D view first", name(app, comp)))?;
    let body = app.doc.component(comp).and_then(|c| app.model_result()?.body(c.body)).ok_or("the component's body is not built yet")?;
    Frame::from_face(body, face as usize).ok_or_else(|| "pick a flat or cylindrical face".to_string())
}

/// Toggle, slider and factor of the exploded view, trail lines, and the steps (each moves one component).
pub fn explode(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.separator();
    ui.heading("Explode steps");
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        changed |= w::checkbox(ui, &mut app.explode_on, "Exploded view").changed();
        changed |= w::checkbox(ui, &mut app.explode_trails, "Trail lines").changed();
    });
    ui.horizontal_wrapped(|ui| {
        changed |= w::add(ui, "Explode slider", egui::Slider::new(&mut app.explode, 0.0..=1.0).show_value(false)).changed();
        changed |= w::add(ui, "Explode factor", egui::DragValue::new(&mut app.explode).speed(0.01).range(0.0..=1.0)).changed();
    });
    if changed {
        // The scene is rebuilt when this changes (see the stamp in `view3d::show`).
        app.sel_rev += 1;
    }
    if w::add_enabled(ui, app.component_sel.is_some(), "Add explode step", egui::Button::new("Add explode step")).clicked()
        && let Some(c) = app.component_sel
    {
        let mut all = app.doc.explode_steps().to_vec();
        all.push(ExplodeStep { component: c, direction: ["0".into(), "0".into(), "1".into()], distance: "50 mm".into() });
        app.explode_edit = None;
        app.exec(vec![Command::SetExplode(all)]);
    }
    steps(app, ui);
}

/// One row per step: its component, direction, distance and "Delete explode N"; an expression error in red below it.
fn steps(app: &mut CadApp, ui: &mut egui::Ui) {
    if app.explode_edit.as_ref().is_none_or(|b| b.len() != app.doc.explode_steps().len()) {
        app.explode_edit = Some(app.doc.explode_steps().to_vec());
    }
    let names: Vec<String> = app.doc.explode_steps().iter().map(|s| name(app, s.component)).collect();
    let errors: Vec<Option<String>> = app.doc.explode_steps().iter().map(|s| app.doc.explode_vector(s).err()).collect();
    let (mut commit, mut delete) = (false, None);
    {
        let Some(buffer) = app.explode_edit.as_mut() else { return };
        for (i, s) in buffer.iter_mut().enumerate() {
            let n = i + 1;
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Step {n}: {}", names.get(i).map_or("", String::as_str)));
                let texts = s.direction.iter_mut().zip(["X", "Y", "Z"]).chain(std::iter::once((&mut s.distance, "distance")));
                for (text, part) in texts {
                    let label = format!("Explode {n} {part}");
                    commit |= w::add(ui, &label, TextEdit::singleline(text).id_salt(("explode", n, part)).desired_width(50.0)).lost_focus();
                }
                if w::button(ui, &format!("Delete explode {n}")).clicked() {
                    delete = Some(i);
                }
            });
            if let Some(Some(e)) = errors.get(i) {
                ui.small(RichText::new(e.as_str()).color(Color32::RED));
            }
        }
        if let Some(i) = delete {
            buffer.remove(i);
            commit = true;
        }
    }
    if commit {
        let edited = app.explode_edit.take().unwrap_or_default();
        if edited.as_slice() != app.doc.explode_steps() {
            app.exec(vec![Command::SetExplode(edited)]);
        }
    }
}
