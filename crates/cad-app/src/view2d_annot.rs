use crate::app::CadApp;
use cad_geometry::annot::{Align, fills, labels, strokes};
use cad_geometry::{DVec2, Shape};
use eframe::egui::{self, Color32, FontId, Stroke, Vec2, emath::Rot2, epaint::TextShape};

/// Draws a string so that the point `local` of its box (pixels from the top-left) lands on `at`, rotated by `angle` radians counter-clockwise.
fn string(painter: &egui::Painter, at: egui::Pos2, local: impl Fn(Vec2) -> Vec2, size: f32, angle: f64, s: &str, color: Color32) {
    if size < 2.0 {
        return;
    }
    let galley = painter.layout_no_wrap(s.to_owned(), FontId::proportional(size), color);
    let r = -angle as f32; // screen Y points down, egui rotates clockwise
    let tl = at - Rot2::from_angle(r) * local(galley.size());
    painter.add(TextShape::new(tl, galley, color).with_angle(r));
}

/// Draws the strokes, filled arrows and labels of any shape.
pub fn draw(app: &CadApp, painter: &egui::Painter, rect: egui::Rect, shape: &Shape, stroke: Stroke, dash_px: &[f32]) {
    let to_screen = |p: DVec2| app.cam2d.world_to_screen(rect, p);
    if let Shape::XLine { point, dir, ray } = shape {
        // Clipped to the view: the 10 km reach must never reach the painter.
        let (lo, hi) = (app.cam2d.screen_to_world(rect, rect.left_bottom()), app.cam2d.screen_to_world(rect, rect.right_top()));
        if let Some((a, b)) = cad_geometry::xline::clip(*point, *dir, *ray, lo, hi) {
            crate::view2d::stroke_path(painter, vec![to_screen(a), to_screen(b)], stroke, dash_px);
        }
        return;
    }
    for path in strokes(shape) {
        crate::view2d::stroke_path(painter, path.iter().map(|p| to_screen(*p)).collect(), stroke, dash_px);
    }
    for t in fills(shape) {
        painter.add(egui::Shape::convex_polygon(t.iter().map(|p| to_screen(*p)).collect(), stroke.color, Stroke::NONE));
    }
    for l in labels(shape) {
        let size = (l.height * app.cam2d.zoom) as f32;
        let text = app.doc.settings().label_text(&l.text);
        // the font size stands in for the ascent, so the baseline is `size` below the top-left
        let local = |s: Vec2| match l.align {
            Align::Left => Vec2::new(0.0, size),
            Align::Center => s / 2.0,
            Align::Right => Vec2::new(s.x, size),
        };
        string(painter, to_screen(l.anchor), local, size, l.angle, &text, stroke.color);
    }
}
