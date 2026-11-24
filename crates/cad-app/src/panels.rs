use crate::app::{CadApp, Pending, SideTab, ViewMode};
use crate::settings::{ACCENTS, Dock, Layout, Theme};
use crate::widgets as w;
use crate::{constraints_panel, layers_panel, model_panel, properties_panel};
use cad_document::{Command, EntityId, Units};
use eframe::egui::{self, Button};

use crate::camera::StdView;
use crate::keymap::Action;

/// Runs a shortcut or menu action.
pub fn run(app: &mut CadApp, ctx: &egui::Context, action: Action) {
    use crate::app::Tool;
    let tool = |app: &mut CadApp, t: Tool| {
        app.tool = t;
        app.points.clear();
        app.op_target = None;
    };
    match action {
        Action::New => app.request(ctx, Pending::New),
        Action::Open => app.request(ctx, Pending::Open),
        Action::Save => {
            app.save();
        }
        Action::SaveAs => {
            app.save_as();
        }
        Action::Settings => app.settings_open = !app.settings_open,
        Action::CommandSearch => crate::palette::open(app),
        Action::Undo => app.undo(),
        Action::Redo => app.redo(),
        Action::Delete => app.delete_selection(),
        Action::Duplicate => app.duplicate_selection(),
        Action::SelectAll => {
            let ids: Vec<EntityId> = app.doc.entities_in(app.sketch).filter(|e| app.pickable(e)).map(|e| e.id).collect();
            app.set_selection(ids);
        }
        Action::DeselectAll => {
            app.set_selection([]);
            crate::assembly_panel::select(app, None);
        }
        Action::InvertSelection => {
            let ids: Vec<EntityId> = app.doc.entities_in(app.sketch).filter(|e| app.pickable(e) && !app.selection.contains(&e.id)).map(|e| e.id).collect();
            app.set_selection(ids);
        }
        Action::Grab => crate::transform::start(app, ctx, crate::transform::Mode::Grab),
        Action::Rotate => crate::transform::start(app, ctx, crate::transform::Mode::Rotate),
        Action::Scale => crate::transform::start(app, ctx, crate::transform::Mode::Scale),
        Action::NudgeLeft | Action::NudgeRight | Action::NudgeUp | Action::NudgeDown => nudge(app, ctx, action),
        Action::Fit => app.fit_active(),
        Action::FocusSelection => focus_selection(app),
        Action::Toggle2D3D => {
            app.view = if app.view == ViewMode::TwoD { ViewMode::ThreeD } else { ViewMode::TwoD };
        }
        Action::ViewFront => std_view(app, StdView::Front),
        Action::ViewBack => std_view(app, StdView::Back),
        Action::ViewRight => std_view(app, StdView::Right),
        Action::ViewLeft => std_view(app, StdView::Left),
        Action::ViewTop => std_view(app, StdView::Top),
        Action::ViewBottom => std_view(app, StdView::Bottom),
        Action::ViewIso => std_view(app, StdView::Iso),
        Action::ToggleOrtho => {
            app.view = ViewMode::ThreeD;
            app.cam3d.ortho = !app.cam3d.ortho;
        }
        Action::TogglePanels => app.focus_mode = !app.focus_mode,
        Action::ToggleOsnap => app.osnap = !app.osnap,
        Action::ToggleGridSnap => app.grid_snap = !app.grid_snap,
        Action::ToggleGlyphs => app.show_glyphs = !app.show_glyphs,
        Action::GizmoAll => app.gizmo_mode = crate::gizmo::GizmoMode::All,
        Action::GizmoMove => app.gizmo_mode = crate::gizmo::GizmoMode::Move,
        Action::GizmoRotate => app.gizmo_mode = crate::gizmo::GizmoMode::Rotate,
        Action::ToolSelect => tool(app, Tool::Select),
        Action::ToolLine => tool(app, Tool::Line),
        Action::ToolCircle => tool(app, Tool::Circle),
        Action::ToolRect => tool(app, Tool::Rect),
        Action::ToolArc => tool(app, Tool::Arc),
        Action::ToolPolyline => tool(app, Tool::Polyline),
        Action::ToolText => tool(app, Tool::Text),
        Action::ToolDimension => tool(app, Tool::Dimension),
        Action::ToolWall => tool(app, Tool::Wall),
        Action::ToolTrim => tool(app, Tool::Trim),
        Action::ToolOffset => tool(app, Tool::Offset),
        Action::ToolMove => tool(app, Tool::Move),
        Action::ToolCopy => tool(app, Tool::Copy),
        Action::ToolRotateRef => tool(app, Tool::RotateRef),
        Action::ToolScaleRef => tool(app, Tool::ScaleRef),
        Action::ToolMirror => tool(app, Tool::Mirror),
        Action::ToolStretch => tool(app, Tool::Stretch),
    }
}

