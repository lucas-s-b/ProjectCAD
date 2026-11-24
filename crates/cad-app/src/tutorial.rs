//! The first-run tutorial: a few steps about the window with Back, Next and Skip. It opens at the first start (until
//! finished or skipped, `Settings::tutorial_done`) and again from Help > Tutorial; never in MCP mode.
use crate::app::CadApp;
use crate::i18n::{tr, tr_or};
use crate::widgets as w;
use eframe::egui::{self, Button, RichText};

/// (key, English title, English text) per step; `<key>.title` and `<key>.text` name the translations in `i18n/*.txt`.
pub const STEPS: [(&str, &str, &str); 8] = [
    (
        "tut.welcome",
        "Welcome to ProjectCAD",
        "This short tour shows the main parts of the window. Skip it at any time; Help > Tutorial opens it again. First, pick your language:",
    ),
    (
        "tut.menus",
        "Menus and command search",
        "The menu bar holds File (new, open, save, import and export), Edit, View, Help and Plugins. Press Ctrl+K to search every command by name.",
    ),
    (
        "tut.tools",
        "Drawing tools",
        "The toolbar on the left switches between the 2D and 3D views and holds the drawing tools: lines, circles, rectangles, arcs, text, dimensions, walls, doors and more. Pick a tool, then click points in the 2D view. Esc stops a tool that chains, such as Line or Wall.",
    ),
    (
        "tut.snap",
        "Snapping and exact input",
        "Object snap (F3) catches ends, midpoints, centres and intersections; grid snap (F9) catches the grid. For exact points, type coordinates in the box in the status bar: x,y for absolute, @dx,dy for relative, @d<a for a distance and an angle.",
    ),
    (
        "tut.nav",
        "Moving around the view",
        "The mouse wheel zooms. Middle drag pans and right drag orbits the 3D view. View > Fit shows everything; View > Standard views turns the camera to the front, the top, an isometric view and more. Settings > Navigation changes the mouse buttons.",
    ),
    (
        "tut.panels",
        "Panels",
        "The Sketch, Model and Assembly panels sit on the right. Sketch lists objects, layers, properties and constraints; Model holds sketches, 3D features and studies; Assembly holds components, joints and the bill of materials. The buttons at the top right switch panels; the View menu docks them left, right or bottom.",
    ),
    (
        "tut.model",
        "From sketch to solid",
        "Draw a closed profile in 2D, then use the Model panel to extrude, revolve, sweep or loft it into a solid and add fillets, chamfers, shells and patterns. The Box, Cylinder and Sphere buttons in the toolbar add ready-made solids. Switch to 3D to see the result.",
    ),
    (
        "tut.settings",
        "Settings and shortcuts",
        "Settings (Ctrl+Comma) change the theme, the UI scale, the language, the mouse navigation and every keyboard shortcut, with presets that match other programs. That is the tour: happy designing!",
    ),
];

pub fn show(app: &mut CadApp, ctx: &egui::Context) {
    let Some(step) = app.tutorial else { return };
    let step = step.min(STEPS.len() - 1);
    let (key, title, text) = STEPS[step];
    let last = step + 1 == STEPS.len();
    let (mut next, mut done) = (Some(step), false);
    egui::Window::new(tr("Tutorial"))
        .id(egui::Id::unique("tutorial"))
        .collapsible(false)
        .resizable(false)
        .default_width(440.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(RichText::new(format!("{} {} / {}", tr("Step"), step + 1, STEPS.len())).weak().small());
            ui.heading(tr_or(&format!("{key}.title"), title));
            ui.add_space(4.0);
            ui.label(tr_or(&format!("{key}.text"), text));
            if step == 0 {
                ui.add_space(6.0);
                crate::settings_window::language_row(app, ui);
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if w::add_enabled(ui, step > 0, "Tutorial: back", Button::new(tr("Previous"))).clicked() {
                    next = Some(step.saturating_sub(1));
                }
                if w::add(ui, "Tutorial: next", Button::new(tr(if last { "Finish" } else { "Next" }))).clicked() {
                    if last {
                        done = true;
                    } else {
                        next = Some(step + 1);
                    }
                }
                if !last && w::add(ui, "Tutorial: skip", Button::new(tr("Skip tutorial"))).clicked() {
                    done = true;
                }
            });
        });
    if done {
        app.tutorial = None;
        app.settings.tutorial_done = true;
    } else {
        app.tutorial = next;
    }
}
