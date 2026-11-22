use crate::camera::{Camera2D, OrbitCamera};
use crate::mcp::{self, bridge::Bridge};
use crate::rebuild_worker::RebuildWorker;
use crate::{panels, view2d, view3d};
use cad_document::{Plane3, Command, ConstraintId, Document, Entity, EntityId, History, LayerId, BodyOp, Feature, FeatureId, PointRef, RebuildResult, RevolveAxis, SketchId, BasePlane, Target};
use cad_geometry::snap::Snap;
use cad_geometry::{DVec2, DVec3, HatchPattern, Opening, OpeningKind, Roof, RoofKind, Shape, Wall, WallAlign};
use std::collections::BTreeSet;
use eframe::egui;
use std::path::PathBuf;

/// Display-only section: keeps the side of the plane `axis = offset` with smaller coordinates, or the other side when flipped.
pub struct SectionView {
    pub on: bool,
    pub axis: usize,
    pub offset: String,
    pub flip: bool,
}

impl SectionView {
    /// Axis and offset of the active plane; None when off or when the offset does not parse.
    pub fn plane(&self) -> Option<(usize, f64)> {
        let d = self.offset.trim().parse::<f64>().ok()?;
        self.on.then_some((self.axis, d))
    }

    /// Plane for the shader; everything is kept when off or when the offset does not parse.
    pub fn clip(&self) -> [f32; 4] {
        let Some((axis, d)) = self.plane() else { return [0.0, 0.0, 0.0, 1.0] };
        let s = if self.flip { -1.0 } else { 1.0 };
        let mut c = [0.0f32; 4];
        c[axis] = s;
        c[3] = (d * f64::from(s)) as f32;
        c
    }
}

/// Which other levels the 2D view draws in grey under the active one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelView {
    Active,
    Below,
    All,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tool {
    Select,
    Line,
    Circle,
    Rect,
    Arc,
    Polyline,
    Ellipse,
    Spline,
    Polygon,
    Text,
    Dimension,
    Leader,
    RadiusDim,
    AngleDim,
    Wall,
    Door,
    Window,
    Room,
    Stair,
    Trim,
    Extend,
    Offset,
    Fillet,
    Chamfer,
    Split,
}