/// Turns the 3D camera to a standard view (switching to 3D), gliding when transitions are on.
pub fn std_view(app: &mut CadApp, v: StdView) {
    app.view = ViewMode::ThreeD;
    crate::navigation::glide(app, |app| app.cam3d.set_view(v));
}

/// Arrow keys: in 2D they move the selection by the nudge distance (Shift: ten times); in 3D they orbit 15 degrees.
fn nudge(app: &mut CadApp, ctx: &egui::Context, action: Action) {
    let dir = match action {
        Action::NudgeLeft => cad_geometry::DVec2::NEG_X,
        Action::NudgeRight => cad_geometry::DVec2::X,
        Action::NudgeUp => cad_geometry::DVec2::Y,
        _ => cad_geometry::DVec2::NEG_Y,
    };
    if app.view == ViewMode::ThreeD {
        // `OrbitCamera::orbit` turns 0.01 rad per point.
        let step = 15f32.to_radians() / 0.01;
        app.cam3d.orbit(-dir.x as f32 * step, -dir.y as f32 * step);
        return;
    }
    if app.selection.is_empty() {
        return;
    }
    let big = if ctx.input(|i| i.modifiers.shift) { 10.0 } else { 1.0 };
    let d = (dir * app.settings.general.nudge_mm * big).extend(0.0);
    let cmds: Vec<Command> = app
        .selection
        .iter()
        .filter_map(|id| app.doc.get(*id))
        .map(|e| {
            let mut shape = e.shape.clone();
            shape.translate(d);
            Command::SetShape { id: e.id, shape }
        })
        .collect();
    let pinned = app.selection.clone();
    app.exec_pinned(cmds, &pinned);
}

/// Fits the active view to the selection: entities (2D: sketch coordinates; 3D: world), else the selected
/// component; everything when nothing is selected.
fn focus_selection(app: &mut CadApp) {
    let ents: Vec<_> = app.selection.iter().filter_map(|id| app.doc.get(*id)).cloned().collect();
    let bounds = match app.view {
        ViewMode::TwoD => ents.iter().map(|e| e.shape.bbox()).reduce(|(a, b), (c, d)| (a.min(c), b.max(d))),
        ViewMode::ThreeD => ents
            .iter()
            .filter_map(|e| app.entity_bounds(e))
            .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
            .or_else(|| app.component_sel.and_then(|id| app.model_result()?.component(id)).map(|b| b.bbox())),
    };
    let Some((lo, hi)) = bounds else {
        app.fit_active();
        return;
    };
    crate::navigation::glide(app, |app| match app.view {
        ViewMode::TwoD => {
            let pad = (hi - lo).truncate().max(cad_geometry::DVec2::splat(10.0)) * 0.25;
            let r = app.view_rect;
            app.cam2d.fit(r, lo.truncate() - pad, hi.truncate() + pad);
        }
        ViewMode::ThreeD => app.cam3d.fit(lo, hi),
    });
}

