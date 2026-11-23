use crate::app::{CadApp, PickFace};
use cad_document::{ComponentId, EntityId, Overlap, TET_FACES};
use cad_geometry::{DAffine3, DVec2, DVec3, distance_2d, outline, ray_hit, tessellate};
use cad_renderer::{Draw, Instance, MeshData, SceneRenderer, Vertex};
use eframe::egui::{self, PointerButton};
use eframe::{egui_wgpu, wgpu};
use glam::Vec2;
use std::collections::BTreeSet;
use std::sync::Arc;

/// One 3D scene build: the per-scene buffers, every body mesh in use, and the instances that draw them.
struct Scene {
    tris: Vec<Vertex>,
    lines: Vec<Vertex>,
    meshes: Vec<(u64, Arc<MeshData>)>,
    draws: Vec<Draw>,
}

const SELECTED: [f32; 3] = [1.0, 0.63, 0.0];
const GREY: [f32; 3] = [0.72, 0.75, 0.80];
const PICKED_FACE: [f32; 3] = [1.0, 0.85, 0.3];
const CAP: [f32; 3] = [0.80, 0.42, 0.32];
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
    geometry: Option<Arc<Scene>>,
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
                r.set_geometry(device, &g.tris, &g.lines);
                r.set_instances(device, &g.meshes, &g.draws);
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