impl Tool {
    /// Tools that collect points by clicking.
    pub fn is_draw(self) -> bool {
        !matches!(self, Tool::Select | Tool::Trim | Tool::Extend | Tool::Offset | Tool::Fillet | Tool::Chamfer | Tool::Split)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SideTab {
    Sketch,
    Model,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ViewMode {
    TwoD,
    ThreeD,
}

/// What the next click on a planar face in the 3D view does.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PickFace {
    Off,
    /// Start a sketch on the face.
    Sketch,
    /// Open the face of a new shell feature.
    Shell,
}

/// Action waiting for the unsaved-changes dialog.
#[derive(Clone, Copy)]
pub enum Pending {
    New,
    Open,
    Import,
    ImportSvg,
    Quit,
}

pub struct CadApp {
    pub doc: Document,
    pub history: History,
    pub path: Option<PathBuf>,
    pub saved_rev: u64,
    pub selection: BTreeSet<EntityId>,
    /// Bumped on every selection change.
    pub sel_rev: u64,
    pub tool: Tool,
    pub view: ViewMode,
    pub cam2d: Camera2D,
    pub cam3d: OrbitCamera,
    /// Clicks collected by the active draw tool.
    pub points: Vec<DVec2>,
    /// Move drag: (start world, current world).
    pub drag: Option<(DVec2, DVec2)>,
    /// Uncommitted numeric edit.
    pub inspector: Option<(EntityId, Shape)>,
    pub status: String,
    pub cursor_world: DVec2,
    pub confirm: Option<Pending>,
    /// (doc.revision, model revision, sel_rev, section clip) last uploaded to the GPU.
    pub gpu_stamp: Option<(u64, u64, u64, [f32; 4])>,
    /// Central panel rect of the last frame, used to fit the 2D camera.
    pub view_rect: egui::Rect,
    pub last_title: String,
    pub allow_close: bool,
    /// Current layer for new shapes.
    pub layer: LayerId,
    /// The sketch the 2D view shows and new entities go to.
    pub sketch: SketchId,
    pub osnap: bool,
    pub grid_snap: bool,
    /// Constraint labels in the 2D view.
    pub show_glyphs: bool,
    /// Snap found this frame, for the marker.
    pub snap: Option<Snap>,
    pub rot_deg: f64,
    pub scale_factor: f64,
    /// Rename buffer of the layers panel.
    pub layer_name: String,
    /// First pick of a two-pick edit tool.
    pub op_target: Option<(EntityId, DVec2)>,
    pub offset_dist: f64,
    pub fillet_radius: f64,
    pub chamfer_dist: f64,
    pub text_input: String,
    pub text_height: f64,
    pub dim_text_height: f64,
    pub polygon_sides: u32,
    /// Thickness of new walls, mm.
    pub wall_thickness: f64,
    /// Alignment of new walls.
    pub wall_align: WallAlign,
    /// Door tool settings (host, wall and at are filled in on placement).
    pub door: Opening,
    /// Window tool settings.
    pub window: Opening,
    /// Thickness of floors made with "Make floor".
    pub floor_thickness: f64,
    /// Kind, pitch, thickness and overhang of roofs made with "Make roof" (the outline is unused).
    pub roof: Roof,
    /// Stair tool settings: width, highest riser, tread depth.
    pub stair_width: f64,
    pub stair_max_riser: f64,
    pub stair_tread: f64,
    /// Other levels drawn in the 2D view.
    pub level_view: LevelView,
    /// Elevation and height fields of the Levels controls.
    pub level_elevation: String,
    pub level_height: String,
    pub radial_diameter: bool,
    pub hatch_angle_deg: f64,
    pub hatch_spacing: f64,
    pub hatch_pattern: HatchPattern,
    /// Add-metadata row: (key, value).
    pub meta_new: (String, String),
    pub array: crate::app_ops::ArrayParams,
    /// Text of the coordinate field in the status bar.
    pub coord_input: String,
    /// Uncommitted text of a constraint value field.
    pub constraint_edit: Option<(ConstraintId, String)>,
    /// Constraint status text per document revision.
    pub constraint_cache: Option<(u64, Option<String>)>,
    pub side_tab: SideTab,
    /// Uncommitted parameter expression edit: (name, text).
    pub param_edit: Option<(String, String)>,
    /// Add-parameter row: (name, expression).
    pub param_new: (String, String),
    pub feat_distance: String,
    pub feat_angle: String,
    /// Taper field of the Extrude controls.
    pub feat_taper: String,
    pub feat_op: BodyOp,
    pub feat_axis: RevolveAxis,
    pub feat_flip: bool,
    pub feat_target: Target,
    /// Selections captured for a sweep (profile, path) or a loft (sections).
    pub captures: Vec<Vec<EntityId>>,
    /// Smooth checkbox of the Loft controls.
    pub loft_smooth: bool,
    /// The next click in the 3D view picks a planar face.
    pub pick_face: PickFace,
    /// Thickness field of the Shell controls.
    pub shell_thickness: String,
    pub pick_edges: Option<crate::edge_pick::EdgePick>,
    pub blend_size: String,
    /// Which part "Add part" makes: an index into `PartDef::defaults()`.
    pub part_choice: usize,
    pub new_sketch_base: BasePlane,
    pub new_sketch_offset: String,
    pub feature_sel: Option<FeatureId>,
    /// Uncommitted edit of the selected feature.
    pub feature_edit: Option<(FeatureId, Feature)>,
    pub worker: RebuildWorker,
    /// Document revision last sent to the worker.
    pub requested: Option<u64>,
    /// Newest rebuild result and the document revision it was built from; may lag behind `doc`.
    pub model: Option<(u64, RebuildResult)>,
    /// Fit the 3D camera to the first result after a document was replaced.
    pub fit_on_model: bool,
    /// Ray-pick acceleration structures of the bodies in `model`, see `face_pick::ray_body`.
    /// Selected body face: (body id, index into `body.faces`).
    pub face_sel: Option<(FeatureId, u32)>,
    /// Body edge selected in the 3D view, for its measurements.
    pub edge_sel: Option<(FeatureId, cad_document::BodyEdge)>,
    /// Gizmo drag in progress in the 3D view.
    pub gizmo: Option<crate::gizmo::GizmoDrag>,
    /// Gizmo screen points of the last frame; used for hit tests.
    pub gizmo_screen: Option<[egui::Pos2; 4]>,
    /// Section plane of the 3D view.
    pub section: SectionView,
    pub bvhs: Vec<(std::sync::Arc<cad_document::Body>, std::sync::Arc<cad_document::Bvh>)>,
    /// Named widgets of the last complete frame.
    pub widgets: Vec<crate::widgets::WidgetHit>,
    /// Present only with --mcp.
    pub mcp: Option<Bridge>,
}

impl CadApp {
    pub fn new(cc: &eframe::CreationContext<'_>, mcp: bool) -> Self {
        view3d::init_renderer(cc);
        Self {
            doc: Document::new("Untitled"),
            history: History::default(),
            path: None,
            saved_rev: 0,
            selection: BTreeSet::new(),
            sel_rev: 0,
            tool: Tool::Select,
            view: ViewMode::TwoD,
            cam2d: Camera2D::default(),
            cam3d: OrbitCamera::default(),
            points: Vec::new(),
            drag: None,
            inspector: None,
            status: String::new(),
            cursor_world: DVec2::ZERO,
            confirm: None,
            gpu_stamp: None,
            view_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0)),
            last_title: String::new(),
            allow_close: false,
            layer: 0,
            sketch: 0,
            osnap: true,
            grid_snap: false,
            show_glyphs: true,
            snap: None,
            rot_deg: 90.0,
            scale_factor: 2.0,
            layer_name: String::new(),
            op_target: None,
            offset_dist: 5.0,
            fillet_radius: 5.0,
            chamfer_dist: 5.0,
            text_input: "Text".into(),
            text_height: 5.0,
            dim_text_height: 3.5,
            polygon_sides: 6,
            wall_thickness: 200.0,
            wall_align: WallAlign::Center,
            door: opening_template(OpeningKind::Door, 900.0, 2100.0, 0.0),
            window: opening_template(OpeningKind::Window, 1200.0, 1200.0, 900.0),
            floor_thickness: 200.0,
            roof: Roof { outline: Vec::new(), kind: RoofKind::Gable, pitch: 30.0, thickness: 200.0, overhang: 500.0 },
            stair_width: 1000.0,
            stair_max_riser: 180.0,
            stair_tread: 280.0,
            level_view: LevelView::Active,
            level_elevation: "0".into(),
            level_height: "3000 mm".into(),
            radial_diameter: false,
            hatch_angle_deg: 45.0,
            hatch_spacing: 2.0,
            hatch_pattern: HatchPattern::default(),
            meta_new: Default::default(),
            array: Default::default(),
            coord_input: String::new(),
            constraint_edit: None,
            constraint_cache: None,
            side_tab: SideTab::Sketch,
            param_edit: None,
            param_new: Default::default(),
            feat_distance: "10 mm".into(),
            feat_angle: "360 deg".into(),
            feat_taper: "0 deg".into(),
            feat_op: BodyOp::Add,
            feat_axis: RevolveAxis::Y,
            feat_flip: false,
            feat_target: Target::Auto,
            captures: Vec::new(),
            loft_smooth: false,
            pick_face: PickFace::Off,
            shell_thickness: "2 mm".into(),
            pick_edges: None,
            blend_size: "2 mm".into(),
            part_choice: 0,
            new_sketch_base: BasePlane::XY,
            new_sketch_offset: "0".into(),
            feature_sel: None,
            feature_edit: None,
            worker: RebuildWorker::spawn(cc.egui_ctx.clone()),
            requested: None,
            model: None,
            fit_on_model: false,
            bvhs: Vec::new(),
            section: SectionView { on: false, axis: 2, offset: "0".into(), flip: false },
            face_sel: None,
            edge_sel: None,
            gizmo: None,
            gizmo_screen: None,
            widgets: Vec::new(),
            mcp: mcp.then(|| mcp::spawn(cc.egui_ctx.clone())),
        }
    }

