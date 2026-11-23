//! Mass properties sections of the Model and Assembly tabs and the centre of mass marker.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::{MassRow, Settings};
use cad_geometry::DVec3;
use eframe::egui::{self, Color32, Pos2, Stroke};

/// "(20.00, 10.00, 5.00) mm".
pub(crate) fn point(s: &Settings, p: DVec3) -> String {
    format!("({}, {}, {}) {}", s.format_length(p.x), s.format_length(p.y), s.format_length(p.z), s.units.suffix())
}

/// The accuracy label as a small tagged line (so the MCP `widgets` list shows it).
fn label(ui: &mut egui::Ui, row: &MassRow) {
    let text = row.accuracy.to_string();
    let r = ui.small(&text);
    w::tag(ui, &text, r);
}

/// One solid: title, mass with its density, centre of mass, label; `full` adds the inertia tensor about the centre
/// of mass, the principal moments and the principal axes.
fn block(ui: &mut egui::Ui, s: &Settings, title: &str, row: &MassRow, full: bool) {
    let p = &row.props;
    ui.label(title);
    let density = if row.assumed { format!("{:.3} g/cm\u{b3} assumed", row.density) } else { format!("{:.3} g/cm\u{b3}", row.density) };
    ui.label(format!("Mass {} ({density})", s.format_mass(p.mass)));
    ui.label(format!("Centre of mass {}", point(s, p.com)));
    if full {
        let i = p.inertia;
        let f = |v: f64| s.format_inertia(v);
        ui.label(format!("Ixx {}, Iyy {}, Izz {}", f(i.x_axis.x), f(i.y_axis.y), f(i.z_axis.z)));
        ui.label(format!("Ixy {}, Iyz {}, Izx {}", f(i.y_axis.x), f(i.z_axis.y), f(i.x_axis.z)));
        let (m, axes) = p.principal();
        ui.label(format!("Principal {}, {}, {}", f(m[0]), f(m[1]), f(m[2])));
        for (k, a) in axes.iter().enumerate() {
            ui.small(format!("Axis {}: ({:.4}, {:.4}, {:.4})", k + 1, a.x, a.y, a.z));
        }
    }
    label(ui, row);
}

/// Model tab: one block per body (bodies carry no material, so 1 g/cm³ is assumed), then the marker toggle.
pub fn bodies(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some(rep) = app.mass.clone() else { return };
    if rep.bodies.is_empty() {
        return;
    }
    ui.heading("Mass properties");
    let s = *app.doc.settings();
    for (id, row) in &rep.bodies {
        let name = app.doc.feature(*id).map_or_else(String::new, |f| f.name.clone());
        block(ui, &s, &format!("Body {id}: {name}"), row, rep.bodies.len() == 1);
    }
    w::checkbox(ui, &mut app.show_com, "Show centre of mass");
}

/// Assembly tab: the assembly total in full, then one short block per component, then the marker toggle.
pub fn assembly(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.separator();
    ui.heading("Mass properties");
    let Some(rep) = app.mass.clone() else { return };
    let Some(total) = &rep.assembly else {
        ui.label("No components");
        return;
    };
    let s = *app.doc.settings();
    block(ui, &s, "Assembly (about its centre of mass)", total, true);
    for (id, row) in &rep.components {
        block(ui, &s, &crate::assembly_joints::name(app, *id), row, false);
    }
    w::checkbox(ui, &mut app.show_com, "Show centre of mass");
}

/// The centre of mass shown by the marker: the assembly's when there are placed components, else that of all bodies
/// together. None while exploded (the marker belongs to the assembled placement).
pub fn marker_point(app: &CadApp) -> Option<DVec3> {
    let rep = app.mass.as_ref()?;
    if app.explode_factor() != 0.0 {
        return None;
    }
    rep.assembly.as_ref().map(|r| r.props.com).or_else(|| {
        let parts: Vec<_> = rep.bodies.iter().map(|(_, r)| r.props).collect();
        cad_document::MassProps::combine(&parts).map(|p| p.com)
    })
}

/// The usual centre of mass symbol: a circle of radius 7 px with alternate black and white quarters, drawn on top of
/// the scene (the point is usually inside the solid).
pub fn paint_marker(painter: &egui::Painter, c: Pos2) {
    let r = 7.0;
    for k in 0..4 {
        let a0 = k as f32 * std::f32::consts::FRAC_PI_2;
        let rim = (0..=8).map(|i| {
            let a = a0 + i as f32 * std::f32::consts::FRAC_PI_2 / 8.0;
            c + egui::vec2(a.cos(), -a.sin()) * r
        });
        let color = if k % 2 == 0 { Color32::BLACK } else { Color32::WHITE };
        painter.add(egui::Shape::convex_polygon(std::iter::once(c).chain(rim).collect(), color, Stroke::NONE));
    }
    painter.circle_stroke(c, r, Stroke::new(1.5, Color32::BLACK));
}
