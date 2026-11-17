use crate::app::{CadApp, PickFace};
use cad_document::EntityId;
use cad_geometry::{DVec2, DVec3, distance_2d, outline, ray_hit, tessellate};
use cad_renderer::{SceneRenderer, Vertex};
use eframe::egui::{self, PointerButton};
use eframe::{egui_wgpu, wgpu};
use glam::Vec2;
use std::sync::Arc;

type Geometry = (Vec<Vertex>, Vec<Vertex>);

const SELECTED: [f32; 3] = [1.0, 0.63, 0.0];
const CAPTURED: [f32; 3] = [0.3, 0.8, 1.0];
/// Dash and gap of construction geometry in sketch millimetres.
const DASH_MM: (f64, f64) = (3.0, 2.0);

pub fn init_renderer(cc: &eframe::CreationContext<'_>) {
    let rs = cc.wgpu_render_state.as_ref().expect("wgpu backend required");
    let renderer = SceneRenderer::new(
        &rs.device,
        rs.target_format,
        egui_wgpu::depth_format_from_bits(32, 0),
        1,
    );
    rs.renderer.write().callback_resources.insert(renderer);
}

struct SceneCallback {
    view_proj: [[f32; 4]; 4],
    clip: [f32; 4],
    geometry: Option<Arc<Geometry>>,
}

impl egui_wgpu::CallbackTrait for SceneCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(r) = resources.get_mut::<SceneRenderer>() {
            if let Some(g) = &self.geometry {
                r.set_geometry(device, &g.0, &g.1);
            }
            r.set_camera(queue, self.view_proj, self.clip);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(r) = resources.get::<SceneRenderer>() {
            r.draw(pass);
        }
    }
}

fn vertex(p: DVec3, normal: [f32; 3], c: [f32; 3]) -> Vertex {
    Vertex { pos: p.as_vec3().to_array(), normal, color: [c[0], c[1], c[2], 1.0] }
}

/// The "on" parts of a segment under a dash pattern; `phase` is the distance walked in the pattern and carries over to the next segment.
fn dashed(a: DVec3, b: DVec3, phase: &mut f64) -> Vec<[DVec3; 2]> {
    let (len, period) = ((b - a).length(), DASH_MM.0 + DASH_MM.1);
    let (mut out, mut s) = (Vec::new(), 0.0);
    while s < len {
        let p = *phase % period;
        let (on, left) = if p < DASH_MM.0 { (true, DASH_MM.0 - p) } else { (false, period - p) };
        let e = (s + left.max(1e-9)).min(len);
        if on {
            out.push([a + (b - a) * (s / len), a + (b - a) * (e / len)]);
        }
        *phase += e - s;
        s = e;
    }
    out
}

