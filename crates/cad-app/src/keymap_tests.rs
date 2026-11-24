use super::*;

#[test]
fn chords_parse_and_print() {
    for text in ["Ctrl+Shift+Z", "F3", "Delete", "Alt+A", "Ctrl+Comma", "Shift+Left", "1", "Period", "Backtick", "Ctrl+Space"] {
        let c = Chord::parse(text).unwrap_or_else(|| panic!("{text} does not parse"));
        assert_eq!(c.to_string(), text);
    }
    assert_eq!(Chord::parse("cmd+z"), Some(Chord { key: Key::Z, ctrl: true, shift: false, alt: false }));
    assert_eq!(Chord::parse("Hyper+Z"), None);
    assert_eq!(Chord::parse("Ctrl+Nope"), None);
    assert_eq!(Chord::parse(""), None);
}

#[test]
fn chords_match_exact_modifiers() {
    let c = Chord::parse("Ctrl+Z").unwrap();
    assert!(c.matches(Modifiers::COMMAND));
    assert!(!c.matches(Modifiers::COMMAND | Modifiers::SHIFT));
    assert!(!c.matches(Modifiers::NONE));
    assert!(Chord::parse("G").unwrap().is_plain());
    assert!(Chord::parse("Shift+D").unwrap().is_plain());
    assert!(!Chord::parse("F3").unwrap().is_plain());
    assert!(!Chord::parse("Ctrl+K").unwrap().is_plain());
}

#[test]
fn every_preset_parses_without_conflicts() {
    for p in Preset::ALL {
        let mut c = Controls::default();
        c.apply_preset(p);
        for (a, chords) in p.bindings() {
            assert_eq!(chords.len(), c.chords(a).len(), "{}: unparsable chord of {a:?}", p.name());
        }
        for (a, chord) in c.all() {
            assert!(c.conflicts(a, chord).is_empty(), "{}: {chord} is bound to {a:?} and {:?}", p.name(), c.conflicts(a, chord));
        }
    }
}

#[test]
fn default_preset_keeps_the_old_shortcuts() {
    let c = Controls::default();
    for (a, text) in [(Action::Undo, "Ctrl+Z"), (Action::Save, "Ctrl+S"), (Action::Duplicate, "Ctrl+D"), (Action::Fit, "F"), (Action::Delete, "Delete"), (Action::ToggleOsnap, "F3"), (Action::ToggleGridSnap, "F9"), (Action::CommandSearch, "Ctrl+K")] {
        assert!(c.chords(a).contains(&Chord::parse(text).unwrap()), "{a:?} lost {text}");
    }
    assert_eq!(c.chords(Action::Redo), vec![Chord::parse("Ctrl+Y").unwrap(), Chord::parse("Ctrl+Shift+Z").unwrap()]);
    // Most modifiers first: Ctrl+Shift+Z must be tried before Ctrl+Z.
    let all = c.all();
    let pos = |t: &str| all.iter().position(|(_, x)| *x == Chord::parse(t).unwrap()).unwrap();
    assert!(pos("Ctrl+Shift+Z") < pos("Ctrl+Z"));
}

#[test]
fn overrides_replace_and_reset() {
    let mut c = Controls::default();
    c.set(Action::Fit, &[Chord::parse("Home").unwrap()]);
    assert_eq!(c.label(Action::Fit), "Home");
    assert_eq!(c.overrides.len(), 1);
    c.set(Action::Fit, &[]);
    assert_eq!(c.label(Action::Fit), "");
    c.set(Action::Fit, &[Chord::parse("F").unwrap()]);
    assert!(c.overrides.is_empty(), "the preset's own chord drops the override");
    c.set(Action::Grab, &[Chord::parse("Ctrl+Z").unwrap()]);
    assert_eq!(c.conflicts(Action::Grab, Chord::parse("Ctrl+Z").unwrap()), vec![Action::Undo]);
    c.apply_preset(Preset::Blender);
    assert!(c.overrides.is_empty());
    assert_eq!(c.nav.orbit, Gesture::Middle);
    assert_eq!(c.label(Action::Delete), "X");
}

#[test]
fn controls_round_trip_through_json() {
    let mut c = Controls::default();
    c.apply_preset(Preset::Roblox);
    c.set(Action::ToolLine, &[Chord::parse("Alt+L").unwrap()]);
    c.nav.zoom_speed = 2.0;
    let text = serde_json::to_string(&c).unwrap();
    assert_eq!(serde_json::from_str::<Controls>(&text).unwrap(), c);
    assert_eq!(serde_json::from_str::<Controls>("{}").unwrap(), Controls::default());
}
