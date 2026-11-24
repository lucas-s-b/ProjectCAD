//! Modal transforms in the Blender manner: Grab, Rotate or Scale (G, R, S by default) moves the selection with the
//! mouse, no button held. X, Y or Z locks an axis (again: unlocks; Shift+axis in 3D: moves in the plane normal to
//! it), typed digits give an exact value (mm, degrees or a factor), Ctrl snaps, Enter or a left click confirms, Esc or
//! a right click cancels. Sketch entities transform in the 2D view; solids (grab only) and free components (grab and
//! rotate) in the 3D view.
use crate::app::{CadApp, ViewMode};
use crate::gizmo::{AXES, axis_param, wrap_pi};
use cad_document::{Command, ComponentId};
use cad_geometry::xform::{rotation_about, scale_about};
use cad_geometry::{DAffine2, DAffine3, DVec2, DVec3, Shape};
use eframe::egui::{self, Color32, Key, Pos2, Stroke};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Grab,
    Rotate,
    Scale,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Grab => "Grab",
            Mode::Rotate => "Rotate",
            Mode::Scale => "Scale",
        }
    }
}

/// What the modal moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// The selected entities of the active sketch, in sketch coordinates (2D view).
    Sketch,
    /// The selected solids, in world coordinates (3D view).
    Solids,
    /// A free component (3D view, nothing selected).
    Component(ComponentId),
}

/// The motion the modal makes now, about `Modal::pivot`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    Move(DVec3),
    /// About `AXES[axis]` (sketch entities always turn about Z), radians.
    Turn { axis: usize, angle: f64 },
    Scale(f64),
}

/// A modal transform in progress.
#[derive(Clone, Debug)]
pub struct Modal {
    pub mode: Mode,
    pub target: Target,
    pub axis: Option<usize>,
    /// Shift+axis: move in the plane normal to `axis` (3D grab).
    pub plane: bool,
    /// Typed value text.
    pub typed: String,
    /// Pointer at the start and now, screen points.
    pub start: Pos2,
    pub now: Pos2,
    /// Turn so far, unwrapped (radians), and the pointer angle of the last frame (None before the first).
    pub turned: f64,
    pub last_angle: Option<f64>,
    /// The view at the start.
    pub rect: egui::Rect,
    /// Sketch coordinates (z = 0) for `Sketch`, world otherwise.
    pub pivot: DVec3,
    pub snap: bool,
    pub motion: Motion,
}

impl Modal {
    /// The typed value; None while empty or not a number yet ("-", ".").
    pub fn value(&self) -> Option<f64> {
        self.typed.parse::<f64>().ok()
    }
}

/// Starts `mode` on what the active view has selected; a reason in the status line when there is nothing to move.
pub fn start(app: &mut CadApp, ctx: &egui::Context, mode: Mode) {
    let target = match app.view {
        ViewMode::TwoD if !app.selection.is_empty() => Some((Target::Sketch, app.selection_center().map(|c| c.extend(0.0)))),
        ViewMode::TwoD => None,
        ViewMode::ThreeD if !app.selection.is_empty() => match crate::gizmo::center(app) {
            Some(c) => Some((Target::Solids, Some(c))),
            None => {
                app.status = "Transform sketch shapes in the 2D view".into();
                return;
            }
        },
        ViewMode::ThreeD => app.component_sel.zip(crate::gizmo::center(app)).map(|(id, c)| (Target::Component(id), Some(c))),
    };
    let Some((target, Some(pivot))) = target else {
        app.status = "Nothing selected to transform".into();
        return;
    };
    if !allowed(mode, target) {
        app.status = if target == Target::Solids { "Solids can only be grabbed" } else { "Components cannot be scaled" }.into();
        return;
    }
    let p = ctx.input(|i| i.pointer.latest_pos()).filter(|p| app.view_rect.contains(*p)).unwrap_or(app.view_rect.center());
    let identity = if mode == Mode::Scale { Motion::Scale(1.0) } else { Motion::Move(DVec3::ZERO) };
    let rect = app.view_rect;
    app.modal = Some(Modal { mode, target, axis: None, plane: false, typed: String::new(), start: p, now: p, turned: 0.0, last_angle: None, rect, pivot, snap: false, motion: identity });
}

/// The modes a target allows: solids only grab, components grab and rotate.
pub fn allowed(mode: Mode, target: Target) -> bool {
    !matches!((mode, target), (Mode::Rotate | Mode::Scale, Target::Solids) | (Mode::Scale, Target::Component(_)))
}

