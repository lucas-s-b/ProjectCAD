//! The Settings window: General, Interface, Navigation and Controls pages. Every change lands in `app.settings` and
//! is written by `settings::sync`; the Controls page rebinds a shortcut by capturing the next key press.
use crate::app::CadApp;
use crate::i18n::{Language, tr};
use crate::keymap::{Action, Chord, Gesture, Preset};
use crate::settings::{ACCENTS, General, Interface, Theme};
use crate::widgets as w;
use eframe::egui::{self, Button, DragValue, RichText};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    General,
    Interface,
    Navigation,
    Controls,
}

impl Page {
    const ALL: [Page; 4] = [Page::General, Page::Interface, Page::Navigation, Page::Controls];

    fn name(self) -> &'static str {
        match self {
            Page::General => "General",
            Page::Interface => "Interface",
            Page::Navigation => "Navigation",
            Page::Controls => "Controls",
        }
    }
}

/// UI scales offered on the Interface page (a slider would rescale under the pointer while dragging).
const SCALES: [f32; 8] = [0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

/// While a shortcut waits for its key: takes the first key press of the frame (Escape cancels) before anything else
/// sees it, and stores it as that action's chord.
pub fn capture(app: &mut CadApp, ctx: &egui::Context) {
    let Some((action, slot)) = app.rebind else { return };
    let mut pressed = None;
    ctx.input_mut(|i| {
        i.events.retain(|ev| match ev {
            egui::Event::Key { key, pressed: true, modifiers, .. } if pressed.is_none() => {
                pressed = Some(Chord { key: *key, ctrl: modifiers.command, shift: modifiers.shift, alt: modifiers.alt });
                false
            }
            _ => true,
        });
    });
    let Some(chord) = pressed else { return };
    app.rebind = None;
    if chord == Chord::key(egui::Key::Escape) {
        return;
    }
    let controls = &mut app.settings.controls;
    let mut chords = controls.chords(action);
    match slot {
        Some(i) if i < chords.len() => chords[i] = chord,
        _ => chords.push(chord),
    }
    chords.dedup();
    controls.set(action, &chords);
    let others = controls.conflicts(action, chord);
    app.status = if others.is_empty() {
        format!("{} bound to {chord}", action.label())
    } else {
        format!("{chord} also runs: {}", others.iter().map(|a| a.label()).collect::<Vec<_>>().join(", "))
    };
}

pub fn show(app: &mut CadApp, ctx: &egui::Context) {
    if !app.settings_open {
        return;
    }
    let mut open = true;
    egui::Window::new(tr("Settings"))
        .id(egui::Id::unique("settings_window"))
        .open(&mut open)
        .collapsible(false)
        .default_size([560.0, 520.0])
        .min_width(460.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for p in Page::ALL {
                    w::selectable_value(ui, &mut app.settings_page, p, p.name());
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().id_salt("settings_page").auto_shrink([false, true]).show(ui, |ui| match app.settings_page {
                Page::General => general(app, ui),
                Page::Interface => interface(app, ui),
                Page::Navigation => navigation(app, ui),
                Page::Controls => controls(app, ui),
            });
        });
    if !open {
        app.settings_open = false;
        app.rebind = None;
    }
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(tr(title)).strong());
}

/// A labelled row: caption on the left, the widget after it.
fn row<R>(ui: &mut egui::Ui, caption: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| {
        caption_cell(ui, 190.0, caption);
        add(ui)
    })
    .inner
}

/// A left-aligned caption of fixed width, so the widgets after it line up.
fn caption_cell(ui: &mut egui::Ui, width: f32, caption: &str) {
    let size = egui::vec2(width, ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_size(size);
        ui.add(egui::Label::new(tr(caption)).truncate());
    });
}

/// The language picker, in each language's own name (also on the tutorial's first step).
pub fn language_row(app: &mut CadApp, ui: &mut egui::Ui) {
    let lang = &mut app.settings.language;
    row(ui, "Language", |ui| {
        w::combo(ui, "Language", egui::ComboBox::from_id_salt("language").selected_text(lang.native_name()).width(150.0), |ui| {
            for l in Language::ALL {
                w::selectable_value(ui, lang, l, l.native_name());
            }
        });
    });
}

