//! App settings (theme, accent colour, panel layout) in a JSON file of their own, never in the project file.
use crate::app::SideTab;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// A panel slot beside or below the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dock {
    Left,
    Right,
    Bottom,
}

impl Dock {
    /// In drawing order: the right slot first, as before docking existed.
    pub const ALL: [Dock; 3] = [Dock::Right, Dock::Left, Dock::Bottom];

    /// Index into `Layout::shown` and `Layout::sizes`: Left 0, Right 1, Bottom 2.
    pub fn index(self) -> usize {
        match self {
            Dock::Left => 0,
            Dock::Right => 1,
            Dock::Bottom => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Dock::Left => "Left",
            Dock::Right => "Right",
            Dock::Bottom => "Bottom",
        }
    }
}

/// Where the Sketch, Model and Assembly panels are docked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    /// Slot of the Sketch, Model and Assembly panels (`SideTab::ALL` order).
    pub docks: [Dock; 3],
    /// Left, right and bottom slot drawn (`Dock::index`); a hidden slot keeps its panels.
    pub shown: [bool; 3],
    /// Width of the left and right slots and height of the bottom slot, points (`Dock::index`).
    pub sizes: [f32; 3],
}

impl Default for Layout {
    fn default() -> Self {
        Layout { docks: [Dock::Right; 3], shown: [true; 3], sizes: [240.0, 240.0, 220.0] }
    }
}

impl Layout {
    /// The panels docked in `slot`, in `SideTab::ALL` order.
    pub fn tabs_in(&self, slot: Dock) -> Vec<SideTab> {
        SideTab::ALL.into_iter().filter(|t| self.docks[t.index()] == slot).collect()
    }

    /// The panel `slot` shows: `current` when it is docked there, else the first panel docked there; None for an empty slot.
    pub fn shown_tab(&self, slot: Dock, current: SideTab) -> Option<SideTab> {
        if self.docks[current.index()] == slot {
            return Some(current);
        }
        self.tabs_in(slot).first().copied()
    }
}

/// Look and feel of the window and the views.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Interface {
    /// UI zoom factor (egui points per native point).
    pub scale: f32,
    /// Hover fades, menu and panel animations; off sets every egui animation to zero.
    pub animations: bool,
    /// Duration of the egui animations, milliseconds.
    pub animation_ms: u32,
    /// Fit, focus and the standard views glide to the new camera instead of jumping.
    pub camera_transitions: bool,
    /// Duration of a camera transition, milliseconds.
    pub camera_ms: u32,
    /// The entity under the cursor is drawn brighter in the 2D view.
    pub hover_highlight: bool,
    pub grid_2d: bool,
    pub grid_3d: bool,
    /// Axis triad in the corner of the 3D view; its labels set the standard views.
    pub triad: bool,
    /// Frame time in the status bar.
    pub frame_time: bool,
}

impl Default for Interface {
    fn default() -> Self {
        Interface {
            scale: 1.0,
            animations: true,
            animation_ms: 120,
            camera_transitions: true,
            camera_ms: 220,
            hover_highlight: true,
            grid_2d: true,
            grid_3d: true,
            triad: true,
            frame_time: false,
        }
    }
}

/// Behaviour that is not looks or controls.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    /// Snap states at startup.
    pub osnap_on_start: bool,
    pub grid_snap_on_start: bool,
    /// Minutes between autosaves of a project with unsaved changes; 0: off.
    pub autosave_min: u32,
    /// Entries kept in File > Open recent.
    pub recent_max: usize,
    /// Arrow-key nudge of the 2D selection, mm; Shift moves ten times as far.
    pub nudge_mm: f64,
}

impl Default for General {
    fn default() -> Self {
        General { osnap_on_start: true, grid_snap_on_start: false, autosave_min: 5, recent_max: 10, nudge_mm: 1.0 }
    }
}

/// Everything the settings file holds; missing fields take their defaults, so older files keep loading.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    /// sRGB selection and link colour.
    pub accent: [u8; 3],
    pub layout: Layout,
    pub ui: Interface,
    pub general: General,
    pub controls: crate::keymap::Controls,
    /// Recently opened or saved projects, newest first.
    pub recent: Vec<String>,
    /// Language of the menus, panels and dialogs.
    pub language: crate::i18n::Language,
    /// The first-run tutorial was finished or skipped.
    pub tutorial_done: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme::Dark,
            accent: ACCENTS[0].1,
            layout: Layout::default(),
            ui: Interface::default(),
            general: General::default(),
            controls: Default::default(),
            recent: Vec::new(),
            language: Default::default(),
            tutorial_done: false,
        }
    }
}

impl Settings {
    /// Puts `path` first in the recent list (once), keeping at most `general.recent_max` entries.
    pub fn push_recent(&mut self, path: &Path) {
        let p = path.to_string_lossy().into_owned();
        self.recent.retain(|r| *r != p);
        self.recent.insert(0, p);
        self.recent.truncate(self.general.recent_max);
    }
}

/// The accent colours of the View menu; the first is egui's dark selection colour.
pub const ACCENTS: [(&str, [u8; 3]); 4] = [("Blue", [0, 92, 128]), ("Green", [40, 130, 60]), ("Orange", [200, 100, 20]), ("Purple", [120, 70, 170])];

