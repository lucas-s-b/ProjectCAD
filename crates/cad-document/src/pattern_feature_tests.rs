use crate::rebuild_pattern::tests::{circle, extrude, linear, mirror, push, put, rect};
use crate::*;

fn doc() -> Document {
    let mut d = Document::new("t");
    put(&mut d, 1, 0, rect(0.0, 0.0, 40.0, 20.0));
    put(&mut d, 2, 0, circle(5.0, 5.0, 2.0));
    push(&mut d, extrude(1, 1, "5 mm", BodyOp::Add));
    push(&mut d, extrude(2, 2, "5 mm", BodyOp::Cut));
    d
}

fn circular(id: FeatureId, source: FeatureId) -> Feature {
    let kind = FeatureKind::CircularPattern {
        source,
        count: "4".into(),
        angle: "180 deg".into(),
        axis: Axis3::Y,
        cx: "1 mm".into(),
        cy: "0".into(),
        cz: "2 mm".into(),
    };
    Feature { id, name: "c".into(), suppressed: false, kind }
}

#[test]
fn source_checks_and_use_tracking() {
    let mut d = doc();
    let add = |index, feature| Command::AddFeature { index, feature };
    assert!(matches!(d.apply(add(2, linear(3, 3, "2", ["1", "0", "0"]))), Err(DocError::BadFeature("a feature cannot use itself"))));
    assert!(matches!(d.apply(add(2, linear(3, 9, "2", ["1", "0", "0"]))), Err(DocError::FeatureNotFound(9))));
    assert!(matches!(d.apply(add(2, linear(3, 2, "2", ["1", "0", "0 +"]))), Err(DocError::Param(_))));
    d.apply(add(2, linear(3, 2, "2", ["1", "0", "0"]))).unwrap();
    assert!(matches!(d.apply(Command::SetFeature(linear(3, 3, "2", ["1", "0", "0"]))), Err(DocError::BadFeature(_))));
    assert!(matches!(d.apply(Command::RemoveFeature(2)), Err(DocError::FeatureInUse(2))));
    assert_eq!(d.features().len(), 3);
    d.apply(Command::RemoveFeature(3)).unwrap();
    d.apply(Command::RemoveFeature(2)).unwrap();
    // A mirror of all bodies uses no feature.
    d.apply(add(1, mirror(4, MirrorSource::Bodies, BasePlane::XY, "0"))).unwrap();
    d.apply(Command::RemoveFeature(1)).unwrap();
}

#[test]
fn expressions_belong_to_the_feature() {
    let mut d = doc();
    d.apply(Command::SetParam { name: "n".into(), expr: Some("3".into()) }).unwrap();
    push(&mut d, linear(3, 2, "n", ["1", "0", "0"]));
    assert_eq!(d.features_using_param("n"), [3]);
    assert_eq!(circular(4, 2).expressions(), ["4", "180 deg", "1 mm", "0", "2 mm"]);
    assert_eq!(circular(4, 2).source(), Some(2));
    assert_eq!(mirror(5, MirrorSource::Bodies, BasePlane::XY, "0").source(), None);
}

#[test]
fn undo_and_redo_of_pattern_commands() {
    let mut d = doc();
    let mut h = History::default();
    let before = d.features().to_vec();
    h.execute(&mut d, vec![Command::AddFeature { index: 2, feature: linear(3, 2, "2", ["1", "0", "0"]) }]).unwrap();
    h.execute(&mut d, vec![Command::AddFeature { index: 3, feature: circular(4, 3) }]).unwrap();
    let after = d.features().to_vec();
    assert!(h.undo(&mut d) && h.undo(&mut d));
    assert_eq!(d.features(), before);
    assert!(h.redo(&mut d) && h.redo(&mut d));
    assert_eq!(d.features(), after);
}

#[test]
fn save_load_roundtrip_and_old_files() {
    let mut d = doc();
    push(&mut d, linear(3, 2, "2", ["1", "0", "0"]));
    push(&mut d, circular(4, 3));
    push(&mut d, mirror(5, MirrorSource::Feature(4), BasePlane::XZ, "2 mm"));
    push(&mut d, mirror(6, MirrorSource::Bodies, BasePlane::YZ, "0"));
    let p = std::env::temp_dir().join(format!("cadtest_{}_pattern.cadproj", std::process::id()));
    d.save(&p).unwrap();
    let loaded = Document::load(&p);
    std::fs::remove_file(&p).unwrap();
    assert_eq!(loaded.unwrap().features(), d.features());

    let entity = r#""1":{"id":1,"name":"c","shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}}"#;
    let extrude = r#"{"id":4,"name":"e","suppressed":false,"kind":{"Extrude":{"profile":[1],"distance":"t","offset":"0","op":"Add"}}}"#;
    let v7 = format!(r#"{{"version":7,"name":"x","next_id":2,"next_feature_id":1,"entities":{{{entity}}},"features":[{extrude}]}}"#);
    std::fs::write(&p, &v7).unwrap();
    let old = Document::load(&p);
    let dangling = v7.replace(r#"]}"#, r#",{"id":5,"name":"p","suppressed":false,"kind":{"Mirror":{"source":{"Feature":9},"plane":"XY","offset":"0"}}}]}"#);
    std::fs::write(&p, dangling).unwrap();
    let bad = Document::load(&p);
    std::fs::remove_file(&p).unwrap();
    assert_eq!(old.unwrap().features().len(), 1);
    assert!(matches!(bad, Err(DocError::FeatureNotFound(9))));
}