/// Writes a copy of a dirty project to `<settings folder>/autosave/<name>.cadproj` every `autosave_min` minutes. The
/// project file, its path and its saved state are untouched; MCP mode and a missing settings file write nothing.
pub fn autosave(app: &mut CadApp) {
    let min = app.settings.general.autosave_min;
    if min == 0 || app.mcp.is_some() || app.autosaved.elapsed().as_secs() < u64::from(min) * 60 {
        return;
    }
    app.autosaved = std::time::Instant::now();
    let Some(dir) = app.settings_path.as_deref().and_then(|p| p.parent()).map(|d| d.join("autosave")) else { return };
    if !app.is_dirty() {
        return;
    }
    let name: String = app.doc.name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let p = dir.join(format!("{name}.cadproj"));
    let r = std::fs::create_dir_all(&dir).map_err(|e| e.to_string()).and_then(|()| app.doc.save(&p).map_err(|e| e.to_string()));
    match r {
        Ok(()) => app.status = format!("Autosaved to {}", p.display()),
        Err(err) => app.status = format!("Autosave failed: {err}"),
    }
}

pub fn shortcuts(app: &mut CadApp, ctx: &egui::Context) {
    // A running modal, a key being captured in the Controls settings, or fly keys take the keyboard first.
    if app.modal.is_some() || app.rebind.is_some() || crate::navigation::flying(app, ctx) {
        return;
    }
    for a in crate::keymap::consume(&app.settings.controls, ctx) {
        run(app, ctx, a);
    }
}

/// A menu entry that runs `action`, showing its first chord.
fn action_item(app: &mut CadApp, ui: &mut egui::Ui, ctx: &egui::Context, enabled: bool, label: &str, action: Action) {
    let chord = app.settings.controls.label(action);
    if menu_item(ui, enabled, label, &chord) {
        run(app, ctx, action);
    }
}

fn menu_item(ui: &mut egui::Ui, enabled: bool, label: &str, shortcut: &str) -> bool {
    let clicked = w::add_enabled(ui, enabled, label, Button::new(label).shortcut_text(shortcut)).clicked();
    if clicked {
        ui.close();
    }
    clicked
}

