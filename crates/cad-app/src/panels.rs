use crate::app::{CadApp, Pending, SideTab, ViewMode};
use crate::settings::{ACCENTS, Dock, Layout, Theme};
use crate::widgets as w;
use crate::{constraints_panel, layers_panel, model_panel, properties_panel};
use cad_document::{Command, EntityId, Units};
use eframe::egui::{self, Button, Key, Modifiers};

#[derive(Clone, Copy)]
enum Action {
    New,
    Open,
    Import,
    ImportSvg,
    Save,
    Undo,
    Redo,
    Delete,
    Fit,
    Duplicate,
}

fn run(app: &mut CadApp, ctx: &egui::Context, action: Action) {
    match action {
        Action::New => app.request(ctx, Pending::New),
        Action::Open => app.request(ctx, Pending::Open),
        Action::Import => app.request(ctx, Pending::Import),
        Action::ImportSvg => app.request(ctx, Pending::ImportSvg),
        Action::Save => {
            app.save();
        }
        Action::Undo => app.undo(),
        Action::Redo => app.redo(),
        Action::Delete => app.delete_selection(),
        Action::Fit => app.fit_active(),
        Action::Duplicate => app.duplicate_selection(),
    }
}

pub fn shortcuts(app: &mut CadApp, ctx: &egui::Context) {
    let plain_keys_ok = !ctx.egui_wants_keyboard_input();
    let mut actions = Vec::new();
    let mut toggles = (false, false);
    let mut open = false;
    ctx.input_mut(|i| {
        let ctrl = Modifiers::COMMAND;
        if i.consume_key(ctrl, Key::K) {
            open = true;
        }
        // Shift variants first: consume_key ignores extra Shift.
        if i.consume_key(ctrl | Modifiers::SHIFT, Key::Z) || i.consume_key(ctrl, Key::Y) {
            actions.push(Action::Redo);
        }
        if i.consume_key(ctrl, Key::Z) {
            actions.push(Action::Undo);
        }
        if i.consume_key(ctrl, Key::N) {
            actions.push(Action::New);
        }
        if i.consume_key(ctrl, Key::O) {
            actions.push(Action::Open);
        }
        if i.consume_key(ctrl, Key::S) {
            actions.push(Action::Save);
        }
        if i.consume_key(ctrl, Key::D) {
            actions.push(Action::Duplicate);
        }
        if plain_keys_ok && i.consume_key(Modifiers::NONE, Key::F3) {
            toggles.0 = true;
        }
        if plain_keys_ok && i.consume_key(Modifiers::NONE, Key::F9) {
            toggles.1 = true;
        }
        if plain_keys_ok && i.consume_key(Modifiers::NONE, Key::Delete) {
            actions.push(Action::Delete);
        }
        if plain_keys_ok && i.consume_key(Modifiers::NONE, Key::F) {
            actions.push(Action::Fit);
        }
    });
    if open {
        crate::palette::open(app);
    }
    app.osnap ^= toggles.0;
    app.grid_snap ^= toggles.1;
    for a in actions {
        run(app, ctx, a);
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
                if menu_item(ui, true, "New", "Ctrl+N") {
                    run(app, &ctx, Action::New);
                }
                if menu_item(ui, true, "Open...", "Ctrl+O") {
                    run(app, &ctx, Action::Open);
                }
                if menu_item(ui, true, "Import SVG...", "") {
                    run(app, &ctx, Action::ImportSvg);
                }
                if menu_item(ui, true, "Import DXF...", "") {
                    run(app, &ctx, Action::Import);
                }
                if menu_item(ui, true, "Import STL/OBJ mesh...", "") {
                    app.import_mesh_dialog();
                }
                if menu_item(ui, true, "Save", "Ctrl+S") {
                    run(app, &ctx, Action::Save);
                }
                if menu_item(ui, true, "Save As...", "") {
                    app.save_as();
                }
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
            });
            w::menu_button(ui, "Edit", |ui| {
                if menu_item(ui, app.history.can_undo(), "Undo", "Ctrl+Z") {
                    run(app, &ctx, Action::Undo);
                }
                if menu_item(ui, app.history.can_redo(), "Redo", "Ctrl+Y") {
                    run(app, &ctx, Action::Redo);
                }
                if menu_item(ui, !app.selection.is_empty(), "Delete", "Del") {
                    run(app, &ctx, Action::Delete);
                }
            });
            w::menu_button(ui, "View", |ui| {
                if menu_item(ui, true, "2D", "") {
                    app.view = ViewMode::TwoD;
                }
                if menu_item(ui, true, "3D", "") {
                    app.view = ViewMode::ThreeD;
                }
                if menu_item(ui, true, "Fit", "F") {
                    run(app, &ctx, Action::Fit);
                }
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
