use crate::*;
use cad_geometry::{DVec2, DVec3, Shape};

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs(), "{a} vs {b}");
}

fn feature(id: FeatureId, kind: FeatureKind) -> Feature {
    Feature { id, name: format!("f{id}"), suppressed: false, kind }
}

fn push(d: &mut Document, f: Feature) {
    let index = d.features().len();
    d.apply(Command::AddFeature { index, feature: f }).unwrap();
}

fn shell(id: FeatureId, thickness: &str, open: Option<DVec3>, at: DVec3) -> Feature {
    feature(id, FeatureKind::Shell { thickness: thickness.into(), open, at })
}

fn top() -> DVec3 {
    DVec3::new(20.0, 10.0, 10.0)
}

/// A 40 x 20 x 10 plate (feature 1) and t = 2 mm.
fn plate() -> Document {
    let mut d = Document::new("plate");
    d.apply(Command::Add(Entity { id: 1, name: "r".into(), shape: Shape::rect(DVec2::ZERO, DVec2::new(40.0, 20.0)), layer: 0, sketch: 0 })).unwrap();
    d.apply(Command::SetParam { name: "t".into(), expr: Some("2 mm".into()) }).unwrap();
    let kind = FeatureKind::Extrude { profile: vec![1], distance: "10 mm".into(), offset: "0".into(), op: BodyOp::Add, flip: false, target: Target::Auto };
    push(&mut d, feature(1, kind));
    d
}

#[test]
fn open_shell_follows_its_parameter() {
    let mut d = plate();
    push(&mut d, shell(2, "t", Some(top()), top()));
    let mut rb = Rebuilder::default();
    let r = rb.rebuild(&d);
    assert_eq!((&r.states[&2], r.recomputed), (&FeatureState::Ok, 2));
    assert_eq!(r.bodies.len(), 1);
    close(r.total_volume(), 8000.0 - 36.0 * 16.0 * 8.0, 1e-6);
    d.apply(Command::SetParam { name: "t".into(), expr: Some("1 mm".into()) }).unwrap();
    let r = rb.rebuild(&d);
    assert_eq!((&r.states[&2], r.recomputed), (&FeatureState::Ok, 1));
    close(r.total_volume(), 8000.0 - 38.0 * 18.0 * 9.0, 1e-6);
    assert_eq!(d.features_using_param("t"), [2]);
}

#[test]
fn closed_shell_keeps_two_surfaces() {
    let mut d = plate();
    push(&mut d, shell(2, "2 mm", None, DVec3::new(0.0, 10.0, 5.0)));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    close(r.total_volume(), 8000.0 - 36.0 * 16.0 * 6.0, 1e-6);
}

#[test]
fn errors_leave_the_bodies_alone() {
    let mut d = plate();
    push(&mut d, shell(2, "2 mm", None, DVec3::new(100.0, 100.0, 100.0)));
    push(&mut d, shell(3, "6 mm", None, DVec3::new(0.0, 10.0, 5.0)));
    push(&mut d, shell(4, "2 mm", Some(DVec3::new(20.0, 10.0, 12.0)), DVec3::new(0.0, 10.0, 5.0)));
    push(&mut d, shell(5, "1 mm", None, DVec3::new(0.0, 10.0, 5.0)));
    push(&mut d, feature(6, FeatureKind::LinearPattern { source: 5, count: "2".into(), dx: "50 mm".into(), dy: "0".into(), dz: "0".into() }));
    let r = Rebuilder::default().rebuild(&d);
    let error = |id| match &r.states[&id] {
        FeatureState::Error(e) => e.clone(),
        s => panic!("feature {id}: {s:?}"),
    };
    assert!(error(2).contains("not found"));
    assert_eq!(error(3), "shell thickness is too large for this body");
    assert_eq!(error(4), "face not found; pick it again");
    assert_eq!(r.states[&5], FeatureState::Ok);
    assert_eq!(error(6), "this feature cannot be used as a source");
    close(r.total_volume(), 8000.0 - 38.0 * 18.0 * 8.0, 1e-6);
}

#[test]
fn undo_redo_and_save_load() {
    let mut d = plate();
    let mut h = History::default();
    h.execute(&mut d, vec![Command::AddFeature { index: 1, feature: shell(2, "t", Some(top()), top()) }]).unwrap();
    let after = d.features().to_vec();
    assert!(h.undo(&mut d));
    assert_eq!(d.features().len(), 1);
    close(Rebuilder::default().rebuild(&d).total_volume(), 8000.0, 1e-9);
    assert!(h.redo(&mut d));
    assert_eq!(d.features(), after);
    let p = std::env::temp_dir().join(format!("cadtest_{}_shell.cadproj", std::process::id()));
    d.save(&p).unwrap();
    let loaded = Document::load(&p);
    std::fs::remove_file(&p).unwrap();
    let loaded = loaded.unwrap();
    assert_eq!(loaded.features(), after);
    close(Rebuilder::default().rebuild(&loaded).total_volume(), 8000.0 - 36.0 * 16.0 * 8.0, 1e-6);
}

#[test]
fn bad_shell_points_are_rejected() {
    let mut d = plate();
    let nan = DVec3::new(f64::NAN, 0.0, 0.0);
    let index = d.features().len();
    assert!(d.apply(Command::AddFeature { index, feature: shell(2, "2 mm", None, nan) }).is_err());
    assert!(d.apply(Command::AddFeature { index, feature: shell(2, "2 mm", Some(nan), top()) }).is_err());
    assert!(d.apply(Command::AddFeature { index, feature: shell(2, "2 +", None, top()) }).is_err());
}

#[test]
fn version_9_file_loads() {
    let entity = r#""1":{"id":1,"name":"c","shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}}"#;
    let extrude = r#"{"id":1,"name":"e","suppressed":false,"kind":{"Extrude":{"profile":[1],"distance":"5 mm","offset":"0","op":"Add"}}}"#;
    let text = format!(r#"{{"version":9,"name":"x","next_id":2,"next_feature_id":2,"entities":{{{entity}}},"features":[{extrude}]}}"#);
    let p = std::env::temp_dir().join(format!("cadtest_{}_v9.cadproj", std::process::id()));
    std::fs::write(&p, text).unwrap();
    let d = Document::load(&p);
    std::fs::remove_file(&p).unwrap();
    let d = d.unwrap();
    assert_eq!(d.features().len(), 1);
    assert!(Rebuilder::default().rebuild(&d).total_volume() > 0.0);
}