/// One frame of the modal: reads keys and pointer, updates the motion, confirms or cancels. True while a modal was
/// running this frame (the view then skips its own input).
pub fn update(app: &mut CadApp, ui: &egui::Ui, rect: egui::Rect) -> bool {
    let Some(mut m) = app.modal.take() else { return false };
    let mut confirm = false;
    let mut cancel = false;
    let mut restart = None;
    ui.ctx().input_mut(|i| {
        if let Some(p) = i.pointer.latest_pos() {
            m.now = p;
        }
        m.snap = i.modifiers.command;
        if i.pointer.primary_released() {
            confirm = true;
        }
        if i.pointer.secondary_pressed() {
            cancel = true;
        }
        i.events.retain(|ev| match ev {
            egui::Event::Key { key, pressed: true, modifiers, .. } => {
                match key {
                    Key::X | Key::Y | Key::Z => {
                        let axis = *key as usize - Key::X as usize;
                        let plane = modifiers.shift && app.view == ViewMode::ThreeD && m.mode == Mode::Grab;
                        if m.axis == Some(axis) && m.plane == plane {
                            m.axis = None;
                            m.plane = false;
                        } else {
                            m.axis = Some(axis);
                            m.plane = plane;
                        }
                    }
                    Key::Enter => confirm = true,
                    Key::Escape => cancel = true,
                    Key::Backspace => {
                        m.typed.pop();
                    }
                    Key::Minus => toggle_sign(&mut m.typed),
                    Key::G => restart = Some(Mode::Grab),
                    Key::R => restart = Some(Mode::Rotate),
                    Key::S => restart = Some(Mode::Scale),
                    _ => return true,
                }
                false
            }
            egui::Event::Text(t) => {
                m.typed.extend(t.chars().filter(|c| c.is_ascii_digit() || *c == '.'));
                false
            }
            _ => true,
        });
    });
    if let Some(mode) = restart.filter(|r| *r != m.mode && allowed(*r, m.target)) {
        // A new mode starts from the current pointer, as a fresh modal would.
        start(app, ui.ctx(), mode);
        return true;
    }
    let rect = if rect.width() > 0.0 { rect } else { m.rect };
    step(app, &mut m, rect);
    if cancel {
        app.status = format!("{} cancelled", m.mode.name());
    } else if confirm {
        commit(app, &m);
    } else {
        app.modal = Some(m);
    }
    true
}

/// '-' flips the sign of the typed value.
fn toggle_sign(typed: &mut String) {
    if let Some(rest) = typed.strip_prefix('-') {
        *typed = rest.to_string();
    } else {
        typed.insert(0, '-');
    }
}

/// Screen point of the pivot.
fn pivot_screen(app: &CadApp, m: &Modal, rect: egui::Rect) -> Option<Pos2> {
    match m.target {
        Target::Sketch => Some(app.cam2d.world_to_screen(rect, m.pivot.truncate())),
        _ => crate::view3d::project(app, rect, m.pivot),
    }
}

/// Angle of the screen point `p` about `c`, counter-clockwise on screen (y up), radians.
pub fn screen_angle(c: Pos2, p: Pos2) -> f64 {
    f64::from(c.y - p.y).atan2(f64::from(p.x - c.x))
}

/// The grid step of a 3D snap: a power of ten about a tenth of the camera distance.
fn snap_step_3d(distance: f32) -> f64 {
    10f64.powf((f64::from(distance) / 10.0).max(1e-6).log10().floor())
}

/// Where the ray meets the plane through `c` normal to `n`; None when nearly parallel.
fn plane_hit(c: DVec3, n: DVec3, origin: DVec3, dir: DVec3) -> Option<DVec3> {
    let den = dir.dot(n);
    (den.abs() > 1e-6).then(|| origin + dir * ((c - origin).dot(n) / den))
}

/// Updates `m.turned` and `m.motion` from the pointer and the typed value.
fn step(app: &CadApp, m: &mut Modal, rect: egui::Rect) {
    let value = m.value();
    m.motion = match m.mode {
        Mode::Grab => Motion::Move(grab(app, m, rect, value)),
        Mode::Rotate => {
            let ps = pivot_screen(app, m, rect).unwrap_or(rect.center());
            let a = screen_angle(ps, m.now);
            let last = m.last_angle.unwrap_or_else(|| screen_angle(ps, m.start));
            m.turned += wrap_pi(a - last);
            m.last_angle = Some(a);
            let axis = if m.target == Target::Sketch { 2 } else { m.axis.unwrap_or(2) };
            // Counter-clockwise on screen is positive when the axis points at the viewer.
            let sign = if m.target == Target::Sketch || AXES[axis].dot(app.cam3d.eye().as_dvec3() - m.pivot) >= 0.0 { 1.0 } else { -1.0 };
            let angle = match value {
                Some(deg) => deg.to_radians(),
                None if m.snap => (m.turned * sign / crate::gizmo::SNAP).round() * crate::gizmo::SNAP,
                None => m.turned * sign,
            };
            Motion::Turn { axis, angle }
        }
        Mode::Scale => {
            let ps = pivot_screen(app, m, rect).unwrap_or(rect.center());
            let f = f64::from(m.now.distance(ps)) / f64::from(m.start.distance(ps).max(1.0));
            Motion::Scale(match value {
                Some(v) => v,
                None if m.snap => (f * 10.0).round() / 10.0,
                None => f,
            })
        }
    };
}

