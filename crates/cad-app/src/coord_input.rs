use crate::app::CadApp;
use crate::view2d_tools::tool_point;
use cad_geometry::DVec2;
use eframe::egui::{self, Key, Modifiers};

const USAGE: &str = "use x,y  @dx,dy  or @d<a";

fn number(s: &str) -> Result<f64, String> {
    let v: f64 = s.trim().parse().map_err(|_| format!("'{}' is not a number", s.trim()))?;
    if v.is_finite() { Ok(v) } else { Err(format!("'{}' is not finite", s.trim())) }
}

/// "x,y" absolute; "@dx,dy" relative to `last`; "@d<a" distance and angle in degrees relative to `last`.
/// Numbers are in document units and are converted to millimetres with `mm_per_unit`.
pub fn parse_coord(input: &str, last: Option<DVec2>, mm_per_unit: f64) -> Result<DVec2, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("empty input".into());
    }
    let Some(rel) = s.strip_prefix('@') else {
        let (x, y) = s.split_once(',').ok_or(USAGE)?;
        return Ok(DVec2::new(number(x)?, number(y)?) * mm_per_unit);
    };
    let offset = if let Some((d, a)) = rel.split_once('<') {
        DVec2::from_angle(number(a)?.to_radians()) * number(d)? * mm_per_unit
    } else {
        let (dx, dy) = rel.split_once(',').ok_or(USAGE)?;
        DVec2::new(number(dx)?, number(dy)?) * mm_per_unit
    };
    last.map(|l| l + offset).ok_or_else(|| "no previous point".into())
}

/// The status-bar field of the draw tools. Enter turns the text into a point for the active tool.
pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    if !app.tool.is_draw() {
        return;
    }
    let resp = crate::widgets::add(ui, "Coordinates", egui::TextEdit::singleline(&mut app.coord_input).desired_width(140.0).hint_text("x,y  @dx,dy  @d<a"));
    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        // The canvas must not see this Enter: it would also finish a polyline or spline.
        ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        let mm = app.doc.settings().units.mm_per_unit();
        match parse_coord(&app.coord_input, app.points.last().copied(), mm) {
            Ok(p) => {
                app.coord_input.clear();
                app.status.clear();
                tool_point(app, p);
                resp.request_focus();
            }
            Err(e) => app.status = e,
        }
    }
}

#[cfg(test)]
#[path = "coord_input_tests.rs"]
mod tests;
