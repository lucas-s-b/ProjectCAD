use crate::camera::{Camera2D, OrbitCamera};
use crate::mcp::{self, bridge::Bridge};
use crate::rebuild_worker::RebuildWorker;
use crate::settings::Dock;
use crate::{panels, view2d, view3d};
use cad_document::{Check, ExplodeStep, Joint, JointId, Component, ComponentId, Plane3, Command, ConstraintId, Document, Entity, EntityId, History, LayerId, BodyOp, Feature, FeatureId, PointRef, RebuildResult, RevolveAxis, SketchId, BasePlane, Target};
use cad_geometry::snap::Snap;
use cad_geometry::{DVec2, DVec3, HatchPattern, Opening, OpeningKind, Roof, RoofKind, Shape, Wall, WallAlign};
use std::collections::BTreeSet;
use std::sync::Arc;
use eframe::egui;
use std::path::PathBuf;

/// The last 3D scene build (MCP `state` key `perf`), timed in `view3d::show`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Perf {
    /// Longest frame (the whole `ui` call) since the last MCP `state` read, ms: work that holds the UI thread shows here.
    pub frame_ms: f64,
    /// Milliseconds of the last `build_scene`.
    pub scene_ms: f64,
    /// Vertices that build produced on the CPU (triangle corners and line ends) for the per-scene buffers.
    pub scene_vertices: usize,
    /// Instanced draws of that build (one per shown body and component); 0 before instancing.
    pub instances: usize,
    /// Body meshes kept on the GPU for instancing; 0 before instancing.
    pub meshes: usize,
}

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
    /// Infinite construction line, and its one-sided form.
    XLine,
    Ray,
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
    /// Transform tools by picked points (`view2d_xform`).
    Move,
    Copy,
    RotateRef,
    ScaleRef,
    Mirror,
    Stretch,
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
    Assembly,
}

impl SideTab {
    pub const ALL: [SideTab; 3] = [SideTab::Sketch, SideTab::Model, SideTab::Assembly];

    /// Index into `Layout::docks`.
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            SideTab::Sketch => "Sketch",
            SideTab::Model => "Model",
            SideTab::Assembly => "Assembly",
        }
    }
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
    /// Add faces to a draft: the first click makes the draft feature (its id is kept here), later clicks add faces of the same body.
    Draft(Option<FeatureId>),
}

