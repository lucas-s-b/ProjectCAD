use crate::app::{CadApp, Tool};
use crate::view2d::{draw_shape, pick, snapped};
use cad_geometry::snap::SnapKind;
use cad_geometry::{DVec2, Shape, Tolerance, Wall};
use std::f64::consts::{PI, TAU};
use eframe::egui::{self, Color32, PointerButton, Stroke, StrokeKind};

const POLAR_STEP: f64 = 15.0 * std::f64::consts::PI / 180.0;

/// Closed polyline with `sides` vertices on the circle through `vertex`, starting at it.
fn polygon(center: DVec2, vertex: DVec2, sides: u32) -> Shape {
    let d = vertex - center;
    let (r, a0) = (d.length(), d.y.atan2(d.x));
    let points = (0..sides)
        .map(|k| center + DVec2::from_angle(a0 + std::f64::consts::TAU * f64::from(k) / f64::from(sides)) * r)
        .collect();
    Shape::Polyline { points, closed: true }
}

/// Construction line (or ray) from `first` through `through`; None when the points coincide.
fn xline(first: DVec2, through: DVec2, ray: bool) -> Option<Shape> {
    let d = through - first;
    (d.length() > 1e-9).then(|| Shape::XLine { point: first, dir: d.normalize(), ray })
}

/// Ellipse from its centre, the end of the first axis and a point that sets the second radius.
fn ellipse(center: DVec2, axis_end: DVec2, third: DVec2) -> Shape {
    let d = axis_end - center;
    let ry = if d.length() > 0.0 { d.normalize().perp_dot(third - center).abs() } else { 0.0 };
    Shape::Ellipse { center, rx: d.length(), ry, angle: d.y.atan2(d.x) }
}

/// Angle at `v` from the ray to `p1` to the ray to `p2`, the smaller way round, arc radius `|p2 - v|`.
fn angle_dim(v: DVec2, p1: DVec2, p2: DVec2, text_height: f64) -> Option<Shape> {
    let (d1, d2) = (p1 - v, p2 - v);
    let (mut start, mut sweep) = (d1.to_angle(), (d2.to_angle() - d1.to_angle()).rem_euclid(TAU));
    if sweep > PI {
        start = d2.to_angle();
        sweep = TAU - sweep;
    }
    Some(Shape::AngularDim { center: v, start, sweep, radius: d2.length(), text_height })
}

/// First click of the radius tool: remembers the circle or arc under the cursor as centre and a point at its radius.
fn pick_circle(app: &mut CadApp, rect: egui::Rect, pos: egui::Pos2) {
    let hit = pick(app, rect, pos).and_then(|id| app.doc.get(id)).map(|e| e.shape.clone());
    match hit {
        Some(Shape::Circle { center, radius } | Shape::Arc { center, radius, .. }) => {
            app.points = vec![center, center + DVec2::new(radius, 0.0)];
            app.status.clear();
        }
        _ => app.status = "Click a circle or an arc".into(),
    }
}

