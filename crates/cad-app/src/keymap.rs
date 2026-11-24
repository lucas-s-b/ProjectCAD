//! Keyboard shortcuts and mouse navigation: actions, the chords that run them, and the control presets (ProjectCAD,
//! Blender, Godot, Roblox Studio) that fill both in. The user's own bindings are kept as overrides of the preset, so a
//! preset change keeps nothing stale and a reset is one map clear.
use eframe::egui::{self, Key, Modifiers, PointerButton};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A key with exact modifiers, written "Ctrl+Shift+Z" (Ctrl stands for Cmd on macOS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Chord {
    pub const fn key(key: Key) -> Self {
        Chord { key, ctrl: false, shift: false, alt: false }
    }

    /// Parses "Ctrl+Shift+Z", "Alt+A", "F3", "Delete"; modifier names are case-insensitive. None for an unknown key.
    pub fn parse(text: &str) -> Option<Chord> {
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        // "Ctrl++" names the plus key.
        if text.ends_with("++") {
            parts.pop();
            parts.pop();
            parts.push("+");
        }
        let key = Key::from_name(parts.pop()?)?;
        let mut c = Chord::key(key);
        for m in parts {
            match m.to_ascii_lowercase().as_str() {
                "ctrl" | "cmd" | "command" => c.ctrl = true,
                "shift" => c.shift = true,
                "alt" | "option" => c.alt = true,
                _ => return None,
            }
        }
        Some(c)
    }

    /// True when `m` holds exactly this chord's modifiers (Ctrl and Cmd count as one).
    pub fn matches(self, m: Modifiers) -> bool {
        self.ctrl == m.command && self.shift == m.shift && self.alt == m.alt
    }

    /// Runs with no modifier but Shift: such chords are ignored while a text field has the keyboard.
    pub fn is_plain(self) -> bool {
        !self.ctrl && !self.alt && !(self.key as u32 >= Key::F1 as u32 && self.key as u32 <= Key::F35 as u32)
    }
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (on, name) in [(self.ctrl, "Ctrl+"), (self.shift, "Shift+"), (self.alt, "Alt+")] {
            if on {
                f.write_str(name)?;
            }
        }
        f.write_str(self.key.name())
    }
}

/// Everything a shortcut can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Action {
    New,
    Open,
    Save,
    SaveAs,
    Settings,
    CommandSearch,
    Undo,
    Redo,
    Delete,
    Duplicate,
    SelectAll,
    DeselectAll,
    InvertSelection,
    Grab,
    Rotate,
    Scale,
    NudgeLeft,
    NudgeRight,
    NudgeUp,
    NudgeDown,
    Fit,
    FocusSelection,
    Toggle2D3D,
    ViewFront,
    ViewBack,
    ViewRight,
    ViewLeft,
    ViewTop,
    ViewBottom,
    ViewIso,
    ToggleOrtho,
    TogglePanels,
    ToggleOsnap,
    ToggleGridSnap,
    ToggleGlyphs,
    GizmoAll,
    GizmoMove,
    GizmoRotate,
    ToolSelect,
    ToolLine,
    ToolCircle,
    ToolRect,
    ToolArc,
    ToolPolyline,
    ToolText,
    ToolDimension,
    ToolWall,
    ToolTrim,
    ToolOffset,
}

impl Action {
    pub const ALL: [Action; 49] = [
        Action::New,
        Action::Open,
        Action::Save,
        Action::SaveAs,
        Action::Settings,
        Action::CommandSearch,
        Action::Undo,
        Action::Redo,
        Action::Delete,
        Action::Duplicate,
        Action::SelectAll,
        Action::DeselectAll,
        Action::InvertSelection,
        Action::Grab,
        Action::Rotate,
        Action::Scale,
        Action::NudgeLeft,
        Action::NudgeRight,
        Action::NudgeUp,
        Action::NudgeDown,
        Action::Fit,
        Action::FocusSelection,
        Action::Toggle2D3D,
        Action::ViewFront,
        Action::ViewBack,
        Action::ViewRight,
        Action::ViewLeft,
        Action::ViewTop,
        Action::ViewBottom,
        Action::ViewIso,
        Action::ToggleOrtho,
        Action::TogglePanels,
        Action::ToggleOsnap,
        Action::ToggleGridSnap,
        Action::ToggleGlyphs,
        Action::GizmoAll,
        Action::GizmoMove,
        Action::GizmoRotate,
        Action::ToolSelect,
        Action::ToolLine,
        Action::ToolCircle,
        Action::ToolRect,
        Action::ToolArc,
        Action::ToolPolyline,
        Action::ToolText,
        Action::ToolDimension,
        Action::ToolWall,
        Action::ToolTrim,
        Action::ToolOffset,
    ];

