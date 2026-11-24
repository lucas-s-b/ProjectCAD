//! The Model tab's "Analysis mesh" section: a body's tetrahedral mesh for FEA, on demand, and what the 3D view draws.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::MeshReport;
use eframe::egui::{self, Color32, RichText, TextEdit};

/// The last mesh run: the report or the error, and how long it took.
pub struct MeshRun {
    pub result: Result<MeshReport, String>,
    pub ms: f64,
}

/// Body choice, element size, "Mesh for analysis", then the result. The body defaults to the one of the selected
/// face, else the last body.
pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let labels = crate::assembly_panel::body_labels(app);
    if labels.is_empty() {
        return;
    }
    ui.separator();
    ui.heading(crate::i18n::tr("Analysis mesh"));
    if !labels.iter().any(|(id, _)| *id == app.mesh_body) {
        app.mesh_body = app.face_sel.map(|(b, _)| b).filter(|b| labels.iter().any(|(id, _)| id == b)).unwrap_or(labels[labels.len() - 1].0);
    }
    let current = labels.iter().find(|(id, _)| *id == app.mesh_body).map_or_else(String::new, |(_, t)| t.clone());
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::i18n::tr("Body"));
        w::combo(ui, "Mesh body", egui::ComboBox::from_id_salt("mesh_body").selected_text(current).width(140.0), |ui| {
            for (id, text) in &labels {
                w::selectable_value(ui, &mut app.mesh_body, *id, text);
            }
        });
    });
    // The mesher reads the last rebuild, so it waits until that matches the document.
    let (ready, mut run) = (app.model_current(), false);
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::i18n::tr("Element size"));
        w::add(ui, "Element size", TextEdit::singleline(&mut app.mesh_size).desired_width(70.0));
        run = w::add_enabled(ui, ready, "Mesh for analysis", egui::Button::new("Mesh for analysis")).clicked();
    });
    if run {
        mesh(app);
    }
    rows(app, ui);
}

/// Meshes the chosen body of the current rebuild on the UI thread (at most `MAX_CELLS` grid cells keeps it short);
/// the status line gives the counts or the error.
fn mesh(app: &mut CadApp) {
    let Some(r) = app.model_result() else { return };
    let start = std::time::Instant::now();
    let result = app.doc.mesh_body(r, app.mesh_body, &app.mesh_size);
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    app.status = match &result {
        Ok(rep) => format!("Mesh: {} nodes, {} elements", rep.stats.nodes, rep.stats.elements),
        Err(err) => format!("mesh: {err}"),
    };
    app.fea_mesh = Some((app.doc.revision(), MeshRun { result, ms }));
    app.show_mesh = true;
    // The scene shows the mesh (see the stamp in `view3d::show`).
    app.sel_rev += 1;
}

/// Counts, quality, volume against the body, the accuracy label and the display toggles; the error in red.
fn rows(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some(run) = app.mesh_result() else {
        ui.small("Meshing runs on demand; any edit clears the result.");
        return;
    };
    let rep = match &run.result {
        Ok(rep) => rep,
        Err(err) => {
            ui.label(RichText::new(err.as_str()).color(Color32::RED));
            return;
        }
    };
    let s = app.doc.settings();
    let st = &rep.stats;
    ui.label(format!("Nodes {}, elements {} (10-node tetrahedra)", st.nodes, st.elements));
    ui.label(format!("Min radius ratio {:.3}, min dihedral {:.1}\u{b0}, inverted {}", st.min_ratio, st.min_dihedral, st.inverted));
    ui.label(format!("Volume {} of {} (error {:+.2} %)", s.format_volume(st.volume), s.format_volume(rep.body_volume), 100.0 * rep.volume_error()));
    if rep.mesh.unsnapped > 0 {
        ui.small(format!("{} boundary nodes kept off the surface (moving them would spoil elements)", rep.mesh.unsnapped));
    }
    crate::motion_panel::accuracy(ui, rep.accuracy);
    let changed = ui
        .horizontal_wrapped(|ui| w::checkbox(ui, &mut app.show_mesh, "Show mesh").changed() | w::checkbox(ui, &mut app.mesh_shrink, "Shrink elements").changed())
        .inner;
    if changed {
        app.sel_rev += 1;
    }
}

/// The mesh the 3D view draws: the last run's while the document is unchanged and "Show mesh" is on.
pub fn shown(app: &CadApp) -> Option<&MeshReport> {
    if !app.show_mesh {
        return None;
    }
    app.mesh_result()?.result.as_ref().ok()
}