/// Action waiting for the unsaved-changes dialog.
#[derive(Clone, Copy)]
pub enum Pending {
    New,
    Open,
    Import,
    ImportSvg,
    /// Open the entry of `Settings::recent` at this index.
    OpenRecent(usize),
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
    /// Timing of the last 3D scene build.
    pub perf: Perf,
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
    /// Direction of the straight edge last hovered while drawing, for the parallel snap.
    pub parallel_ref: Option<DVec2>,
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
    /// The Mirror tool keeps the original and adds the mirrored copy.
    pub mirror_keep: bool,
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
    /// Angle of new drafts.
    pub draft_angle: String,
    pub pick_edges: Option<crate::edge_pick::EdgePick>,
    pub blend_size: String,
    /// Which part "Add part" makes: an index into `PartDef::defaults()`.
    pub part_choice: usize,
    pub new_sketch_base: BasePlane,
    pub new_sketch_offset: String,
    pub feature_sel: Option<FeatureId>,
    /// Uncommitted edit of the selected feature.
    pub feature_edit: Option<(FeatureId, Feature)>,
    /// Selected component (Assembly tab or a click in 3D); the gizmo moves it while no entity is selected.
    pub component_sel: Option<ComponentId>,
    /// Uncommitted edit of the selected component.
    pub component_edit: Option<Component>,
    /// Body that "Add component" places (0: none chosen yet).
    pub component_body: u64,
    /// Face of a component last clicked in 3D (component, face index of its body); "frame from face" reads it.
    pub component_face: Option<(ComponentId, u32)>,
    /// Parent that "Add joint" uses (0: none chosen yet).
    pub joint_parent: ComponentId,
    /// Selected joint in the Assembly tab and its uncommitted edit.
    pub joint_sel: Option<JointId>,
    pub joint_edit: Option<Joint>,
    /// The joint value being dragged in the joint editor (display only until released).
    pub joint_drag: Option<crate::assembly_joints::JointDrag>,
    /// Exploded view: shown when on, at `explode` (0 assembled .. 1 fully exploded), with trail lines when asked.
    pub explode_on: bool,
    pub explode: f64,
    pub explode_trails: bool,
    /// Uncommitted edit of the explode steps.
    pub explode_edit: Option<Vec<ExplodeStep>>,
    /// Clearance limit of the interference check, a length expression.
    pub check_limit: String,
    /// Mass properties of `model`, computed when a rebuild result arrives.
    pub mass: Option<cad_document::MassReport>,
    /// Draw the centre of mass marker in the 3D view.
    pub show_com: bool,
    /// The last interference check and the document revision it ran on (see `check_result`).
    pub check: Option<(u64, Check)>,
    /// Element size of the analysis mesh (a length expression) and the body to mesh (Model tab).
    pub mesh_size: String,
    pub mesh_body: FeatureId,
    /// The last analysis mesh and the document revision it ran on (see `mesh_result`).
    pub fea_mesh: Option<(u64, crate::mesh_panel::MeshRun)>,
    /// Draw the analysis mesh in 3D, as boundary faces with edges or as shrunken elements.
    pub show_mesh: bool,
    pub mesh_shrink: bool,
    /// Static studies (Model tab): the selected study (index into `Document::studies`), the study being edited and
    /// the revision and index it was copied at (text fields commit when they lose focus, like `joint_edit`).
    pub study_sel: usize,
    pub study_edit: Option<(u64, usize, cad_document::Study)>,
    /// The face index chosen for a support or load, and the 3D face selection last copied into it.
    pub study_face: Option<u32>,
    pub study_face_seen: Option<(FeatureId, u32)>,
    /// Load values typed before "Add force" (N) and "Add pressure" (MPa).
    pub study_force: [String; 3],
    pub study_pressure: String,
    /// The last study run and the document revision it ran on (see `study_result`).
    pub fea_study: Option<(u64, Result<cad_document::StudyReport, String>)>,
    /// Draw the result in 3D, coloured by displacement instead of von Mises, deformed by `study_scale` (0: undeformed).
    pub show_study: bool,
    pub study_by_displacement: bool,
    pub study_scale: f64,
    /// The last run of the analytic benchmarks (milliseconds, rows; Model tab "Validation report"). Independent of the
    /// document, so edits, new and open keep it.
    pub validation: Option<(f64, Result<Vec<cad_document::Benchmark>, String>)>,
    /// Motion study inputs (Assembly tab) and whether "Run study" also checks collisions.
    pub motion_study: cad_document::MotionStudy,
    pub motion_check: bool,
    /// The last motion study and the document revision it ran on (see `motion_result`).
    pub motion: Option<(u64, crate::motion_panel::StudyRun)>,
    /// Playback: the shown sample, whether it advances by itself, and the time (s) of the last advance.
    pub motion_step: usize,
    pub motion_play: bool,
    pub motion_clock: f64,
    /// The analysis running off the UI thread (see `analysis_worker`).
    pub jobs: crate::analysis_worker::Jobs,
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
    /// Screen points of the rotate rings of the last frame (X, Y, Z); used for hit tests.
    pub gizmo_rings: Option<[Vec<egui::Pos2>; 3]>,
    /// Screen points of a jointed component's gizmo handle of the last frame: (joint kind, start, points); see `gizmo::joint_screen`.
    pub gizmo_joint: Option<(cad_document::JointKind, egui::Pos2, Vec<egui::Pos2>)>,
    /// Section plane of the 3D view.
    pub section: SectionView,
    pub bvhs: Vec<(std::sync::Arc<cad_document::Body>, std::sync::Arc<cad_document::Bvh>)>,
    /// GPU meshes of the bodies in `model` for instanced drawing: (body, key, mesh); see `scene_mesh`.
    pub scene_meshes: Vec<(std::sync::Arc<cad_document::Body>, u64, std::sync::Arc<cad_renderer::MeshData>)>,
    /// The last key handed out in `scene_meshes`; keys are never reused.
    pub mesh_key: u64,
    /// Named widgets of the last complete frame.
    pub widgets: Vec<crate::widgets::WidgetHit>,
    /// Theme, accent and panel layout (`settings.rs`); not part of the document or its undo history.
    pub settings: crate::settings::Settings,
    /// The settings as last written or read, so only changes are written.
    pub settings_saved: crate::settings::Settings,
    /// The settings file; None in MCP mode without `CAD_SETTINGS`.
    pub settings_path: Option<std::path::PathBuf>,
    /// The theme and accent the egui visuals were last set from.
    pub theme_applied: Option<crate::settings::Applied>,
    /// The language `i18n` and the fonts were last set to.
    pub lang_applied: Option<crate::i18n::Language>,
    /// The tutorial window's current step while it is open (`tutorial.rs`).
    pub tutorial: Option<usize>,
    /// Bumped by "Reset layout" so egui forgets the slots' dragged sizes (part of the panel ids).
    pub layout_gen: u32,
    /// The plugins folder (`plugins` beside the settings file); None without a settings file.
    pub plugins_dir: Option<std::path::PathBuf>,
    /// Plugin manifests read at startup or by "Reload plugins", sorted by name.
    pub plugins: Vec<crate::plugins::Manifest>,
    /// Manifest files that could not be used, "plugins/<file>: <reason>".
    pub plugin_errors: Vec<String>,
    /// The plugin started from the Plugins menu while it runs (one at a time).
    pub plugin: Option<crate::plugins::Running>,
    /// The open command search (Ctrl+K), see `palette.rs`.
    pub palette: Option<crate::palette::Palette>,
    /// The chosen search result being clicked, one step per frame.
    pub palette_click: Option<crate::palette::PaletteClick>,
    /// The Grab / Rotate / Scale modal in progress (`transform.rs`).
    pub modal: Option<crate::transform::Modal>,
    /// The camera glide in progress (`navigation.rs`).
    pub glide: Option<crate::navigation::Glide>,
    /// The Settings window: open, its page, the action waiting for a key press (and the chord slot it replaces:
    /// None adds one), and the filter of the Controls page.
    pub settings_open: bool,
    pub settings_page: crate::settings_window::Page,
    pub rebind: Option<(crate::keymap::Action, Option<usize>)>,
    pub controls_filter: String,
    /// Toolbar and side panels hidden (Toggle panels).
    pub focus_mode: bool,
    /// Which handles the 3D gizmo shows.
    pub gizmo_mode: crate::gizmo::GizmoMode,
    /// When the project was last autosaved (or the app started).
    pub autosaved: std::time::Instant,
    /// Smoothed frame time for the status bar, ms.
    pub frame_avg: f64,
    /// Entity under the cursor in the 2D view (hover highlight).
    pub hover: Option<EntityId>,
    /// Present only with --mcp.
    pub mcp: Option<Bridge>,
}

