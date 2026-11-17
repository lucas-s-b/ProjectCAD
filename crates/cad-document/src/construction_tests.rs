use crate::*;
use cad_geometry::{DVec2, Shape};

fn add(id: EntityId, shape: Shape, construction: bool) -> Command {
    Command::Add(Entity { id, name: format!("e{id}"), shape, layer: 0, sketch: 0, construction, group: None, meta: Default::default() })
}

fn diagonal() -> Shape {
    Shape::Line { a: DVec2::ZERO, b: DVec2::new(40.0, 20.0) }
}

fn extrude(profile: Vec<EntityId>) -> Feature {
    let kind = FeatureKind::Extrude { profile, distance: "5 mm".into(), offset: "0".into(), taper: "0 deg".into(), op: BodyOp::Add, flip: false, target: Target::Auto };
    Feature { id: 1, name: "f".into(), suppressed: false, kind }
}

fn rebuilt(d: &Document) -> RebuildResult {
    Rebuilder::default().rebuild(d)
}

#[test]
fn set_construction_inverts_and_undoes() {
    let mut d = Document::new("t");
    let mut h = History::default();
    h.execute(&mut d, vec![add(1, diagonal(), false)]).unwrap();
    let inv = d.apply(Command::SetConstruction { id: 1, on: true }).unwrap();
    assert_eq!(inv, Command::SetConstruction { id: 1, on: false });
    assert!(d.get(1).unwrap().construction);
    assert!(matches!(d.apply(Command::SetConstruction { id: 9, on: true }), Err(DocError::NotFound(9))));
    h.execute(&mut d, vec![Command::SetConstruction { id: 1, on: false }]).unwrap();
    assert!(!d.get(1).unwrap().construction);
    assert!(h.undo(&mut d));
    assert!(d.get(1).unwrap().construction);
    assert!(h.redo(&mut d));
    assert!(!d.get(1).unwrap().construction);
}

#[test]
fn a_construction_line_does_not_spoil_a_profile() {
    let mut d = Document::new("t");
    d.apply(add(1, Shape::rect(DVec2::ZERO, DVec2::new(40.0, 20.0)), false)).unwrap();
    d.apply(add(2, diagonal(), true)).unwrap();
    d.apply(Command::AddFeature { index: 0, feature: extrude(vec![1, 2]) }).unwrap();
    let r = rebuilt(&d);
    assert_eq!(r.states[&1], FeatureState::Ok);
    assert!((r.total_volume() - 4000.0).abs() < 1e-6);
}

#[test]
fn a_profile_of_only_construction_entities_is_an_error() {
    let mut d = Document::new("t");
    d.apply(add(1, Shape::rect(DVec2::ZERO, DVec2::new(40.0, 20.0)), true)).unwrap();
    d.apply(Command::AddFeature { index: 0, feature: extrude(vec![1]) }).unwrap();
    assert!(matches!(rebuilt(&d).states[&1], FeatureState::Error(_)));
}

#[test]
fn construction_survives_save_and_old_files_load() {
    let mut d = Document::new("t");
    d.apply(add(1, diagonal(), true)).unwrap();
    let p = std::env::temp_dir().join(format!("cadtest_{}_construction.cadproj", std::process::id()));
    d.save(&p).unwrap();
    let loaded = Document::load(&p).unwrap();
    assert!(loaded.get(1).unwrap().construction);
    let v10 = r#"{"version":10,"name":"x","next_id":2,"entities":{"1":{"id":1,"name":"l","shape":{"Line":{"a":[0.0,0.0],"b":[1.0,0.0]}}}}}"#;
    std::fs::write(&p, v10).unwrap();
    let old = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    assert!(!old.get(1).unwrap().construction);
}
