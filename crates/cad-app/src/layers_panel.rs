use crate::app::CadApp;
use cad_document::document::{MAX_LINE_WEIGHT, MIN_LINE_WEIGHT};
use cad_document::{Command, Layer, LineType};
use crate::widgets as w;
use eframe::egui::{self, Button};

pub fn layers_section(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.heading("Layers");
    let layers: Vec<Layer> = app.doc.layers().cloned().collect();
    for l in layers {
        ui.horizontal(|ui| {
            if w::selectable(ui, app.layer == l.id, &l.name).clicked() {
                app.layer = l.id;
            }
            let key = |field: u64| (u64::from(l.id) << 8) | field;
            let mut visible = l.visible;
            if w::checkbox(ui, &mut visible, "V").changed() {
                app.exec_merged(vec![Command::SetLayer(Layer { visible, ..l.clone() })], key(1));
            }
            let mut locked = l.locked;
            if w::checkbox(ui, &mut locked, "L").changed() {
                app.exec_merged(vec![Command::SetLayer(Layer { locked, ..l.clone() })], key(2));
            }
            let mut color = l.color;
            let c = ui.color_edit_button_srgb(&mut color);
            if w::tag(ui, "Layer colour", c).changed() {
                app.exec_merged(vec![Command::SetLayer(Layer { color, ..l.clone() })], key(3));
            }
        });
    }
    ui.horizontal(|ui| {
        if w::button(ui, "+ Layer").clicked() {
            let id = app.doc.alloc_layer_id();
            app.exec(vec![Command::AddLayer(Layer::new(id, format!("Layer {id}")))]);
            if app.doc.layer(id).is_some() {
                app.layer = id;
            }
        }
        if w::add_enabled(ui, app.layer != 0, "Delete layer", Button::new("Delete layer")).clicked() {
            app.exec(vec![Command::RemoveLayer(app.layer)]);
        }
    });
    let resp = w::add(ui, "Layer name", egui::TextEdit::singleline(&mut app.layer_name));
    if resp.lost_focus()
        && let Some(cur) = app.doc.layer(app.layer).cloned()
    {
        let name = app.layer_name.trim().to_string();
        if !name.is_empty() && name != cur.name {
            app.exec(vec![Command::SetLayer(Layer { name, ..cur })]);
        }
    }
    if !resp.has_focus() {
        app.layer_name = app.doc.layer(app.layer).map(|l| l.name.clone()).unwrap_or_default();
    }
    if let Some(cur) = app.doc.layer(app.layer).cloned() {
        let key = |field: u64| (u64::from(cur.id) << 8) | field;
        ui.horizontal(|ui| {
            ui.label("Type");
            let mut line_type = cur.line_type;
            w::combo(ui, "Type", egui::ComboBox::from_id_salt("line_type").selected_text(line_type.name()), |ui| {
                for t in LineType::ALL {
                    w::selectable_value(ui, &mut line_type, t, t.name());
                }
            });
            if line_type != cur.line_type {
                app.exec_merged(vec![Command::SetLayer(Layer { line_type, ..cur.clone() })], key(4));
            }
        });
        ui.horizontal(|ui| {
            ui.label("Weight");
            let mut line_weight = cur.line_weight;
            w::add(ui, "Weight", egui::DragValue::new(&mut line_weight).speed(0.01).range(MIN_LINE_WEIGHT..=MAX_LINE_WEIGHT));
            if line_weight != cur.line_weight {
                app.exec_merged(vec![Command::SetLayer(Layer { line_weight, ..cur.clone() })], key(5));
            }
        });
    }
}
