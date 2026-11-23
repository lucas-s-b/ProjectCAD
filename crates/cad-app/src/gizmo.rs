//! Gizmo in the 3D view: three axis arrows at the centre of the selected solids (drag one to move along it), three
//! rings around a free component (drag one to turn it; Shift snaps to 15 degrees), or one handle for a component on a
//! revolute or slider joint (drag it to drive the joint).
use crate::app::CadApp;
use crate::assembly_joints::JointDrag;
use cad_document::{Command, Joint, JointId, JointKind};
use cad_geometry::{DAffine3, DVec3};
use eframe::egui::{self, Pos2};
use std::f64::consts::{PI, TAU};

pub const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];
const COLORS: [egui::Color32; 3] = [egui::Color32::from_rgb(230, 50, 50), egui::Color32::from_rgb(50, 200, 50), egui::Color32::from_rgb(80, 100, 255)];

const JOINT_COLOR: egui::Color32 = egui::Color32::from_rgb(240, 180, 40);

/// Turns snap to 15 degrees while Shift is held.
pub const SNAP: f64 = PI / 12.0;
/// Points per drawn ring; point 6 is at 45 degrees, point 18 at 135.
pub const RING_POINTS: usize = 48;
/// Start direction `u` of the ring about each of `AXES` (`v = axis × u`): Y for X, Z for Y, X for Z.
pub const RING_U: [DVec3; 3] = [DVec3::Y, DVec3::Z, DVec3::X];

/// What a gizmo drag changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GizmoKind {
    /// Along `AXES[axis]`: `start` and `now` are positions on the axis line (mm).
    Move,
    /// About `AXES[axis]` through the centre: `start` and `now` are ring angles (radians; `now` unwrapped).
    Turn,
    /// The selected component's parent joint from its value `v0` at the press (radians or mm): ring angles for a
    /// revolute joint, positions along the joint axis for a slider.
    Joint { joint: JointId, v0: f64 },
}

/// A drag in progress.
pub struct GizmoDrag {
    pub kind: GizmoKind,
    pub axis: usize,
    pub start: f64,
    pub now: f64,
    /// Shift held at the last pointer move.
    pub snap: bool,
}

impl GizmoDrag {
    /// The move of a `Move` drag; zero for the others.
    pub fn delta(&self) -> DVec3 {
        if self.kind == GizmoKind::Move { AXES[self.axis] * (self.now - self.start) } else { DVec3::ZERO }
    }

    /// The turn of the drag (radians), whole `SNAP` steps while `snap`.
    pub fn angle(&self) -> f64 {
        let a = self.now - self.start;
        if self.snap { (a / SNAP).round() * SNAP } else { a }
    }

    /// The rigid motion the selected component shows while dragging, about `centre`: the move, the turn, or the
    /// identity for a joint drag (its pose is the `JointDrag`).
    pub fn motion(&self, centre: DVec3) -> DAffine3 {
        match self.kind {
            GizmoKind::Move => DAffine3::from_translation(self.delta()),
            GizmoKind::Turn => DAffine3::from_translation(centre) * DAffine3::from_axis_angle(AXES[self.axis], self.angle()) * DAffine3::from_translation(-centre),
            GizmoKind::Joint { .. } => DAffine3::IDENTITY,
        }
    }
}

/// `a` reduced to [-π, π).
pub fn wrap_pi(a: f64) -> f64 {
    (a + PI).rem_euclid(TAU) - PI
}

/// The point at `angle` on the circle about `axis` through `c` with radius `r`, from the unit `u` (normal to `axis`)
/// counter-clockwise about `axis`.
pub fn ring_point(c: DVec3, axis: DVec3, u: DVec3, r: f64, angle: f64) -> DVec3 {
    c + (u * angle.cos() + axis.cross(u) * angle.sin()) * r
}