pub fn menu_bar(app: &mut CadApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    egui::Panel::top("menu").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            w::menu_button(ui, "File", |ui| {
                action_item(app, ui, &ctx, true, "New", Action::New);
                action_item(app, ui, &ctx, true, "Open...", Action::Open);
                w::menu_button(ui, "Open recent", |ui| {
                    if app.settings.recent.is_empty() {
                        ui.label("No recent projects");
                    }
                    for (i, p) in app.settings.recent.clone().iter().enumerate() {
                        let name = std::path::Path::new(p).file_name().map_or(p.clone(), |n| n.to_string_lossy().into_owned());
                        if w::add(ui, &format!("Recent: {name}"), Button::new(&name)).on_hover_text(p).clicked() {
                            app.request(&ctx, Pending::OpenRecent(i));
                            ui.close();
                        }
                    }
                    ui.separator();
                    if menu_item(ui, !app.settings.recent.is_empty(), "Clear recent", "") {
                        app.settings.recent.clear();
                    }
                });
                if menu_item(ui, true, "Import SVG...", "") {
                    app.request(&ctx, Pending::ImportSvg);
                }
                if menu_item(ui, true, "Import DXF...", "") {
                    app.request(&ctx, Pending::Import);
                }
                if menu_item(ui, true, "Import STL/OBJ mesh...", "") {
                    app.import_mesh_dialog();
                }
                action_item(app, ui, &ctx, true, "Save", Action::Save);
                action_item(app, ui, &ctx, true, "Save As...", Action::SaveAs);
                if menu_item(ui, true, "Export SVG...", "") {
                    app.export_dialog("svg");
                }
                if menu_item(ui, true, "Export DXF...", "") {
                    app.export_dialog("dxf");
                }
                if menu_item(ui, true, "Export PDF...", "") {
                    app.export_dialog("pdf");
                }
                if menu_item(ui, true, "Export STL...", "") {
                    app.export_dialog("stl");
                }
                if menu_item(ui, true, "Export OBJ...", "") {
                    app.export_dialog("obj");
                }
                if menu_item(ui, true, "Export STEP...", "") {
                    app.export_dialog("step");
                }
                if menu_item(ui, true, "Export glTF...", "") {
                    app.export_dialog("glb");
                }
                if menu_item(ui, true, "Export 3MF...", "") {
                    app.export_dialog("3mf");
                }
                if menu_item(ui, true, "Export IFC...", "") {
                    app.export_dialog("ifc");
                }
                if menu_item(ui, true, "Export BOM CSV...", "") {
                    app.export_dialog("csv");
                }
                ui.separator();
                action_item(app, ui, &ctx, true, "Settings...", Action::Settings);
            });
            w::menu_button(ui, "Edit", |ui| {
                action_item(app, ui, &ctx, app.history.can_undo(), "Undo", Action::Undo);
                action_item(app, ui, &ctx, app.history.can_redo(), "Redo", Action::Redo);
                ui.separator();
                let some = !app.selection.is_empty();
                action_item(app, ui, &ctx, some, "Delete", Action::Delete);
                action_item(app, ui, &ctx, some, "Duplicate", Action::Duplicate);
                action_item(app, ui, &ctx, true, "Select all", Action::SelectAll);
                action_item(app, ui, &ctx, true, "Deselect all", Action::DeselectAll);
                action_item(app, ui, &ctx, true, "Invert selection", Action::InvertSelection);
                ui.separator();
                action_item(app, ui, &ctx, true, "Grab", Action::Grab);
                action_item(app, ui, &ctx, true, "Rotate", Action::Rotate);
                action_item(app, ui, &ctx, true, "Scale", Action::Scale);
            });
            w::menu_button(ui, "View", |ui| {
                if menu_item(ui, true, "2D", "") {
                    app.view = ViewMode::TwoD;
                }
                if menu_item(ui, true, "3D", "") {
                    app.view = ViewMode::ThreeD;
                }
                action_item(app, ui, &ctx, true, "Fit", Action::Fit);
                action_item(app, ui, &ctx, true, "Focus selection", Action::FocusSelection);
                w::menu_button(ui, "Standard views", |ui| {
                    for (label, a) in [
                        ("Front", Action::ViewFront),
                        ("Back", Action::ViewBack),
                        ("Right", Action::ViewRight),
                        ("Left", Action::ViewLeft),
                        ("Top", Action::ViewTop),
                        ("Bottom", Action::ViewBottom),
                        ("Isometric", Action::ViewIso),
                    ] {
                        action_item(app, ui, &ctx, true, label, a);
                    }
                });
                let ortho = if app.cam3d.ortho { "Perspective" } else { "Orthographic" };
                action_item(app, ui, &ctx, true, ortho, Action::ToggleOrtho);
                action_item(app, ui, &ctx, true, if app.focus_mode { "Show panels" } else { "Hide panels" }, Action::TogglePanels);
                w::checkbox(ui, &mut app.show_glyphs, "Constraint glyphs");
                ui.separator();
                // Each entry closes the menu like `menu_item`, so a script reopens "View" before the next one.
                for (theme, name) in [(Theme::Dark, "Dark theme"), (Theme::Light, "Light theme")] {
                    if w::selectable(ui, app.settings.theme == theme, name).clicked() {
                        app.settings.theme = theme;
                        ui.close();
                    }
                }
                for (name, rgb) in ACCENTS {
                    if w::selectable(ui, app.settings.accent == rgb, &format!("Accent: {name}")).clicked() {
                        app.settings.accent = rgb;
                        ui.close();
                    }
                }
                ui.separator();
                for tab in SideTab::ALL {
                    ui.horizontal(|ui| {
                        ui.label(tab.name());
                        for slot in [Dock::Left, Dock::Right, Dock::Bottom] {
                            let on = app.settings.layout.docks[tab.index()] == slot;
                            if w::add(ui, &format!("{}: {}", tab.name(), slot.name()), Button::selectable(on, slot.name())).clicked() {
                                app.settings.layout.docks[tab.index()] = slot;
                                app.settings.layout.shown[slot.index()] = true;
                                ui.close();
                            }
                        }
                    });
                }
                for slot in [Dock::Left, Dock::Right, Dock::Bottom] {
                    if w::checkbox(ui, &mut app.settings.layout.shown[slot.index()], &format!("{} panel", slot.name())).clicked() {
                        ui.close();
                    }
                }
                if menu_item(ui, true, "Reset layout", "") {
                    app.settings.layout = Layout::default();
                    app.layout_gen += 1;
                }
            });
            w::menu_button(ui, "Help", |ui| {
                if menu_item(ui, true, "Keyboard shortcuts", "") {
                    app.settings_open = true;
                    app.settings_page = crate::settings_window::Page::Controls;
                }
                ui.label(format!("ProjectCAD {}", env!("CARGO_PKG_VERSION")));
            });
            w::menu_button(ui, "Plugins", |ui| {
                for m in app.plugins.clone() {
                    if menu_item(ui, app.plugin.is_none(), &format!("Plugin: {}", m.name), "") {
                        crate::plugins::start(app, &m, &ctx);
                    }
                }
                if app.plugins.is_empty() {
                    ui.label(match &app.plugins_dir {
                        Some(d) => format!("No plugins in {}", d.display()),
                        None => "No plugins folder (no settings file)".to_string(),
                    });
                }
                for err in &app.plugin_errors {
                    ui.label(err);
                }
                ui.separator();
                if menu_item(ui, app.plugin.is_some(), "Stop plugin", "") {
                    app.plugin = None;
                    app.status = "Plugin stopped".into();
                }
                if menu_item(ui, true, "Reload plugins", "") {
                    crate::plugins::reload(app);
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if w::add(ui, "Open settings", Button::selectable(app.settings_open, "Settings")).clicked() {
                    app.settings_open = !app.settings_open;
                }
                ui.separator();
                // Picking a panel shows its slot if it was hidden.
                for tab in [SideTab::Assembly, SideTab::Model, SideTab::Sketch] {
                    if w::selectable_value(ui, &mut app.side_tab, tab, tab.name()).clicked() {
                        app.settings.layout.shown[app.settings.layout.docks[tab.index()].index()] = true;
                    }
                }
            });
        });
    });
}