fn general(app: &mut CadApp, ui: &mut egui::Ui) {
    section(ui, "Language");
    language_row(app, ui);
    if w::button(ui, "Show the tutorial").clicked() {
        app.tutorial = Some(0);
    }
    let g = &mut app.settings.general;
    section(ui, "Startup");
    w::checkbox(ui, &mut g.osnap_on_start, "Object snap on at startup");
    w::checkbox(ui, &mut g.grid_snap_on_start, "Grid snap on at startup");
    section(ui, "Files");
    row(ui, "Autosave every (minutes, 0 = off)", |ui| w::add(ui, "Autosave minutes", DragValue::new(&mut g.autosave_min).range(0..=120)));
    if let Some(dir) = app.settings_path.as_deref().and_then(|p| p.parent()) {
        ui.label(RichText::new(format!("{} {}", tr("Autosaves go to"), dir.join("autosave").display())).weak().small());
    }
    row(ui, "Recent files kept", |ui| w::add(ui, "Recent files kept", DragValue::new(&mut g.recent_max).range(1..=30)));
    let max = g.recent_max;
    app.settings.recent.truncate(max);
    let g = &mut app.settings.general;
    section(ui, "Editing");
    row(ui, "Arrow-key nudge (mm, Shift x10)", |ui| w::add(ui, "Nudge distance", DragValue::new(&mut g.nudge_mm).speed(0.1).range(0.001..=1000.0)));
    ui.add_space(12.0);
    if w::button(ui, "Reset general settings").clicked() {
        *g = General::default();
    }
}

fn interface(app: &mut CadApp, ui: &mut egui::Ui) {
    let s = &mut app.settings;
    section(ui, "Look");
    row(ui, "Theme", |ui| {
        for (t, name) in [(Theme::Dark, "Theme: Dark"), (Theme::Light, "Theme: Light")] {
            w::selectable_value(ui, &mut s.theme, t, name);
        }
    });
    row(ui, "Accent", |ui| {
        for (name, rgb) in ACCENTS {
            let c = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            let r = w::add(ui, &format!("Accent preset: {name}"), Button::new(RichText::new("  ").background_color(c)).selected(s.accent == rgb));
            if r.on_hover_text(name).clicked() {
                s.accent = rgb;
            }
        }
        let r = ui.color_edit_button_srgb(&mut s.accent);
        w::tag(ui, "Accent color", r);
    });
    row(ui, "UI scale", |ui| {
        let text = format!("{:.0} %", s.ui.scale * 100.0);
        w::combo(ui, "UI scale", egui::ComboBox::from_id_salt("ui_scale").selected_text(text).width(90.0), |ui| {
            for f in SCALES {
                w::selectable_value(ui, &mut s.ui.scale, f, &format!("{:.0} %", f * 100.0));
            }
        });
    });
    section(ui, "Animation");
    w::checkbox(ui, &mut s.ui.animations, "Animations");
    ui.add_enabled_ui(s.ui.animations, |ui| {
        row(ui, "UI animation (ms)", |ui| w::add(ui, "UI animation ms", DragValue::new(&mut s.ui.animation_ms).range(0..=500)));
        w::checkbox(ui, &mut s.ui.camera_transitions, "Smooth camera transitions");
        row(ui, "Camera transition (ms)", |ui| w::add(ui, "Camera transition ms", DragValue::new(&mut s.ui.camera_ms).range(0..=1000)));
    });
    ui.label(RichText::new(tr("Animations only draw extra frames while they run; an idle window stays idle.")).weak().small());
    section(ui, "Views");
    w::checkbox(ui, &mut s.ui.hover_highlight, "Highlight the entity under the cursor (2D)");
    w::checkbox(ui, &mut s.ui.grid_2d, "2D grid");
    w::checkbox(ui, &mut s.ui.grid_3d, "3D grid");
    w::checkbox(ui, &mut s.ui.triad, "Axis triad in the 3D view");
    w::checkbox(ui, &mut s.ui.frame_time, "Frame time in the status bar");
    ui.add_space(12.0);
    if w::button(ui, "Reset interface settings").clicked() {
        s.ui = Interface::default();
        s.theme = Theme::Dark;
        s.accent = ACCENTS[0].1;
    }
}

