use cad_geometry::{DVec2, DVec3};
use eframe::egui;
use glam::{Mat4, Vec2, Vec3};

const FOV: f32 = std::f32::consts::FRAC_PI_4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera2D {
    pub center: DVec2,
    /// Pixels per millimetre.
    pub zoom: f64,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self { center: DVec2::ZERO, zoom: 4.0 }
    }
}

impl Camera2D {
    pub fn world_to_screen(&self, rect: egui::Rect, p: DVec2) -> egui::Pos2 {
        let c = rect.center();
        egui::pos2(
            c.x + ((p.x - self.center.x) * self.zoom) as f32,
            c.y - ((p.y - self.center.y) * self.zoom) as f32,
        )
    }

    pub fn screen_to_world(&self, rect: egui::Rect, p: egui::Pos2) -> DVec2 {
        let c = rect.center();
        DVec2::new(
            self.center.x + f64::from(p.x - c.x) / self.zoom,
            self.center.y - f64::from(p.y - c.y) / self.zoom,
        )
    }

    pub fn zoom_at(&mut self, rect: egui::Rect, cursor: egui::Pos2, factor: f64) {
        let world = self.screen_to_world(rect, cursor);
        self.zoom = (self.zoom * factor).clamp(0.001, 10000.0);
        let c = rect.center();
        self.center = DVec2::new(
            world.x - f64::from(cursor.x - c.x) / self.zoom,
            world.y + f64::from(cursor.y - c.y) / self.zoom,
        );
    }

    pub fn fit(&mut self, rect: egui::Rect, min: DVec2, max: DVec2) {
        self.center = (min + max) / 2.0;
        let size = (max - min).max(DVec2::splat(1e-6)) * 1.1;
        let zoom = (f64::from(rect.width()) / size.x).min(f64::from(rect.height()) / size.y);
        self.zoom = zoom.clamp(0.001, 10000.0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    /// Orthographic projection; the visible height at the target is the perspective one, so toggling keeps the size.
    pub ortho: bool,
}

/// A standard view direction of the 3D camera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StdView {
    Front,
    Back,
    Right,
    Left,
    Top,
    Bottom,
    Iso,
}

impl StdView {
    /// (yaw, pitch) in degrees; the eye sits at yaw/pitch from the target (front: on -Y, looking along +Y).
    pub fn angles(self) -> (f32, f32) {
        match self {
            StdView::Front => (-90.0, 0.0),
            StdView::Back => (90.0, 0.0),
            StdView::Right => (0.0, 0.0),
            StdView::Left => (180.0, 0.0),
            StdView::Top => (-90.0, 89.9),
            StdView::Bottom => (-90.0, -89.9),
            StdView::Iso => (-45.0, 35.264),
        }
    }
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self { target: Vec3::ZERO, yaw: -60f32.to_radians(), pitch: 30f32.to_radians(), distance: 150.0, ortho: false }
    }
}

impl OrbitCamera {
    pub fn eye(&self) -> Vec3 {
        self.target
            + self.distance
                * Vec3::new(
                    self.pitch.cos() * self.yaw.cos(),
                    self.pitch.cos() * self.yaw.sin(),
                    self.pitch.sin(),
                )
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let proj = if self.ortho {
            let h = self.distance * (FOV / 2.0).tan();
            // shortcut: the near plane sits a camera length behind the eye, so pick tolerances (scaled by the ray
            // length) are about 2x looser in ortho; pass a pixel size from the camera if that bites
            glam::camera::rh::proj::directx::orthographic(-h * aspect, h * aspect, -h, h, -self.distance, self.distance * 100.0)
        } else {
            glam::camera::rh::proj::directx::perspective(FOV, aspect, self.distance * 0.01, self.distance * 100.0)
        };
        proj * glam::camera::rh::view::look_at_mat4(self.eye(), self.target, Vec3::Z)
    }

    /// Turns the camera to a standard view, keeping target and distance.
    pub fn set_view(&mut self, v: StdView) {
        let (yaw, pitch) = v.angles();
        self.yaw = yaw.to_radians();
        self.pitch = pitch.to_radians();
    }