    /// Label in the Controls settings and the menus.
    pub fn label(self) -> &'static str {
        match self {
            Action::New => "New project",
            Action::Open => "Open project",
            Action::Save => "Save",
            Action::SaveAs => "Save as",
            Action::Settings => "Settings",
            Action::CommandSearch => "Command search",
            Action::Undo => "Undo",
            Action::Redo => "Redo",
            Action::Delete => "Delete selection",
            Action::Duplicate => "Duplicate",
            Action::SelectAll => "Select all",
            Action::DeselectAll => "Deselect all",
            Action::InvertSelection => "Invert selection",
            Action::Grab => "Grab / move (modal)",
            Action::Rotate => "Rotate (modal)",
            Action::Scale => "Scale (modal)",
            Action::NudgeLeft => "Nudge left",
            Action::NudgeRight => "Nudge right",
            Action::NudgeUp => "Nudge up",
            Action::NudgeDown => "Nudge down",
            Action::Fit => "Fit all",
            Action::FocusSelection => "Focus selection",
            Action::Toggle2D3D => "Toggle 2D / 3D",
            Action::ViewFront => "View front",
            Action::ViewBack => "View back",
            Action::ViewRight => "View right",
            Action::ViewLeft => "View left",
            Action::ViewTop => "View top",
            Action::ViewBottom => "View bottom",
            Action::ViewIso => "View isometric",
            Action::ToggleOrtho => "Toggle orthographic",
            Action::TogglePanels => "Toggle panels (focus view)",
            Action::ToggleOsnap => "Toggle object snap",
            Action::ToggleGridSnap => "Toggle grid snap",
            Action::ToggleGlyphs => "Toggle constraint glyphs",
            Action::GizmoAll => "Gizmo: move and rotate",
            Action::GizmoMove => "Gizmo: move only",
            Action::GizmoRotate => "Gizmo: rotate only",
            Action::ToolSelect => "Tool: select",
            Action::ToolLine => "Tool: line",
            Action::ToolCircle => "Tool: circle",
            Action::ToolRect => "Tool: rectangle",
            Action::ToolArc => "Tool: arc",
            Action::ToolPolyline => "Tool: polyline",
            Action::ToolText => "Tool: text",
            Action::ToolDimension => "Tool: dimension",
            Action::ToolWall => "Tool: wall",
            Action::ToolTrim => "Tool: trim",
            Action::ToolOffset => "Tool: offset",
        }
    }

    /// Heading the action is listed under in the Controls settings.
    pub fn category(self) -> &'static str {
        use Action::*;
        match self {
            New | Open | Save | SaveAs | Settings | CommandSearch => "File",
            Undo | Redo | Delete | Duplicate | SelectAll | DeselectAll | InvertSelection => "Edit",
            Grab | Rotate | Scale | NudgeLeft | NudgeRight | NudgeUp | NudgeDown | GizmoAll | GizmoMove | GizmoRotate => "Transform",
            Fit | FocusSelection | Toggle2D3D | ViewFront | ViewBack | ViewRight | ViewLeft | ViewTop | ViewBottom | ViewIso | ToggleOrtho
            | TogglePanels => "View",
            ToggleOsnap | ToggleGridSnap | ToggleGlyphs => "Snapping",
            _ => "Tools",
        }
    }

    /// Runs even while a text field has the keyboard (Ctrl+Z undoes the document there, as before).
    pub fn global(self) -> bool {
        matches!(self, Action::New | Action::Open | Action::Save | Action::SaveAs | Action::Settings | Action::CommandSearch | Action::Undo | Action::Redo)
    }
}

/// A control scheme: default chords and mouse navigation modelled on another program.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Preset {
    #[default]
    ProjectCad,
    Blender,
    Godot,
    Roblox,
}

impl Preset {
    pub const ALL: [Preset; 4] = [Preset::ProjectCad, Preset::Blender, Preset::Godot, Preset::Roblox];

