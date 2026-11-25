// Release builds on Windows are GUI programs: no console window opens with the app. Piped stdin/stdout (the MCP
// mode) still work, since a parent that pipes them hands the handles over.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod analysis_worker;
mod app;
mod app_files;
mod app_ops;
mod arrange;
mod assembly_check;
mod assembly_joints;
mod assembly_panel;
mod camera;
mod constraint_options;
mod coord_input;
mod dim_link;
mod constraints_panel;
mod layers_panel;
mod mass_panel;
mod mesh_panel;
mod study_panel;
mod validation_panel;
mod motion_panel;
mod edge_pick;
mod face_pick;
mod gizmo;
mod keymap;
mod navigation;
mod glyphs;
mod groups;
mod i18n;
mod floor_panel;
mod hatch_panel;
mod mcp;
mod model_levels;
mod model_sketches;
mod model_features;
mod model_shell;
mod model_section;
mod model_pattern;
mod model_parts;
mod model_sweep;
mod model_panel;
mod palette;
mod panels;
mod plugins;
mod rebuild_worker;
mod scene_mesh;
mod settings;
mod settings_window;
mod transform;
mod tutorial;
mod properties_panel;
mod toolbar;
mod view2d;
mod view2d_annot;
mod view2d_edit;
mod view2d_tools;
mod view2d_walls;
mod view2d_xform;
mod view3d;
mod widgets;

use app::CadApp;
use eframe::egui;

fn main() -> eframe::Result {
    let mcp = std::env::args().any(|a| a == "--mcp");
    let mut viewport = egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]);
    if mcp {
        // The window must not steal the focus from the user.
        viewport = viewport.with_active(false).with_title("ProjectCAD (MCP)");
        // --background: off-screen, not minimized (a minimized window stops rendering and screenshots fail).
        // CAD_MCP_SHOW=1 overrides it to watch a test run.
        if std::env::args().any(|a| a == "--background") && std::env::var_os("CAD_MCP_SHOW").is_none() {
            viewport = viewport.with_position([-20000.0, -20000.0]).with_taskbar(false);
        }
    }
    let options = eframe::NativeOptions { renderer: eframe::Renderer::Wgpu, depth_buffer: 32, viewport, ..Default::default() };
    let run = || eframe::run_native("ProjectCAD", options, Box::new(move |cc| Ok(Box::new(CadApp::new(cc, mcp)))));
    if mcp {
        return run();
    }
    // The app has no console (GUI subsystem), so a start-up error or a panic that ends the app would vanish: report
    // them in crash.log and a message box. Panics the app catches itself (rebuild, studies) are not reported.
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        if std::thread::current().name() == Some("main") {
            *LAST_PANIC.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(format!("{info}\n\n{}", std::backtrace::Backtrace::force_capture()));
        }
    }));
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(Err(err)) => {
            report_crash(&format!("ProjectCAD could not start: {err}"));
            Err(err)
        }
        Ok(ok) => ok,
        Err(payload) => {
            let text = LAST_PANIC.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default();
            report_crash(&format!("ProjectCAD crashed: {text}"));
            std::panic::resume_unwind(payload)
        }
    }
}

/// The message and backtrace of the main thread's last panic, for `report_crash`.
static LAST_PANIC: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn report_crash(text: &str) {
    let log = settings::path(false).and_then(|p| Some(p.parent()?.join("crash.log")));
    let mut shown = text.to_string();
    if let Some(log) = &log {
        let _ = std::fs::create_dir_all(log.parent().unwrap_or(log));
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let entry = format!("--- {stamp} (unix time), version {}\n{text}\n", env!("CARGO_PKG_VERSION"));
        let written = std::fs::OpenOptions::new().create(true).append(true).open(log).and_then(|mut f| {
            use std::io::Write;
            f.write_all(entry.as_bytes())
        });
        if written.is_ok() {
            // The backtrace is long: the box keeps the first lines and points to the file.
            shown = text.lines().take(12).collect::<Vec<_>>().join("\n");
            shown.push_str(&format!("\n\nDetails were saved to {}", log.display()));
        }
    }
    rfd::MessageDialog::new().set_level(rfd::MessageLevel::Error).set_title("ProjectCAD").set_description(shown).show();
}
