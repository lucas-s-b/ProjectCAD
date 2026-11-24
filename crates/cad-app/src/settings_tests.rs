use super::*;

fn sample() -> Settings {
    Settings {
        theme: Theme::Light,
        accent: [200, 100, 20],
        layout: Layout { docks: [Dock::Right, Dock::Left, Dock::Bottom], shown: [true, false, true], sizes: [300.0, 250.0, 180.0] },
        ui: Interface { scale: 1.25, animations: false, ..Interface::default() },
        general: General { autosave_min: 0, nudge_mm: 0.5, ..General::default() },
        controls: {
            let mut c = crate::keymap::Controls::default();
            c.apply_preset(crate::keymap::Preset::Blender);
            c
        },
        recent: vec!["C:/a.cadproj".into()],
        language: crate::i18n::Language::French,
        tutorial_done: true,
    }
}

#[test]
fn settings_round_trip() {
    let s = sample();
    let text = serde_json::to_string(&s).unwrap();
    assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), s);
}

#[test]
fn missing_fields_take_defaults() {
    let s = serde_json::from_str::<Settings>(r#"{"theme":"Light"}"#).unwrap();
    assert_eq!(s.theme, Theme::Light);
    assert_eq!(s.accent, [0, 92, 128]);
    assert_eq!(s.layout, Layout::default());
    let s = serde_json::from_str::<Settings>(r#"{"layout":{"sizes":[200.0,260.0,150.0]}}"#).unwrap();
    assert_eq!(s.layout.docks, [Dock::Right; 3]);
    assert_eq!(s.layout.shown, [true; 3]);
    assert_eq!(s.layout.sizes, [200.0, 260.0, 150.0]);
}

#[test]
fn save_and_load_file() {
    let p = std::env::temp_dir().join("cad-settings-test/settings.json");
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
    assert_eq!(load(&p), Ok(Settings::default()));
    save(&p, &sample()).unwrap();
    assert_eq!(load(&p), Ok(sample()));
    assert!(std::fs::read_to_string(&p).unwrap().contains("\"theme\": \"Light\""));
    std::fs::write(&p, "{").unwrap();
    assert!(load(&p).unwrap_err().starts_with("Settings file ignored:"));
}

#[test]
fn slots_show_the_current_tab() {
    let mut l = Layout::default();
    assert_eq!(l.tabs_in(Dock::Right), SideTab::ALL.to_vec());
    assert!(l.tabs_in(Dock::Left).is_empty());
    assert_eq!(l.shown_tab(Dock::Right, SideTab::Model), Some(SideTab::Model));
    assert_eq!(l.shown_tab(Dock::Left, SideTab::Model), None);
    l.docks = [Dock::Right, Dock::Left, Dock::Right];
    assert_eq!(l.shown_tab(Dock::Left, SideTab::Sketch), Some(SideTab::Model));
    assert_eq!(l.shown_tab(Dock::Right, SideTab::Model), Some(SideTab::Sketch));
    assert_eq!(l.shown_tab(Dock::Right, SideTab::Assembly), Some(SideTab::Assembly));
}

#[test]
fn visuals_use_the_accent() {
    let light = visuals(&Settings { theme: Theme::Light, accent: [200, 100, 20], ..Settings::default() });
    assert!(!light.dark_mode);
    assert_eq!(light.selection.bg_fill, egui::Color32::from_rgb(200, 100, 20));
    assert_eq!(light.selection.stroke.color, egui::Color32::WHITE);
    let dark = visuals(&Settings { theme: Theme::Dark, accent: [144, 209, 255], ..Settings::default() });
    assert!(dark.dark_mode);
    assert_eq!(dark.selection.stroke.color, egui::Color32::BLACK);
}

#[test]
fn mcp_has_no_settings_file_without_cad_settings() {
    if std::env::var_os("CAD_SETTINGS").is_none() {
        assert_eq!(path(true), None);
    }
}

#[test]
fn old_files_load_with_new_defaults() {
    let s = serde_json::from_str::<Settings>(r#"{"theme":"Dark","accent":[0,92,128],"layout":{}}"#).unwrap();
    assert_eq!(s.ui, Interface::default());
    assert_eq!(s.general, General::default());
    assert_eq!(s.controls, crate::keymap::Controls::default());
    assert!(s.recent.is_empty());
}

#[test]
fn recent_list_moves_to_front_and_is_capped() {
    let mut s = Settings { general: General { recent_max: 3, ..General::default() }, ..Settings::default() };
    for p in ["a", "b", "c", "d", "b"] {
        s.push_recent(Path::new(p));
    }
    assert_eq!(s.recent, vec!["b", "d", "c"]);
}

#[test]
fn reset_layout_is_the_default() {
    assert_eq!(Layout::default().sizes[Dock::Right.index()], 240.0);
    assert_eq!(Dock::ALL[0], Dock::Right);
}