    pub fn name(self) -> &'static str {
        match self {
            Preset::ProjectCad => "ProjectCAD",
            Preset::Blender => "Blender",
            Preset::Godot => "Godot",
            Preset::Roblox => "Roblox Studio",
        }
    }

    pub fn navigation(self) -> Navigation {
        let base = Navigation::default();
        match self {
            Preset::ProjectCad => base,
            Preset::Blender => Navigation { orbit: Gesture::Middle, pan: Gesture::ShiftMiddle, zoom: Gesture::CtrlMiddle, fly: false, ..base },
            Preset::Godot => Navigation { orbit: Gesture::Middle, pan: Gesture::ShiftMiddle, zoom: Gesture::CtrlMiddle, fly: true, ..base },
            Preset::Roblox => Navigation { orbit: Gesture::Right, pan: Gesture::Middle, zoom: Gesture::None, fly: true, ..base },
        }
    }

    /// The preset's chords per action; actions it leaves out are unbound.
    pub fn bindings(self) -> Vec<(Action, &'static [&'static str])> {
        use Action::*;
        // Shared by every preset: file commands and the arrows.
        let mut b: Vec<(Action, &'static [&'static str])> = vec![
            (New, &["Ctrl+N"]),
            (Open, &["Ctrl+O"]),
            (Save, &["Ctrl+S"]),
            (SaveAs, &["Ctrl+Shift+S"]),
            (Settings, &["Ctrl+Comma"]),
            (Undo, &["Ctrl+Z"]),
            (Redo, &["Ctrl+Y", "Ctrl+Shift+Z"]),
            (NudgeLeft, &["Left", "Shift+Left"]),
            (NudgeRight, &["Right", "Shift+Right"]),
            (NudgeUp, &["Up", "Shift+Up"]),
            (NudgeDown, &["Down", "Shift+Down"]),
            (ToggleOsnap, &["F3"]),
            (ToggleGridSnap, &["F9"]),
            (Toggle2D3D, &["Backtick"]),
        ];
        let own: &[(Action, &'static [&'static str])] = match self {
            Preset::ProjectCad => &[
                (CommandSearch, &["Ctrl+K"]),
                (Delete, &["Delete"]),
                (Duplicate, &["Ctrl+D"]),
                (SelectAll, &["Ctrl+A"]),
                (DeselectAll, &["Ctrl+Shift+A"]),
                (InvertSelection, &["Ctrl+I"]),
                (Grab, &["G"]),
                (Rotate, &["R"]),
                (Scale, &["S"]),
                (Fit, &["F"]),
                (FocusSelection, &["Period"]),
                (ViewFront, &["1"]),
                (ViewBack, &["Ctrl+1"]),
                (ViewRight, &["3"]),
                (ViewLeft, &["Ctrl+3"]),
                (ViewTop, &["7"]),
                (ViewBottom, &["Ctrl+7"]),
                (ViewIso, &["0"]),
                (ToggleOrtho, &["5"]),
                (TogglePanels, &["Ctrl+Space"]),
                (ToolSelect, &["Q"]),
                (ToolLine, &["L"]),
                (ToolCircle, &["C"]),
                (ToolRect, &["B"]),
                (ToolArc, &["A"]),
                (ToolPolyline, &["P"]),
                (ToolText, &["T"]),
                (ToolDimension, &["D"]),
                (ToolWall, &["W"]),
                (ToolTrim, &["Shift+T"]),
                (ToolOffset, &["O"]),
            ],
            Preset::Blender => &[
                (CommandSearch, &["F3", "Ctrl+K"]),
                (Delete, &["X", "Delete"]),
                (Duplicate, &["Shift+D"]),
                (SelectAll, &["A"]),
                (DeselectAll, &["Alt+A"]),
                (InvertSelection, &["Ctrl+I"]),
                (Grab, &["G"]),
                (Rotate, &["R"]),
                (Scale, &["S"]),
                (Fit, &["Home"]),
                (FocusSelection, &["Period"]),
                (ViewFront, &["1"]),
                (ViewBack, &["Ctrl+1"]),
                (ViewRight, &["3"]),
                (ViewLeft, &["Ctrl+3"]),
                (ViewTop, &["7"]),
                (ViewBottom, &["Ctrl+7"]),
                (ViewIso, &["0"]),
                (ToggleOrtho, &["5"]),
                (TogglePanels, &["Ctrl+Space"]),
                (ToggleOsnap, &["Shift+S"]),
                (ToggleGridSnap, &["F9"]),
                (ToolSelect, &["W"]),
                (ToolLine, &["Shift+L"]),
                (ToolCircle, &["Shift+C"]),
                (ToolRect, &["Shift+B"]),
            ],
            Preset::Godot => &[
                (CommandSearch, &["Ctrl+K", "Ctrl+Shift+P"]),
                (Delete, &["Delete"]),
                (Duplicate, &["Ctrl+D"]),
                (SelectAll, &["Ctrl+A"]),
                (Grab, &["G"]),
                (Scale, &["R"]),
                (GizmoAll, &["Q"]),
                (GizmoMove, &["W"]),
                (GizmoRotate, &["E"]),
                (Fit, &["Home"]),
                (FocusSelection, &["F"]),
                (ViewFront, &["1"]),
                (ViewBack, &["Ctrl+1"]),
                (ViewRight, &["3"]),
                (ViewLeft, &["Ctrl+3"]),
                (ViewTop, &["7"]),
                (ViewBottom, &["Ctrl+7"]),
                (ToggleOrtho, &["5"]),
                (TogglePanels, &["Shift+F12"]),
            ],
            Preset::Roblox => &[
                (CommandSearch, &["Ctrl+K"]),
                (Delete, &["Delete", "Backspace"]),
                (Duplicate, &["Ctrl+D"]),
                (SelectAll, &["Ctrl+A"]),
                (GizmoAll, &["Ctrl+1"]),
                (GizmoMove, &["Ctrl+2"]),
                (Scale, &["Ctrl+3"]),
                (GizmoRotate, &["Ctrl+4"]),
                (Grab, &["Ctrl+G"]),
                (Rotate, &["Ctrl+R"]),
                (Fit, &["Home"]),
                (FocusSelection, &["F"]),
                (TogglePanels, &["Ctrl+Space"]),
            ],
        };
        for (a, c) in own {
            match b.iter_mut().find(|(x, _)| x == a) {
                Some(slot) => slot.1 = c,
                None => b.push((*a, c)),
            }
        }
        b
    }
}

