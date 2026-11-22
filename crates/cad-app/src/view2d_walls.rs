//! The plan of the walls of the active level: filled outlines with clean joins (`cad_arch::plan`).
use crate::app::CadApp;
use crate::view2d::SELECTED;
use cad_document::EntityId;
use cad_geometry::{DVec2, Opening, Shape, Wall};
use eframe::egui::{self, Align2, Color32, FontId, Stroke};

/// Draws the visible, non-construction walls of `app.sketch` in their layer colour; selected walls and openings get
/// their outline in the selection colour; openings are gaps with their symbol. Uses `display_shape`, so drags and inspector edits preview.
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
    // Openings on drawn walls, placed on the wall as displayed, so they follow it while it is dragged or edited.
    let openings: Vec<(EntityId, usize, [u8; 3], Opening)> = app
        .doc
        .entities_in(app.sketch)
        .filter(|x| app.visible(x) && !x.construction)
        .filter_map(|x| match app.display_shape(x) {
            Shape::Opening(o) => {
                let i = walls.iter().position(|w| w.0 == o.host)?;
                Some((x.id, i, app.color(x), Opening { wall: walls[i].2, ..o }))
            }
            _ => None,
        })
        .collect();
    let gaps: Vec<(usize, (f64, f64))> = openings.iter().map(|(_, i, _, o)| (*i, cad_arch::gap(o))).collect();
    let plan = cad_arch::plan(&walls.iter().map(|w| w.2).collect::<Vec<_>>(), &gaps);
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
    for (id, _, [r, g, b], o) in &openings {
        let color = if app.selection.contains(id) { SELECTED } else { Color32::from_rgb(*r, *g, *b) };
        for [p, q] in cad_arch::opening_symbol(o) {
            painter.line_segment([to_screen(p), to_screen(q)], Stroke::new(1.0, color));
        }
        if app.selection.contains(id) {
            let c = o.corners();
            let pts: Vec<egui::Pos2> = c.iter().chain(c.first()).map(|p| to_screen(*p)).collect();
            painter.add(egui::Shape::line(pts, Stroke::new(2.0, SELECTED)));
        }
    }
}

/// Tags of the visible rooms of `app.sketch` at their seed: name and number, then the net area from the last rebuild
/// (or "not enclosed"). A selected room shows its area's outline and islands in the selection colour.
pub fn draw_rooms(app: &CadApp, painter: &egui::Painter, rect: egui::Rect) {
    let to_screen = |p: DVec2| app.cam2d.world_to_screen(rect, p);
    for x in app.doc.entities_in(app.sketch).filter(|x| app.visible(x)) {
        let Shape::Room(r) = app.display_shape(x) else { continue };
        let region = app.model.as_ref().and_then(|(_, m)| m.rooms.iter().find(|g| g.0 == x.id)).map(|g| &g.1);
        let selected = app.selection.contains(&x.id);
        let [cr, cg, cb] = app.color(x);
        let color = if selected { SELECTED } else { Color32::from_rgb(cr, cg, cb) };
        let second = region.map_or("not enclosed".to_string(), |g| app.doc.settings().format_area(g.area));
        painter.text(to_screen(r.seed), Align2::CENTER_CENTER, format!("{} {}
{second}", r.name, r.number), FontId::proportional(12.0), color);
        if let Some(g) = region.filter(|_| selected) {
            for l in std::iter::once(&g.outline).chain(&g.islands) {
                let pts: Vec<egui::Pos2> = l.iter().chain(l.first()).map(|p| to_screen(*p)).collect();
                painter.add(egui::Shape::line(pts, Stroke::new(2.0, SELECTED)));
            }
        }
    }
}