    /// Takes the newest finished rebuild and asks for one when the document has moved on.
    pub fn sync_model(&mut self) {
        if let Some(m) = self.worker.poll() {
            self.model = Some(m);
            self.face_sel = None;
            self.edge_sel = None;
            if std::mem::take(&mut self.fit_on_model) {
                let (lo, hi) = self.scene_bounds();
                self.cam3d.fit(lo, hi);
            }
        }
        let rev = self.doc.revision();
        if self.requested != Some(rev) {
            self.requested = Some(rev);
            self.worker.request(rev, self.doc.clone());
        }
    }

    /// Forgets the model of the old document; the next frame rebuilds from scratch.
    pub fn reset_model(&mut self) {
        self.worker.reset();
        self.requested = None;
        self.model = None;
        self.face_sel = None;
        self.edge_sel = None;
        self.fit_on_model = true;
    }

    pub fn model_current(&self) -> bool {
        self.model.as_ref().is_some_and(|(r, _)| *r == self.doc.revision())
    }

    pub fn rebuilding(&self) -> bool {
        !self.doc.features().is_empty() && !self.model_current()
    }

    pub fn model_result(&self) -> Option<&RebuildResult> {
        self.model.as_ref().map(|(_, r)| r)
    }

    /// The plane of `sketch` as the last rebuild placed it (a sketch on a face follows its face), else as stored.
    pub fn sketch_plane(&self, sketch: SketchId) -> Result<Plane3, String> {
        self.model_result().and_then(|r| r.planes.get(&sketch).cloned()).unwrap_or_else(|| self.doc.resolve_plane(sketch))
    }