/// The Sketch, Model and Assembly panels in their slots (`Layout::docks`): right, then left, then bottom (above the
/// status bar), each resizable and drawn only when shown and not empty. A slot shows the current tab when it holds
/// it, else its first panel; the units row heads the slot that shows the current tab. Dragged sizes go back into
/// the layout while a pointer button is down (written at the release, `settings::sync`).
pub fn side_panels(app: &mut CadApp, ui: &mut egui::Ui, slots: &[Dock]) {
    let ctx = ui.ctx().clone();
    for &slot in slots {
        let layout = &app.settings.layout;
        let Some(tab) = layout.shown_tab(slot, app.side_tab).filter(|_| layout.shown[slot.index()]) else { continue };
        let size = layout.sizes[slot.index()];
        // The right slot keeps today's id and 240 points, so the default screen is unchanged.
        let id = format!("{}{}", if slot == Dock::Right { "side" } else { slot.name() }, app.layout_gen);
        let (panel, range) = match slot {
            Dock::Left => (egui::Panel::left(id), 180.0..=600.0),
            Dock::Right => (egui::Panel::right(id), 180.0..=600.0),
            Dock::Bottom => (egui::Panel::bottom(id), 120.0..=500.0),
        };
        let resp = panel.resizable(true).default_size(size).size_range(range).show(ui, |ui| {
            if tab == app.side_tab {
                units_row(app, ui);
                ui.separator();
            }
            tab_contents(app, ui, tab, &ctx);
        });
        if ui.input(|i| i.pointer.any_down()) {
            let r = resp.response.rect;
            app.settings.layout.sizes[slot.index()] = if slot == Dock::Bottom { r.height() } else { r.width() }.round();
        }
    }
}