    /// World-space ray (origin on the near plane, unit direction) through an NDC point.
    pub fn ray(&self, aspect: f32, ndc: Vec2) -> (DVec3, DVec3) {
        let inv = self.view_proj(aspect).inverse();
        let near = inv.project_point3(ndc.extend(0.0));
        let far = inv.project_point3(ndc.extend(1.0));
        (near.as_dvec3(), (far - near).as_dvec3().normalize())
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        let limit = 89f32.to_radians();
        self.yaw -= dx * 0.01;
        self.pitch = (self.pitch + dy * 0.01).clamp(-limit, limit);
    }

    pub fn pan(&mut self, dx: f32, dy: f32, viewport_h: f32) {
        let forward = (self.target - self.eye()).normalize();
        let right = forward.cross(Vec3::Z).normalize();
        let up = right.cross(forward);
        let per_pixel = 2.0 * self.distance * (FOV / 2.0).tan() / viewport_h.max(1.0);
        self.target += (-right * dx + up * dy) * per_pixel;
    }

    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(0.1, 1e6);
    }

    pub fn fit(&mut self, min: DVec3, max: DVec3) {
        self.target = ((min + max) / 2.0).as_vec3();
        self.distance = (1.5 * (max - min).length() as f32).max(10.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(800.0, 600.0))
    }

    #[test]
    fn camera2d_round_trip() {
        let cam = Camera2D { center: DVec2::new(12.0, -7.0), zoom: 3.5 };
        let p = DVec2::new(-30.25, 44.5);
        let back = cam.screen_to_world(rect(), cam.world_to_screen(rect(), p));
        assert!(back.distance(p) < 1e-3);
        assert!(cam.world_to_screen(rect(), cam.center).distance(rect().center()) < 1e-4);
    }

    #[test]
    fn camera2d_zoom_keeps_cursor_point() {
        let mut cam = Camera2D::default();
        let cursor = egui::pos2(250.0, 420.0);
        let before = cam.screen_to_world(rect(), cursor);
        cam.zoom_at(rect(), cursor, 2.5);
        assert!((cam.zoom - 10.0).abs() < 1e-9);
        assert!(cam.screen_to_world(rect(), cursor).distance(before) < 1e-6);
    }

    #[test]
    fn orbit_ray_points_at_target() {
        let cam = OrbitCamera::default();
        let (origin, dir) = cam.ray(1.5, Vec2::ZERO);
        let expected = (cam.target - cam.eye()).normalize().as_dvec3();
        assert!(dir.distance(expected) < 1e-4);
        let to_target = (cam.target.as_dvec3() - origin).normalize();
        assert!(to_target.distance(expected) < 1e-4);
    }

    #[test]
    fn ortho_keeps_target_centred_and_rays_parallel() {
        let cam = OrbitCamera { ortho: true, ..OrbitCamera::default() };
        let p = cam.view_proj(1.5).project_point3(cam.target);
        assert!(p.x.abs() < 1e-5 && p.y.abs() < 1e-5 && p.z > 0.0 && p.z < 1.0);
        let (_, d0) = cam.ray(1.5, Vec2::ZERO);
        let (_, d1) = cam.ray(1.5, Vec2::new(0.8, -0.6));
        assert!(d0.distance(d1) < 1e-4, "ortho rays are parallel");
    }

    #[test]
    fn standard_views_look_along_the_axes() {
        let mut cam = OrbitCamera::default();
        cam.set_view(StdView::Front);
        let d = (cam.target - cam.eye()).normalize();
        assert!(d.distance(Vec3::Y) < 1e-4, "front looks along +Y: {d}");
        cam.set_view(StdView::Top);
        assert!((cam.target - cam.eye()).normalize().z < -0.99);
    }

    #[test]
    fn orbit_view_proj_maps_target_to_center() {
        let cam = OrbitCamera::default();
        let p = cam.view_proj(1.5).project_point3(cam.target);
        assert!(p.x.abs() < 1e-5 && p.y.abs() < 1e-5);
        assert!(p.z > 0.0 && p.z < 1.0);
    }
}