    /// Ids of the bodies of the latest rebuild.
    pub fn body_ids(&self) -> Vec<u64> {
        self.model_result().into_iter().flat_map(|r| &r.bodies).map(|(id, _)| *id).collect()
    }

    pub fn is_dirty(&self) -> bool {
        self.doc.revision() != self.saved_rev
    }

    pub fn exec(&mut self, cmds: Vec<Command>) {
        // A stale inspector buffer would be committed over this change on the next frame.
        self.inspector = None;
        self.status.clear();
        if let Err(e) = self.history.execute(&mut self.doc, cmds) {
            self.status = e.to_string();
        }
        self.sanitize();
    }

    /// Like `exec`, but the solve afterwards keeps the pinned entities where the commands put them.
    pub fn exec_pinned(&mut self, cmds: Vec<Command>, pinned: &BTreeSet<EntityId>) {
        self.inspector = None;
        self.status.clear();
        if let Err(e) = self.history.execute_pinned(&mut self.doc, cmds, pinned) {
            self.status = e.to_string();
        }
        self.sanitize();
    }

    /// Like `exec`, but a run with the same key is one undo step.
    pub fn exec_merged(&mut self, cmds: Vec<Command>, key: u64) {
        self.inspector = None;
        self.status.clear();
        if let Err(e) = self.history.execute_merged(&mut self.doc, cmds, key) {
            self.status = e.to_string();
        }
        self.sanitize();
    }

    pub fn add_shape(&mut self, shape: Shape) {
        self.add_shape_linked(shape, None);
    }

    /// Adds the shape; a linear dimension also gets its link in the same undo step.
    pub fn add_shape_linked(&mut self, shape: Shape, link: Option<(PointRef, PointRef)>) {
        if !self.doc.layer(self.layer).is_some_and(|l| l.visible && !l.locked) {
            self.status = "Current layer is hidden or locked".into();
            return;
        }
        let id = self.doc.alloc_id();
        let name = format!("{} {}", shape.kind(), id);
        let mut cmds = vec![Command::Add(Entity { id, name, shape, layer: self.layer, sketch: self.sketch, construction: false, group: None, meta: Default::default() })];
        cmds.extend(link.map(|l| Command::SetDimLink { id, link: Some(l) }));
        self.exec(cmds);
        if self.doc.get(id).is_some() {
            self.set_selection([id]);
        }
    }