fn tab_contents(app: &mut CadApp, ui: &mut egui::Ui, tab: SideTab, ctx: &egui::Context) {
    if tab == SideTab::Model {
        egui::ScrollArea::vertical().id_salt("model_side").show(ui, |ui| model_panel::show(app, ui));
        return;
    }
    if tab == SideTab::Assembly {
        egui::ScrollArea::vertical().id_salt("assembly_side").show(ui, |ui| crate::assembly_panel::show(app, ui));
        return;
    }
    ui.heading("Objects");
    let items: Vec<(EntityId, String, bool)> =
        app.doc.entities_in(app.sketch).map(|e| (e.id, e.name.clone(), app.pickable(e))).collect();
    egui::ScrollArea::vertical().id_salt("objects").max_height(ui.available_height() / 2.0).show(ui, |ui| {
        for (id, name, pickable) in items {
            let selected = app.selection.contains(&id);
            if w::selectable(ui, selected, &name).clicked() && pickable {
                let shift = ui.input(|i| i.modifiers.shift);
                app.click_select(Some(id), shift);
            }
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().id_salt("side_rest").show(ui, |ui| {
        layers_panel::layers_section(app, ui);
        ui.separator();
        ui.heading("Properties");
        properties_panel::properties(app, ui, ctx);
        ui.separator();
        constraints_panel::show(app, ui);
    });
}

fn units_row(app: &mut CadApp, ui: &mut egui::Ui) {
    let mut s = *app.doc.settings();
    let (old_units, old_precision) = (s.units, s.precision);
    ui.horizontal(|ui| {
        ui.label("Units");
        w::combo(ui, "Units", egui::ComboBox::from_id_salt("units").selected_text(s.units.suffix()).width(50.0), |ui| {
            for u in Units::ALL {
                w::selectable_value(ui, &mut s.units, u, u.suffix());
            }
        });
        ui.label("Decimals");
        w::add(ui, "Decimals", egui::DragValue::new(&mut s.precision).range(0..=8));
    });
    if s.units != old_units {
        app.exec_merged(vec![Command::SetSettings(s)], u64::MAX - 1);
    } else if s.precision != old_precision {
        app.exec_merged(vec![Command::SetSettings(s)], u64::MAX - 2);
    }
}

pub fn status_bar(app: &mut CadApp, ui: &mut egui::Ui) {
    egui::Panel::bottom("status").show(ui, |ui| {
        ui.horizontal(|ui| {
            crate::coord_input::show(app, ui);
            if let Some((kind, secs)) = app.jobs.running() {
                ui.spinner();
                ui.label(format!("Running {}: {secs:.1} s", kind.name()));
                if w::button(ui, "Cancel job").clicked() {
                    app.jobs.cancel();
                    app.status = format!("Cancelled the {}", kind.name());
                }
                // The seconds tick while nothing else repaints.
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
                ui.separator();
            }
            ui.label(if app.status.is_empty() && app.rebuilding() { "Rebuilding..." } else { &app.status });
            ui.separator();
            let (s, c) = (app.doc.settings(), app.cursor_world);
            ui.label(format!("X {}  Y {} {}", s.format_length(c.x), s.format_length(c.y), s.units.suffix()));
            ui.separator();
            if app.settings.ui.frame_time {
                ui.label(format!("{:.1} ms", app.frame_avg));
                ui.separator();
            }
            if app.view == ViewMode::ThreeD && app.cam3d.ortho {
                ui.label("Ortho");
                ui.separator();
            }
            ui.label(format!("Entities: {}", app.doc.len()));
            ui.label(format!("Selected: {}", app.selection.len()));
            ui.separator();
            ui.label(format!("{}{}", app.doc.name, if app.is_dirty() { "*" } else { "" }));
        });
    });
}

pub fn update_title(app: &mut CadApp, ctx: &egui::Context) {
    let file = app.path.as_ref().and_then(|p| p.file_name()).map_or("Untitled".to_string(), |n| n.to_string_lossy().into_owned());
    let prefix = if app.mcp.is_some() { "ProjectCAD (MCP)" } else { "ProjectCAD" };
    let title = format!("{prefix} - {file}{}", if app.is_dirty() { "*" } else { "" });
    if title != app.last_title {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
        app.last_title = title;
    }
}

pub fn confirm_dialog(app: &mut CadApp, ctx: &egui::Context) {
    let Some(pending) = app.confirm else { return };
    let mut choice = None;
    egui::Window::new("Unsaved changes")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label("The project has unsaved changes.");
            ui.horizontal(|ui| {
                for (label, c) in [("Save", 0), ("Discard", 1), ("Cancel", 2)] {
                    if w::button(ui, label).clicked() {
                        choice = Some(c);
                    }
                }
            });
        });
    let Some(choice) = choice else { return };
    app.confirm = None;
    if choice == 1 || (choice == 0 && app.save()) {
        app.perform(ctx, pending);
    }
}