/// Where the settings live: `CAD_SETTINGS` when set; in MCP mode nowhere else (scripts start from the defaults and
/// never touch the user's file); otherwise `%APPDATA%\ProjectCAD\settings.json` (`APPDATA` set),
/// `$XDG_CONFIG_HOME/projectcad/settings.json` or `$HOME/.config/projectcad/settings.json`; None when none is known.
pub fn path(mcp: bool) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CAD_SETTINGS") {
        return Some(PathBuf::from(p));
    }
    if mcp {
        return None;
    }
    if let Some(dir) = std::env::var_os("APPDATA") {
        return Some(PathBuf::from(dir).join("ProjectCAD").join("settings.json"));
    }
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(dir).join("projectcad").join("settings.json"));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config").join("projectcad").join("settings.json"))
}

/// The settings in `path`: the defaults when the file does not exist; Err("Settings file ignored: <reason>") when it
/// cannot be read or parsed.
pub fn load(path: &Path) -> Result<Settings, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|err| format!("Settings file ignored: {err}")),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(err) => Err(format!("Settings file ignored: {err}")),
    }
}

/// Writes `s` as pretty JSON to `path` (parent folders created; written to `<path>.tmp`, then renamed over `path`).
pub fn save(path: &Path, s: &Settings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(s).map_err(|err| err.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text).map_err(|err| err.to_string())?;
    std::fs::rename(&tmp, path).map_err(|err| err.to_string())
}

/// The egui visuals of `s`: dark or light, with the accent as selection fill, link colour and hover outline, and
/// black or white selection text, whichever reads on the accent (sRGB luma above 140: black). Corners are small and
/// shadows short for a crisp look; sizes and spacing stay egui's, so the layout does not move.
pub fn visuals(s: &Settings) -> egui::Visuals {
    use egui::{Color32, CornerRadius, Shadow, Stroke};
    let dark = s.theme != Theme::Light;
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let [r, g, b] = s.accent;
    let accent = Color32::from_rgb(r, g, b);
    v.selection.bg_fill = accent;
    v.hyperlink_color = accent;
    let luma = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
    v.selection.stroke.color = if luma > 140.0 { Color32::BLACK } else { Color32::WHITE };
    let gray = |d: u8, l: u8| Color32::from_gray(if dark { d } else { l });
    v.panel_fill = gray(30, 242);
    v.window_fill = gray(34, 248);
    v.extreme_bg_color = gray(20, 255);
    v.faint_bg_color = gray(36, 236);
    v.window_stroke = Stroke::new(1.0, gray(58, 200));
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(3);
    v.window_shadow = Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(if dark { 110 } else { 40 }) };
    v.popup_shadow = Shadow { offset: [0, 2], blur: 6, spread: 0, color: Color32::from_black_alpha(if dark { 90 } else { 30 }) };
    let w = &mut v.widgets;
    for (state, fill) in [(&mut w.noninteractive, gray(30, 242)), (&mut w.inactive, gray(46, 228)), (&mut w.hovered, gray(56, 218)), (&mut w.active, gray(64, 206)), (&mut w.open, gray(50, 222))] {
        state.corner_radius = CornerRadius::same(2);
        state.weak_bg_fill = fill;
        state.bg_fill = fill;
    }
    w.noninteractive.bg_stroke = Stroke::new(1.0, gray(48, 214));
    w.inactive.bg_stroke = Stroke::new(1.0, gray(54, 208));
    w.hovered.bg_stroke = Stroke::new(1.0, accent);
    w.active.bg_stroke = Stroke::new(1.0, accent);
    // No grow-on-hover: a sharper look, and widgets never shift.
    w.hovered.expansion = 0.0;
    w.active.expansion = 0.0;
    v
}

/// What `sync` last applied to egui: theme, accent, UI scale and animation time (ms, 0 when animations are off).
pub type Applied = (Theme, [u8; 3], f32, u32);

fn applied_key(s: &Settings) -> Applied {
    (s.theme, s.accent, s.ui.scale, if s.ui.animations { s.ui.animation_ms } else { 0 })
}

/// Once per frame: sets the egui theme, visuals, zoom and animation time when they changed, and writes the settings
/// file when the settings changed and no pointer button is down (a slot being resized writes once, at the release).
/// A write error goes to the status line and is not retried until the settings change again.
pub fn sync(app: &mut crate::app::CadApp, ctx: &egui::Context) {
    let key = applied_key(&app.settings);
    if app.theme_applied != Some(key) {
        let theme = if key.0 == Theme::Light { egui::Theme::Light } else { egui::Theme::Dark };
        ctx.set_theme(theme);
        ctx.set_visuals_of(theme, visuals(&app.settings));
        ctx.all_styles_mut(|st| st.animation_time = key.3 as f32 / 1000.0);
        let zoom = key.2.clamp(0.5, 3.0);
        if (ctx.zoom_factor() - zoom).abs() > 1e-4 {
            ctx.set_zoom_factor(zoom);
        }
        app.theme_applied = Some(key);
    }
    if app.lang_applied != Some(app.settings.language) {
        let lang = app.settings.language;
        if !crate::i18n::apply(ctx, lang) {
            app.status = format!("No system font found for {}; install one to read it", lang.native_name());
        }
        app.lang_applied = Some(lang);
    }
    if app.settings != app.settings_saved && !ctx.input(|i| i.pointer.any_down()) {
        if app.settings.ui.grid_3d != app.settings_saved.ui.grid_3d {
            // The 3D grid is part of the uploaded scene.
            app.gpu_stamp = None;
        }
        if let Some(p) = &app.settings_path
            && let Err(err) = save(p, &app.settings)
        {
            app.status = format!("Settings not saved: {err}");
        }
        app.settings_saved = app.settings.clone();
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