fn preset_combo(app: &mut CadApp, ui: &mut egui::Ui) {
    let c = &mut app.settings.controls;
    let mut p = c.preset;
    row(ui, "Control preset", |ui| {
        w::combo(ui, "Control preset", egui::ComboBox::from_id_salt("preset").selected_text(p.name()).width(150.0), |ui| {
            for q in Preset::ALL {
                w::selectable_value(ui, &mut p, q, q.name());
            }
        });
    });
    if p != c.preset {
        c.apply_preset(p);
        app.status = format!("Controls: {} preset", p.name());
    }
    ui.label(RichText::new(tr(preset_note(app.settings.controls.preset))).weak().small());
}

fn preset_note(p: Preset) -> &'static str {
    match p {
        Preset::ProjectCad => "Right drag orbits, middle drag pans. G / R / S grab, rotate and scale; letters pick tools (L line, C circle, B rectangle...).",
        Preset::Blender => "Middle drag orbits, Shift+middle pans, Ctrl+middle zooms. G / R / S with X / Y / Z, A selects all, X deletes, Shift+D duplicates, 1 / 3 / 7 views, 5 ortho.",
        Preset::Godot => "Middle drag orbits, Shift+middle pans. Hold right button and fly with W A S D Q E. Q / W / E switch the gizmo, R scales, F focuses.",
        Preset::Roblox => "Right drag orbits, middle drag pans. Hold right button and fly with W A S D Q E. Ctrl+1..4 select, move, scale, rotate; F focuses.",
    }
}

fn gesture_combo(ui: &mut egui::Ui, caption: &str, g: &mut Gesture) {
    row(ui, caption, |ui| {
        w::combo(ui, caption, egui::ComboBox::from_id_salt(caption).selected_text(g.name()).width(170.0), |ui| {
            for x in Gesture::ALL {
                w::selectable_value(ui, g, x, &format!("{caption}: {}", x.name()));
            }
        });
    });
}

fn navigation(app: &mut CadApp, ui: &mut egui::Ui) {
    preset_combo(app, ui);
    let n = &mut app.settings.controls.nav;
    section(ui, "Mouse");
    gesture_combo(ui, "Orbit", &mut n.orbit);
    gesture_combo(ui, "Pan", &mut n.pan);
    gesture_combo(ui, "Drag zoom", &mut n.zoom);
    ui.label(RichText::new(tr("The left button stays for selecting; in 2D the orbit gesture pans too.")).weak().small());
    section(ui, "Speed");
    row(ui, "Orbit speed", |ui| w::add(ui, "Orbit speed", egui::Slider::new(&mut n.orbit_speed, 0.1..=4.0)));
    row(ui, "Pan speed", |ui| w::add(ui, "Pan speed", egui::Slider::new(&mut n.pan_speed, 0.1..=4.0)));
    row(ui, "Zoom speed", |ui| w::add(ui, "Zoom speed", egui::Slider::new(&mut n.zoom_speed, 0.1..=4.0)));
    section(ui, "Options");
    w::checkbox(ui, &mut n.invert_zoom, "Invert wheel zoom");
    w::checkbox(ui, &mut n.invert_orbit_y, "Invert vertical orbit");
    w::checkbox(ui, &mut n.fly, "Fly with W A S D Q E while the orbit button is held (3D)");
    ui.add_space(12.0);
    if w::button(ui, "Reset navigation to preset").clicked() {
        *n = app.settings.controls.preset.navigation();
    }
}

