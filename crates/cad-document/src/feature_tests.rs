use crate::*;
use cad_geometry::{DVec2, Shape};

fn rect_entity(id: EntityId) -> Entity {
    Entity { id, name: format!("r{id}"), shape: Shape::rect(DVec2::ZERO, DVec2::new(40.0, 20.0)), layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() }
}

fn extrude(id: FeatureId, profile: Vec<EntityId>, distance: &str) -> Feature {
    let kind = FeatureKind::Extrude { profile, distance: distance.into(), offset: "0".into(), taper: "0 deg".into(), op: BodyOp::Add, flip: false, target: Target::Auto };
    Feature { id, name: format!("f{id}"), suppressed: false, kind }
}

fn doc() -> Document {
    let mut d = Document::new("t");
    for id in [1, 2] {
        d.apply(Command::Add(rect_entity(id))).unwrap();
    }
    d
}

fn snapshot(d: &Document) -> (Vec<Feature>, Vec<(String, String)>) {
    (d.features().to_vec(), d.params().iter().map(|(n, s)| (n.to_string(), s.to_string())).collect())
}

#[test]
fn inverses_are_correct() {
    let mut d = doc();
    let (f1, f2) = (extrude(1, vec![1], "5"), extrude(2, vec![2], "7"));
    assert_eq!(d.apply(Command::AddFeature { index: 0, feature: f1.clone() }).unwrap(), Command::RemoveFeature(1));
    d.apply(Command::AddFeature { index: 1, feature: f2.clone() }).unwrap();
    let f1b = Feature { suppressed: true, ..extrude(1, vec![1, 2], "9") };
    assert_eq!(d.apply(Command::SetFeature(f1b)).unwrap(), Command::SetFeature(f1.clone()));
    assert_eq!(d.apply(Command::MoveFeature { id: 1, index: 1 }).unwrap(), Command::MoveFeature { id: 1, index: 0 });
    assert_eq!(d.features().iter().map(|f| f.id).collect::<Vec<_>>(), [2, 1]);
    let inv = d.apply(Command::RemoveFeature(2)).unwrap();
    assert_eq!(inv, Command::AddFeature { index: 0, feature: f2 });
    let set = |expr: Option<&str>| Command::SetParam { name: "t".into(), expr: expr.map(Into::into) };
    assert_eq!(d.apply(set(Some("5 mm"))).unwrap(), set(None));
    assert_eq!(d.apply(set(Some("6 mm"))).unwrap(), set(Some("5 mm")));
    assert_eq!(d.apply(set(None)).unwrap(), set(Some("6 mm")));
    assert!(d.params().is_empty());
}

#[test]
fn undo_redo_roundtrip_through_history() {
    let mut d = doc();
    let mut h = History::default();
    let mut states = vec![snapshot(&d)];
    let set = |name: &str, expr: Option<&str>| Command::SetParam { name: name.into(), expr: expr.map(Into::into) };
    let txs = vec![
        vec![Command::AddFeature { index: 0, feature: extrude(1, vec![1], "t") }],
        vec![Command::AddFeature { index: 1, feature: extrude(2, vec![2], "7") }, set("t", Some("5 mm"))],
        vec![Command::MoveFeature { id: 2, index: 0 }],
        vec![Command::SetFeature(Feature { suppressed: true, ..extrude(1, vec![1], "9") })],
        vec![set("t", Some("6 mm"))],
        vec![Command::RemoveFeature(1)],
        vec![set("t", None)],
    ];
    for tx in txs {
        h.execute(&mut d, tx).unwrap();
        states.push(snapshot(&d));
    }
    for i in (0..states.len() - 1).rev() {
        assert!(h.undo(&mut d));
        assert_eq!(snapshot(&d), states[i]);
    }
    for s in &states[1..] {
        assert!(h.redo(&mut d));
        assert_eq!(&snapshot(&d), s);
    }
}

