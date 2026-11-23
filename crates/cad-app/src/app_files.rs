use crate::app::{CadApp, Pending, PickFace};
use cad_document::Document;
use eframe::egui;
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
                self.reset(doc, Some(p));
                Ok(())
            }
            Err(e) => {
                self.status = format!("Open failed: {e}");
                Err(self.status.clone())
            }
        }
    }

    /// Replaces the document with the contents of a DXF or SVG file (by extension), without a dialog and without the unsaved-changes guard.
    pub fn import_path(&mut self, p: PathBuf) -> Result<String, String> {
        let text = std::fs::read_to_string(&p).map_err(|e| format!("Import failed: {e}"));
        let name = p.file_stem().map_or_else(|| "Untitled".to_string(), |s| s.to_string_lossy().into_owned());
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase());
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

    /// Writes SVG, DXF or PDF (visible 2D entities) or STL, OBJ or STEP (shown bodies and placed components) or CSV (bill of materials), chosen by the file extension.
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
            _ => return Err("unknown export format; use .svg, .dxf, .pdf, .stl, .obj, .step or .csv".into()),
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
