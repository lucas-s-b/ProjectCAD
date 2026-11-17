//! The plan of the walls of the active level: filled outlines with clean joins (`cad_arch::plan`).
use crate::app::CadApp;
use crate::view2d::SELECTED;
use cad_document::EntityId;
use cad_geometry::{DVec2, Shape, Wall};
use eframe::egui::{self, Color32, Stroke};

/// Draws the visible, non-construction walls of `app.sketch` in their layer colour; selected walls get their outline
/// in the selection colour. Uses `display_shape`, so drags and inspector edits preview.
// shortcut: the plan is recomputed every frame; cache it per document revision when levels get large
pub fn draw(app: &CadApp, painter: &egui::Painter, rect: egui::Rect) {
    let walls: Vec<(EntityId, [u8; 3], Wall)> = app
        .doc
        .entities_in(app.sketch)
        .filter(|e| app.visible(e) && !e.construction)
        .filter_map(|e| match app.display_shape(e) {
            Shape::Wall(w) => Some((e.id, app.color(e), w)),
            _ => None,
        })
        .collect();
    if walls.is_empty() {
        return;
    }
    let plan = cad_arch::plan(&walls.iter().map(|w| w.2).collect::<Vec<_>>());
    let to_screen = |p: DVec2| app.cam2d.world_to_screen(rect, p);
    let mut mesh = egui::Mesh::default();
    for (i, tri) in &plan.fill {
        let [r, g, b] = walls[*i].1;
        let base = mesh.vertices.len() as u32;
        for p in tri {
            mesh.colored_vertex(to_screen(*p), Color32::from_rgba_unmultiplied(r, g, b, 50));
        }
        mesh.add_triangle(base, base + 1, base + 2);
    }
    painter.add(mesh);
    for (i, [a, b]) in &plan.edges {
        let [r, g, bl] = walls[*i].1;
        painter.line_segment([to_screen(*a), to_screen(*b)], Stroke::new(1.5, Color32::from_rgb(r, g, bl)));
    }
    for ((id, _, _), f) in walls.iter().zip(&plan.footprints) {
        if app.selection.contains(id) && f.len() > 1 {
            let pts: Vec<egui::Pos2> = f.iter().chain(f.first()).map(|p| to_screen(*p)).collect();
            painter.add(egui::Shape::line(pts, Stroke::new(2.0, SELECTED)));
        }
    }
}