/// The move of a grab: the pointer's travel in the sketch (2D) or on the axis, plane or view plane through the pivot
/// (3D), snapped with Ctrl; a typed value moves that far along the locked axis (X when none).
fn grab(app: &CadApp, m: &Modal, rect: egui::Rect, value: Option<f64>) -> DVec3 {
    let along = |axis: usize, v: f64| AXES[axis] * v;
    if m.target == Target::Sketch {
        let axis = m.axis.filter(|a| *a < 2);
        if let Some(v) = value {
            return along(axis.unwrap_or(0), v);
        }
        let mut d = app.cam2d.screen_to_world(rect, m.now) - app.cam2d.screen_to_world(rect, m.start);
        if m.snap {
            let s = crate::view2d::grid_step(app.cam2d.zoom);
            d = (d / s).round() * s;
        }
        return match axis {
            Some(0) => DVec3::new(d.x, 0.0, 0.0),
            Some(_) => DVec3::new(0.0, d.y, 0.0),
            None => d.extend(0.0),
        };
    }
    if let Some(v) = value.filter(|_| !m.plane) {
        return along(m.axis.unwrap_or(0), v);
    }
    let (o0, d0) = crate::view3d::ray(app, rect, m.start);
    let (o1, d1) = crate::view3d::ray(app, rect, m.now);
    let mut d = match (m.axis, m.plane) {
        (Some(a), false) => match (axis_param(m.pivot, AXES[a], o0, d0), axis_param(m.pivot, AXES[a], o1, d1)) {
            (Some(s0), Some(s1)) => AXES[a] * (s1 - s0),
            _ => DVec3::ZERO,
        },
        (axis, _) => {
            let n = axis.map_or_else(|| (app.cam3d.eye() - app.cam3d.target).as_dvec3().normalize(), |a| AXES[a]);
            match (plane_hit(m.pivot, n, o0, d0), plane_hit(m.pivot, n, o1, d1)) {
                (Some(p0), Some(p1)) => p1 - p0,
                _ => DVec3::ZERO,
            }
        }
    };
    if m.snap {
        let s = snap_step_3d(app.cam3d.distance);
        d = (d / s).round() * s;
    }
    d
}

/// The 2D transform of a sketch motion about the pivot.
fn affine_2d(pivot: DVec2, motion: Motion) -> DAffine2 {
    match motion {
        Motion::Move(d) => DAffine2::from_translation(d.truncate()),
        Motion::Turn { angle, .. } => rotation_about(pivot, angle),
        Motion::Scale(f) => scale_about(pivot, f),
    }
}

/// `shape` as the running modal shows it, when the modal moves entities; else unchanged.
pub fn preview(app: &CadApp, shape: &Shape) -> Shape {
    let Some(m) = &app.modal else { return shape.clone() };
    match m.motion {
        Motion::Move(d) => {
            let mut s = shape.clone();
            s.translate(d);
            s
        }
        motion => shape.transformed(&affine_2d(m.pivot.truncate(), motion)).unwrap_or_else(|_| shape.clone()),
    }
}

/// The rigid motion of a component modal, for the 3D preview.
pub fn component_motion(app: &CadApp, id: ComponentId) -> Option<DAffine3> {
    let m = app.modal.as_ref().filter(|m| m.target == Target::Component(id))?;
    Some(match m.motion {
        Motion::Move(d) => DAffine3::from_translation(d),
        Motion::Turn { axis, angle } => DAffine3::from_translation(m.pivot) * DAffine3::from_axis_angle(AXES[axis], angle) * DAffine3::from_translation(-m.pivot),
        Motion::Scale(_) => DAffine3::IDENTITY,
    })
}

