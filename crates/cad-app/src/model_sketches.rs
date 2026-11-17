use crate::app::{CadApp, PickFace, ViewMode};
use cad_document::{BasePlane, Command, PlaneDef, Sketch, SketchId};
use crate::widgets as w;
use eframe::egui::{self, Color32, RichText, TextEdit};

fn describe(plane: &PlaneDef) -> String {
    match plane {
        PlaneDef::Principal { base, offset } => format!("{base:?} + {offset}"),
        PlaneDef::Fixed { .. } => "face plane".to_string(),
        PlaneDef::Face { body, .. } => format!("face of body {body}"),
    }
}

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.separator();
    ui.heading("Sketches");
    let rows: Vec<(SketchId, String, String, Option<String>)> = app
        .doc
        .sketches()
        .map(|s| (s.id, s.name.clone(), describe(&s.plane), app.sketch_plane(s.id).err()))
        .collect();
    for (id, name, text, error) in rows {
        ui.horizontal(|ui| {
            if w::selectable(ui, app.sketch == id, &name).clicked() {
                app.set_sketch(id);
            }
            match error {
                Some(e) => ui.small(RichText::new(format!("plane error: {e}")).color(Color32::RED)),
                None => ui.small(text),
            };
            if w::add_enabled(ui, id != 0, "x", egui::Button::new("x")).clicked() {
                app.exec(vec![Command::RemoveSketch(id)]);
            }
        });
    }
    ui.horizontal(|ui| {
        let base = app.new_sketch_base;
        w::combo(ui, "Base plane", egui::ComboBox::from_id_salt("sketch_base").selected_text(format!("{base:?}")).width(50.0), |ui| {
            for b in [BasePlane::XY, BasePlane::XZ, BasePlane::YZ] {
                w::selectable_value(ui, &mut app.new_sketch_base, b, &format!("{b:?}"));
            }
        });
        w::add(ui, "Sketch offset", TextEdit::singleline(&mut app.new_sketch_offset).desired_width(60.0));
    });
    if w::button(ui, "Add sketch").clicked() {
        let id = app.doc.alloc_sketch_id();
        let plane = PlaneDef::Principal { base: app.new_sketch_base, offset: app.new_sketch_offset.clone() };
        app.exec(vec![Command::AddSketch(Sketch { id, name: format!("Sketch {id}"), plane })]);
        if app.doc.sketch(id).is_some() {
            app.set_sketch(id);
        }
    }
    if w::button(ui, "Sketch on face").clicked() {
        app.pick_face = PickFace::Sketch;
        app.pick_edges = None;
        app.view = ViewMode::ThreeD;
        app.status = "Click a planar face of a body".into();
    }
}