/// Angle about `axis` (from `u`, as `ring_point`) of the point where the ray meets the plane through `c` normal to
/// the unit `axis`; None when the ray is within about 6 degrees of the plane or meets it behind its origin.
// shortcut: an edge-on ring does not turn; map the screen motion along the ring's tangent if users drag rings edge-on
pub fn ring_angle(c: DVec3, axis: DVec3, u: DVec3, origin: DVec3, dir: DVec3) -> Option<f64> {
    let den = dir.dot(axis);
    if den.abs() < 0.1 {
        return None;
    }
    let t = (c - origin).dot(axis) / den;
    if t <= 0.0 {
        return None;
    }
    let p = origin + dir * t - c;
    Some(p.dot(axis.cross(u)).atan2(p.dot(u)))
}

/// Centre of the bounding box of the selection when it is non-empty and holds only solids (stored shapes, not the drag preview);
/// with no entity selected, the centre of the selected component's solid; none for a grounded or jointed component or while the exploded view is shown.
pub fn center(app: &CadApp) -> Option<DVec3> {
    if app.selection.is_empty() {
        let id = app.component_sel?;
        let free = app.explode_factor() == 0.0 && app.doc.component(id).is_some_and(|c| !c.grounded) && app.doc.parent_joint(id).is_none();
        if !free {
            return None;
        }
        let (lo, hi) = app.model_result().and_then(|r| r.component(id))?.bbox();
        return Some((lo + hi) / 2.0);
    }
    let mut bounds: Option<(DVec3, DVec3)> = None;
    for e in app.selection.iter().filter_map(|id| app.doc.get(*id)) {
        if !e.shape.is_solid() {
            return None;
        }
        let (lo, hi) = e.shape.bbox();
        bounds = Some(bounds.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
    }
    bounds.map(|(lo, hi)| (lo + hi) / 2.0)
}

/// Parameter `s` of the point on the line `c + axis * s` closest to the ray; None when line and ray are nearly parallel. `axis` and `dir` are unit.
pub fn axis_param(c: DVec3, axis: DVec3, origin: DVec3, dir: DVec3) -> Option<f64> {
    let (w, b) = (c - origin, axis.dot(dir));
    let den = 1.0 - b * b;
    (den > 1e-6).then(|| (b * dir.dot(w) - axis.dot(w)) / den)
}

/// The arrow (segment from `origin` to `tips[i]`) nearest to `p` within `tol` pixels.
pub fn hit(origin: Pos2, tips: [Pos2; 3], p: Pos2, tol: f32) -> Option<usize> {
    let ap = p - origin;
    let dist = |tip: Pos2| {
        let ab = tip - origin;
        let t = (ap.dot(ab) / ab.length_sq().max(1e-9)).clamp(0.0, 1.0);
        (ap - ab * t).length()
    };
    (0..3).map(|i| (i, dist(tips[i]))).filter(|&(_, d)| d <= tol).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i)
}

/// The closed polyline of `polys` nearest to `p` within `tol` pixels.
pub fn hit_ring(polys: &[Vec<Pos2>], p: Pos2, tol: f32) -> Option<usize> {
    let dist = |poly: &Vec<Pos2>| {
        (0..poly.len())
            .map(|k| {
                let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
                let (ab, ap) = (b - a, p - a);
                let t = (ap.dot(ab) / ab.length_sq().max(1e-9)).clamp(0.0, 1.0);
                (ap - ab * t).length()
            })
            .fold(f32::INFINITY, f32::min)
    };
    polys.iter().enumerate().map(|(i, poly)| (i, dist(poly))).filter(|&(_, d)| d <= tol).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i)
}

/// Screen points (origin, X tip, Y tip, Z tip) of the gizmo, moved by the drag in progress; arrows are 0.2 x the camera distance long.
/// None without a gizmo or when the centre is behind the camera.
pub fn screen(app: &CadApp, rect: egui::Rect) -> Option<[Pos2; 4]> {
    let c = center(app)? + app.gizmo.as_ref().map_or(DVec3::ZERO, GizmoDrag::delta);
    let len = 0.2 * f64::from(app.cam3d.distance);
    let project = |q: DVec3| crate::view3d::project(app, rect, q);
    Some([project(c)?, project(c + AXES[0] * len)?, project(c + AXES[1] * len)?, project(c + AXES[2] * len)?])
}

