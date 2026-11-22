//! The plan of the walls of the active level: filled outlines with clean joins (`cad_arch::plan`).
use crate::app::{CadApp, LevelView};
use crate::view2d::SELECTED;
use cad_document::{Entity, EntityId, SketchId};
use cad_geometry::{DVec2, Opening, Shape, Wall, outline_paths};
use eframe::egui::{self, Align2, Color32, FontId, Stroke};

/// Draws the visible, non-construction walls of `sketch` (fill, cleaned edges, opening gaps and symbols) in their layer
/// colour, selected walls and openings outlined in the selection colour; with `grey`, everything in that colour and
/// nothing highlighted (another level under the active one). Uses `display_shape`, so drags and inspector edits preview.
fn draw_plan(app: &CadApp, painter: &egui::Painter, rect: egui::Rect, sketch: SketchId, grey: Option<Color32>) {
    let color = |e: &Entity| grey.map_or(app.color(e), |g| [g.r(), g.g(), g.b()]);
    let selected = |id: &EntityId| grey.is_none() && app.selection.contains(id);
    let walls: Vec<(EntityId, [u8; 3], Wall)> = app
        .doc
        .entities_in(sketch)
        .filter(|e| app.visible(e) && !e.construction)
        .filter_map(|e| match app.display_shape(e) {
            Shape::Wall(w) => Some((e.id, color(e), w)),
            _ => None,
        })
        .collect();
    if walls.is_empty() {
        return;
    }
    // Openings on drawn walls, placed on the wall as displayed, so they follow it while it is dragged or edited.
    let openings: Vec<(EntityId, usize, [u8; 3], Opening)> = app
        .doc
        .entities_in(sketch)
        .filter(|x| app.visible(x) && !x.construction)
        .filter_map(|x| match app.display_shape(x) {
            Shape::Opening(o) => {
                let i = walls.iter().position(|w| w.0 == o.host)?;
                Some((x.id, i, color(x), Opening { wall: walls[i].2, ..o }))
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
        if selected(id) && f.len() > 1 {
            let pts: Vec<egui::Pos2> = f.iter().chain(f.first()).map(|p| to_screen(*p)).collect();
            painter.add(egui::Shape::line(pts, Stroke::new(2.0, SELECTED)));
        }
    }
    for (id, _, [r, g, b], o) in &openings {
        let stroke = if selected(id) { SELECTED } else { Color32::from_rgb(*r, *g, *b) };
        for [p, q] in cad_arch::opening_symbol(o) {
            painter.line_segment([to_screen(p), to_screen(q)], Stroke::new(1.0, stroke));
        }
        if selected(id) {
            let c = o.corners();
            let pts: Vec<egui::Pos2> = c.iter().chain(c.first()).map(|p| to_screen(*p)).collect();
            painter.add(egui::Shape::line(pts, Stroke::new(2.0, SELECTED)));
        }
    }
}

/// The plan of the walls of the active level (see `draw_plan`).
// shortcut: the plan is recomputed every frame; cache it per document revision when levels get large
pub fn draw(app: &CadApp, painter: &egui::Painter, rect: egui::Rect) {
    draw_plan(app, painter, rect, app.sketch, None);
}

/// The other levels `app.level_view` asks for, in grey: walls as in `draw`, everything else as outlines. Drawn before
/// the active level; not pickable or snappable (picks and snaps only look at `app.sketch`).
pub fn draw_others(app: &CadApp, painter: &egui::Painter, rect: egui::Rect) {
    let others: Vec<SketchId> = match app.level_view {
        LevelView::Active => return,
        LevelView::Below => app.doc.level_below(app.sketch).into_iter().collect(),
        LevelView::All => app.doc.levels().into_iter().map(|l| l.0).filter(|&s| s != app.sketch).collect(),
    };
    let to_screen = |p: DVec2| app.cam2d.world_to_screen(rect, p);
    for s in others {
        draw_plan(app, painter, rect, s, Some(OTHER_LEVEL));
        for x in app.doc.entities_in(s).filter(|x| app.visible(x) && !matches!(x.shape, Shape::Wall(_) | Shape::Opening(_))) {
            for path in outline_paths(&x.shape) {
                let pts: Vec<egui::Pos2> = path.iter().map(|p| to_screen(p.truncate())).collect();
                painter.add(egui::Shape::line(pts, Stroke::new(1.0, OTHER_LEVEL)));
            }
        }
    }
}

/// Colour of other levels in the plan.
const OTHER_LEVEL: Color32 = Color32::from_gray(90);

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
