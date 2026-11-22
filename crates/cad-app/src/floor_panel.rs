//! The Floor group of the properties panel: a floor slab from the selected room's area or closed outline.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::Command;
use cad_geometry::{Shape, Slab};
use eframe::egui;

/// Shown for one selected room with an enclosed area, or one closed curve, on a level. "Make floor" adds a slab with
/// that outline in the same sketch and layer; a curve is replaced by the floor (same undo step), a room stays.
pub fn floor_group(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some(id) = (app.selection.len() == 1).then(|| app.selection.iter().next().copied()).flatten() else { return };
    let Some(x) = app.doc.get(id).filter(|x| !x.construction && app.doc.is_level(x.sketch)) else { return };
    let (layer, sketch, is_room) = (x.layer, x.sketch, matches!(x.shape, Shape::Room(_)));
    let outline = if is_room {
        app.model.as_ref().and_then(|(_, m)| m.rooms.iter().find(|g| g.0 == id)).map(|g| g.1.outline.clone())
    } else {
        crate::hatch_panel::hatch_boundary(&x.shape)
    };
    let Some(outline) = outline else { return };
    ui.group(|ui| {
        ui.label("Floor");
        ui.horizontal(|ui| {
            ui.label("Thickness");
            w::add(ui, "Floor thickness", egui::DragValue::new(&mut app.floor_thickness).speed(1.0).range(1.0..=f64::MAX));
        });
        if w::button(ui, "Make floor").clicked() {
            let mut slab = app.new_entity(Shape::Slab(Slab { outline, thickness: app.floor_thickness }), layer);
            slab.sketch = sketch;
            let mut cmds = vec![Command::Add(slab)];
            if !is_room {
                cmds.insert(0, Command::Remove(id));
            }
            app.exec(cmds);
        }
    });
}
