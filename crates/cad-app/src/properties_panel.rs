use crate::app::CadApp;
use cad_document::{Command, LayerId, Settings};
use cad_geometry::xform::{mirror_across, rotation_about, scale_about};
use cad_geometry::{DVec2, DVec3, OpeningKind, RoofKind, Shape, WallAlign};
use crate::widgets as w;
use eframe::egui;
use std::collections::BTreeSet;

pub fn properties(app: &mut CadApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    if app.selection.is_empty() {
        app.inspector = None;
        return;
    }
    layer_combo(app, ui);
    construction_checkbox(app, ui);
    crate::groups::group_buttons(app, ui);
    transform_group(app, ui);
    crate::arrange::arrange_group(app, ui);
    array_group(app, ui);
    inspector(app, ui, ctx);
    crate::dim_link::show(app, ui);
    crate::groups::metadata(app, ui);
    crate::hatch_panel::hatch_group(app, ui);
    crate::floor_panel::floor_group(app, ui);
    crate::floor_panel::roof_group(app, ui);
}

/// A mixed selection shows unchecked; a click sets every selected entity.
fn construction_checkbox(app: &mut CadApp, ui: &mut egui::Ui) {
    let all = app.selection.iter().filter_map(|id| app.doc.get(*id)).all(|e| e.construction);
    let mut on = all;
    if w::checkbox(ui, &mut on, "Construction").clicked() {
        let cmds = app.selection.iter().map(|id| Command::SetConstruction { id: *id, on }).collect();
        app.exec(cmds);
    }
}

fn layer_combo(app: &mut CadApp, ui: &mut egui::Ui) {
    let used: BTreeSet<LayerId> = app.selection.iter().filter_map(|id| app.doc.get(*id)).map(|e| e.layer).collect();
    let common = (used.len() == 1).then(|| used.iter().next().copied()).flatten();
    let text = common.and_then(|id| app.doc.layer(id)).map_or("(mixed)".to_string(), |l| l.name.clone());
    let mut chosen = None;
    w::combo(ui, "Layer", egui::ComboBox::from_label("Layer").selected_text(text), |ui| {
        for l in app.doc.layers() {
            if w::selectable(ui, common == Some(l.id), &l.name).clicked() {
                chosen = Some(l.id);
            }
        }
    });
    if let Some(layer) = chosen {
        let cmds = app.selection.iter().map(|id| Command::SetEntityLayer { id: *id, layer }).collect();
        app.exec(cmds);
    }
}

fn transform_group(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some(pivot) = app.selection_center() else { return };
    ui.group(|ui| {
        ui.label("Transform");
        ui.horizontal(|ui| {
            w::add(ui, "Rotate angle", egui::DragValue::new(&mut app.rot_deg).speed(0.5));
            if w::button(ui, "Rotate").clicked() {
                app.transform_selection(rotation_about(pivot, app.rot_deg.to_radians()));
            }
        });
        ui.horizontal(|ui| {
            w::add(ui, "Scale factor", egui::DragValue::new(&mut app.scale_factor).speed(0.01));
            if w::button(ui, "Scale").clicked() {
                app.transform_selection(scale_about(pivot, app.scale_factor));
            }
        });
        ui.horizontal(|ui| {
            if w::button(ui, "Mirror H").clicked() {
                app.transform_selection(mirror_across(pivot, DVec2::Y));
            }
            if w::button(ui, "Mirror V").clicked() {
                app.transform_selection(mirror_across(pivot, DVec2::X));
            }
            if w::button(ui, "Duplicate").clicked() {
                app.duplicate_selection();
            }
        });
    });
}

fn array_group(app: &mut CadApp, ui: &mut egui::Ui) {
    let header = egui::CollapsingHeader::new("Array").show(ui, |ui| {
        let mut p = app.array;
        ui.horizontal(|ui| {
            ui.label("nx");
            w::add(ui, "nx", egui::DragValue::new(&mut p.nx).range(1..=100));
            ui.label("ny");
            w::add(ui, "ny", egui::DragValue::new(&mut p.ny).range(1..=100));
        });
        ui.horizontal(|ui| {
            ui.label("dx");
            w::add(ui, "dx", egui::DragValue::new(&mut p.dx).speed(0.1));
            ui.label("dy");
            w::add(ui, "dy", egui::DragValue::new(&mut p.dy).speed(0.1));
        });
        if w::button(ui, "Rectangular").clicked() {
            app.array_rect(p.nx, p.ny, p.dx, p.dy);
        }
        ui.horizontal(|ui| {
            ui.label("count");
            w::add(ui, "count", egui::DragValue::new(&mut p.count).range(2..=360));
            ui.label("angle");
            w::add(ui, "angle", egui::DragValue::new(&mut p.angle_deg).speed(0.5));
        });
        ui.horizontal(|ui| {
            ui.label("cx");
            w::add(ui, "cx", egui::DragValue::new(&mut p.center.x).speed(0.1));
            ui.label("cy");
            w::add(ui, "cy", egui::DragValue::new(&mut p.center.y).speed(0.1));
        });
        if w::button(ui, "Polar").clicked() {
            app.array_polar(p.count, p.angle_deg, p.center);
        }
        app.array = p;
    });
    w::tag(ui, "Array", header.header_response);
}

