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
mod edge_pick;
mod face_pick;
mod gizmo;
mod glyphs;
mod groups;
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
mod panels;
mod rebuild_worker;
mod properties_panel;
mod toolbar;
mod view2d;
mod view2d_annot;
mod view2d_edit;
mod view2d_tools;
mod view2d_walls;
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
    }
    let options = eframe::NativeOptions { renderer: eframe::Renderer::Wgpu, depth_buffer: 32, viewport, ..Default::default() };
    eframe::run_native("ProjectCAD", options, Box::new(move |cc| Ok(Box::new(CadApp::new(cc, mcp)))))
}
