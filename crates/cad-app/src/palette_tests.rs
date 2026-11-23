use super::*;
use eframe::egui::Rect;

fn item(name: &str) -> (String, usize) {
    (name.to_string(), 0)
}

#[test]
fn score_rewards_word_starts_and_runs() {
    assert_eq!(score("", "Line"), Some(0));
    assert_eq!(score("lin", "Line"), Some(51));
    assert_eq!(score("ab", "Add box"), Some(18));
    assert_eq!(score("adb", "Add box"), Some(23));
    assert_eq!(score("xz", "Line"), None);
    assert_eq!(score("LIN", "line"), Some(51));
}

#[test]
fn ranked_orders_by_score_then_length() {
    let items = [item("Text"), item("Extrude"), item("Export STEP...")];
    let names = |q: &str, max: usize| ranked(q, &items, max).iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>();
    assert_eq!(names("ext", 8), ["Extrude", "Export STEP...", "Text"]);
    assert_eq!(ranked("e", &items, 2).len(), 2);
    assert!(ranked("q", &items, 8).is_empty());
}

#[test]
fn items_skip_own_widgets_and_prefer_visible() {
    let hit = |name: &str, visible| WidgetHit { name: name.into(), rect: Rect::NOTHING, visible };
    let hits = [hit("Line", false), hit("Line", true), hit("Command search", true), hit("Run: Line", true), hit("Arc", true)];
    assert_eq!(items(&hits), [("Line".to_string(), 1), ("Arc".to_string(), 0)]);
}
