use crate::app::CadApp;
use cad_document::{Constraint, Settings};
use eframe::egui::{self, Align2, Color32, FontId};
use std::collections::BTreeMap;

const COLOR: Color32 = Color32::from_rgb(120, 200, 120);
const FONT_PX: f32 = 10.0;
const STACK_PX: f32 = 12.0;

/// The short label of a constraint; valued ones show their value in display units.
pub fn glyph_text(c: &Constraint, s: &Settings) -> String {
    match c {
        Constraint::Coincident(..) => "Coi".into(),
        Constraint::Horizontal(_) => "H".into(),
        Constraint::Vertical(_) => "V".into(),
        Constraint::Parallel(..) => "//".into(),
        Constraint::Perpendicular(..) => "\u{22a5}".into(),
        Constraint::Tangent(..) => "T".into(),
        Constraint::Concentric(..) => "\u{25ce}".into(),
        Constraint::Equal(..) => "=".into(),
        Constraint::Collinear(..) => "Col".into(),
        Constraint::Symmetric(..) => "Sym".into(),
        Constraint::PointOnLine(..) | Constraint::PointOnCircle(..) => "On".into(),
        Constraint::Fixed { .. } => "Fix".into(),
        Constraint::Distance(_, _, v) => format!("D {}", s.format_length(*v)),
        Constraint::Angle(_, _, v) => format!("\u{2220} {:.*}\u{b0}", s.precision as usize, v.to_degrees()),
        Constraint::Radius(_, v) => format!("R {}", s.format_length(*v)),
        Constraint::Diameter(_, v) => format!("\u{d8} {}", s.format_length(*v)),
    }
}

/// Like `glyph_text`, with the driving expression in front of the value: "D w = 40.00".
pub fn glyph_label(c: &Constraint, expr: Option<&str>, s: &Settings) -> String {
    let text = glyph_text(c, s);
    match (expr, text.split_once(' ')) {
        (Some(e), Some((name, value))) => format!("{name} {e} = {value}"),
        _ => text,
    }
}

/// Labels of the active sketch's constraints next to the middle of their first entity, stacked when an entity has several.
pub fn draw(app: &CadApp, painter: &egui::Painter, rect: egui::Rect) {
    let mut stacked: BTreeMap<u64, u32> = BTreeMap::new();
    for (id, c) in app.doc.constraints() {
        let Some(e) = c.entities().first().and_then(|id| app.doc.get(*id)) else { continue };
        if e.sketch != app.sketch || !app.visible(e) {
            continue;
        }
        let k = stacked.entry(e.id).or_insert(0);
        let (lo, hi) = app.display_shape(e).bbox();
        let at = app.cam2d.world_to_screen(rect, ((lo + hi) / 2.0).truncate()) + egui::vec2(4.0, STACK_PX * *k as f32);
        *k += 1;
        painter.text(at, Align2::LEFT_CENTER, glyph_label(c, app.doc.constraint_expr(id), app.doc.settings()), FontId::proportional(FONT_PX), COLOR);
    }
}

#[cfg(test)]
#[path = "glyphs_tests.rs"]
mod tests;