fn build_scene(app: &CadApp) -> Geometry {
    let (mut tris, mut lines) = (Vec::new(), Vec::new());
    {
        // Grid and axes never take part in the section (alpha 0, see scene.wgsl).
        let mut fixed = |a: DVec3, b: DVec3, c: [f32; 3]| {
            for p in [a, b] {
                lines.push(Vertex { pos: p.as_vec3().to_array(), normal: [0.0; 3], color: [c[0], c[1], c[2], 0.0] });
            }
        };
        let grey = [0.3, 0.3, 0.3];
        for i in -10..=10 {
            let t = f64::from(i) * 10.0;
            fixed(DVec3::new(t, -100.0, 0.0), DVec3::new(t, 100.0, 0.0), grey);
            fixed(DVec3::new(-100.0, t, 0.0), DVec3::new(100.0, t, 0.0), grey);
        }
        fixed(DVec3::ZERO, DVec3::new(50.0, 0.0, 0.0), [0.9, 0.2, 0.2]);
        fixed(DVec3::ZERO, DVec3::new(0.0, 50.0, 0.0), [0.2, 0.8, 0.2]);
        fixed(DVec3::ZERO, DVec3::new(0.0, 0.0, 50.0), [0.3, 0.4, 1.0]);
    }
    let mut line = |a: DVec3, b: DVec3, c: [f32; 3]| {
        lines.push(vertex(a, [0.0; 3], c));
        lines.push(vertex(b, [0.0; 3], c));
    };
    for e in app.doc.entities().filter(|e| app.visible(e)) {
        let shape = app.display_shape(e);
        let col = (if e.construction { crate::view2d::CONSTRUCTION_RGB } else { app.color(e) }).map(|c| f32::from(c) / 255.0);
        let selected = app.selection.contains(&e.id);
        let col = if !selected && app.captures.iter().any(|c| c.contains(&e.id)) { CAPTURED } else { col };
        // Solids are placed in world space; 2D shapes go through their sketch plane and vanish when it does not resolve.
        let to_world = app.sketch_to_world(e.sketch);
        if !shape.is_solid() && to_world.is_none() {
            continue;
        }
        let mut phase = 0.0;
        for [a, b] in outline(&shape) {
            let (a, b) = match &to_world {
                Some(f) if !shape.is_solid() => (f(a.truncate()), f(b.truncate())),
                _ => (a, b),
            };
            let c = if selected { SELECTED } else { col };
            if e.construction {
                for [p, q] in dashed(a, b, &mut phase) {
                    line(p, q, c);
                }
            } else {
                line(a, b, c);
            }
        }
        if let Some(mesh) = tessellate(&shape) {
            let color = if selected { SELECTED } else { col };
            for (p, n) in mesh.positions.iter().zip(&mesh.normals) {
                tris.push(Vertex { pos: *p, normal: *n, color: [color[0], color[1], color[2], 1.0] });
            }
        }
    }
    for (id, body) in app.model.iter().flat_map(|(_, m)| &m.bodies) {
        let (mesh, grey) = (&body.mesh, [0.72, 0.75, 0.80]);
        for (corner, (&i, n)) in mesh.indices.iter().zip(&mesh.normals).enumerate() {
            let color = if app.face_sel == Some((*id, body.tri_face[corner / 3])) { SELECTED } else { grey };
            tris.push(vertex(mesh.positions[i as usize], n.as_vec3().to_array(), color));
        }
        for [a, b] in body.feature_edges() {
            lines.push(vertex(a, [0.0; 3], [0.12, 0.12, 0.14]));
            lines.push(vertex(b, [0.0; 3], [0.12, 0.12, 0.14]));
        }
    }
    for p in app.pick_edges.iter().flat_map(|p| &p.points) {
        for (_, body) in app.model.iter().flat_map(|(_, m)| &m.bodies) {
            let Some(e) = body.nearest_edge(*p, 1e-3) else { continue };
            let n = e.points.len();
            for i in 0..if e.closed { n } else { n - 1 } {
                for q in [e.points[i], e.points[(i + 1) % n]] {
                    lines.push(vertex(q, [0.0; 3], SELECTED));
                }
            }
        }
    }
    (tris, lines)
}

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    app.view_rect = rect;
    let aspect = rect.width() / rect.height().max(1.0);

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.pick_face = PickFace::Off;
        app.pick_edges = None;
    }
    if app.pick_edges.is_some() && !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        crate::edge_pick::finish(app);
    }
    if resp.dragged_by(PointerButton::Secondary) {
        let d = resp.drag_delta();
        app.cam3d.orbit(d.x, d.y);
    }
    if resp.dragged_by(PointerButton::Middle) {
        let d = resp.drag_delta();
        app.cam3d.pan(d.x, d.y, rect.height());
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta().y);
        if scroll != 0.0 {
            app.cam3d.zoom((-scroll * 0.002).exp());
        }
    }
    if resp.clicked_by(PointerButton::Primary)
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let (origin, dir) = ray(app, rect, pos);
        let hit = if app.pick_face != PickFace::Off || app.pick_edges.is_some() { None } else { pick(app, origin, dir) };
        let shift = ui.input(|i| i.modifiers.shift);
        match (hit, shift) {
            _ if app.pick_face != PickFace::Off => crate::face_pick::click(app, origin, dir),
            _ if app.pick_edges.is_some() => {
                let pixel = 2.0 * 22.5f64.to_radians().tan() / f64::from(rect.height().max(1.0));
                crate::edge_pick::click(app, origin, dir, pixel);
            }
            (hit, shift) => {
                app.click_select(hit, shift);
                if hit.is_none() && !shift {
                    crate::face_pick::select_face(app, origin, dir);
                } else if app.face_sel.take().is_some() {
                    app.sel_rev += 1;
                }
            }
        }
    }

    let stamp = (app.doc.revision(), app.model.as_ref().map_or(u64::MAX, |(r, _)| *r), app.sel_rev);
    // The inspector buffer exists whenever one entity is selected; it only previews once it differs.
    let preview = app.drag.is_some() || app.pick_edges.is_some()
        || app.inspector.as_ref().is_some_and(|(id, s)| app.doc.get(*id).is_none_or(|e| e.shape != *s));
    let geometry = if preview || app.gpu_stamp != Some(stamp) {
        // While previewing the stamp stays unset, so the final state is uploaded afterwards.
        app.gpu_stamp = if preview { None } else { Some(stamp) };
        Some(Arc::new(build_scene(app)))
    } else {
        None
    };
    let view_proj = app.cam3d.view_proj(aspect).to_cols_array_2d();
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(rect, SceneCallback { view_proj, clip: app.section.clip(), geometry }));
    ui.painter_at(rect).text(
        rect.left_top() + egui::vec2(8.0, 8.0),
        egui::Align2::LEFT_TOP,
        "Draw tools work in the 2D view",
        egui::FontId::proportional(12.0),
        egui::Color32::from_gray(150),
    );
    if let Some(p) = &app.pick_edges {
        ui.painter_at(rect).text(
            rect.left_bottom() + egui::vec2(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            format!("Click edges, Enter to finish, Esc to cancel ({} picked)", p.points.len()),
            egui::FontId::proportional(13.0),
            egui::Color32::from_gray(200),
        );
    }
}

