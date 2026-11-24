//! The Floor and Roof groups of the properties panel: a floor slab or a roof from the selected room's area or closed outline.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::{Command, EntityId};
use cad_geometry::{DVec2, Roof, RoofKind, Shape, Slab};
use eframe::egui;

/// What a floor or roof can be made from: one selected room with an enclosed area (last rebuild), or one closed curve,
/// on a level and not construction. Its id, whether it is a room, and the outline.
fn source(app: &CadApp) -> Option<(EntityId, bool, Vec<DVec2>)> {
    let id = (app.selection.len() == 1).then(|| app.selection.iter().next().copied()).flatten()?;
    let x = app.doc.get(id).filter(|x| !x.construction && app.doc.is_level(x.sketch))?;
    let is_room = matches!(x.shape, Shape::Room(_));
    let outline = if is_room {
        app.model.as_ref().and_then(|(_, m)| m.rooms.iter().find(|g| g.0 == id)).map(|g| g.1.outline.clone())
    } else {
        crate::hatch_panel::hatch_boundary(&x.shape)
    }?;
    Some((id, is_room, outline))
}

/// Adds `shape` in the sketch and layer of `id`; a curve (not a room) is replaced by it in the same undo step.
fn make(app: &mut CadApp, id: EntityId, is_room: bool, shape: Shape) {
    let Some((layer, sketch)) = app.doc.get(id).map(|x| (x.layer, x.sketch)) else { return };
    let mut x = app.new_entity(shape, layer);
    x.sketch = sketch;
    let mut cmds = vec![Command::Add(x)];
    if !is_room {
        cmds.insert(0, Command::Remove(id));
    }
    app.exec(cmds);
}

/// Shown for one selected room with an enclosed area, or one closed curve, on a level. "Make floor" adds a slab with
/// that outline in the same sketch and layer; a curve is replaced by the floor (same undo step), a room stays.
pub fn floor_group(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some((id, is_room, outline)) = source(app) else { return };
    ui.group(|ui| {
        ui.label(crate::i18n::tr("Floor"));
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Thickness"));
            w::add(ui, "Floor thickness", egui::DragValue::new(&mut app.floor_thickness).speed(1.0).range(1.0..=f64::MAX));
        });
        if w::button(ui, "Make floor").clicked() {
            make(app, id, is_room, Shape::Slab(Slab { outline, thickness: app.floor_thickness }));
        }
    });
}

/// Shown like the Floor group. "Make roof" adds a roof with the Roof settings over that outline, on top of the level;
/// a curve is replaced by the roof, a room stays. An invalid roof (gable or hip on a concave outline) is refused with
/// the reason in the status bar.
pub fn roof_group(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some((id, is_room, outline)) = source(app) else { return };
    ui.group(|ui| {
        ui.label(crate::i18n::tr("Roof"));
        ui.horizontal(|ui| {
            for k in [RoofKind::Flat, RoofKind::Shed, RoofKind::Gable, RoofKind::Hip] {
                w::selectable_value(ui, &mut app.roof.kind, k, &format!("{k:?}"));
            }
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Pitch"));
            w::add(ui, "Roof pitch", egui::DragValue::new(&mut app.roof.pitch).speed(1.0).range(1.0..=75.0).suffix("°"));
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Thickness"));
            w::add(ui, "Roof thickness", egui::DragValue::new(&mut app.roof.thickness).speed(1.0).range(1.0..=f64::MAX));
            ui.label(crate::i18n::tr("Overhang"));
            w::add(ui, "Roof overhang", egui::DragValue::new(&mut app.roof.overhang).speed(1.0).range(0.0..=f64::MAX));
        });
        if w::button(ui, "Make roof").clicked() {
            make(app, id, is_room, Shape::Roof(Roof { outline, ..app.roof.clone() }));
        }
    });
}