/// Screen points of the X, Y and Z rings (radius 0.12 x the camera distance) of a free component: only with no
/// entity selected and `center` known (it already excludes grounded, jointed and exploded components); moved with a
/// `Move` drag in progress. None when a point is behind the camera.
pub fn rings(app: &CadApp, rect: egui::Rect) -> Option<[Vec<Pos2>; 3]> {
    if !app.selection.is_empty() {
        return None;
    }
    let c = center(app)? + app.gizmo.as_ref().map_or(DVec3::ZERO, GizmoDrag::delta);
    let r = 0.12 * f64::from(app.cam3d.distance);
    let ring = |i: usize| -> Option<Vec<Pos2>> {
        (0..RING_POINTS).map(|k| crate::view3d::project(app, rect, ring_point(c, AXES[i], RING_U[i], r, TAU * k as f64 / RING_POINTS as f64))).collect()
    };
    Some([ring(0)?, ring(1)?, ring(2)?])
}

/// A jointed component's handle in world coordinates.
pub struct JointHandle {
    pub joint: Joint,
    /// Slider: the component's centre (the arrow starts there); revolute: the foot of that centre on the joint axis
    /// (the ring's centre).
    pub centre: DVec3,
    /// Unit joint axis in the world (`Document::joint_axis`).
    pub axis: DVec3,
    /// Revolute: the ring's start direction (towards the component's centre), unit, normal to `axis`.
    pub u: DVec3,
    /// Revolute: ring radius, the centre's distance from the axis, at least 0.12 x the camera distance.
    pub radius: f64,
}

/// The handle of the selected component when the gizmo drags its parent joint: no entity selected, not exploded,
/// the model current, the component's parent joint revolute or slider and not closing a loop (the editor's Drag
/// slider takes the same joints). None otherwise.
pub fn joint_handle(app: &CadApp) -> Option<JointHandle> {
    if !app.selection.is_empty() || app.explode_factor() != 0.0 || !app.model_current() {
        return None;
    }
    let id = app.component_sel?;
    let joint = app.doc.parent_joint(id).filter(|j| j.kind != JointKind::Fixed && !j.closes_loop)?.clone();
    let result = app.model_result()?;
    let (lo, hi) = result.component(id)?.bbox();
    let c = (lo + hi) / 2.0;
    let (o, axis) = app.doc.joint_axis(result, &joint).ok()?;
    let min_r = 0.12 * f64::from(app.cam3d.distance);
    if joint.kind == JointKind::Slider {
        return Some(JointHandle { joint, centre: c, axis, u: DVec3::ZERO, radius: 0.0 });
    }
    let foot = o + axis * (c - o).dot(axis);
    let off = c - foot;
    let u = if off.length() > 1e-6 { off.normalize() } else { axis.any_orthonormal_vector() };
    Some(JointHandle { joint, centre: foot, axis, u, radius: off.length().max(min_r) })
}

/// Screen points of `joint_handle`: (kind, start, points): a slider's arrow (start = the centre, points = [tip],
/// 0.2 x the camera distance along the axis) or a revolute ring (start = its centre, `RING_POINTS` points from `u`).
pub fn joint_screen(app: &CadApp, rect: egui::Rect) -> Option<(JointKind, Pos2, Vec<Pos2>)> {
    let h = joint_handle(app)?;
    let project = |q: DVec3| crate::view3d::project(app, rect, q);
    let start = project(h.centre)?;
    if h.joint.kind == JointKind::Slider {
        let tip = project(h.centre + h.axis * 0.2 * f64::from(app.cam3d.distance))?;
        return Some((JointKind::Slider, start, vec![tip]));
    }
    let ring = (0..RING_POINTS).map(|k| project(ring_point(h.centre, h.axis, h.u, h.radius, TAU * k as f64 / RING_POINTS as f64))).collect::<Option<Vec<_>>>()?;
    Some((JointKind::Revolute, start, ring))
}

