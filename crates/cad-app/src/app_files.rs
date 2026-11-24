use crate::app::{CadApp, Pending, PickFace};
use cad_document::{Document, FeatureId, FeatureKind};
use cad_geometry::{DAffine3, DVec3};
use eframe::egui;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MCP_NO_DIALOGS: &str = "File dialogs are off in MCP mode; use open_project / save_project";

impl CadApp {
    fn reset(&mut self, doc: Document, path: Option<PathBuf>) {
        self.doc = doc;
        self.path = path;
        self.history.clear();
        self.set_selection([]);
        self.inspector = None;
        self.constraint_edit = None;
        self.component_face = None;
        self.joint_sel = None;
        self.joint_edit = None;
        self.explode_edit = None;
        self.check = None;
        self.motion = None;
        self.fea_mesh = None;
        self.fea_study = None;
        self.jobs.cancel();
        self.study_edit = None;
        self.study_sel = 0;
        self.study_face = None;
        self.joint_drag = None;
        self.motion_play = false;
        self.reset_model();
        self.constraint_cache = None;
        self.drag = None;
        self.points.clear();
        self.op_target = None;
        self.snap = None;
        self.layer = 0;
        self.sketch = 0;
        self.pick_face = PickFace::Off;
        self.captures.clear();
        self.pick_edges = None;
        self.saved_rev = self.doc.revision();
        self.gpu_stamp = None;
        let (lo, hi) = self.scene_bounds();
        self.cam2d.fit(self.view_rect, lo.truncate(), hi.truncate());
        self.cam3d.fit(lo, hi);
    }

    /// New, Open, Import and Quit go through the unsaved-changes guard.
    pub fn request(&mut self, ctx: &egui::Context, action: Pending) {
        if self.is_dirty() {
            self.confirm = Some(action);
        } else {
            self.perform(ctx, action);
        }
    }

    pub fn perform(&mut self, ctx: &egui::Context, action: Pending) {
        match action {
            Pending::New => self.new_project(),
            Pending::Open => {
                if self.mcp.is_some() {
                    self.status = MCP_NO_DIALOGS.into();
                    return;
                }
                let picked = rfd::FileDialog::new().add_filter("CAD project", &["cadproj"]).pick_file();
                if let Some(p) = picked {
                    let _ = self.open_path(p);
                }
            }
            Pending::Import | Pending::ImportSvg => {
                if self.mcp.is_some() {
                    self.status = "File dialogs are off in MCP mode; use import_project".into();
                    return;
                }
                let ext = if matches!(action, Pending::ImportSvg) { "svg" } else { "dxf" };
                if let Some(p) = rfd::FileDialog::new().add_filter(ext.to_uppercase(), &[ext]).pick_file() {
                    let _ = self.import_path(p);
                }
            }
            Pending::OpenRecent(i) => {
                let Some(p) = self.settings.recent.get(i).cloned() else { return };
                if self.open_path(PathBuf::from(&p)).is_err() {
                    // A file that is gone leaves the list.
                    self.settings.recent.retain(|r| *r != p);
                }
            }
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    pub fn new_project(&mut self) {
        self.reset(Document::new("Untitled"), None);
    }

    /// Loads without a dialog and without the unsaved-changes guard.
    pub fn open_path(&mut self, p: PathBuf) -> Result<(), String> {
        match Document::load(&p) {
            Ok(doc) => {
                self.settings.push_recent(&p);
                self.reset(doc, Some(p));
                Ok(())
            }
            Err(e) => {
                self.status = format!("Open failed: {e}");
                Err(self.status.clone())
            }
        }
    }

    /// Imports a file by extension, without a dialog and without the unsaved-changes guard. DXF or SVG replace the
    /// document; STL or OBJ add a mesh body (`import_mesh`).
    pub fn import_path(&mut self, p: PathBuf) -> Result<String, String> {
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase());
        if matches!(ext.as_deref(), Some("stl" | "obj")) {
            return self.import_mesh(&p);
        }
        let text = std::fs::read_to_string(&p).map_err(|e| format!("Import failed: {e}"));
        let name = p.file_stem().map_or_else(|| "Untitled".to_string(), |s| s.to_string_lossy().into_owned());
        let parsed = text.and_then(|t| match ext.as_deref() {
            Some("svg") => cad_import_export::from_svg(&t, &name),
            Some("dxf") => cad_import_export::from_dxf(&t, &name),
            _ => return Err("Import failed: unknown format; use .dxf or .svg".to_string()),
        }
        .map_err(|e| format!("Import failed: {e}")));
        match parsed {
            Ok((doc, report)) => {
                let skipped: Vec<String> = report.skipped.iter().map(|(k, n)| format!("{k} x{n}")).collect();
                let mut msg = format!("Imported {} entities, skipped {}", report.imported, report.skipped.values().sum::<usize>());
                if !skipped.is_empty() {
                    msg.push_str(&format!(" ({})", skipped.join(", ")));
                }
                self.reset(doc, None);
                self.status = msg.clone();
                Ok(msg)
            }
            Err(e) => {
                self.status = e.clone();
                Err(e)
            }
        }
    }

    /// Adds the closed triangle mesh of an STL or OBJ file (by extension, mm) as a mesh-body feature at the end of the
    /// feature list (one undo step), keeping the document; a mesh that is not closed is refused here, not left as a failed feature.
    pub fn import_mesh(&mut self, p: &Path) -> Result<String, String> {
        let stem = p.file_stem().map_or_else(|| "Mesh".to_string(), |s| s.to_string_lossy().into_owned());
        let is_obj = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("obj"));
        let parsed = std::fs::read(p).map_err(|err| err.to_string()).and_then(|bytes| {
            let (positions, triangles) =
                if is_obj { cad_import_export::from_obj(&String::from_utf8_lossy(&bytes)) } else { cad_import_export::from_stl(&bytes) }?;
            let points = positions.iter().map(|q| DVec3::from(q.map(f64::from))).collect();
            cad_document::Body::from_mesh(points, &triangles).map_err(|err| err.to_string())?;
            Ok((positions, triangles))
        });
        let (positions, triangles) = match parsed {
            Ok(mesh) => mesh,
            Err(err) => {
                self.status = format!("Import failed: {err}");
                return Err(self.status.clone());
            }
        };
        let n = triangles.len();
        let id = crate::model_features::add_feature(self, &stem, FeatureKind::Mesh { positions, triangles });
        if self.doc.feature(id).is_none() {
            return Err(self.status.clone());
        }
        self.status = format!("Imported mesh {stem}: {n} triangles");
        Ok(self.status.clone())
    }