fn ray(app: &CadApp, rect: egui::Rect, pos: egui::Pos2) -> (DVec3, DVec3) {
    let (w, h) = (rect.width(), rect.height().max(1.0));
    let ndc = Vec2::new(2.0 * (pos.x - rect.left()) / w - 1.0, 1.0 - 2.0 * (pos.y - rect.top()) / h);
    app.cam3d.ray(w / h, ndc)
}

fn pick(app: &CadApp, origin: DVec3, dir: DVec3) -> Option<EntityId> {
    let h = app.view_rect.height().max(1.0);
    let mut best: Option<(EntityId, f64)> = None;
    let plane = app.doc.resolve_plane(app.sketch).ok();
    for e in app.doc.entities_in(app.sketch).filter(|e| app.pickable(e)) {
        let t = if e.shape.is_solid() {
            ray_hit(&e.shape, origin, dir)
        } else if let Some(plane) = &plane {
            let (n, to_hit) = (plane.normal(), plane.origin - origin);
            let denom = dir.dot(n);
            let t = to_hit.dot(n) / denom;
            let local = origin + dir * t - plane.origin;
            let tol = t * 2.0 * 22.5f64.to_radians().tan() / f64::from(h) * 6.0;
            let xy = DVec2::new(local.dot(plane.x), local.dot(plane.y));
            (denom.abs() > 1e-9 && t > 0.0 && distance_2d(&e.shape, xy) <= tol).then_some(t)
        } else {
            None
        };
        if let Some(t) = t
            && best.is_none_or(|(_, b)| t < b)
        {
            best = Some((e.id, t));
        }
    }
    best.map(|(id, _)| id)
}
