use crate::app::{CadApp, PickFace, Tool};
use crate::{view2d_edit, view2d_tools};
use cad_document::{Command, Entity, EntityId, LineType};
use cad_geometry::snap::{grid_snap, object_snap};
use cad_geometry::{DVec2, DVec3, Shape, distance_2d};
use eframe::egui::{self, Color32, PointerButton, Stroke};

const PICK_PIXELS: f64 = 6.0;
const SNAP_PIXELS: f64 = 10.0;
const SELECTED: Color32 = Color32::from_rgb(255, 160, 0);
/// Colour and screen dash pattern (dash, gap) of construction geometry, whatever its layer.
pub const CONSTRUCTION_RGB: [u8; 3] = [120, 120, 160];
const CONSTRUCTION_DASH: [f32; 2] = [6.0, 4.0];

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    app.view_rect = rect;
    app.snap = None;

    navigate(app, ui, &resp, rect);
    if let Some(p) = resp.hover_pos() {
        app.cursor_world = app.cam2d.screen_to_world(rect, p);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.points.clear();
        app.op_target = None;
        app.pick_face = PickFace::Off;
        app.tool = Tool::Select;
    }
    let mut cursor = app.cursor_world;
    match app.tool {
        Tool::Select => select(app, ui, &resp, rect),
        Tool::Trim | Tool::Extend | Tool::Offset | Tool::Fillet | Tool::Chamfer | Tool::Split => {
            view2d_edit::edit_tool(app, ui, &resp, rect)
        }
        _ => cursor = view2d_tools::draw_tool(app, ui, &resp, rect),
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_gray(24));
    draw_grid(app, &painter, rect);
    draw_reference(app, &painter, rect);
    for e in app.doc.entities_in(app.sketch).filter(|e| app.visible(e)) {
        let [r, g, b] = if e.construction { CONSTRUCTION_RGB } else { app.color(e) };
        let marked = app.selection.contains(&e.id) || app.op_target.is_some_and(|(id, _)| id == e.id);
        let color = if marked { SELECTED } else { Color32::from_rgb(r, g, b) };
        let stroke = Stroke::new(stroke_width(app, e, marked), color);
        let dash = if e.construction { CONSTRUCTION_DASH.to_vec() } else { dash_pixels(app, e) };
        draw_styled(app, &painter, rect, &app.display_shape(e), stroke, &dash);
    }
    if app.show_glyphs {
        crate::glyphs::draw(app, &painter, rect);
    }
    view2d_tools::draw_preview(app, &painter, rect, cursor);
    view2d_tools::draw_marker(app, &painter, rect);
    view2d_edit::draw_hint(app, &painter, rect);
    let name = app.doc.sketch(app.sketch).map_or("?", |s| s.name.as_str());
    painter.text(
        rect.right_top() + egui::vec2(-8.0, 8.0),
        egui::Align2::RIGHT_TOP,
        format!("Sketch: {name}"),
        egui::FontId::proportional(12.0),
        Color32::from_gray(150),
    );
}

/// Body edges projected onto the active sketch plane; not pickable, not snappable.
// shortcut: edges are recomputed every frame, cache them per document revision when models get large
fn draw_reference(app: &CadApp, painter: &egui::Painter, rect: egui::Rect) {
    let (Ok(plane), Some((_, model))) = (app.sketch_plane(app.sketch), app.model.as_ref()) else { return };
    let project = |p: DVec3| {
        let d = p - plane.origin;
        app.cam2d.world_to_screen(rect, DVec2::new(d.dot(plane.x), d.dot(plane.y)))
    };
    let stroke = Stroke::new(1.0, Color32::from_rgb(70, 70, 80));
    for (_, body) in &model.bodies {
        for [a, b] in body.feature_edges() {
            painter.line_segment([project(a), project(b)], stroke);
        }
    }
}

fn navigate(app: &mut CadApp, ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect) {
    if resp.dragged_by(PointerButton::Middle) || resp.dragged_by(PointerButton::Secondary) {
        let d = resp.drag_delta();
        let zoom = app.cam2d.zoom;
        app.cam2d.center.x -= f64::from(d.x) / zoom;
        app.cam2d.center.y += f64::from(d.y) / zoom;
    }
    if resp.hovered()
        && let Some(cursor) = resp.hover_pos()
    {
        let scroll = ui.input(|i| i.smooth_scroll_delta().y);
        if scroll != 0.0 {
            app.cam2d.zoom_at(rect, cursor, f64::from(scroll * 0.002).exp());
        }
    }
}

/// World spacing of the visible grid: a power of 10 with at least 20 px on screen.
pub fn grid_step(zoom: f64) -> f64 {
    10f64.powf((20.0 / zoom).log10().ceil())
}

/// Cursor position in world space after object snap, then grid snap. Sets `app.snap`.
pub fn snapped(app: &mut CadApp, rect: egui::Rect, pos: egui::Pos2, exclude_selection: bool, from: Option<DVec2>) -> DVec2 {
    let zoom = app.cam2d.zoom;
    let world = app.cam2d.screen_to_world(rect, pos);
    let hit = if app.osnap {
        let shapes = app
            .doc
            .entities_in(app.sketch)
            .filter(|e| app.visible(e) && !(exclude_selection && app.selection.contains(&e.id)))
            .map(|e| &e.shape);
        object_snap(world, SNAP_PIXELS / zoom, from, shapes)
    } else {
        None
    };
    app.snap = hit.or_else(|| app.grid_snap.then(|| grid_snap(world, grid_step(zoom))));
    app.snap.map_or(world, |s| s.point)
}

