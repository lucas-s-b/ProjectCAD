//! Lookup of named widgets for the MCP tools `widgets`, `click_widget` and `set_text`.
use crate::app::CadApp;
use crate::widgets::WidgetHit;
use eframe::egui;
use serde_json::{Value, json};

/// Occurrence index of each hit among the hits with the same name, in draw order.
fn indexed(hits: &[WidgetHit]) -> impl Iterator<Item = (usize, &WidgetHit)> {
    hits.iter().enumerate().map(|(i, h)| (hits[..i].iter().filter(|o| o.name == h.name).count(), h))
}

/// All widgets of the last frame, in screenshot pixels (centre x, y and size).
pub fn list(app: &CadApp, ctx: &egui::Context) -> Value {
    let ppp = ctx.pixels_per_point();
    let items: Vec<Value> = indexed(&app.widgets)
        .map(|(index, h)| {
            let (c, s) = (h.rect.center(), h.rect.size());
            json!({ "name": h.name, "index": index, "x": c.x * ppp, "y": c.y * ppp, "w": s.x * ppp, "h": s.y * ppp, "visible": h.visible })
        })
        .collect();
    Value::Array(items)
}

/// The widget with this name and occurrence index, or the error text of the tool.
pub fn find<'a>(hits: &'a [WidgetHit], name: &str, index: usize) -> Result<&'a WidgetHit, String> {
    if let Some((_, h)) = indexed(hits).find(|(i, h)| h.name == name && *i == index) {
        return Ok(h);
    }
    let needle = name.to_lowercase();
    let mut similar: Vec<&str> = Vec::new();
    for h in hits {
        if h.name.to_lowercase().contains(&needle) && !similar.contains(&h.name.as_str()) && similar.len() < 5 {
            similar.push(&h.name);
        }
    }
    let hint = if similar.is_empty() { String::new() } else { format!("; similar names: {}", similar.join(", ")) };
    Err(format!("widget '{name}' (index {index}) not found{hint}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(name: &str) -> WidgetHit {
        WidgetHit { name: name.into(), rect: egui::Rect::from_min_size(egui::pos2(1.0, 2.0), egui::vec2(10.0, 4.0)), visible: true }
    }

    #[test]
    fn find_by_name_and_index() {
        let hits = [hit("x"), hit("Line"), hit("x")];
        assert_eq!(find(&hits, "x", 1).unwrap().name, "x");
        let e = find(&hits, "x", 2).unwrap_err();
        assert!(e.contains("'x' (index 2) not found") && e.contains("similar names: x"), "{e}");
        let e = find(&hits, "lin", 0).unwrap_err();
        assert!(e.contains("similar names: Line"), "{e}");
        assert!(!find(&hits, "zzz", 0).unwrap_err().contains("similar"));
    }
}