/// Shape the active tool would create from the collected points plus the cursor.
fn build_shape(app: &CadApp, points: &[DVec2], cursor: DVec2) -> Option<Shape> {
    let first = *points.first()?;
    let (tool, text_height, sides) = (app.tool, app.dim_text_height, app.polygon_sides);
    match (tool, points.len()) {
        (Tool::Line, 1) => Some(Shape::Line { a: first, b: cursor }),
        (Tool::XLine | Tool::Ray, 1) => xline(first, cursor, tool == Tool::Ray),
        (Tool::Circle, 1) => Some(Shape::Circle { center: first, radius: first.distance(cursor) }),
        (Tool::Rect, 1) => Some(Shape::rect(first, cursor)),
        (Tool::Arc, 1) => Some(Shape::Line { a: first, b: cursor }),
        (Tool::Arc, 2) => Some(Shape::arc_through(first, points[1], cursor)),
        (Tool::Polygon, 1) => Some(polygon(first, cursor, sides)),
        (Tool::Ellipse, 2) => Some(ellipse(first, points[1], cursor)),
        (Tool::Dimension, 2) => {
            let d = points[1] - first;
            // signed distance of the cursor from the line a->b, positive on the left
            let offset = if d.length() > 0.0 { d.normalize().perp_dot(cursor - first) } else { 0.0 };
            Some(Shape::Dimension { a: first, b: points[1], offset, text_height })
        }
        (Tool::RadiusDim, 2) => {
            let d = cursor - first;
            Some(Shape::RadialDim { center: first, radius: first.distance(points[1]), angle: d.y.atan2(d.x), text_height, diameter: app.radial_diameter })
        }
        (Tool::AngleDim, 2) => angle_dim(first, points[1], cursor, text_height),
        // the preview is 2D, its height is never shown
        (Tool::Wall, 1) => Some(Shape::Wall(Wall { a: first, b: cursor, thickness: app.wall_thickness, height: 1.0, align: app.wall_align })),
        (Tool::Stair, 1) => app.stair_from(first, cursor).map(Shape::Stair),
        (Tool::Polyline | Tool::Spline | Tool::Leader, _) => {
            let mut pts = points.to_vec();
            pts.push(cursor);
            Some(if tool == Tool::Spline {
                Shape::Spline { points: pts, closed: false }
            } else if tool == Tool::Leader {
                Shape::Leader { points: pts, text: app.text_input.clone(), text_height }
            } else {
                Shape::Polyline { points: pts, closed: false }
            })
        }
        _ => None,
    }
}

/// Feeds one world point to the active draw tool; creates the shape when enough points exist.
pub fn tool_point(app: &mut CadApp, p: DVec2) {
    if crate::view2d_xform::is_xform(app.tool) {
        crate::view2d_xform::point(app, p);
        return;
    }
    app.points.push(p);
    if matches!(app.tool, Tool::XLine | Tool::Ray) {
        // The first point stays: every later click adds another line through it, until Escape.
        if let [first, through] = app.points[..] {
            app.points.truncate(1);
            if let Some(shape) = xline(first, through, app.tool == Tool::Ray) {
                app.add_construction(shape);
            }
        }
        return;
    }
    if app.tool == Tool::Wall {
        // Walls chain: each click ends one wall and starts the next; Escape stops.
        if let [a, b] = app.points[..] {
            app.points = vec![b];
            app.add_wall(a, b);
        }
        return;
    }
    if matches!(app.tool, Tool::Door | Tool::Window) {
        // One click per opening; the tool stays active.
        app.points.clear();
        app.add_opening(p);
        return;
    }
    if app.tool == Tool::Room {
        // One click per room; the tool stays active.
        app.points.clear();
        app.add_room(p);
        return;
    }
    if app.tool == Tool::Stair {
        // Start point, then direction; the tool stays active.
        if let [a, b] = app.points[..] {
            app.points.clear();
            app.add_stair(a, b);
        }
        return;
    }
    let needed = match app.tool {
        Tool::Line | Tool::Circle | Tool::Rect | Tool::Polygon => 2,
        Tool::Arc | Tool::Dimension | Tool::Ellipse | Tool::RadiusDim | Tool::AngleDim => 3,
        Tool::Text => 1,
        _ => usize::MAX,
    };
    if app.points.len() < needed {
        return;
    }
    let pts = std::mem::take(&mut app.points);
    let shape = match app.tool {
        Tool::Arc => Some(Shape::arc_through(pts[0], pts[1], pts[2])),
        Tool::Ellipse => Some(ellipse(pts[0], pts[1], pts[2])),
        Tool::Text => Some(Shape::Text { pos: pts[0], height: app.text_height, angle: 0.0, text: app.text_input.clone() }),
        Tool::Dimension | Tool::RadiusDim | Tool::AngleDim => build_shape(app, &pts[..2], pts[2]),
        _ => build_shape(app, &pts[..1], pts[1]),
    };
    if let Some(shape) = shape {
        let link = (app.tool == Tool::Dimension).then(|| app.point_ref_at(pts[0]).zip(app.point_ref_at(pts[1]))).flatten();
        app.add_shape_linked(shape, link);
    }
}