pub fn pick(app: &CadApp, rect: egui::Rect, pos: egui::Pos2) -> Option<EntityId> {
    let w = app.cam2d.screen_to_world(rect, pos);
    app.doc
        .entities_in(app.sketch)
        .filter(|e| app.pickable(e))
        .map(|e| (e.id, distance_2d(&e.shape, w) * app.cam2d.zoom))
        .filter(|(_, d)| *d <= PICK_PIXELS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

fn select(app: &mut CadApp, ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect) {
    let shift = ui.input(|i| i.modifiers.shift);
    if resp.drag_started_by(PointerButton::Primary) {
        let press = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
        if let Some(press) = press
            && let Some(id) = pick(app, rect, press)
        {
            if !app.selection.contains(&id) {
                app.click_select(Some(id), false);
            }
            let w = snapped(app, rect, press, false, None);
            app.drag = Some((w, w));
        }
    }
    if resp.dragged_by(PointerButton::Primary)
        && let Some(pos) = resp.interact_pointer_pos()
        && let Some((start, _)) = app.drag
    {
        let now = snapped(app, rect, pos, true, None);
        app.drag = Some((start, now));
    }
    if resp.drag_stopped_by(PointerButton::Primary) {
        if let Some((start, now)) = app.drag.take()
            && now != start
        {
            let delta = (now - start).extend(0.0);
            let cmds: Vec<Command> = app
                .selection
                .iter()
                .filter_map(|id| app.doc.get(*id))
                .map(|e| {
                    let mut shape = e.shape.clone();
                    shape.translate(delta);
                    Command::SetShape { id: e.id, shape }
                })
                .collect();
            let pinned = app.selection.clone();
            app.exec_pinned(cmds, &pinned);
        }
    } else if resp.clicked_by(PointerButton::Primary)
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let hit = pick(app, rect, pos);
        app.click_select(hit, shift);
    }
}

/// Strokes a screen path, dashed when `dash_px` (dash, gap, dash, gap...) is not empty.
pub fn stroke_path(painter: &egui::Painter, pts: Vec<egui::Pos2>, stroke: Stroke, dash_px: &[f32]) {
    if dash_px.is_empty() {
        painter.add(egui::Shape::line(pts, stroke));
        return;
    }
    let dashes: Vec<f32> = dash_px.iter().step_by(2).copied().collect();
    let gaps: Vec<f32> = dash_px.iter().skip(1).step_by(2).copied().collect();
    painter.extend(egui::Shape::dashed_line_with_offset(&pts, stroke, &dashes, &gaps, 0.0));
}

/// Screen dash pattern of a layer's line type: 3 pixels per paper millimetre.
pub fn dash_pixels(app: &CadApp, e: &Entity) -> Vec<f32> {
    let lt = app.doc.layer(e.layer).map_or(LineType::Continuous, |l| l.line_type);
    lt.pattern_mm().iter().map(|v| (v * 3.0) as f32).collect()
}

/// Stroke width in pixels for a layer line weight; selected entities keep at least 1.5.
pub fn stroke_width(app: &CadApp, e: &Entity, selected: bool) -> f32 {
    let w = app.doc.layer(e.layer).map_or(0.25, |l| l.line_weight);
    let px = (w / 0.25).clamp(1.0, 6.0) as f32;
    if selected { px.max(1.5) } else { px }
}

pub fn draw_shape(app: &CadApp, painter: &egui::Painter, rect: egui::Rect, shape: &Shape, stroke: Stroke) {
    draw_styled(app, painter, rect, shape, stroke, &[]);
}

pub fn draw_styled(app: &CadApp, painter: &egui::Painter, rect: egui::Rect, shape: &Shape, stroke: Stroke, dash_px: &[f32]) {
    crate::view2d_annot::draw(app, painter, rect, shape, stroke, dash_px);
}

fn draw_grid(app: &CadApp, painter: &egui::Painter, rect: egui::Rect) {
    let cam = &app.cam2d;
    let step = grid_step(cam.zoom);
    let lo = cam.screen_to_world(rect, rect.left_bottom());
    let hi = cam.screen_to_world(rect, rect.right_top());
    let grid = Stroke::new(1.0, Color32::from_gray(48));
    let mut x = (lo.x / step).floor() * step;
    while x <= hi.x {
        let (a, b) = (cam.world_to_screen(rect, DVec2::new(x, lo.y)), cam.world_to_screen(rect, DVec2::new(x, hi.y)));
        painter.line_segment([a, b], grid);
        x += step;
    }
    let mut y = (lo.y / step).floor() * step;
    while y <= hi.y {
        let (a, b) = (cam.world_to_screen(rect, DVec2::new(lo.x, y)), cam.world_to_screen(rect, DVec2::new(hi.x, y)));
        painter.line_segment([a, b], grid);
        y += step;
    }
    let (o, red, green) = (DVec2::ZERO, Color32::from_rgb(200, 60, 60), Color32::from_rgb(60, 180, 60));
    painter.line_segment(
        [cam.world_to_screen(rect, DVec2::new(lo.x, o.y)), cam.world_to_screen(rect, DVec2::new(hi.x, o.y))],
        Stroke::new(1.5, red),
    );
    painter.line_segment(
        [cam.world_to_screen(rect, DVec2::new(o.x, lo.y)), cam.world_to_screen(rect, DVec2::new(o.x, hi.y))],
        Stroke::new(1.5, green),
    );
}