fn build_scene(app: &CadApp) -> Scene {
    let (mut tris, mut lines) = (Vec::new(), Vec::new());
    let mut draws = Vec::new();
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
    // The selected component follows a gizmo drag in progress (the gizmo moves it when no entity is selected).
    let moving = app.gizmo.as_ref().filter(|_| app.selection.is_empty()).map_or(DVec3::ZERO, crate::gizmo::GizmoDrag::delta);
    let offsets = app.doc.explode_offsets(app.explode_factor());
    // The motion study's shown sample; display only (picking stays on the assembled pose).
    let moves = crate::motion_panel::shown_moves(app);
    // A shown study result replaces the analysis mesh drawing (both draw the same body).
    let study = crate::study_panel::shown(app);
    let mesh = crate::mesh_panel::shown(app).filter(|_| study.is_none());
    let collision = crate::motion_panel::shown_collision(app);
    // The last check, drawn with the assembled placements, so only while not exploded and no motion study is shown.
    let check = app.check_result().filter(|_| app.explode_factor() == 0.0 && moves.is_none());
    // Components that interfere are drawn as edges only, so the red common solid inside them shows (no x-ray pass).
    let wire: BTreeSet<ComponentId> = check
        .iter()
        .flat_map(|c| &c.meetings)
        .filter(|m| matches!(m.kind, Ok(Overlap::Interference { .. })))
        .flat_map(|m| [m.a, m.b])
        .chain(collision.iter().flat_map(|c| [c.moving, c.other]))
        .collect();
    let model = app.model.as_ref().map(|(_, m)| m);
    let shown = app.shown_bodies().map(|(id, b)| (Some(*id), None, b, b, DAffine3::IDENTITY));
    // A component is drawn as its source body under its placement, so the components of one body share one mesh.
    let placed = app.placed_components().map(|(c, b)| {
        let source = model.and_then(|m| Some((m.body(app.doc.component(c)?.body)?, *m.placements.get(&c)?)));
        let (src, base) = source.unwrap_or((b, DAffine3::IDENTITY));
        (None, Some(c), b, src, base)
    });
    for (id, comp, body, source, base) in shown.chain(placed) {
        let meshed = id.is_some() && (mesh.is_some_and(|m| Some(m.body) == id) || study.is_some_and(|s| Some(s.body) == id));
        let selected = comp.is_some() && comp == app.component_sel;
        let shift = if selected { moving } else { DVec3::ZERO } + comp.and_then(|c| offsets.get(&c)).copied().unwrap_or(DVec3::ZERO);
        let turn = comp.and_then(|c| moves.and_then(|m| m.get(&c))).copied().unwrap_or(DAffine3::IDENTITY);
        let at = |p: DVec3| turn.transform_point3(p) + shift;
        let mesh = &body.mesh;
        let faces = !comp.is_some_and(|c| wire.contains(&c)) && !meshed;
        // A body with a highlighted face is drawn per triangle as before (instances colour whole meshes).
        let highlight = id.is_some_and(|id| app.face_sel.is_some_and(|(b, _)| b == id)) || (selected && app.component_face.is_some_and(|(c, _)| Some(c) == comp));
        match crate::scene_mesh::key(app, source).filter(|_| !highlight) {
            Some(key) => {
                let world = DAffine3::from_translation(shift) * turn * base;
                let tint = if selected { SELECTED } else { GREY };
                let instance = Instance { model: glam::DMat4::from(world).as_mat4().to_cols_array_2d(), color: [tint[0], tint[1], tint[2], 1.0] };
                draws.push(Draw { mesh: key, instance, faces });
            }
            None => {
                if faces {
                    for (corner, (&i, n)) in mesh.indices.iter().zip(&mesh.normals).enumerate() {
                        let face = id.is_some_and(|id| app.face_sel == Some((id, body.tri_face[corner / 3])));
                        let picked = selected && app.component_face.is_some_and(|(c, f)| Some(c) == comp && f == body.tri_face[corner / 3]);
                        let color = if picked { PICKED_FACE } else if selected || face { SELECTED } else { GREY };
                        tris.push(vertex(at(mesh.positions[i as usize]), turn.transform_vector3(*n).as_vec3().to_array(), color));
                    }
                }
                for [a, b] in body.feature_edges() {
                    lines.push(vertex(at(a), [0.0; 3], crate::scene_mesh::EDGE));
                    lines.push(vertex(at(b), [0.0; 3], crate::scene_mesh::EDGE));
                }
            }
        }
        // A cap of a moved solid would sit on the wrong plane.
        if shift == DVec3::ZERO
            && turn == DAffine3::IDENTITY
            && let Some((axis, d)) = app.section.plane()
        {
            let mut n = [0.0f32; 3];
            n[axis] = if app.section.flip { -1.0 } else { 1.0 };
            // Caps lie on the section plane: alpha 0 keeps the shader from clipping them.
            for t in body.section_cap(axis, d) {
                for p in t {
                    tris.push(Vertex { pos: p.as_vec3().to_array(), normal: n, color: [CAP[0], CAP[1], CAP[2], 0.0] });
                }
            }
        }
    }
    if app.explode_trails {
        const TRAIL: [f32; 3] = [0.35, 0.45, 0.95];
        for (c, b) in app.placed_components() {
            let off = offsets.get(&c).copied().unwrap_or(DVec3::ZERO);
            if off.length() > 1e-9 {
                let (lo, hi) = b.bbox();
                let m = (lo + hi) / 2.0;
                lines.push(vertex(m, [0.0; 3], TRAIL));
                lines.push(vertex(m + off, [0.0; 3], TRAIL));
            }
        }
    }
    // The analysis mesh in body coordinates: its boundary faces with their edges, or every element shrunk to 80 %
    // about its centroid (the inside shows, and the section plane cuts through it).
    if let Some(rep) = mesh {
        const MESH_FACE: [f32; 3] = [0.62, 0.78, 0.92];
        const MESH_EDGE: [f32; 3] = [0.10, 0.25, 0.55];
        let m = &rep.mesh;
        let mut face = |p: [DVec3; 3]| {
            let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero().as_vec3().to_array();
            for q in p {
                tris.push(vertex(q, n, MESH_FACE));
            }
        };
        if app.mesh_shrink {
            for k in 0..m.elements.len() {
                let c = m.corners_of(k);
                let mid = (c[0] + c[1] + c[2] + c[3]) / 4.0;
                let c = c.map(|p| mid + (p - mid) * 0.8);
                for f in TET_FACES {
                    face(f.map(|q| c[q]));
                }
            }
        } else {
            for t in m.boundary() {
                let p = t.map(|v| m.nodes[v as usize]);
                face(p);
                for q in 0..3 {
                    lines.push(vertex(p[q], [0.0; 3], MESH_EDGE));
                    lines.push(vertex(p[(q + 1) % 3], [0.0; 3], MESH_EDGE));
                }
            }
        }
    }
    // The study result: boundary triangles at their displaced corners (deformation scale), coloured per corner node.
    // shortcut: colours and shape are linear between the corners; split each 6-node triangle into 4 if curved
    // deformations look too coarse.
    if let Some(rep) = study {
        let m = &rep.mesh;
        let v = crate::study_panel::values(app, rep);
        let (lo, hi) = crate::study_panel::range(&v);
        let span = (hi - lo).max(1e-30);
        for t in m.boundary() {
            let p = t.map(|q| m.nodes[q as usize] + rep.result.displacement[q as usize] * app.study_scale);
            let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero().as_vec3().to_array();
            for (q, at) in t.iter().zip(p) {
                tris.push(vertex(at, n, crate::study_panel::ramp(((v[*q as usize] - lo) / span) as f32)));
            }
        }
    }
    const INTERFERENCE: [f32; 3] = [0.9, 0.12, 0.1];
    const CLEARANCE: [f32; 3] = [0.1, 0.75, 0.3];
    for m in check.iter().flat_map(|c| &c.meetings) {
        if let Ok(Overlap::Interference { region, .. }) = &m.kind {
            for (&i, n) in region.mesh.indices.iter().zip(&region.mesh.normals) {
                tris.push(vertex(region.mesh.positions[i as usize], n.as_vec3().to_array(), INTERFERENCE));
            }
        }
    }
    if let Some(c) = collision {
        for (&i, n) in c.region.mesh.indices.iter().zip(&c.region.mesh.normals) {
            tris.push(vertex(c.region.mesh.positions[i as usize], n.as_vec3().to_array(), INTERFERENCE));
        }
    }
    for (_, _, k) in check.iter().flat_map(|c| &c.clearances) {
        if let Some((p, q)) = k.points {
            lines.push(vertex(p, [0.0; 3], CLEARANCE));
            lines.push(vertex(q, [0.0; 3], CLEARANCE));
        }
    }
    // Wall solids in a warm grey; the generic outline at their foot is drawn with the entities above.
    for (id, body) in app.model.iter().flat_map(|(_, m)| &m.walls) {
        let Some(e) = app.doc.get(*id).filter(|e| app.visible(e)) else { continue };
        let color = if app.selection.contains(&e.id) { SELECTED } else { [0.86, 0.84, 0.78] };
        for (&i, n) in body.mesh.indices.iter().zip(&body.mesh.normals) {
            tris.push(vertex(body.mesh.positions[i as usize], n.as_vec3().to_array(), color));
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
    if let Some((_, e)) = &app.edge_sel {
        let n = e.points.len();
        for i in 0..if e.closed { n } else { n - 1 } {
            for q in [e.points[i], e.points[(i + 1) % n]] {
                lines.push(vertex(q, [0.0; 3], SELECTED));
            }
        }
    }
    let meshes = app.scene_meshes.iter().map(|(_, k, m)| (*k, m.clone())).collect();
    Scene { tris, lines, meshes, draws }
}

/// Screen position of the world point `q` in the 3D view `rect`; None behind the camera.
pub(crate) fn project(app: &CadApp, rect: egui::Rect, q: DVec3) -> Option<egui::Pos2> {
    let c = app.cam3d.view_proj(rect.width() / rect.height().max(1.0)) * q.as_vec3().extend(1.0);
    if c.w <= 1e-6 {
        return None;
    }
    let ndc = c.truncate() / c.w;
    Some(egui::Pos2::new(rect.left() + (ndc.x + 1.0) / 2.0 * rect.width(), rect.top() + (1.0 - ndc.y) / 2.0 * rect.height()))
}

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    app.view_rect = rect;
    crate::motion_panel::tick(app, ui.ctx());
    let aspect = rect.width() / rect.height().max(1.0);

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.pick_face = PickFace::Off;
        app.pick_edges = None;
        app.gizmo = None;
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
    crate::gizmo::input(app, ui, &resp, rect);
    if resp.clicked_by(PointerButton::Primary)
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let (origin, dir) = ray(app, rect, pos);
        let hit = if app.pick_face != PickFace::Off || app.pick_edges.is_some() { None } else { pick(app, origin, dir) };
        let shift = ui.input(|i| i.modifiers.shift);
        let pixel = 2.0 * 22.5f64.to_radians().tan() / f64::from(rect.height().max(1.0));
        match (hit, shift) {
            _ if app.pick_face != PickFace::Off => crate::face_pick::click(app, origin, dir),
            _ if app.pick_edges.is_some() => crate::edge_pick::click(app, origin, dir, pixel),
            (hit, shift) => {
                app.click_select(hit, shift);
                let comp = if hit.is_none() && !shift { crate::assembly_panel::pick(app, origin, dir) } else { None };
                if app.component_face != comp {
                    app.component_face = comp;
                    app.sel_rev += 1;
                }
                crate::assembly_panel::select(app, comp.map(|(c, _)| c));
                let had_edge = app.edge_sel.take().is_some();
                if comp.is_some() {
                    app.face_sel = None;
                } else if hit.is_none() && !shift && crate::edge_pick::select_edge(app, origin, dir, pixel) {
                    app.face_sel = None;
                    app.sel_rev += 1;
                } else if hit.is_none() && !shift {
                    crate::face_pick::select_face(app, origin, dir);
                    if had_edge {
                        app.sel_rev += 1;
                    }
                } else if app.face_sel.take().is_some() || had_edge {
                    app.sel_rev += 1;
                }
            }
        }
    }

    let stamp = (app.doc.revision(), app.model.as_ref().map_or(u64::MAX, |(r, _)| *r), app.sel_rev, app.section.clip());
    // The inspector buffer exists whenever one entity is selected; it only previews once it differs.
    let preview = app.drag.is_some() || app.pick_edges.is_some() || app.gizmo.is_some()
        || app.inspector.as_ref().is_some_and(|(id, s)| app.doc.get(*id).is_none_or(|e| e.shape != *s));
    let geometry = if preview || app.gpu_stamp != Some(stamp) {
        // While previewing the stamp stays unset, so the final state is uploaded afterwards.
        app.gpu_stamp = if preview { None } else { Some(stamp) };
        crate::scene_mesh::refresh(app);
        let start = std::time::Instant::now();
        let scene = build_scene(app);
        app.perf = crate::app::Perf {
            scene_ms: start.elapsed().as_secs_f64() * 1000.0,
            scene_vertices: scene.tris.len() + scene.lines.len(),
            instances: scene.draws.len(),
            meshes: scene.meshes.len(),
            ..app.perf
        };
        Some(Arc::new(scene))
    } else {
        None
    };
    let view_proj = app.cam3d.view_proj(aspect).to_cols_array_2d();
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(rect, SceneCallback { view_proj, clip: app.section.clip(), geometry }));
    app.gizmo_screen = crate::gizmo::screen(app, rect);
    if let Some(p) = app.gizmo_screen {
        crate::gizmo::paint(&ui.painter_at(rect), p, app.gizmo.as_ref().map(|g| g.axis));
    }
    if app.show_com
        && let Some(p) = crate::mass_panel::marker_point(app).and_then(|q| project(app, rect, q))
    {
        crate::mass_panel::paint_marker(&ui.painter_at(rect), p);
    }
    if let Some(rep) = crate::study_panel::shown(app) {
        crate::study_panel::paint_legend(app, rep, &ui.painter_at(rect), rect);
    }
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

pub(crate) fn ray(app: &CadApp, rect: egui::Rect, pos: egui::Pos2) -> (DVec3, DVec3) {
    let (w, h) = (rect.width(), rect.height().max(1.0));
    let ndc = Vec2::new(2.0 * (pos.x - rect.left()) / w - 1.0, 1.0 - 2.0 * (pos.y - rect.top()) / h);
    app.cam3d.ray(w / h, ndc)
}

fn pick(app: &CadApp, origin: DVec3, dir: DVec3) -> Option<EntityId> {
    let h = app.view_rect.height().max(1.0);
    let mut best: Option<(EntityId, f64)> = None;
    let plane = app.sketch_plane(app.sketch).ok();
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
