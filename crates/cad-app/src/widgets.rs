//! Registry of named widgets, so the MCP mode can click a widget by name instead of by pixel. The labelled helpers
//! draw the label in the current language (`i18n::tr`) and register it under its English text.
use crate::i18n::tr;
use eframe::egui;

#[derive(Clone, Debug)]
pub struct WidgetHit {
    pub name: String,
    pub rect: egui::Rect,
    pub visible: bool,
}

#[derive(Clone, Default)]
struct Registry {
    hits: Vec<WidgetHit>,
    /// (name, occurrence index) of a widget to scroll into view when it is next drawn.
    scroll: Option<(String, usize)>,
}

fn with<R>(ctx: &egui::Context, f: impl FnOnce(&mut Registry) -> R) -> R {
    ctx.data_mut(|d| f(d.get_temp_mut_or_default::<Registry>(egui::Id::NULL)))
}

/// Records where a named widget was drawn in this frame, so the MCP mode can find it by name.
pub fn tag(ui: &egui::Ui, name: &str, resp: egui::Response) -> egui::Response {
    // The panel clip rect reaches below the window, so also require the widget to be inside the window.
    let visible = ui.is_rect_visible(resp.rect) && ui.ctx().content_rect().contains_rect(resp.rect);
    let scroll = with(ui.ctx(), |r| {
        let index = r.hits.iter().filter(|h| h.name == name).count();
        r.hits.push(WidgetHit { name: name.to_string(), rect: resp.rect, visible });
        let hit = r.scroll.as_ref().is_some_and(|(n, i)| n == name && *i == index);
        if hit {
            r.scroll = None;
        }
        hit
    });
    if scroll {
        // No animation: the click is queued a few frames after the request.
        resp.scroll_to_me_animation(Some(egui::Align::Center), egui::style::ScrollAnimation::none());
    }
    resp
}

pub fn add(ui: &mut egui::Ui, name: &str, widget: impl egui::Widget) -> egui::Response {
    let r = ui.add(widget);
    tag(ui, name, r)
}

pub fn add_enabled(ui: &mut egui::Ui, enabled: bool, name: &str, widget: impl egui::Widget) -> egui::Response {
    let r = ui.add_enabled(enabled, widget);
    tag(ui, name, r)
}

pub fn button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let r = ui.button(tr(label));
    tag(ui, label, r)
}

pub fn selectable(ui: &mut egui::Ui, selected: bool, label: &str) -> egui::Response {
    let r = ui.selectable_label(selected, tr(label));
    tag(ui, label, r)
}

/// Like `ui.selectable_value`, named by its label.
pub fn selectable_value<T: PartialEq>(ui: &mut egui::Ui, current: &mut T, value: T, label: &str) -> egui::Response {
    let mut r = selectable(ui, *current == value, label);
    if r.clicked() && *current != value {
        *current = value;
        r.mark_changed();
    }
    r
}

pub fn checkbox(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    let r = ui.checkbox(value, tr(label));
    tag(ui, label, r)
}

/// Returns the widgets of the frame that just ended and starts a new frame.
pub fn begin_frame(ctx: &egui::Context) -> Vec<WidgetHit> {
    with(ctx, |r| std::mem::take(&mut r.hits))
}

/// The widget with this name and occurrence index calls `scroll_to_me` when it is next drawn.
pub fn request_scroll(ctx: &egui::Context, name: &str, index: usize) {
    with(ctx, |r| r.scroll = Some((name.to_string(), index)));
}

pub fn menu_button(ui: &mut egui::Ui, label: &str, contents: impl FnOnce(&mut egui::Ui)) {
    let r = ui.menu_button(tr(label), contents);
    tag(ui, label, r.response);
}

/// Shows a combo box and names its button.
pub fn combo(ui: &mut egui::Ui, name: &str, combo: egui::ComboBox, contents: impl FnOnce(&mut egui::Ui)) {
    let mut resp = combo.show_ui(ui, contents).response;
    // A combo box with a label answers with the button and the label; click only the button.
    resp.rect.set_width(resp.rect.width().min(ui.spacing().combo_width));
    tag(ui, name, resp);
}