fn controls(app: &mut CadApp, ui: &mut egui::Ui) {
    preset_combo(app, ui);
    ui.horizontal(|ui| {
        ui.label(tr("Filter"));
        w::add(ui, "Filter shortcuts", egui::TextEdit::singleline(&mut app.controls_filter).desired_width(180.0).hint_text(tr("action or key")));
        if w::add_enabled(ui, !app.settings.controls.overrides.is_empty(), "Reset all shortcuts", Button::new(tr("Reset all shortcuts"))).clicked() {
            app.settings.controls.overrides.clear();
        }
    });
    ui.label(RichText::new(tr("Click a key to change it, then press the new key (Esc cancels). + adds another key.")).weak().small());
    let filter = app.controls_filter.to_lowercase();
    // Resolved once per frame: conflicts are looked up here instead of re-resolving every action per key.
    let all = app.settings.controls.all();
    let mut category = "";
    for a in Action::ALL {
        let chords = app.settings.controls.chords(a);
        let text: Vec<String> = chords.iter().map(Chord::to_string).collect();
        if !filter.is_empty() && !a.label().to_lowercase().contains(&filter) && !text.iter().any(|t| t.to_lowercase().contains(&filter)) {
            continue;
        }
        if a.category() != category {
            category = a.category();
            section(ui, category);
        }
        action_row(app, ui, a, &chords, &all);
    }
    section(ui, "While grabbing, rotating or scaling");
    for line in [
        "X / Y / Z: lock to an axis (again: unlock); Shift+X / Y / Z in 3D: move in the plane across it",
        "Type a number for an exact distance (mm), angle (degrees) or factor; - flips the sign",
        "Hold Ctrl to snap: grid steps, 15 degrees, or 0.1 scale steps",
        "Enter or left click confirms; Esc or right click cancels",
    ] {
        ui.label(RichText::new(tr(line)).small());
    }
}

fn action_row(app: &mut CadApp, ui: &mut egui::Ui, a: Action, chords: &[Chord], all: &[(Action, Chord)]) {
    let label = a.label();
    let others = |c: Chord| -> Vec<&'static str> { all.iter().filter(|(x, y)| *x != a && *y == c).map(|(x, _)| x.label()).collect() };
    ui.horizontal(|ui| {
        caption_cell(ui, 200.0, label);
        for (i, c) in chords.iter().enumerate() {
            let waiting = app.rebind == Some((a, Some(i)));
            let clash = others(*c);
            let conflict = !clash.is_empty();
            let text = if waiting { tr("Press a key...").to_string() } else if conflict { format!("⚠ {c}") } else { c.to_string() };
            let r = w::add(ui, &format!("{label}: key {i}"), Button::new(text).selected(waiting).min_size(egui::vec2(70.0, 0.0)));
            let r = if conflict { r.on_hover_text(format!("{} {}", tr("Also bound to:"), clash.iter().map(|l| tr(l)).collect::<Vec<_>>().join(", "))) } else { r };
            if r.clicked() {
                app.rebind = Some((a, Some(i)));
            }
            if r.secondary_clicked() {
                let mut rest = chords.to_vec();
                rest.remove(i);
                app.settings.controls.set(a, &rest);
            }
        }
        let waiting = app.rebind == Some((a, None));
        if w::add(ui, &format!("{label}: add key"), Button::new(if waiting { tr("Press a key...") } else { "+" }).selected(waiting)).on_hover_text(tr("Add a key")).clicked() {
            app.rebind = Some((a, None));
        }
        if !chords.is_empty() && w::add(ui, &format!("{label}: clear"), Button::new("×")).on_hover_text(tr("Unbind (right-click a key removes just that one)")).clicked() {
            app.settings.controls.set(a, &[]);
        }
        if app.settings.controls.overrides.contains_key(&a) && w::add(ui, &format!("{label}: reset"), Button::new("↺")).on_hover_text(tr("Back to the preset's keys")).clicked() {
            app.settings.controls.overrides.remove(&a);
        }
    });
}
