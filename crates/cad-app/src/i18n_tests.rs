use super::*;
use std::collections::BTreeSet;

#[test]
fn all_is_in_discriminant_order() {
    // `current` indexes `Language::ALL` with the stored discriminant.
    for (i, l) in Language::ALL.iter().enumerate() {
        assert_eq!(*l as usize, i, "{l:?}");
    }
}

#[test]
fn every_language_translates_the_same_strings() {
    let reference: BTreeSet<&str> = table(Language::Spanish).keys().copied().collect();
    assert!(reference.len() > 300, "Spanish table has {} entries", reference.len());
    for lang in &Language::ALL[1..] {
        let src = lang.source();
        let lines = src.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).count();
        let t = table(*lang);
        assert_eq!(t.len(), lines, "{lang:?}: a line without ' => ' or a duplicate key");
        let keys: BTreeSet<&str> = t.keys().copied().collect();
        let missing: Vec<_> = reference.difference(&keys).collect();
        let extra: Vec<_> = keys.difference(&reference).collect();
        assert!(missing.is_empty() && extra.is_empty(), "{lang:?}: missing {missing:?}, extra {extra:?}");
        assert!(t.values().all(|v| !v.is_empty()), "{lang:?} has an empty translation");
    }
}

#[test]
fn every_language_has_the_tutorial() {
    for lang in &Language::ALL[1..] {
        for (key, title, text) in crate::tutorial::STEPS {
            assert_ne!(lookup(*lang, &format!("{key}.title"), title), title, "{lang:?} {key}");
            assert_ne!(lookup(*lang, &format!("{key}.text"), text), text, "{lang:?} {key}");
        }
    }
}

#[test]
fn lookup_translates_or_falls_back_to_english() {
    assert_eq!(lookup(Language::Spanish, "Sketch", "Sketch"), "Boceto");
    assert_eq!(lookup(Language::German, "Settings", "Settings"), "Einstellungen");
    assert_eq!(lookup(Language::Japanese, "No such text", "No such text"), "No such text");
    assert_eq!(lookup(Language::English, "Sketch", "Sketch"), "Sketch");
}

#[test]
fn settings_default_to_english_and_the_tutorial() {
    let s: crate::settings::Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(s.language, Language::English);
    assert!(!s.tutorial_done);
    let json = serde_json::to_string(&crate::settings::Settings { language: Language::Korean, ..Default::default() }).unwrap();
    let back: crate::settings::Settings = serde_json::from_str(&json).unwrap();
    assert_eq!(back.language, Language::Korean);
}
