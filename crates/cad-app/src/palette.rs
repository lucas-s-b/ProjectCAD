//! Ctrl+K command search over the named widgets of the last frame (the registry MCP reads): a fuzzy match, then a
//! click on the chosen widget through egui input events inside the app (never OS input).
// shortcut: menu items are searchable only while their menu is open (the registry holds drawn widgets); keep a list of menu commands if users ask for them
use crate::app::CadApp;
use crate::widgets::{self as w, WidgetHit};
use eframe::egui::{self, Event, Modifiers};
use std::collections::VecDeque;

/// Results shown and reported in the MCP state.
pub const SHOWN: usize = 8;
/// Names the search draws itself; never results.
const OWN: [&str; 1] = ["Command search"];
const RUN_PREFIX: &str = "Run: ";

/// The open search window.
pub struct Palette {
    pub query: String,
    /// Name and occurrence index of each widget drawn in the frame before Ctrl+K, one per name: its first visible
    /// occurrence, else occurrence 0.
    pub items: Vec<(String, usize)>,
    /// The text field still needs the keyboard focus.
    pub focus: bool,
}

/// A chosen widget being clicked, one step per frame.
pub enum PaletteClick {
    /// Look the widget up in the last frame's registry (`scrolled`: it was already asked to scroll into view).
    Find { name: String, index: usize, scrolled: bool },
    /// Frames to wait for a scroll into view, then `Find` again.
    Wait { name: String, index: usize, frames: u8 },
    /// Event batches still to feed, one per frame.
    Events(VecDeque<Vec<Event>>),
}

/// Fuzzy score of `name` for `query`, case-insensitive: None unless every query character appears in `name` in
/// order (matched greedily, left to right). Each matched character scores 1, plus 8 when it starts a word (the first
/// character or after a space, '-', '_', '/', '(', ':', '#' or '.'), plus 4 when it directly follows the previous
/// match; the whole query being a prefix of the name adds 32. An empty query scores 0.
pub fn score(query: &str, name: &str) -> Option<i32> {
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let mut total = 0;
    let mut at = 0;
    let mut last: Option<usize> = None;
    for &c in &q {
        let i = at + n[at..].iter().position(|&x| x == c)?;
        total += 1;
        if i == 0 || matches!(n[i - 1], ' ' | '-' | '_' | '/' | '(' | ':' | '#' | '.') {
            total += 8;
        }
        if last.is_some_and(|l| l + 1 == i) {
            total += 4;
        }
        last = Some(i);
        at = i + 1;
    }
    if !q.is_empty() && n.starts_with(&q) {
        total += 32;
    }
    Some(total)
}

/// The items matching `query`, best first (score, then the shorter name, then the name), at most `max`.
pub fn ranked<'a>(query: &str, items: &'a [(String, usize)], max: usize) -> Vec<&'a (String, usize)> {
    let mut scored: Vec<(i32, &(String, usize))> = items.iter().filter_map(|it| score(query, &it.0).map(|s| (s, it))).collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.0.chars().count().cmp(&b.1.0.chars().count())).then(a.1.0.cmp(&b.1.0)));
    scored.into_iter().take(max).map(|(_, it)| it).collect()
}

/// One entry per name of `hits` (draw order), skipping the search's own widgets: (name, first visible occurrence or 0).
pub fn items(hits: &[WidgetHit]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for h in hits {
        if h.name.is_empty() || h.name.starts_with(RUN_PREFIX) || OWN.contains(&h.name.as_str()) || out.iter().any(|(n, _)| *n == h.name) {
            continue;
        }
        // Occurrence index as `widget_req::indexed` counts it: among the hits of this name before it.
        let index = hits.iter().position(|o| o.name == h.name && o.visible).map_or(0, |j| hits[..j].iter().filter(|o| o.name == h.name).count());
        out.push((h.name.clone(), index));
    }
    out
}

/// Opens the search over the widgets of the last frame.
pub fn open(app: &mut CadApp) {
    app.palette = Some(Palette { query: String::new(), items: items(&app.widgets), focus: true });
}

/// Draws the window while open: the query field (named "Command search", focused when opened), then the best
/// `SHOWN` results as buttons named "Run: <name>". Enter runs the first result, a click runs that one, Escape closes.
pub fn show(app: &mut CadApp, ctx: &egui::Context) {
    let Some(mut p) = app.palette.take() else { return };
    let mut chosen: Option<(String, usize)> = None;
    let mut close = false;
    egui::Window::new("Command search")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 60.0])
        .show(ctx, |ui| {
            let field = w::add(ui, "Command search", egui::TextEdit::singleline(&mut p.query).hint_text("Search commands").desired_width(320.0));
            if p.focus {
                field.request_focus();
                p.focus = false;
            }
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let results = ranked(&p.query, &p.items, SHOWN);
            for (name, index) in &results {
                if w::add(ui, &format!("{RUN_PREFIX}{name}"), egui::Button::new(name.as_str()).frame(false)).clicked() {
                    chosen = Some((name.clone(), *index));
                }
            }
            if enter && chosen.is_none() {
                chosen = results.first().map(|(name, index)| (name.clone(), *index));
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
        });
    if let Some((name, index)) = chosen {
        app.palette_click = Some(PaletteClick::Find { name, index, scrolled: false });
    } else if !close {
        app.palette = Some(p);
    }
}

/// From `raw_input_hook`, after the MCP bridge: feeds this frame's events of a chosen widget's click (pointer move,
/// move, press, release, like MCP `click_widget`), asking a hidden widget to scroll into view first (3 frames).
/// A widget that is gone or stays hidden puts "Command search: …" in the status line.
pub fn feed(app: &mut CadApp, ctx: &egui::Context, raw: &mut egui::RawInput) {
    let Some(step) = app.palette_click.take() else { return };
    ctx.request_repaint();
    app.palette_click = match step {
        PaletteClick::Wait { name, index, frames: 0 } => Some(PaletteClick::Find { name, index, scrolled: true }),
        PaletteClick::Wait { name, index, frames } => Some(PaletteClick::Wait { name, index, frames: frames - 1 }),
        PaletteClick::Find { name, index, scrolled } => match crate::mcp::widget_req::find(&app.widgets, &name, index) {
            Err(err) => {
                app.status = format!("Command search: {err}");
                None
            }
            Ok(h) if !h.visible && !scrolled => {
                crate::widgets::request_scroll(ctx, &name, index);
                Some(PaletteClick::Wait { name, index, frames: 3 })
            }
            Ok(h) if !h.visible => {
                app.status = format!("Command search: widget '{name}' is not visible");
                None
            }
            Ok(h) => {
                let p = h.rect.center();
                let b = |pressed| Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
                Some(PaletteClick::Events(VecDeque::from([vec![Event::PointerMoved(p)], vec![Event::PointerMoved(p)], vec![b(true)], vec![b(false)]])))
            }
        },
        PaletteClick::Events(mut q) => {
            raw.events.extend(q.pop_front().unwrap_or_default());
            (!q.is_empty()).then_some(PaletteClick::Events(q))
        }
    };
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