/// Writes the modal's motion as one undo step.
fn commit(app: &mut CadApp, m: &Modal) {
    match (m.target, m.motion) {
        (_, Motion::Move(d)) if d.length() < 1e-12 => {}
        (_, Motion::Turn { angle, .. }) if angle.abs() < 1e-12 => {}
        (_, Motion::Scale(f)) if (f - 1.0).abs() < 1e-12 => {}
        (_, Motion::Scale(f)) if f.abs() < 1e-9 => app.status = "Scale factor 0 is not allowed".into(),
        (Target::Sketch | Target::Solids, Motion::Move(d)) => {
            let cmds: Vec<Command> = app
                .selection
                .iter()
                .filter_map(|id| app.doc.get(*id))
                .map(|e| {
                    let mut shape = e.shape.clone();
                    shape.translate(d);
                    Command::SetShape { id: e.id, shape }
                })
                .collect();
            let pinned = app.selection.clone();
            app.exec_pinned(cmds, &pinned);
        }
        (Target::Sketch | Target::Solids, motion) => app.transform_selection(affine_2d(m.pivot.truncate(), motion)),
        (Target::Component(id), motion) => {
            let Some(comp) = app.doc.component(id) else { return };
            let changed = match motion {
                Motion::Move(d) => app.doc.component_moved(comp, d),
                Motion::Turn { axis, angle } => app.doc.component_turned(comp, m.pivot, AXES[axis], angle),
                Motion::Scale(_) => return,
            };
            match changed {
                Ok(c) => {
                    app.component_edit = None;
                    app.exec(vec![Command::SetComponent(c)]);
                }
                Err(err) => app.status = err,
            }
        }
    }
}

/// The header line of a running modal: mode, value and the keys.
pub fn header(app: &CadApp, m: &Modal) -> String {
    let s = app.doc.settings();
    let axis = match (m.axis, m.plane) {
        (Some(a), false) => format!(" along {}", ["X", "Y", "Z"][a]),
        (Some(a), true) => format!(" in plane ⟂ {}", ["X", "Y", "Z"][a]),
        (None, _) => String::new(),
    };
    let typed = if m.typed.is_empty() { String::new() } else { format!("  [{}]", m.typed) };
    let value = match m.motion {
        Motion::Move(d) => format!("ΔX {}  ΔY {}  ΔZ {} {}", s.format_length(d.x), s.format_length(d.y), s.format_length(d.z), s.units.suffix()),
        Motion::Turn { angle, .. } => format!("{:.2}°", angle.to_degrees()),
        Motion::Scale(f) => format!("× {f:.3}"),
    };
    format!("{}{axis}: {value}{typed}    X/Y/Z axis · type a value · Ctrl snap · Enter/click confirm · Esc/right-click cancel", m.mode.name())
}

/// Draws the header and guides: the locked axis through the pivot, and a dashed line from pivot to pointer for
/// rotate and scale.
pub fn paint(app: &CadApp, painter: &egui::Painter, rect: egui::Rect) {
    let Some(m) = &app.modal else { return };
    const COLORS: [Color32; 3] = [Color32::from_rgb(230, 70, 70), Color32::from_rgb(80, 200, 80), Color32::from_rgb(90, 120, 255)];
    if let Some(a) = m.axis
        && !m.plane
    {
        let far = match m.target {
            Target::Sketch => 1e5 / app.cam2d.zoom.max(1e-9),
            _ => f64::from(app.cam3d.distance) * 50.0,
        };
        let ends = [m.pivot - AXES[a] * far, m.pivot + AXES[a] * far];
        let pts: Option<Vec<Pos2>> = match m.target {
            Target::Sketch => Some(ends.iter().map(|p| app.cam2d.world_to_screen(rect, p.truncate())).collect()),
            _ => ends.iter().map(|p| crate::view3d::project(app, rect, *p)).collect(),
        };
        if let Some(pts) = pts {
            painter.line_segment([pts[0], pts[1]], Stroke::new(1.0, COLORS[a]));
        }
    }
    if m.mode != Mode::Grab
        && let Some(ps) = pivot_screen(app, m, rect)
    {
        painter.extend(egui::Shape::dashed_line(&[ps, m.now], Stroke::new(1.0, Color32::from_gray(200)), 4.0, 3.0));
        painter.circle_stroke(ps, 3.0, Stroke::new(1.0, Color32::from_gray(220)));
    }
    let text = header(app, m);
    let galley = painter.layout_no_wrap(text, egui::FontId::proportional(13.0), Color32::from_gray(235));
    let at = egui::pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 8.0);
    painter.rect_filled(egui::Rect::from_min_size(at, galley.size()).expand2(egui::vec2(8.0, 4.0)), 3.0, Color32::from_black_alpha(170));
    painter.galley(at, galley, Color32::from_gray(235));
}

#[cfg(test)]
#[path = "transform_tests.rs"]
mod tests;