/// Draws a jointed component's handle in `JOINT_COLOR`: a slider as one arrow (width 3, 5 while dragged, circle of
/// radius 4 at the tip), a revolute joint as a ring (width 2, 4 while dragged).
pub fn paint_joint(painter: &egui::Painter, kind: JointKind, start: Pos2, pts: &[Pos2], active: bool) {
    if kind == JointKind::Slider {
        painter.line_segment([start, pts[0]], egui::Stroke::new(if active { 5.0 } else { 3.0 }, JOINT_COLOR));
        painter.circle_filled(pts[0], 4.0, JOINT_COLOR);
    } else {
        painter.add(egui::Shape::closed_line(pts.to_vec(), egui::Stroke::new(if active { 4.0 } else { 2.0 }, JOINT_COLOR)));
    }
}

/// The joint parameter under the ray: the position along the axis (slider) or the ring angle (revolute).
fn joint_param(h: &JointHandle, origin: DVec3, dir: DVec3) -> Option<f64> {
    if h.joint.kind == JointKind::Slider { axis_param(h.centre, h.axis, origin, dir) } else { ring_angle(h.centre, h.axis, h.u, origin, dir) }
}

/// Draws the arrows (width 3, `COLORS`, width 5 for the dragged axis) and a filled circle of radius 4 at each tip.
pub fn paint(painter: &egui::Painter, pts: [Pos2; 4], active: Option<usize>) {
    for i in 0..3 {
        let width = if active == Some(i) { 5.0 } else { 3.0 };
        painter.line_segment([pts[0], pts[i + 1]], egui::Stroke::new(width, COLORS[i]));
        painter.circle_filled(pts[i + 1], 4.0, COLORS[i]);
    }
}

/// Draws the rings (width 2, `COLORS`, width 4 for the ring being dragged).
pub fn paint_rings(painter: &egui::Painter, rings: &[Vec<Pos2>; 3], active: Option<usize>) {
    for (i, ring) in rings.iter().enumerate() {
        let width = if active == Some(i) { 4.0 } else { 2.0 };
        painter.add(egui::Shape::closed_line(ring.clone(), egui::Stroke::new(width, COLORS[i])));
    }
}