    /// "Import STL/OBJ mesh": asks for a path and imports.
    pub fn import_mesh_dialog(&mut self) {
        if self.mcp.is_some() {
            self.status = "File dialogs are off in MCP mode; use import_project".into();
            return;
        }
        if let Some(p) = rfd::FileDialog::new().add_filter("Mesh", &["stl", "obj"]).pick_file()
            && let Err(err) = self.import_mesh(&p)
        {
            self.status = err;
        }
    }

    /// Saves to `p`, or to the current file when `p` is None.
    pub fn save_path(&mut self, p: Option<PathBuf>) -> Result<(), String> {
        let Some(p) = p.or_else(|| self.path.clone()) else {
            self.status = "no file name yet; pass a path".into();
            return Err(self.status.clone());
        };
        if self.save_to(p) { Ok(()) } else { Err(self.status.clone()) }
    }

    /// Returns true when the project was written.
    pub fn save(&mut self) -> bool {
        match self.path.clone() {
            Some(p) => self.save_to(p),
            None => self.save_as(),
        }
    }

    pub fn save_as(&mut self) -> bool {
        if self.mcp.is_some() {
            self.status = MCP_NO_DIALOGS.into();
            return false;
        }
        let picked = rfd::FileDialog::new()
            .add_filter("CAD project", &["cadproj"])
            .set_file_name(format!("{}.cadproj", self.doc.name))
            .save_file();
        let Some(mut p) = picked else { return false };
        if !p.to_string_lossy().to_lowercase().ends_with(".cadproj") {
            let mut s = p.into_os_string();
            s.push(".cadproj");
            p = PathBuf::from(s);
        }
        self.save_to(p)
    }