impl CadApp {
    pub fn new(cc: &eframe::CreationContext<'_>, mcp: bool) -> Self {
        view3d::init_renderer(cc);
        let settings_path = crate::settings::path(mcp);
        let (settings, warning) = match settings_path.as_deref().map(crate::settings::load) {
            Some(Ok(s)) => (s, None),
            Some(Err(err)) => (crate::settings::Settings::default(), Some(err)),
            None => (crate::settings::Settings::default(), None),
        };
        let plugins_dir = crate::plugins::dir(settings_path.as_deref());
        let (plugins, plugin_errors) = plugins_dir.as_deref().map(crate::plugins::scan).unwrap_or_default();
        // Not in MCP mode: scripts start from the default settings and expect no window over the views.
        let tutorial = (!mcp && !settings.tutorial_done).then_some(0);
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
            status: warning.unwrap_or_default(),
            cursor_world: DVec2::ZERO,
            confirm: None,
            gpu_stamp: None,
            perf: Perf::default(),
            view_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0)),
            last_title: String::new(),
            allow_close: false,
            layer: 0,
            sketch: 0,
            osnap: settings.general.osnap_on_start,
            grid_snap: settings.general.grid_snap_on_start,
            show_glyphs: true,
            snap: None,
            parallel_ref: None,
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
            mirror_keep: true,
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
            draft_angle: "3 deg".into(),
            pick_edges: None,
            blend_size: "2 mm".into(),
            part_choice: 0,
            new_sketch_base: BasePlane::XY,
            new_sketch_offset: "0".into(),
            feature_sel: None,
            feature_edit: None,
            component_sel: None,
            component_edit: None,
            component_body: 0,
            component_face: None,
            joint_parent: 0,
            joint_sel: None,
            joint_edit: None,
            joint_drag: None,
            explode_on: false,
            explode: 1.0,
            explode_trails: false,
            explode_edit: None,
            check_limit: "5 mm".into(),
            mass: None,
            show_com: false,
            check: None,
            mesh_size: "5 mm".into(),
            mesh_body: 0,
            fea_mesh: None,
            show_mesh: true,
            mesh_shrink: false,
            study_sel: 0,
            study_edit: None,
            study_face: None,
            study_face_seen: None,
            study_force: ["0".into(), "0".into(), "-100".into()],
            study_pressure: "1".into(),
            fea_study: None,
            show_study: true,
            study_by_displacement: false,
            study_scale: 1.0,
            validation: None,
            motion_study: Default::default(),
            motion_check: false,
            motion: None,
            motion_step: 0,
            motion_play: false,
            motion_clock: 0.0,
            jobs: Default::default(),
            worker: RebuildWorker::spawn(cc.egui_ctx.clone()),
            requested: None,
            model: None,
            fit_on_model: false,
            bvhs: Vec::new(),
            scene_meshes: Vec::new(),
            mesh_key: 0,
            section: SectionView { on: false, axis: 2, offset: "0".into(), flip: false },
            face_sel: None,
            edge_sel: None,
            gizmo: None,
            gizmo_screen: None,
            gizmo_rings: None,
            gizmo_joint: None,
            widgets: Vec::new(),
            settings_saved: settings.clone(),
            settings,
            settings_path,
            theme_applied: None,
            lang_applied: None,
            tutorial,
            layout_gen: 0,
            plugins_dir,
            plugins,
            plugin_errors,
            plugin: None,
            palette: None,
            palette_click: None,
            modal: None,
            glide: None,
            settings_open: false,
            settings_page: Default::default(),
            rebind: None,
            controls_filter: String::new(),
            focus_mode: false,
            gizmo_mode: Default::default(),
            autosaved: std::time::Instant::now(),
            frame_avg: 0.0,
            hover: None,
            mcp: mcp.then(|| mcp::spawn(cc.egui_ctx.clone())),
        }
    }

    /// Takes the newest finished rebuild and asks for one when the document has moved on.
    pub fn sync_model(&mut self) {
        if let Some(m) = self.worker.poll() {
            self.model = Some(m);
            self.mass = self.model.as_ref().map(|(_, r)| self.doc.mass(r));
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
        self.mass = None;
        self.face_sel = None;
        self.edge_sel = None;
        self.fit_on_model = true;
    }

    pub fn model_current(&self) -> bool {
        self.model.as_ref().is_some_and(|(r, _)| *r == self.doc.revision())
    }

    /// No rebuild pending and no analysis running: MCP replies and held requests wait for this.
    pub fn idle(&self) -> bool {
        self.model_current() && self.jobs.running().is_none() && self.palette_click.is_none() && self.glide.is_none()
    }

    /// Cancels a job whose document has changed, then hands a finished job's output to its panel, which stores it
    /// under the current revision (equal to the job's, since a stale job was just dropped).
    pub fn poll_jobs(&mut self) {
        use crate::analysis_worker::JobOutput;
        self.jobs.drop_stale(self.doc.revision());
        let Some((kind, out)) = self.jobs.poll() else { return };
        match out {
            Err(msg) => self.status = format!("{}: internal error: {msg}", kind.name()),
            Ok(JobOutput::Study(r)) => crate::study_panel::finish(self, r),
            Ok(JobOutput::Motion(r)) => crate::motion_panel::finish(self, r),
            Ok(JobOutput::Check(r)) => crate::assembly_check::finish(self, r),
            Ok(JobOutput::Validation(ms, rows)) => self.validation = Some((ms, rows)),
        }
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

    /// Bodies of the latest rebuild that are drawn and picked: a body placed by a component is shown only through its components.
    pub fn shown_bodies(&self) -> impl Iterator<Item = &(FeatureId, Arc<cad_document::Body>)> + '_ {
        let placed: BTreeSet<FeatureId> = self.doc.components().map(|c| c.body).collect();
        self.model.iter().flat_map(|(_, m)| &m.bodies).filter(move |(id, _)| !placed.contains(id))
    }

    /// The last interference check while the document is unchanged since it ran; any edit, undo or redo drops it.
    pub fn check_result(&self) -> Option<&Check> {
        self.check.as_ref().filter(|(rev, _)| *rev == self.doc.revision()).map(|(_, c)| c)
    }

    /// The last motion study while the document is unchanged since it ran; any edit, undo or redo drops it.
    pub fn motion_result(&self) -> Option<&crate::motion_panel::StudyRun> {
        self.motion.as_ref().filter(|(rev, _)| *rev == self.doc.revision()).map(|(_, m)| m)
    }

    /// The last analysis mesh while the document is unchanged since it ran; any edit, undo or redo drops it.
    pub fn mesh_result(&self) -> Option<&crate::mesh_panel::MeshRun> {
        self.fea_mesh.as_ref().filter(|(rev, _)| *rev == self.doc.revision()).map(|(_, m)| m)
    }

    /// The last study run while the document is unchanged since it ran; any edit, undo or redo drops it.
    pub fn study_result(&self) -> Option<&Result<cad_document::StudyReport, String>> {
        self.fea_study.as_ref().filter(|(rev, _)| *rev == self.doc.revision()).map(|(_, r)| r)
    }

    /// The explode factor in effect: 0 while the exploded view is off.
    pub fn explode_factor(&self) -> f64 {
        if self.explode_on { self.explode } else { 0.0 }
    }

    /// Solids of the components in the latest rebuild; components whose placement failed are left out.
    pub fn placed_components(&self) -> impl Iterator<Item = (ComponentId, &Arc<cad_document::Body>)> + '_ {
        self.model.iter().flat_map(|(_, m)| &m.components).filter_map(|(id, r)| r.as_ref().ok().map(|b| (*id, b)))
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

    /// Like `exec`, but hands the error back instead of showing it in the status line (MCP `apply_commands`).
    pub fn try_exec(&mut self, cmds: Vec<Command>) -> Result<(), cad_document::DocError> {
        self.inspector = None;
        self.status.clear();
        let r = self.history.execute(&mut self.doc, cmds);
        self.sanitize();
        r
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
        self.add_entity(shape, None, false);
    }

    /// Adds the shape as construction geometry (drawn dashed, left out of profiles and exports).
    pub fn add_construction(&mut self, shape: Shape) {
        self.add_entity(shape, None, true);
    }

    /// Adds the shape; a linear dimension also gets its link in the same undo step.
    pub fn add_shape_linked(&mut self, shape: Shape, link: Option<(PointRef, PointRef)>) {
        self.add_entity(shape, link, false);
    }

    fn add_entity(&mut self, shape: Shape, link: Option<(PointRef, PointRef)>, construction: bool) {
        if !self.doc.layer(self.layer).is_some_and(|l| l.visible && !l.locked) {
            self.status = "Current layer is hidden or locked".into();
            return;
        }
        let id = self.doc.alloc_id();
        let name = format!("{} {}", shape.kind(), id);
        let mut cmds = vec![Command::Add(Entity { id, name, shape, layer: self.layer, sketch: self.sketch, construction, group: None, meta: Default::default() })];
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
        if self.modal.as_ref().is_some_and(|m| !matches!(m.target, crate::transform::Target::Component(_))) && self.selection.contains(&e.id) {
            return crate::transform::preview(self, &e.shape);
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
        self.component_edit = None;
        self.joint_edit = None;
        self.explode_edit = None;
        self.constraint_edit = None;
    }

    /// Union of all entity bboxes; a default box for an empty document.
    pub fn scene_bounds(&self) -> (DVec3, DVec3) {
        let bodies = self.shown_bodies().map(|(_, b)| b.bbox()).chain(self.placed_components().map(|(_, b)| b.bbox()));
        let walls = self.model.iter().flat_map(|(_, m)| &m.walls).map(|(_, b)| b.bbox());
        self.doc
            .entities()
            .filter_map(|e| self.entity_bounds(e))
            .chain(bodies)
            .chain(walls)
            .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
            .unwrap_or((DVec3::new(-50.0, -50.0, 0.0), DVec3::new(50.0, 50.0, 50.0)))
    }

    /// Fits the active view to everything, gliding when camera transitions are on.
    pub fn fit_active(&mut self) {
        crate::navigation::glide(self, |app| match app.view {
            ViewMode::TwoD => app.fit_sketch(),
            ViewMode::ThreeD => {
                let (lo, hi) = app.scene_bounds();
                app.cam3d.fit(lo, hi);
            }
        });
    }
}

impl eframe::App for CadApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(b) = &mut self.mcp {
            b.feed(raw);
        }
        crate::palette::feed(self, ctx, raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let frame_start = std::time::Instant::now();
        let ctx = ui.ctx().clone();
        self.widgets = crate::widgets::begin_frame(&ctx);
        crate::settings::sync(self, &ctx);
        if let Some(mut b) = self.mcp.take() {
            b.poll(self, &ctx);
            self.mcp = Some(b);
        }
        if let Some(mut p) = self.plugin.take() {
            p.bridge.poll(self, &ctx);
            match p.finished() {
                Some(msg) => self.status = msg,
                None => {
                    // Keep frames coming so the plugin's requests and its exit are seen.
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    self.plugin = Some(p);
                }
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.is_dirty() && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Quit);
        }
        self.sync_model();
        self.poll_jobs();
        crate::navigation::tick(self, &ctx);
        panels::autosave(self);
        crate::settings_window::capture(self, &ctx);
        panels::shortcuts(self, &ctx);
        panels::menu_bar(self, ui);
        if !self.focus_mode {
            crate::toolbar::toolbar(self, ui);
            panels::side_panels(self, ui, &Dock::ALL[..2]);
        }
        panels::status_bar(self, ui);
        if !self.focus_mode {
            panels::side_panels(self, ui, &Dock::ALL[2..]);
        }
        panels::confirm_dialog(self, &ctx);
        crate::settings_window::show(self, &ctx);
        crate::tutorial::show(self, &ctx);
        crate::palette::show(self, &ctx);
        panels::update_title(self, &ctx);
        egui::CentralPanel::default().show(ui, |ui| match self.view {
            ViewMode::TwoD => view2d::show(self, ui),
            ViewMode::ThreeD => view3d::show(self, ui),
        });
        // Edits made during this frame go to the worker now, not a frame later.
        self.sync_model();
        // Longest frame since the last MCP state read: a job that holds the UI thread shows here.
        let ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        self.perf.frame_ms = self.perf.frame_ms.max(ms);
        self.frame_avg = if self.frame_avg == 0.0 { ms } else { self.frame_avg * 0.9 + ms * 0.1 };
    }
}

/// An opening without a host, as the Door and Window tools keep their settings.
fn opening_template(kind: OpeningKind, width: f64, height: f64, sill: f64) -> Opening {
    let wall = Wall { a: DVec2::ZERO, b: DVec2::X, thickness: 1.0, height: 1.0, align: WallAlign::Center };
    Opening { host: 0, wall, at: 0.0, width, height, sill, kind, hinge_b: false, swing_left: true }
}