/// Starts, updates and commits a gizmo drag with the primary button.
pub fn input(app: &mut CadApp, ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect) {
    let param = |app: &CadApp, axis: usize, pos: Pos2| {
        let (o, d) = crate::view3d::ray(app, rect, pos);
        axis_param(center(app)?, AXES[axis], o, d)
    };
    let angle = |app: &CadApp, axis: usize, pos: Pos2| {
        let (o, d) = crate::view3d::ray(app, rect, pos);
        ring_angle(center(app)?, AXES[axis], RING_U[axis], o, d)
    };
    let shift = ui.input(|i| i.modifiers.shift);
    if resp.drag_started_by(egui::PointerButton::Primary)
        && let Some(press) = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos())
    {
        // Arrows win over rings.
        if let Some(pts) = app.gizmo_screen
            && let Some(axis) = hit(pts[0], [pts[1], pts[2], pts[3]], press, 8.0)
            && let Some(s) = param(app, axis, press)
        {
            app.gizmo = Some(GizmoDrag { kind: GizmoKind::Move, axis, start: s, now: s, snap: false });
        } else if let Some(rings) = &app.gizmo_rings
            && let Some(axis) = hit_ring(rings, press, 8.0)
            && let Some(a) = angle(app, axis, press)
        {
            app.gizmo = Some(GizmoDrag { kind: GizmoKind::Turn, axis, start: a, now: a, snap: shift });
        } else if let Some((kind, start, pts)) = &app.gizmo_joint
            && let Some(h) = joint_handle(app)
            && if *kind == JointKind::Slider { hit(*start, [pts[0]; 3], press, 8.0).is_some() } else { hit_ring(std::slice::from_ref(pts), press, 8.0).is_some() }
            && let Some(s) = {
                let (o, d) = crate::view3d::ray(app, rect, press);
                joint_param(&h, o, d)
            }
            // The value now: the stored one, or the loops' solved one.
            && let Some(v0) = app.model_result().and_then(|r| app.doc.joint_value(r, &h.joint).ok())
        {
            app.gizmo = Some(GizmoDrag { kind: GizmoKind::Joint { joint: h.joint.id, v0 }, axis: 0, start: s, now: s, snap: shift });
            // `motion_panel::shown_moves` shows a drag of the selected joint.
            app.joint_sel = Some(h.joint.id);
            app.joint_edit = None;
        }
    }
    if resp.dragged_by(egui::PointerButton::Primary)
        && let Some((kind, axis)) = app.gizmo.as_ref().map(|g| (g.kind, g.axis))
        && let Some(pos) = resp.interact_pointer_pos()
    {
        if let Some(g) = app.gizmo.as_mut() {
            g.snap = shift;
        }
        match kind {
            GizmoKind::Move => {
                if let Some(s) = param(app, axis, pos)
                    && let Some(g) = app.gizmo.as_mut()
                {
                    g.now = s;
                }
            }
            GizmoKind::Turn => {
                if let Some(a) = angle(app, axis, pos)
                    && let Some(g) = app.gizmo.as_mut()
                {
                    g.now += wrap_pi(a - g.now);
                }
            }
            GizmoKind::Joint { joint, v0 } => {
                if let Some(h) = joint_handle(app).filter(|h| h.joint.id == joint) {
                    let (o, d) = crate::view3d::ray(app, rect, pos);
                    if let Some(s) = joint_param(&h, o, d)
                        && let Some(g) = app.gizmo.as_mut()
                    {
                        g.now = if h.joint.kind == JointKind::Slider { s } else { g.now + wrap_pi(s - g.now) };
                    }
                    // value = start value + ring angle (revolute; snapped with Shift) or + distance along the axis (slider).
                    let Some(g) = app.gizmo.as_ref() else { return };
                    let value = v0 + if h.joint.kind == JointKind::Revolute { g.angle() } else { g.now - g.start };
                    let Some(result) = app.model_result() else { return };
                    let mut guess = app.joint_drag.as_ref().filter(|d| d.joint == joint).map_or_else(|| result.joint_values.clone(), |d| d.guess.clone());
                    let moves = app.doc.pose(result, joint, value, &mut guess);
                    app.joint_drag = Some(JointDrag { joint, rev: app.doc.revision(), value, guess, moves });
                }
            }
        }
    }
    if resp.drag_stopped_by(egui::PointerButton::Primary)
        && let Some(g) = app.gizmo.take()
    {
        match g.kind {
            GizmoKind::Move if g.delta().length() > 1e-9 => commit_move(app, g.delta()),
            GizmoKind::Turn if g.angle().abs() > 1e-9 => {
                // The centre does not move during a turn.
                let turned = center(app).zip(app.component_sel.and_then(|id| app.doc.component(id))).map(|(c, comp)| app.doc.component_turned(comp, c, AXES[g.axis], g.angle()));
                match turned {
                    Some(Ok(c)) => {
                        app.component_edit = None;
                        app.exec(vec![Command::SetComponent(c)]);
                    }
                    Some(Err(err)) => app.status = err,
                    None => {}
                }
            }
            GizmoKind::Joint { v0, .. } => {
                // A drag that never moved writes nothing.
                if let Some(d) = app.joint_drag.take()
                    && (d.value - v0).abs() > 1e-12
                    && let Some(j) = app.doc.joint(d.joint).cloned()
                {
                    match d.moves {
                        Ok(_) => crate::assembly_joints::commit_joint_value(app, &j, d.value, &d.guess),
                        Err(err) => app.status = err,
                    }
                }
            }
            _ => {}
        }
    }
}

/// Writes a finished `Move` drag: the selected component's `at`, or the selected solids' shapes.
fn commit_move(app: &mut CadApp, delta: DVec3) {
    if app.selection.is_empty() {
        // The gizmo of the selected component (see `center`).
        let moved = app.component_sel.and_then(|id| app.doc.component(id)).map(|c| app.doc.component_moved(c, delta));
        match moved {
            Some(Ok(c)) => {
                app.component_edit = None;
                app.exec(vec![Command::SetComponent(c)]);
            }
            Some(Err(err)) => app.status = err,
            None => {}
        }
        return;
    }
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

#[cfg(test)]
#[path = "gizmo_tests.rs"]
mod tests;
