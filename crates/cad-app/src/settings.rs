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

/// Everything the settings file holds; missing fields take their defaults, so older files keep loading.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    /// sRGB selection and link colour.
    pub accent: [u8; 3],
    pub layout: Layout,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { theme: Theme::Dark, accent: ACCENTS[0].1, layout: Layout::default() }
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

/// The egui visuals of `s`: dark or light, with the accent as selection fill and link colour, and black or white
/// selection text, whichever reads on the accent (sRGB luma above 140: black).
pub fn visuals(s: &Settings) -> egui::Visuals {
    let mut v = if s.theme == Theme::Light { egui::Visuals::light() } else { egui::Visuals::dark() };
    let [r, g, b] = s.accent;
    let accent = egui::Color32::from_rgb(r, g, b);
    v.selection.bg_fill = accent;
    v.hyperlink_color = accent;
    let luma = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
    v.selection.stroke.color = if luma > 140.0 { egui::Color32::BLACK } else { egui::Color32::WHITE };
    v
}

/// Once per frame: sets the egui theme and visuals when theme or accent changed, and writes the settings file when
/// the settings changed and no pointer button is down (a slot being resized writes once, at the release). A write
/// error goes to the status line and is not retried until the settings change again.
pub fn sync(app: &mut crate::app::CadApp, ctx: &egui::Context) {
    let key = (app.settings.theme, app.settings.accent);
    if app.theme_applied != Some(key) {
        let theme = if key.0 == Theme::Light { egui::Theme::Light } else { egui::Theme::Dark };
        ctx.set_theme(theme);
        ctx.set_visuals_of(theme, visuals(&app.settings));
        app.theme_applied = Some(key);
    }
    if app.settings != app.settings_saved && !ctx.input(|i| i.pointer.any_down()) {
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