    pub fn delete_selection(&mut self) {
        let constraints: BTreeSet<ConstraintId> = self.selection.iter().flat_map(|id| self.doc.constraints_of(*id)).collect();
        let links = self.selection.iter().filter(|id| self.doc.dim_link(**id).is_some()).map(|id| Command::SetDimLink { id: *id, link: None });
        // A wall's openings go with it, and before it: the document refuses to remove a wall that still hosts one.
        let mut doomed: BTreeSet<EntityId> = self.selection.clone();
        doomed.extend(self.selection.iter().flat_map(|id| self.doc.openings_of(*id)));
        let (openings, rest): (Vec<EntityId>, Vec<EntityId>) =
            doomed.into_iter().partition(|id| matches!(self.doc.get(*id).map(|x| &x.shape), Some(Shape::Opening(_))));
        let cmds: Vec<Command> = constraints
            .into_iter()
            .map(Command::RemoveConstraint)
            .chain(links)
            .chain(openings.into_iter().chain(rest).map(Command::Remove))
            .collect();
        self.exec(cmds);
        self.set_selection([]);
        self.inspector = None;
    }

    /// The shape as it must be drawn now: inspector edit, move-drag preview, or the stored shape.
    pub fn display_shape(&self, e: &Entity) -> Shape {
        if let Some((id, shape)) = &self.inspector
            && *id == e.id
        {
            return shape.clone();
        }
        if let Some((start, now)) = self.drag
            && self.selection.contains(&e.id)
        {
            let mut s = e.shape.clone();
            s.translate((now - start).extend(0.0));
            return s;
        }
        if let Some(g) = &self.gizmo
            && self.selection.contains(&e.id)
        {
            let mut s = e.shape.clone();
            s.translate(g.delta());
            return s;
        }
        e.shape.clone()
    }

    pub fn set_selection(&mut self, ids: impl IntoIterator<Item = EntityId>) {
        self.selection = ids.into_iter().collect();
        self.sel_rev += 1;
    }

    pub fn undo(&mut self) {
        if self.history.undo(&mut self.doc) {
            self.after_history_change();
        }
    }

    pub fn redo(&mut self) {
        if self.history.redo(&mut self.doc) {
            self.after_history_change();
        }
    }

    fn after_history_change(&mut self) {
        self.sanitize();
        self.inspector = None;
        self.param_edit = None;
        self.feature_edit = None;
        self.constraint_edit = None;
    }

    /// Union of all entity bboxes; a default box for an empty document.
    pub fn scene_bounds(&self) -> (DVec3, DVec3) {
        let bodies = self.model.iter().flat_map(|(_, m)| &m.bodies).map(|(_, b)| b.bbox());
        let walls = self.model.iter().flat_map(|(_, m)| &m.walls).map(|(_, b)| b.bbox());
        self.doc
            .entities()
            .filter_map(|e| self.entity_bounds(e))
            .chain(bodies)
            .chain(walls)
            .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
            .unwrap_or((DVec3::new(-50.0, -50.0, 0.0), DVec3::new(50.0, 50.0, 50.0)))
    }

    pub fn fit_active(&mut self) {
        match self.view {
            ViewMode::TwoD => self.fit_sketch(),
            ViewMode::ThreeD => {
                let (lo, hi) = self.scene_bounds();
                self.cam3d.fit(lo, hi);
            }
        }
    }
}

impl eframe::App for CadApp {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(b) = &mut self.mcp {
            b.feed(raw);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.widgets = crate::widgets::begin_frame(&ctx);
        if let Some(mut b) = self.mcp.take() {
            b.poll(self, &ctx);
            self.mcp = Some(b);
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.is_dirty() && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Quit);
        }
        self.sync_model();
        panels::shortcuts(self, &ctx);
        panels::menu_bar(self, ui);
        crate::toolbar::toolbar(self, ui);
        panels::right_panel(self, ui);
        panels::status_bar(self, ui);
        panels::confirm_dialog(self, &ctx);
        panels::update_title(self, &ctx);
        egui::CentralPanel::default().show(ui, |ui| match self.view {
            ViewMode::TwoD => view2d::show(self, ui),
            ViewMode::ThreeD => view3d::show(self, ui),
        });
        // Edits made during this frame go to the worker now, not a frame later.
        self.sync_model();
    }
}

/// An opening without a host, as the Door and Window tools keep their settings.
fn opening_template(kind: OpeningKind, width: f64, height: f64, sill: f64) -> Opening {
    let wall = Wall { a: DVec2::ZERO, b: DVec2::X, thickness: 1.0, height: 1.0, align: WallAlign::Center };
    Opening { host: 0, wall, at: 0.0, width, height, sill, kind, hinge_b: false, swing_left: true }
}