fn inspector(app: &mut CadApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let single = (app.selection.len() == 1).then(|| app.selection.iter().next().copied()).flatten();
    let Some(id) = single.filter(|id| app.doc.get(*id).is_some()) else {
        app.inspector = None;
        return;
    };
    if app.inspector.as_ref().is_none_or(|(i, _)| *i != id) {
        app.inspector = app.doc.get(id).map(|e| (id, e.shape.clone()));
    }
    let settings = *app.doc.settings();
    let Some((_, buffer)) = app.inspector.as_mut() else { return };
    ui.label(buffer.kind());
    edit_shape(ui, buffer, settings);

    let stored = app.doc.get(id).map(|e| e.shape.clone());
    let Some((_, buffer)) = app.inspector.clone() else { return };
    if stored.as_ref() == Some(&buffer) {
        return;
    }
    let editing = ctx.egui_is_using_pointer() || ctx.memory(|m| m.focused().is_some());
    if editing {
        ctx.request_repaint();
    } else {
        app.exec(vec![Command::SetShape { id, shape: buffer }]);
        app.inspector = None;
    }
}

fn num(ui: &mut egui::Ui, label: &str, v: &mut f64) {
    ui.horizontal(|ui| {
        ui.label(label);
        w::add(ui, label, egui::DragValue::new(v).speed(0.1));
    });
}

/// Edits a radian value in degrees; the buffer only changes when the user edits it.
fn degrees(ui: &mut egui::Ui, label: &str, v: &mut f64) {
    let shown = v.to_degrees();
    let mut d = shown;
    num(ui, label, &mut d);
    if d != shown {
        *v = d.to_radians();
    }
}

fn vec2(ui: &mut egui::Ui, label: &str, v: &mut DVec2) {
    num(ui, &format!("{label} x"), &mut v.x);
    num(ui, &format!("{label} y"), &mut v.y);
}

fn vec3(ui: &mut egui::Ui, label: &str, v: &mut DVec3) {
    num(ui, &format!("{label} x"), &mut v.x);
    num(ui, &format!("{label} y"), &mut v.y);
    num(ui, &format!("{label} z"), &mut v.z);
}