#[test]
fn errors_change_nothing() {
    let mut d = doc();
    d.apply(Command::AddFeature { index: 0, feature: extrude(1, vec![1], "5") }).unwrap();
    let (before, rev) = (snapshot(&d), d.revision());
    let revolve = |axis, angle: &str| Feature {
        kind: FeatureKind::Revolve { profile: vec![1], axis, angle: angle.into(), op: BodyOp::Add, target: Target::Auto },
        ..extrude(5, vec![], "")
    };
    let add = |index, feature| Command::AddFeature { index, feature };
    let set = |name: &str, expr: Option<&str>| Command::SetParam { name: name.into(), expr: expr.map(Into::into) };
    let bad_name = Feature { name: " ".into(), ..extrude(3, vec![1], "5") };
    type Check = fn(&DocError) -> bool;
    let cases: Vec<(Command, Check)> = vec![
        (add(0, extrude(1, vec![2], "5")), |e| matches!(e, DocError::FeatureExists(1))),
        (add(2, extrude(3, vec![1], "5")), |e| matches!(e, DocError::BadFeature("index out of range"))),
        (add(0, bad_name), |e| matches!(e, DocError::BadFeature("empty name"))),
        (add(0, extrude(3, vec![], "5")), |e| matches!(e, DocError::BadFeature("empty profile"))),
        (add(0, extrude(3, vec![9], "5")), |e| matches!(e, DocError::NotFound(9))),
        (add(0, extrude(3, vec![1], "5 +")), |e| matches!(e, DocError::Param(ParamError::Syntax { .. }))),
        (add(0, revolve(RevolveAxis::Line(9), "90 deg")), |e| matches!(e, DocError::NotFound(9))),
        (add(0, revolve(RevolveAxis::Y, "90 deg +")), |e| matches!(e, DocError::Param(_))),
        (Command::SetFeature(extrude(9, vec![1], "5")), |e| matches!(e, DocError::FeatureNotFound(9))),
        (Command::SetFeature(extrude(1, vec![], "5")), |e| matches!(e, DocError::BadFeature("empty profile"))),
        (Command::SetFeature(Feature { name: String::new(), ..extrude(1, vec![1], "5") }), |e| {
            matches!(e, DocError::BadFeature("empty name"))
        }),
        (Command::SetFeature(extrude(1, vec![8], "5")), |e| matches!(e, DocError::NotFound(8))),
        (Command::RemoveFeature(9), |e| matches!(e, DocError::FeatureNotFound(9))),
        (Command::MoveFeature { id: 9, index: 0 }, |e| matches!(e, DocError::FeatureNotFound(9))),
        (Command::MoveFeature { id: 1, index: 1 }, |e| matches!(e, DocError::BadFeature("index out of range"))),
        (set("1x", Some("5")), |e| matches!(e, DocError::Param(ParamError::InvalidName(_)))),
        (set("a", Some("5 +")), |e| matches!(e, DocError::Param(ParamError::Syntax { .. }))),
        (set("nope", None), |e| matches!(e, DocError::Param(ParamError::UnknownName(_)))),
    ];
    for (i, (cmd, expected)) in cases.into_iter().enumerate() {
        let e = d.apply(cmd).unwrap_err();
        assert!(expected(&e), "case {i}: {e}");
        assert_eq!((snapshot(&d), d.revision()), (before.clone(), rev), "case {i}");
    }
}

#[test]
fn profile_entity_cannot_be_removed() {
    let mut d = doc();
    d.apply(Command::AddFeature { index: 0, feature: extrude(1, vec![1], "5") }).unwrap();
    assert!(matches!(d.apply(Command::Remove(1)), Err(DocError::EntityInFeature(1))));
    assert!(d.get(1).is_some());
    assert_eq!(d.features_using_entity(1), [1]);
    assert!(d.features_using_entity(2).is_empty());
    d.apply(Command::Remove(2)).unwrap();
    d.apply(Command::RemoveFeature(1)).unwrap();
    d.apply(Command::Remove(1)).unwrap();
}

#[test]
fn line_axis_counts_as_a_used_entity() {
    let mut d = doc();
    let kind = FeatureKind::Revolve { profile: vec![1], axis: RevolveAxis::Line(2), angle: "360 deg".into(), op: BodyOp::Add, target: Target::Auto };
    d.apply(Command::AddFeature { index: 0, feature: Feature { id: 1, name: "rev".into(), suppressed: false, kind } }).unwrap();
    assert_eq!(d.features_using_entity(2), [1]);
    assert!(matches!(d.apply(Command::Remove(2)), Err(DocError::EntityInFeature(2))));
}

