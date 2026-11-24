//! Mouse navigation of the 2D and 3D views by the gestures of the control settings, fly keys, and camera glides:
//! fit, focus and the standard views ease to the new camera over a short time instead of jumping. A glide only asks
//! for frames while it runs, so an idle window stays idle.
use crate::app::CadApp;
use crate::camera::{Camera2D, OrbitCamera};
use eframe::egui::{self, Key};
use std::time::Instant;

/// A camera glide in progress: both cameras move from `from` to `to`.
pub struct Glide {
    pub from: (Camera2D, OrbitCamera),
    pub to: (Camera2D, OrbitCamera),
    pub start: Instant,
    pub secs: f32,
}

/// Ease-out cubic of `t` in [0, 1].
pub fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// The cameras at `e` (eased, 0..1) of the way from `a` to `b`: centres and targets linear, zoom and distance
/// geometric, yaw the short way round.
pub fn lerp(a: &(Camera2D, OrbitCamera), b: &(Camera2D, OrbitCamera), e: f32) -> (Camera2D, OrbitCamera) {
    let ed = f64::from(e);
    let c2 = Camera2D { center: a.0.center.lerp(b.0.center, ed), zoom: a.0.zoom * (b.0.zoom / a.0.zoom).powf(ed) };
    let dyaw = crate::gizmo::wrap_pi(f64::from(b.1.yaw - a.1.yaw)) as f32;
    let c3 = OrbitCamera {
        target: a.1.target.lerp(b.1.target, e),
        yaw: a.1.yaw + dyaw * e,
        pitch: a.1.pitch + (b.1.pitch - a.1.pitch) * e,
        distance: a.1.distance * (b.1.distance / a.1.distance).powf(e),
        ortho: b.1.ortho,
    };
    (c2, c3)
}

/// Runs `f`, which moves the cameras, and turns the jump into a glide when camera transitions are on.
pub fn glide(app: &mut CadApp, f: impl FnOnce(&mut CadApp)) {
    let from = (app.cam2d, app.cam3d);
    f(app);
    let ui = app.settings.ui;
    if !(ui.animations && ui.camera_transitions && ui.camera_ms > 0) {
        return;
    }
    let to = (app.cam2d, app.cam3d);
    (app.cam2d, app.cam3d) = (from.0, OrbitCamera { ortho: to.1.ortho, ..from.1 });
    app.glide = Some(Glide { from, to, start: Instant::now(), secs: ui.camera_ms as f32 / 1000.0 });
}

/// Advances the glide; asks for the next frame while it runs.
pub fn tick(app: &mut CadApp, ctx: &egui::Context) {
    let Some(g) = &app.glide else { return };
    let t = g.start.elapsed().as_secs_f32() / g.secs;
    if t >= 1.0 {
        (app.cam2d, app.cam3d) = g.to;
        app.glide = None;
        return;
    }
    (app.cam2d, app.cam3d) = lerp(&g.from, &g.to, ease(t));
    ctx.request_repaint();
}

/// Pan and wheel zoom of the 2D view: the pan gesture or the orbit gesture (2D has no orbit) drags the sheet.
pub fn input_2d(app: &mut CadApp, ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect) {
    let nav = app.settings.controls.nav;
    let m = ui.input(|i| i.modifiers);
    if nav.pan.active(resp, m) || nav.orbit.active(resp, m) {
        app.glide = None;
        let d = resp.drag_delta() * nav.pan_speed;
        let zoom = app.cam2d.zoom;
        app.cam2d.center.x -= f64::from(d.x) / zoom;
        app.cam2d.center.y += f64::from(d.y) / zoom;
    }
    if nav.zoom.active(resp, m)
        && let Some(p) = ui.input(|i| i.pointer.press_origin())
    {
        app.glide = None;
        app.cam2d.zoom_at(rect, p, f64::from((-resp.drag_delta().y * 0.01 * nav.zoom_speed).exp()));
    }
    if resp.hovered()
        && let Some(cursor) = resp.hover_pos()
    {
        let scroll = wheel(ui, nav);
        if scroll != 0.0 {
            app.glide = None;
            app.cam2d.zoom_at(rect, cursor, f64::from(scroll * 0.002).exp());
        }
    }
}