/// A mouse drag that drives the camera: a button and the modifier held with it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gesture {
    #[default]
    None,
    Middle,
    Right,
    ShiftMiddle,
    CtrlMiddle,
    AltMiddle,
    ShiftRight,
    CtrlRight,
}

impl Gesture {
    pub const ALL: [Gesture; 8] =
        [Gesture::None, Gesture::Middle, Gesture::Right, Gesture::ShiftMiddle, Gesture::CtrlMiddle, Gesture::AltMiddle, Gesture::ShiftRight, Gesture::CtrlRight];

    pub fn name(self) -> &'static str {
        match self {
            Gesture::None => "None",
            Gesture::Middle => "Middle drag",
            Gesture::Right => "Right drag",
            Gesture::ShiftMiddle => "Shift + middle drag",
            Gesture::CtrlMiddle => "Ctrl + middle drag",
            Gesture::AltMiddle => "Alt + middle drag",
            Gesture::ShiftRight => "Shift + right drag",
            Gesture::CtrlRight => "Ctrl + right drag",
        }
    }

    /// Button and (shift, ctrl, alt) of the gesture; None for `None`.
    fn parts(self) -> Option<(PointerButton, bool, bool, bool)> {
        use PointerButton::{Middle, Secondary};
        Some(match self {
            Gesture::None => return None,
            Gesture::Middle => (Middle, false, false, false),
            Gesture::Right => (Secondary, false, false, false),
            Gesture::ShiftMiddle => (Middle, true, false, false),
            Gesture::CtrlMiddle => (Middle, false, true, false),
            Gesture::AltMiddle => (Middle, false, false, true),
            Gesture::ShiftRight => (Secondary, true, false, false),
            Gesture::CtrlRight => (Secondary, false, true, false),
        })
    }

    /// True while `resp` is dragged by this gesture's button with exactly its modifiers.
    pub fn active(self, resp: &egui::Response, m: Modifiers) -> bool {
        self.parts().is_some_and(|(b, shift, ctrl, alt)| resp.dragged_by(b) && m.shift == shift && m.command == ctrl && m.alt == alt)
    }

    /// The button of the gesture is held (any modifiers): the fly keys work while the orbit button is down.
    pub fn button_down(self, i: &egui::InputState) -> bool {
        self.parts().is_some_and(|(b, ..)| i.pointer.button_down(b))
    }
}