    /// Writes SVG, DXF or PDF (visible 2D entities) or STL, OBJ or STEP (shown bodies and placed components) or CSV (bill of materials), chosen by the file extension;
    /// also glTF (.glb) or 3MF (shown bodies and placed components), IFC (the architectural model).
    pub fn export_path(&mut self, p: PathBuf) -> Result<(), String> {
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase());
        let bytes = match ext.as_deref() {
            Some("svg") => cad_import_export::to_svg(&self.doc, self.sketch).into_bytes(),
            Some("dxf") => cad_import_export::to_dxf(&self.doc, self.sketch).into_bytes(),
            Some("pdf") => cad_import_export::to_pdf(&self.doc, self.sketch),
            Some(mesh @ ("stl" | "obj" | "step" | "stp")) => {
                if !self.model_current() {
                    return Err("the model is still rebuilding; try again in a moment".into());
                }
                self.mesh_bytes(mesh)?
            }
            Some(ext @ ("glb" | "3mf")) => {
                if !self.model_current() {
                    return Err("the model is still rebuilding; try again in a moment".into());
                }
                let scene = self.scene()?;
                if ext == "glb" { cad_import_export::to_glb(&scene) } else { cad_import_export::to_3mf(&scene)? }
            }
            Some("ifc") => {
                if !self.model_current() {
                    return Err("the model is still rebuilding; try again in a moment".into());
                }
                cad_import_export::to_ifc(&self.doc, self.model_result().ok_or("no rebuild yet")?)?.into_bytes()
            }
            Some("csv") => {
                if !self.model_current() {
                    return Err("the model is still rebuilding; try again in a moment".into());
                }
                let rows = self.model_result().map(|r| self.doc.bom(r)).unwrap_or_default();
                if rows.is_empty() {
                    return Err("no components for a bill of materials".into());
                }
                cad_document::bom_csv(&rows).into_bytes()
            }
            _ => return Err("unknown export format; use .svg, .dxf, .pdf, .stl, .obj, .step, .glb, .3mf, .ifc or .csv".into()),
        };
        self.write_export(&p, bytes)
    }

    /// Writes `bytes` to `p` through a temporary file beside it, then sets the "Exported <name>" status.
    fn write_export(&mut self, p: &Path, bytes: Vec<u8>) -> Result<(), String> {
        // Write beside the target, then rename, like the project save.
        let mut tmp = p.to_path_buf().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        let result = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, p));
        if let Err(e) = result {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("Export failed: {e}"));
        }
        self.status = format!("Exported {}", p.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()));
        Ok(())
    }

    /// Writes the last motion study as CSV (`Document::motion_csv`).
    pub fn export_motion(&mut self, p: PathBuf) -> Result<(), String> {
        let run = self.motion_result().ok_or("no motion study; run one first")?;
        let collision = run.collision.as_ref().and_then(|c| c.as_ref().ok()).and_then(Option::as_ref);
        let bytes = self.doc.motion_csv(&run.motion, collision).into_bytes();
        self.write_export(&p, bytes)
    }

    /// "Export motion CSV": asks for a path and exports.
    pub fn export_motion_dialog(&mut self) {
        if self.mcp.is_some() {
            self.status = "File dialogs are off in MCP mode; use export_motion".into();
            return;
        }
        let picked = rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name(format!("{}_motion.csv", self.doc.name)).save_file();
        if let Some(p) = picked
            && let Err(err) = self.export_motion(p)
        {
            self.status = err;
        }
    }

    /// STL, OBJ or STEP (by extension) of the shown bodies and the placed components of the current rebuild (joints
    /// applied, no explode); solids are named `Body_<id>` or by the component name.
    fn mesh_bytes(&self, ext: &str) -> Result<Vec<u8>, String> {
        let mut named: Vec<(String, &cad_document::Body)> = self.shown_bodies().map(|(id, b)| (format!("Body_{id}"), &**b)).collect();
        named.extend(self.placed_components().map(|(id, b)| (self.doc.component(id).map_or_else(|| format!("Component_{id}"), |c| c.name.clone()), &**b)));
        let bodies: Vec<(&str, &cad_document::Body)> = named.iter().map(|(n, b)| (n.as_str(), *b)).collect();
        if bodies.is_empty() {
            return Err("no solid body to export".into());
        }
        Ok(match ext {
            "obj" => cad_import_export::to_obj(&bodies).into_bytes(),
            "step" | "stp" => cad_import_export::to_step(&bodies, &self.doc.name).into_bytes(),
            _ => cad_import_export::to_stl(&bodies.iter().map(|(_, b)| *b).collect::<Vec<_>>()),
        })
    }

    /// The shown bodies (each its own mesh, unplaced) and the placed components (one mesh per source body, placed by
    /// `RebuildResult::placements`) of the current rebuild, for glTF and 3MF.
    fn scene(&self) -> Result<cad_import_export::Scene<'_>, String> {
        let model = self.model_result().ok_or("no rebuild yet")?;
        let mut scene = cad_import_export::Scene { meshes: Vec::new(), items: Vec::new() };
        for (id, b) in self.shown_bodies() {
            scene.items.push((format!("Body_{id}"), scene.meshes.len(), DAffine3::IDENTITY));
            scene.meshes.push((format!("Body_{id}"), &**b));
        }
        let mut mesh_of: BTreeMap<FeatureId, usize> = BTreeMap::new();
        for (c, _) in self.placed_components() {
            let Some(comp) = self.doc.component(c) else { continue };
            let (Some(src), Some(at)) = (model.body(comp.body), model.placements.get(&c)) else { continue };
            let mesh = *mesh_of.entry(comp.body).or_insert_with(|| {
                scene.meshes.push((format!("Body_{}", comp.body), &**src));
                scene.meshes.len() - 1
            });
            scene.items.push((comp.name.clone(), mesh, *at));
        }
        if scene.items.is_empty() { Err("no solid body to export".into()) } else { Ok(scene) }
    }

    /// File menu entry: asks for a path and exports.
    pub fn export_dialog(&mut self, ext: &str) {
        if self.mcp.is_some() {
            self.status = "File dialogs are off in MCP mode; use export_project".into();
            return;
        }
        let picked = rfd::FileDialog::new()
            .add_filter(ext.to_uppercase(), &[ext])
            .set_file_name(format!("{}.{ext}", self.doc.name))
            .save_file();
        if let Some(p) = picked
            && let Err(e) = self.export_path(p)
        {
            self.status = e;
        }
    }

    fn save_to(&mut self, p: PathBuf) -> bool {
        let old_name = self.doc.name.clone();
        if let Some(stem) = p.file_stem() {
            self.doc.name = stem.to_string_lossy().into_owned();
        }
        match self.doc.save(&p) {
            Ok(()) => {
                self.saved_rev = self.doc.revision();
                self.settings.push_recent(&p);
                self.path = Some(p);
                self.status = "Saved".into();
                true
            }
            Err(e) => {
                self.doc.name = old_name;
                self.status = format!("Save failed: {e}");
                false
            }
        }
    }
}