fn edit_shape(ui: &mut egui::Ui, shape: &mut Shape, settings: Settings) {
    if let Some(class) = cad_arch::ifc_class(shape) {
        ui.label(format!("IFC class: {class}"));
    }
    match shape {
        Shape::Line { a, b } => {
            vec2(ui, "a", a);
            vec2(ui, "b", b);
        }
        Shape::Circle { center, radius } => {
            vec2(ui, "center", center);
            num(ui, "radius", radius);
        }
        Shape::Rect { min, max } => {
            vec2(ui, "min", min);
            vec2(ui, "max", max);
        }
        Shape::Box { center, size } => {
            vec3(ui, "center", center);
            vec3(ui, "size", size);
        }
        Shape::Cylinder { center, radius, height } => {
            vec3(ui, "center", center);
            num(ui, "radius", radius);
            num(ui, "height", height);
        }
        Shape::Sphere { center, radius } => {
            vec3(ui, "center", center);
            num(ui, "radius", radius);
        }
        Shape::Arc { center, radius, start, sweep } => {
            vec2(ui, "center", center);
            num(ui, "radius", radius);
            degrees(ui, "start", start);
            degrees(ui, "sweep", sweep);
        }
        Shape::Polyline { points, closed } => {
            ui.label(format!("{} points", points.len()));
            w::checkbox(ui, closed, "closed");
        }
        Shape::Ellipse { center, rx, ry, angle } => {
            vec2(ui, "center", center);
            num(ui, "rx", rx);
            num(ui, "ry", ry);
            degrees(ui, "angle", angle);
        }
        Shape::Spline { points, closed } => {
            ui.label(format!("{} points", points.len()));
            w::checkbox(ui, closed, "closed");
        }
        Shape::Text { pos, height, angle, text } => {
            vec2(ui, "pos", pos);
            num(ui, "height", height);
            degrees(ui, "angle", angle);
            w::add(ui, "text", egui::TextEdit::singleline(text));
        }
        Shape::Dimension { a, b, offset, text_height } => {
            vec2(ui, "a", a);
            vec2(ui, "b", b);
            num(ui, "offset", offset);
            num(ui, "text height", text_height);
        }
        Shape::Hatch { boundary, angle, spacing, pattern } => {
            ui.label(format!("{} boundary points", boundary.len()));
            degrees(ui, "angle", angle);
            num(ui, "spacing", spacing);
            crate::hatch_panel::pattern_combo(ui, "hatch_pattern_inspector", pattern);
        }
        Shape::Leader { points, text, text_height } => {
            ui.label(format!("{} points", points.len()));
            w::add(ui, "text", egui::TextEdit::singleline(text));
            num(ui, "text height", text_height);
        }
        Shape::RadialDim { center, radius, angle, text_height, diameter } => {
            vec2(ui, "center", center);
            num(ui, "radius", radius);
            degrees(ui, "angle", angle);
            num(ui, "text height", text_height);
            w::checkbox(ui, diameter, "diameter");
        }
        Shape::AngularDim { center, start, sweep, radius, text_height } => {
            vec2(ui, "center", center);
            degrees(ui, "start", start);
            degrees(ui, "sweep", sweep);
            num(ui, "radius", radius);
            num(ui, "text height", text_height);
        }
        Shape::Wall(wall) => {
            vec2(ui, "a", &mut wall.a);
            vec2(ui, "b", &mut wall.b);
            num(ui, "thickness", &mut wall.thickness);
            num(ui, "height", &mut wall.height);
            let align = wall.align;
            w::combo(ui, "align", egui::ComboBox::from_id_salt("wall_align_inspector").selected_text(format!("{align:?}")), |ui| {
                for a in [WallAlign::Center, WallAlign::Left, WallAlign::Right] {
                    w::selectable_value(ui, &mut wall.align, a, &format!("{a:?}"));
                }
            });
        }
        Shape::Opening(o) => {
            ui.label(format!("in wall {}", o.host));
            num(ui, "at", &mut o.at);
            num(ui, "width", &mut o.width);
            num(ui, "height", &mut o.height);
            num(ui, "sill", &mut o.sill);
            let kind = o.kind;
            w::combo(ui, "kind", egui::ComboBox::from_id_salt("opening_kind_inspector").selected_text(format!("{kind:?}")), |ui| {
                for k in [OpeningKind::Door, OpeningKind::Window] {
                    w::selectable_value(ui, &mut o.kind, k, &format!("{k:?}"));
                }
            });
            if o.kind == OpeningKind::Door {
                w::checkbox(ui, &mut o.hinge_b, "hinge at b");
                w::checkbox(ui, &mut o.swing_left, "swings left");
            }
        }
        Shape::Room(r) => {
            vec2(ui, "seed", &mut r.seed);
            w::add(ui, "name", egui::TextEdit::singleline(&mut r.name));
            w::add(ui, "number", egui::TextEdit::singleline(&mut r.number));
        }
        Shape::Slab(s) => {
            ui.label(format!("{} outline points", s.outline.len()));
            num(ui, "thickness", &mut s.thickness);
        }
        Shape::Roof(r) => {
            ui.label(format!("{} outline points", r.outline.len()));
            let kind = r.kind;
            w::combo(ui, "kind", egui::ComboBox::from_id_salt("roof_kind_inspector").selected_text(format!("{kind:?}")), |ui| {
                for k in [RoofKind::Flat, RoofKind::Shed, RoofKind::Gable, RoofKind::Hip] {
                    w::selectable_value(ui, &mut r.kind, k, &format!("{k:?}"));
                }
            });
            num(ui, "pitch", &mut r.pitch);
            num(ui, "thickness", &mut r.thickness);
            num(ui, "overhang", &mut r.overhang);
        }
        Shape::Stair(s) => {
            vec2(ui, "a", &mut s.a);
            vec2(ui, "b", &mut s.b);
            num(ui, "width", &mut s.width);
            num(ui, "height", &mut s.height);
            w::add(ui, "risers", egui::DragValue::new(&mut s.risers).range(2..=500));
            ui.label(format!("tread {}, riser {}", settings.format_length(s.tread()), settings.format_length(s.riser())));
        }
    }
}