/// How the mouse moves the cameras.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Navigation {
    /// Orbit in 3D; also pans in 2D, which has no orbit.
    pub orbit: Gesture,
    pub pan: Gesture,
    /// Zoom by dragging up and down.
    pub zoom: Gesture,
    /// Multipliers of the orbit, pan and wheel zoom rates.
    pub orbit_speed: f32,
    pub pan_speed: f32,
    pub zoom_speed: f32,
    /// Wheel up zooms out.
    pub invert_zoom: bool,
    /// Dragging up tilts the camera down.
    pub invert_orbit_y: bool,
    /// While the orbit button is held in 3D, W A S D move the camera and Q E lower and raise it (Shift: faster).
    pub fly: bool,
}

impl Default for Navigation {
    fn default() -> Self {
        Navigation {
            orbit: Gesture::Right,
            pan: Gesture::Middle,
            zoom: Gesture::None,
            orbit_speed: 1.0,
            pan_speed: 1.0,
            zoom_speed: 1.0,
            invert_zoom: false,
            invert_orbit_y: false,
            fly: false,
        }
    }
}

/// The control settings: a preset, the user's chord changes on top of it, and the navigation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Controls {
    pub preset: Preset,
    /// Chords that replace the preset's for an action; an empty list unbinds it.
    pub overrides: BTreeMap<Action, Vec<String>>,
    pub nav: Navigation,
}

impl Controls {
    /// Switches to `p`: its navigation, no overrides.
    pub fn apply_preset(&mut self, p: Preset) {
        *self = Controls { preset: p, overrides: BTreeMap::new(), nav: p.navigation() };
    }

    /// The chords of `a`: the override when there is one, else the preset's; unparsable entries are skipped.
    pub fn chords(&self, a: Action) -> Vec<Chord> {
        match self.overrides.get(&a) {
            Some(list) => list.iter().filter_map(|s| Chord::parse(s)).collect(),
            None => self.preset.bindings().into_iter().find(|(x, _)| *x == a).map_or_else(Vec::new, |(_, c)| c.iter().filter_map(|s| Chord::parse(s)).collect()),
        }
    }

    /// Every (action, chord), most modifiers first, so Ctrl+Shift+Z is tried before Ctrl+Z.
    pub fn all(&self) -> Vec<(Action, Chord)> {
        let mut v: Vec<(Action, Chord)> = Action::ALL.iter().flat_map(|a| self.chords(*a).into_iter().map(move |c| (*a, c))).collect();
        v.sort_by_key(|(_, c)| std::cmp::Reverse(u8::from(c.ctrl) + u8::from(c.shift) + u8::from(c.alt)));
        v
    }

    /// Sets the chords of `a`; the preset's own chords drop the override.
    pub fn set(&mut self, a: Action, chords: &[Chord]) {
        let preset: Vec<Chord> = Controls { overrides: BTreeMap::new(), ..self.clone() }.chords(a);
        if preset == chords {
            self.overrides.remove(&a);
        } else {
            self.overrides.insert(a, chords.iter().map(Chord::to_string).collect());
        }
    }

    /// The first chord of `a` as text, for menus; "" when unbound.
    pub fn label(&self, a: Action) -> String {
        self.chords(a).first().map(Chord::to_string).unwrap_or_default()
    }

    /// Other actions bound to `c`.
    pub fn conflicts(&self, a: Action, c: Chord) -> Vec<Action> {
        Action::ALL.into_iter().filter(|x| *x != a && self.chords(*x).contains(&c)).collect()
    }
}

/// Takes the key presses of this frame that match a binding, most specific first, and returns their actions. Plain
/// chords (no Ctrl or Alt, not F-keys) and non-global actions are skipped while a text field has the keyboard.
pub fn consume(controls: &Controls, ctx: &egui::Context) -> Vec<Action> {
    if !ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))) {
        return Vec::new();
    }
    let typing = ctx.egui_wants_keyboard_input();
    let bindings = controls.all();
    let mut out = Vec::new();
    ctx.input_mut(|i| {
        i.events.retain(|ev| {
            let egui::Event::Key { key, pressed: true, modifiers, .. } = ev else { return true };
            let hit = bindings.iter().find(|(a, c)| c.key == *key && c.matches(*modifiers) && (!typing || (a.global() && !c.is_plain())));
            match hit {
                Some((a, _)) => {
                    out.push(*a);
                    false
                }
                None => true,
            }
        });
    });
    out
}

#[cfg(test)]
#[path = "keymap_tests.rs"]
mod tests;
