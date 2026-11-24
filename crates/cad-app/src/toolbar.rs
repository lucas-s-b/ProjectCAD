use crate::app::{CadApp, Tool, ViewMode};
use cad_geometry::{DVec3, Shape, WallAlign};
use crate::widgets as w;
use eframe::egui;

pub fn toolbar(app: &mut CadApp, ui: &mut egui::Ui) {
    egui::Panel::left("tools").resizable(false).exact_size(110.0).show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                w::selectable_value(ui, &mut app.view, ViewMode::TwoD, "2D");
                w::selectable_value(ui, &mut app.view, ViewMode::ThreeD, "3D");
            });
            ui.separator();
            let before = app.tool;
            for (tool, label) in [
                (Tool::Select, "Select"),
                (Tool::Line, "Line"),
                (Tool::Circle, "Circle"),
                (Tool::Rect, "Rect"),
                (Tool::Arc, "Arc"),
                (Tool::Polyline, "Polyline"),
                (Tool::Ellipse, "Ellipse"),
                (Tool::Spline, "Spline"),
                (Tool::Polygon, "Polygon"),
                (Tool::Text, "Text"),
                (Tool::Dimension, "Dimension"),
                (Tool::Leader, "Leader"),
                (Tool::RadiusDim, "Radius dim"),
                (Tool::AngleDim, "Angle dim"),
                (Tool::Wall, "Wall"),
                (Tool::Door, "Door"),
                (Tool::Window, "Window"),
                (Tool::Room, "Room"),
                (Tool::Stair, "Stair"),
            ] {
                w::selectable_value(ui, &mut app.tool, tool, label);
            }
            ui.separator();
            for (tool, label) in [
                (Tool::Trim, "Trim"),
                (Tool::Extend, "Extend"),
                (Tool::Offset, "Offset"),
                (Tool::Fillet, "Fillet"),
                (Tool::Chamfer, "Chamfer"),
                (Tool::Split, "Split"),
            ] {
                w::selectable_value(ui, &mut app.tool, tool, label);
            }
            ui.separator();
            for (tool, label) in [
                (Tool::Move, "Move"),
                (Tool::Copy, "Copy"),
                (Tool::RotateRef, "Rotate…"),
                (Tool::ScaleRef, "Scale…"),
                (Tool::Mirror, "Mirror…"),
                (Tool::Stretch, "Stretch"),
            ] {
                w::selectable_value(ui, &mut app.tool, tool, label);
            }
            if app.tool == Tool::Mirror {
                w::checkbox(ui, &mut app.mirror_keep, "Keep original");
            }
            if app.tool != before {
                app.points.clear();
                app.op_target = None;
            }
            let (label, value) = match app.tool {
                Tool::Offset => ("Distance", Some(&mut app.offset_dist)),
                Tool::Fillet => ("Radius", Some(&mut app.fillet_radius)),
                Tool::Chamfer => ("Distance", Some(&mut app.chamfer_dist)),
                _ => ("", None),
            };
            match app.tool {
                Tool::Text => {
                    w::add(ui, "Text input", egui::TextEdit::singleline(&mut app.text_input).desired_width(90.0));
                    ui.label("Height");
                    w::add(ui, "Height", egui::DragValue::new(&mut app.text_height).speed(0.1).range(0.0..=f64::MAX));
                }
                Tool::Polygon => {
                    ui.label("Sides");
                    w::add(ui, "Sides", egui::DragValue::new(&mut app.polygon_sides).range(3..=64));
                }
                Tool::Leader => {
                    w::add(ui, "Text input", egui::TextEdit::singleline(&mut app.text_input).desired_width(90.0));
                    ui.label("Text height");
                    w::add(ui, "Text height", egui::DragValue::new(&mut app.dim_text_height).speed(0.1).range(0.0..=f64::MAX));
                }
                Tool::Wall => {
                    ui.label("Thickness");
                    w::add(ui, "Wall thickness", egui::DragValue::new(&mut app.wall_thickness).speed(1.0).range(1.0..=f64::MAX));
                    let align = app.wall_align;
                    w::combo(ui, "Wall align", egui::ComboBox::from_id_salt("wall_align").selected_text(format!("{align:?}")).width(80.0), |ui| {
                        for a in [WallAlign::Center, WallAlign::Left, WallAlign::Right] {
                            w::selectable_value(ui, &mut app.wall_align, a, &format!("{a:?}"));
                        }
                    });
                }
                Tool::Stair => {
                    ui.label("Width");
                    w::add(ui, "Stair width", egui::DragValue::new(&mut app.stair_width).speed(1.0).range(1.0..=f64::MAX));
                    ui.label("Max riser");
                    w::add(ui, "Max riser", egui::DragValue::new(&mut app.stair_max_riser).speed(1.0).range(1.0..=f64::MAX));
                    ui.label("Tread");
                    w::add(ui, "Tread", egui::DragValue::new(&mut app.stair_tread).speed(1.0).range(1.0..=f64::MAX));
                }
                Tool::Door | Tool::Window => {
                    let o = if app.tool == Tool::Door { &mut app.door } else { &mut app.window };
                    ui.label("Width");
                    w::add(ui, "Opening width", egui::DragValue::new(&mut o.width).speed(1.0).range(1.0..=f64::MAX));
                    ui.label("Height");
                    w::add(ui, "Opening height", egui::DragValue::new(&mut o.height).speed(1.0).range(1.0..=f64::MAX));
                    ui.label("Sill");
                    w::add(ui, "Opening sill", egui::DragValue::new(&mut o.sill).speed(1.0).range(0.0..=f64::MAX));
                }
                Tool::RadiusDim => {
                    w::checkbox(ui, &mut app.radial_diameter, "Diameter");
                }
                Tool::Dimension => {
                    ui.label("Text height");
                    w::add(ui, "Text height", egui::DragValue::new(&mut app.dim_text_height).speed(0.1).range(0.0..=f64::MAX));
                }
                _ => {}
            }
            if let Some(v) = value {
                ui.label(label);
                w::add(ui, label, egui::DragValue::new(v).speed(0.1).range(0.0..=f64::MAX));
            }
            ui.separator();
            if w::button(ui, "Box").clicked() {
                app.add_shape(Shape::Box { center: DVec3::new(0.0, 0.0, 10.0), size: DVec3::splat(20.0) });
            }
            if w::button(ui, "Cylinder").clicked() {
                app.add_shape(Shape::Cylinder { center: DVec3::new(0.0, 0.0, 15.0), radius: 10.0, height: 30.0 });
            }
            if w::button(ui, "Sphere").clicked() {
                app.add_shape(Shape::Sphere { center: DVec3::new(0.0, 0.0, 10.0), radius: 10.0 });
            }
            ui.separator();
            w::checkbox(ui, &mut app.osnap, "Object snap (F3)");
            w::checkbox(ui, &mut app.grid_snap, "Grid snap (F9)");
        });
    });
}