/// Handles clicks and Enter of a draw tool. Returns the snapped cursor position.
pub fn draw_tool(app: &mut CadApp, ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect) -> DVec2 {
    let Some(pos) = resp.hover_pos().or(resp.interact_pointer_pos()) else { return app.cursor_world };
    let mut p = snapped(app, rect, pos, false, app.points.last().copied());
    let object_hit = app.snap.is_some_and(|s| s.kind != SnapKind::Grid);
    if ui.input(|i| i.modifiers.shift)
        && !object_hit
        && let Some(last) = app.points.last().copied()
    {
        let d = app.cam2d.screen_to_world(rect, pos) - last;
        let a = (d.y.atan2(d.x) / POLAR_STEP).round() * POLAR_STEP;
        p = last + DVec2::new(a.cos(), a.sin()) * d.length();
        app.snap = None;
    }
    if resp.clicked_by(PointerButton::Primary) {
        if app.tool == Tool::RadiusDim && app.points.is_empty() {
            pick_circle(app, rect, pos);
        } else {
            tool_point(app, p);
        }
    }
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter)) && !ui.ctx().egui_wants_keyboard_input();
    if enter && matches!(app.tool, Tool::Polyline | Tool::Spline | Tool::Leader) && app.points.len() >= 2 {
        let points = std::mem::take(&mut app.points);
        let text_height = app.dim_text_height;
        app.add_shape(if app.tool == Tool::Spline {
            Shape::Spline { points, closed: false }
        } else if app.tool == Tool::Leader {
            Shape::Leader { points, text: app.text_input.clone(), text_height }
        } else {
            Shape::Polyline { points, closed: false }
        });
    }
    p
}

pub fn draw_preview(app: &CadApp, painter: &egui::Painter, rect: egui::Rect, cursor: DVec2) {
    crate::view2d_xform::preview(app, painter, rect, cursor);
    if let Some(shape) = build_shape(app, &app.points, cursor)
        && shape.validate(&Tolerance::default()).is_ok()
    {
        draw_shape(app, painter, rect, &shape, Stroke::new(1.0, Color32::from_rgb(120, 200, 255)));
    }
}

pub fn draw_marker(app: &CadApp, painter: &egui::Painter, rect: egui::Rect) {
    let Some(snap) = app.snap else { return };
    if app.tool == Tool::Select && app.drag.is_none() {
        return;
    }
    let at = app.cam2d.world_to_screen(rect, snap.point);
    let yellow = Color32::YELLOW;
    if snap.kind == SnapKind::Parallel
        && let (Some(from), Some(dir)) = (app.points.last(), app.parallel_ref)
    {
        let (lo, hi) = (app.cam2d.screen_to_world(rect, rect.left_bottom()), app.cam2d.screen_to_world(rect, rect.right_top()));
        if let Some((a, b)) = cad_geometry::xline::clip(*from, dir, false, lo, hi) {
            let pts = [app.cam2d.world_to_screen(rect, a), app.cam2d.world_to_screen(rect, b)];
            painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.0, Color32::from_rgb(120, 200, 120)), 6.0, 4.0));
        }
    }
    painter.rect_stroke(egui::Rect::from_center_size(at, egui::vec2(10.0, 10.0)), 0.0, Stroke::new(1.5, yellow), StrokeKind::Middle);
    painter.text(at + egui::vec2(10.0, -10.0), egui::Align2::LEFT_BOTTOM, format!("{:?}", snap.kind), egui::FontId::proportional(12.0), yellow);
}