/// Orbit, pan, drag zoom, wheel zoom and fly keys of the 3D view.
pub fn input_3d(app: &mut CadApp, ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect) {
    let nav = app.settings.controls.nav;
    let m = ui.input(|i| i.modifiers);
    if nav.orbit.active(resp, m) {
        app.glide = None;
        let d = resp.drag_delta() * nav.orbit_speed;
        app.cam3d.orbit(d.x, if nav.invert_orbit_y { -d.y } else { d.y });
    }
    if nav.pan.active(resp, m) {
        app.glide = None;
        let d = resp.drag_delta() * nav.pan_speed;
        app.cam3d.pan(d.x, d.y, rect.height());
    }
    if nav.zoom.active(resp, m) {
        app.glide = None;
        app.cam3d.zoom((resp.drag_delta().y * 0.01 * nav.zoom_speed).exp());
    }
    if resp.hovered() {
        let scroll = wheel(ui, nav);
        if scroll != 0.0 {
            app.glide = None;
            app.cam3d.zoom((-scroll * 0.002).exp());
        }
    }
    if flying(app, ui.ctx()) {
        fly(app, ui);
    }
}

/// The wheel delta with the zoom speed and direction settings applied.
fn wheel(ui: &egui::Ui, nav: crate::keymap::Navigation) -> f32 {
    let s = ui.input(|i| i.smooth_scroll_delta().y) * nav.zoom_speed;
    if nav.invert_zoom { -s } else { s }
}

/// Fly keys are on, the 3D view is shown and the orbit button is held over it: W A S D Q E move the camera, so the
/// shortcuts leave those keys alone.
pub fn flying(app: &CadApp, ctx: &egui::Context) -> bool {
    let nav = app.settings.controls.nav;
    nav.fly
        && app.view == crate::app::ViewMode::ThreeD
        && ctx.input(|i| nav.orbit.button_down(i) && i.pointer.press_origin().is_some_and(|p| app.view_rect.contains(p)))
}

/// One frame of flight: forward and back along the view, sideways, down and up; a camera length per second, three
/// times that with Shift.
fn fly(app: &mut CadApp, ui: &egui::Ui) {
    let (dir, dt, fast) = ui.input(|i| {
        let k = |key: Key| if i.key_down(key) { 1.0f32 } else { 0.0 };
        (glam::Vec3::new(k(Key::D) - k(Key::A), k(Key::W) - k(Key::S), k(Key::E) - k(Key::Q)), i.stable_dt.min(0.1), i.modifiers.shift)
    });
    if dir == glam::Vec3::ZERO {
        return;
    }
    app.glide = None;
    let cam = &mut app.cam3d;
    let forward = (cam.target - cam.eye()).normalize();
    let right = forward.cross(glam::Vec3::Z).normalize_or_zero();
    let step = cam.distance * dt * if fast { 3.0 } else { 1.0 };
    cam.target += (right * dir.x + forward * dir.y + glam::Vec3::Z * dir.z) * step;
    ui.ctx().request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;
    use cad_geometry::DVec2;

    #[test]
    fn ease_runs_from_zero_to_one() {
        assert_eq!(ease(0.0), 0.0);
        assert_eq!(ease(1.0), 1.0);
        assert_eq!(ease(2.0), 1.0);
        assert!(ease(0.5) > 0.5, "ease-out is ahead of linear");
    }

    #[test]
    fn lerp_takes_the_short_way_and_keeps_zoom_geometric() {
        let a = (Camera2D { center: DVec2::ZERO, zoom: 1.0 }, OrbitCamera { yaw: 3.0, distance: 10.0, ..Default::default() });
        let b = (Camera2D { center: DVec2::new(10.0, 0.0), zoom: 100.0 }, OrbitCamera { yaw: -3.0, distance: 1000.0, ortho: true, ..Default::default() });
        let (c2, c3) = lerp(&a, &b, 0.5);
        assert!((c2.zoom - 10.0).abs() < 1e-9);
        assert!((c2.center.x - 5.0).abs() < 1e-9);
        assert!((c3.distance - 100.0).abs() < 1e-3);
        // From 3 to -3 rad the short way passes through pi, not 0.
        assert!(c3.yaw.abs() > 3.0, "yaw {}", c3.yaw);
        assert!(c3.ortho);
        let (e2, e3) = lerp(&a, &b, 1.0);
        assert!((e2.zoom - 100.0).abs() < 1e-9 && (e3.distance - 1000.0).abs() < 1e-2);
    }
}