#[test]
fn ids_are_allocated_after_the_largest() {
    let mut d = doc();
    assert_eq!(d.alloc_feature_id(), 1);
    d.apply(Command::AddFeature { index: 0, feature: extrude(7, vec![1], "5") }).unwrap();
    assert_eq!(d.alloc_feature_id(), 8);
}

#[test]
fn features_using_param_follows_parameter_dependencies() {
    let mut d = doc();
    let set = |name: &str, expr: &str| Command::SetParam { name: name.into(), expr: Some(expr.into()) };
    for c in [set("t", "5 mm"), set("d", "t * 2"), set("other", "1 mm")] {
        d.apply(c).unwrap();
    }
    d.apply(Command::AddFeature { index: 0, feature: extrude(1, vec![1], "t") }).unwrap();
    assert_eq!(d.features_using_param("t"), [1]);
    d.apply(Command::AddFeature { index: 1, feature: extrude(2, vec![2], "d") }).unwrap();
    assert_eq!(d.features_using_param("t"), [1, 2]);
    assert_eq!(d.features_using_param("d"), [2]);
    assert!(d.features_using_param("other").is_empty());
}

/// The 10 mm cube as a mesh feature named "Cube" (id 1); `n` triangles of its 12.
fn cube_feature(id: FeatureId, n: usize) -> Feature {
    let positions: Vec<[f32; 3]> = (0..8).map(|i| [(i & 1) as f32 * 10.0, ((i >> 1) & 1) as f32 * 10.0, ((i >> 2) & 1) as f32 * 10.0]).collect();
    let quads: [[u32; 4]; 6] = [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]];
    let triangles: Vec<[u32; 3]> = quads.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).take(n).collect();
    Feature { id, name: "Cube".into(), suppressed: false, kind: FeatureKind::Mesh { positions, triangles } }
}

#[test]
fn mesh_feature_makes_a_body() {
    let mut d = Document::new("m");
    d.apply(Command::AddFeature { index: 0, feature: cube_feature(1, 12) }).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.bodies.len(), 1);
    assert_eq!(r.bodies[0].0, 1);
    assert!((r.bodies[0].1.volume() - 1000.0).abs() < 1e-6);
    assert_eq!(r.states[&1], FeatureState::Ok);
    let kind = FeatureKind::LinearPattern { source: 1, count: "2".into(), dx: "20 mm".into(), dy: "0".into(), dz: "0".into() };
    d.apply(Command::AddFeature { index: 1, feature: Feature { id: 2, name: "Pat".into(), suppressed: false, kind } }).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    let total: f64 = r.bodies.iter().map(|(_, b)| b.volume()).sum();
    assert!((total - 2000.0).abs() < 1e-6, "{total}");
}

#[test]
fn mesh_feature_survives_save_and_load() {
    let mut d = Document::new("m");
    let f = cube_feature(1, 12);
    d.apply(Command::AddFeature { index: 0, feature: f.clone() }).unwrap();
    let p = std::env::temp_dir().join("cad-mesh-feature.cadproj");
    d.save(&p).unwrap();
    let text = std::fs::read_to_string(&p).unwrap();
    let loaded = Document::load(&p).unwrap();
    std::fs::remove_file(&p).ok();
    assert_eq!(loaded.features()[0].kind, f.kind);
    assert!(text.contains("\"version\": 29"), "{}", &text[..text.len().min(80)]);
}

#[test]
fn bad_mesh_feature_is_refused() {
    let mut d = Document::new("m");
    assert!(matches!(d.apply(Command::AddFeature { index: 0, feature: cube_feature(1, 3) }), Err(DocError::BadFeature(_))));
    d.apply(Command::AddFeature { index: 0, feature: cube_feature(1, 4) }).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    assert!(matches!(&r.states[&1], FeatureState::Error(msg) if msg.contains("not closed")), "{:?}", r.states);
}
