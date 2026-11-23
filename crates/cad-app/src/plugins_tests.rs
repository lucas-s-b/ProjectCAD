use super::*;

fn fresh(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn manifest_is_strict() {
    let m: Manifest = serde_json::from_str(r#"{"name":"Bolt circle","command":"bolt.exe","version":"1"}"#).unwrap();
    assert!(m.args.is_empty());
    assert!(serde_json::from_str::<Manifest>(r#"{"name":"a","command":"b","version":"1","auto_run":true}"#).is_err());
    assert!(serde_json::from_str::<Manifest>(r#"{"name":"a","version":"1"}"#).is_err());
}

#[test]
fn scan_sorts_and_reports_bad_files() {
    let d = fresh("cad-plugins-test-scan");
    std::fs::create_dir_all(&d).unwrap();
    let files = [
        ("b.json", r#"{"name":"Bolt circle","command":"bolt.exe","version":"1"}"#),
        ("a.json", r#"{"name":"Stairs","command":"stairs.exe","args":["--steps","12"],"version":"0.2"}"#),
        ("c.json", "{"),
        ("d.txt", "x"),
        ("e.json", r#"{"name":"Stairs","command":"other.exe","version":"1"}"#),
        ("f.json", r#"{"name":"","command":"x","version":"1"}"#),
    ];
    for (f, text) in files {
        std::fs::write(d.join(f), text).unwrap();
    }
    let (found, errors) = scan(&d);
    assert_eq!(found.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Bolt circle", "Stairs"]);
    assert_eq!(found[1].args, ["--steps", "12"]);
    assert_eq!(found[1].command, "stairs.exe");
    assert_eq!(errors.len(), 3, "{errors:?}");
    assert!(errors[0].starts_with("plugins/c.json:"));
    assert!(errors[1].starts_with("plugins/e.json: duplicate plugin name 'Stairs'"));
    assert!(errors[2].starts_with("plugins/f.json:"));
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(scan(&d), (vec![], vec![]));
}

#[test]
fn paths_beside_the_settings_file() {
    assert_eq!(dir(Some(Path::new("a/b/settings.json"))), Some(Path::new("a/b").join("plugins")));
    assert_eq!(dir(None), None);
    assert_eq!(command_path(Path::new("p"), "bolt.exe"), Path::new("p").join("bolt.exe"));
    let abs = std::env::temp_dir().join("x.exe");
    assert_eq!(command_path(Path::new("p"), abs.to_str().unwrap()), abs);
}

#[test]
fn missing_program_is_reported() {
    let m = Manifest { name: "n".into(), command: "cad-no-such-program-7e.exe".into(), args: vec![], version: "1".into() };
    assert!(launch(&std::env::temp_dir(), &m, &egui::Context::default()).is_err());
}
