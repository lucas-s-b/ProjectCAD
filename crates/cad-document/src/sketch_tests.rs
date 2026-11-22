use crate::*;
use cad_geometry::{DVec2, DVec3, Opening, OpeningKind, Room, Shape, Slab, Wall, WallAlign};
use cad_modeler::Plane3;

fn sketch(id: SketchId, name: &str, plane: PlaneDef) -> Sketch {
    Sketch { id, name: name.into(), plane, level: None }
}

fn principal(base: BasePlane, offset: &str) -> PlaneDef {
    PlaneDef::Principal { base, offset: offset.into() }
}

fn line_in(id: EntityId, sketch: SketchId) -> Entity {
    Entity { id, name: format!("l{id}"), shape: Shape::Line { a: DVec2::ZERO, b: DVec2::new(10.0, 0.0) }, layer: 0, sketch, construction: false, group: None, meta: Default::default() }
}

fn doc_with_sketch_1() -> Document {
    let mut d = Document::new("s");
    d.apply(Command::AddSketch(sketch(1, "Top", principal(BasePlane::XY, "5 mm")))).unwrap();
    d
}

fn temp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("cadtest_{}_sk_{name}.cadproj", std::process::id()))
}

fn load_text(name: &str, text: &str) -> Result<Document, DocError> {
    let p = temp(name);
    std::fs::write(&p, text).unwrap();
    let r = Document::load(&p);
    std::fs::remove_file(&p).unwrap();
    r
}

#[test]
fn new_document_has_sketch_0_on_xy() {
    let d = Document::new("n");
    assert_eq!(d.sketches().cloned().collect::<Vec<_>>(), [sketch(0, "Sketch 0", principal(BasePlane::XY, "0"))]);
    assert_eq!(d.entities_in(0).count(), 0);
}

#[test]
fn sketch_commands_invert_and_undo() {
    let mut d = Document::new("s");
    let mut h = History::default();
    let s1 = sketch(1, "A", principal(BasePlane::XZ, "2 mm"));
    assert_eq!(d.apply(Command::AddSketch(s1.clone())).unwrap(), Command::RemoveSketch(1));
    assert_eq!(d.alloc_sketch_id(), 2);
    let s1b = sketch(1, "B", PlaneDef::Fixed { origin: DVec3::new(1.0, 2.0, 3.0), x: DVec3::Y, y: DVec3::Z });
    assert_eq!(d.apply(Command::SetSketch(s1b.clone())).unwrap(), Command::SetSketch(s1.clone()));
    let s0 = d.sketch(0).unwrap().clone();
    let s0b = sketch(0, "Base", principal(BasePlane::YZ, "1"));
    assert_eq!(d.apply(Command::SetSketch(s0b)).unwrap(), Command::SetSketch(s0));
    assert_eq!(d.apply(Command::RemoveSketch(1)).unwrap(), Command::AddSketch(s1b.clone()));
    assert!(d.sketch(1).is_none());

    let before: Vec<_> = d.sketches().cloned().collect();
    h.execute(&mut d, vec![Command::AddSketch(s1b), Command::SetSketch(s1)]).unwrap();
    let after: Vec<_> = d.sketches().cloned().collect();
    assert_ne!(before, after);
    assert!(h.undo(&mut d));
    assert_eq!(d.sketches().cloned().collect::<Vec<_>>(), before);
    assert!(h.redo(&mut d));
    assert_eq!(d.sketches().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn sketch_errors_change_nothing() {
    let mut d = doc_with_sketch_1();
    d.apply(Command::Add(line_in(1, 1))).unwrap();
    let rev = d.revision();
    let bad_fixed = |x: DVec3, y: DVec3| PlaneDef::Fixed { origin: DVec3::ZERO, x, y };
    type Check = fn(&DocError) -> bool;
    let cases: Vec<(Command, Check)> = vec![
        (Command::AddSketch(sketch(1, "dup", principal(BasePlane::XY, "0"))), |e| matches!(e, DocError::SketchExists(1))),
        (Command::AddSketch(sketch(2, " ", principal(BasePlane::XY, "0"))), |e| matches!(e, DocError::BadFeature("empty name"))),
        (Command::AddSketch(sketch(2, "x", principal(BasePlane::XY, "5 +"))), |e| matches!(e, DocError::Param(_))),
        (Command::AddSketch(sketch(2, "x", bad_fixed(DVec3::X * 2.0, DVec3::Y))), |e| matches!(e, DocError::BadPlane)),
        (Command::AddSketch(sketch(2, "x", bad_fixed(DVec3::X, DVec3::X))), |e| matches!(e, DocError::BadPlane)),
        (Command::AddSketch(sketch(2, "x", bad_fixed(DVec3::X, DVec3::new(f64::NAN, 0.0, 1.0)))), |e| {
            matches!(e, DocError::BadPlane)
        }),
        (Command::SetSketch(sketch(7, "x", principal(BasePlane::XY, "0"))), |e| matches!(e, DocError::SketchNotFound(7))),
        (Command::SetSketch(sketch(1, "", principal(BasePlane::XY, "0"))), |e| matches!(e, DocError::BadFeature("empty name"))),
        (Command::SetSketch(sketch(1, "x", bad_fixed(DVec3::Z, DVec3::ZERO))), |e| matches!(e, DocError::BadPlane)),
        (Command::RemoveSketch(0), |e| matches!(e, DocError::ProtectedSketch)),
        (Command::RemoveSketch(7), |e| matches!(e, DocError::SketchNotFound(7))),
        (Command::RemoveSketch(1), |e| matches!(e, DocError::SketchInUse(1))),
        (Command::Add(line_in(2, 9)), |e| matches!(e, DocError::SketchNotFound(9))),
    ];
    for (i, (cmd, expected)) in cases.into_iter().enumerate() {
        let e = d.apply(cmd).unwrap_err();
        assert!(expected(&e), "case {i}: {e}");
        assert_eq!((d.revision(), d.sketches().count(), d.len()), (rev, 2, 1), "case {i}");
    }
}

#[test]
fn solids_only_in_sketch_0() {
    let mut d = doc_with_sketch_1();
    let solid = |sketch| Entity {
        id: 1,
        name: "b".into(),
        shape: Shape::Box { center: DVec3::ZERO, size: DVec3::ONE },
        layer: 0,
        sketch,
        construction: false,
        group: None,
        meta: Default::default(),
    };
    assert!(matches!(d.apply(Command::Add(solid(1))), Err(DocError::SolidOutsideBaseSketch)));
    assert!(d.is_empty());
    d.apply(Command::Add(solid(0))).unwrap();
}

#[test]
fn constraints_and_features_cannot_span_sketches() {
    let mut d = doc_with_sketch_1();
    d.apply(Command::Add(line_in(1, 0))).unwrap();
    d.apply(Command::Add(line_in(2, 1))).unwrap();
    d.apply(Command::Add(line_in(3, 0))).unwrap();
    assert!(matches!(d.apply(Command::AddConstraint(1, Constraint::Parallel(1, 2))), Err(DocError::MixedSketches)));
    d.apply(Command::AddConstraint(1, Constraint::Parallel(1, 3))).unwrap();
    assert!(matches!(d.apply(Command::SetConstraint(1, Constraint::Parallel(2, 3))), Err(DocError::MixedSketches)));

    let extrude = |id, profile: Vec<EntityId>| Feature {
        id,
        name: "f".into(),
        suppressed: false,
        kind: FeatureKind::Extrude { profile, distance: "5".into(), offset: "0".into(), taper: "0 deg".into(), op: BodyOp::Add, flip: false, target: Target::Auto },
    };
    let add = |feature| Command::AddFeature { index: 0, feature };
    assert!(matches!(d.apply(add(extrude(1, vec![1, 2]))), Err(DocError::MixedSketches)));
    let revolve = Feature {
        kind: FeatureKind::Revolve { profile: vec![1], axis: RevolveAxis::Line(2), angle: "90 deg".into(), op: BodyOp::Add, target: Target::Auto },
        ..extrude(1, vec![])
    };
    assert!(matches!(d.apply(add(revolve)), Err(DocError::MixedSketches)));
    d.apply(add(extrude(1, vec![2]))).unwrap();
    assert!(matches!(d.apply(Command::SetFeature(extrude(1, vec![2, 3]))), Err(DocError::MixedSketches)));
    assert_eq!(d.features().len(), 1);
}

fn plane(base: BasePlane, offset: &str, doc: &mut Document) -> Plane3 {
    doc.apply(Command::SetSketch(sketch(0, "p", principal(base, offset)))).unwrap();
    doc.resolve_plane(0).unwrap()
}

#[test]
fn principal_planes_resolve() {
    let mut d = Document::new("p");
    d.apply(Command::SetParam { name: "h".into(), expr: Some("12 mm".into()) }).unwrap();
    let axes = [
        (BasePlane::XY, DVec3::X, DVec3::Y, DVec3::Z),
        (BasePlane::XZ, DVec3::X, DVec3::Z, -DVec3::Y),
        (BasePlane::YZ, DVec3::Y, DVec3::Z, DVec3::X),
    ];
    for (base, x, y, n) in axes {
        let p = plane(base, "0", &mut d);
        assert_eq!((p.origin, p.x, p.y, p.normal()), (DVec3::ZERO, x, y, n));
        let p = plane(base, "h", &mut d);
        assert_eq!((p.origin, p.x, p.y, p.normal()), (n * 12.0, x, y, n));
    }
    assert_eq!(plane(BasePlane::XY, "h", &mut d).origin, DVec3::new(0.0, 0.0, 12.0));
    assert_eq!(plane(BasePlane::XZ, "h", &mut d).origin, DVec3::new(0.0, -12.0, 0.0));
    assert_eq!(plane(BasePlane::YZ, "h", &mut d).origin, DVec3::new(12.0, 0.0, 0.0));
}

#[test]
fn resolve_plane_reports_errors_and_fixed_is_stored() {
    let mut d = Document::new("p");
    d.apply(Command::SetSketch(sketch(0, "p", principal(BasePlane::XY, "30 deg")))).unwrap();
    assert_eq!(d.resolve_plane(0).unwrap_err(), "expected a length");
    d.apply(Command::SetSketch(sketch(0, "p", principal(BasePlane::XY, "nope")))).unwrap();
    assert_eq!(d.resolve_plane(0).unwrap_err(), "unknown name 'nope'");
    assert_eq!(d.resolve_plane(4).unwrap_err(), "sketch not found");
    let fixed = PlaneDef::Fixed { origin: DVec3::new(40.0, 0.0, 0.0), x: DVec3::Y, y: DVec3::Z };
    d.apply(Command::SetSketch(sketch(0, "p", fixed))).unwrap();
    let p = d.resolve_plane(0).unwrap();
    assert_eq!((p.origin, p.x, p.y), (DVec3::new(40.0, 0.0, 0.0), DVec3::Y, DVec3::Z));
}

#[test]
fn version_6_file_loads_with_sketch_0() {
    let text = r#"{"version":6,"name":"x","next_id":3,"entities":{
        "1":{"id":1,"name":"a","shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}},
        "2":{"id":2,"name":"b","shape":{"Circle":{"center":[1.0,0.0],"radius":1.0}}}}}"#;
    let mut d = load_text("v6", text).unwrap();
    assert_eq!(d.sketches().map(|s| s.id).collect::<Vec<_>>(), [0]);
    assert!(d.entities().all(|e| e.sketch == 0));
    assert_eq!(d.entities_in(0).count(), 2);
    assert_eq!(d.alloc_sketch_id(), 1);
}

#[test]
fn sketches_roundtrip_and_load_is_checked() {
    let mut d = doc_with_sketch_1();
    let fixed = PlaneDef::Fixed { origin: DVec3::new(1.0, 2.0, 3.0), x: DVec3::X, y: DVec3::Z };
    d.apply(Command::AddSketch(sketch(4, "Face", fixed))).unwrap();
    d.apply(Command::Add(line_in(1, 4))).unwrap();
    let p = temp("roundtrip");
    d.save(&p).unwrap();
    let mut l = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    assert_eq!(l.sketches().cloned().collect::<Vec<_>>(), d.sketches().cloned().collect::<Vec<_>>());
    assert_eq!(l.get(1).unwrap().sketch, 4);
    assert_eq!(l.alloc_sketch_id(), 5);

    let entity = r#""1":{"id":1,"name":"a","sketch":3,"shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}}"#;
    let file = |extra: &str| format!(r#"{{"version":7,"name":"x","next_id":2,"entities":{{{entity}}}{extra}}}"#);
    assert!(matches!(load_text("nosk", &file("")), Err(DocError::SketchNotFound(3))));
    let sk = |id: u32, key: u32| format!(r#""{key}":{{"id":{id},"name":"s","plane":{{"Principal":{{"base":"XY","offset":"0"}}}}}}"#);
    let no0 = format!(r#","sketches":{{{}}}"#, sk(3, 3));
    assert!(matches!(load_text("no0", &file(&no0)), Err(DocError::SketchNotFound(0))));
    let key = format!(r#","sketches":{{{},{}}}"#, sk(0, 0), sk(1, 3));
    assert!(matches!(load_text("key", &file(&key)), Err(DocError::SketchExists(3))));
    let ok = format!(r#","sketches":{{{},{}}}"#, sk(0, 0), sk(3, 3));
    let mut d = load_text("ok", &file(&ok)).unwrap();
    assert_eq!(d.alloc_sketch_id(), 4);
}

#[test]
fn face_plane_must_hold_its_point() {
    let face = |at: DVec3| PlaneDef::Face { origin: DVec3::new(0.0, 0.0, 10.0), x: DVec3::X, y: DVec3::Y, at, body: 1 };
    let mut d = Document::new("f");
    let bad = d.apply(Command::AddSketch(sketch(1, "off", face(DVec3::new(1.0, 2.0, 11.0)))));
    assert!(matches!(bad, Err(DocError::BadPlane)));
    d.apply(Command::AddSketch(sketch(1, "on", face(DVec3::new(1.0, 2.0, 10.0))))).unwrap();
}

fn wall_in(id: EntityId, sketch: SketchId) -> Entity {
    let w = Wall { a: DVec2::ZERO, b: DVec2::new(4000.0, 0.0), thickness: 200.0, height: 2500.0, align: WallAlign::Center };
    Entity { id, name: format!("w{id}"), shape: Shape::Wall(w), layer: 0, sketch, construction: false, group: None, meta: Default::default() }
}

fn level(id: SketchId, offset: &str) -> Sketch {
    Sketch { id, name: format!("L{id}"), plane: principal(BasePlane::XY, offset), level: Some(Level { height: "3000 mm".into() }) }
}

#[test]
fn walls_only_on_levels() {
    let mut d = doc_with_sketch_1();
    assert!(matches!(d.apply(Command::Add(wall_in(1, 1))), Err(DocError::NotALevel(1))));
    assert!(matches!(d.apply(Command::Add(wall_in(1, 0))), Err(DocError::NotALevel(0))));
    d.apply(Command::AddSketch(level(2, "0"))).unwrap();
    d.apply(Command::Add(wall_in(1, 2))).unwrap();
    assert_eq!(d.level_height(2), Ok(3000.0));
    assert!(d.level_height(1).is_err());
    d.apply(Command::Add(line_in(5, 1))).unwrap();
    let shape = d.get(1).unwrap().shape.clone();
    assert!(matches!(d.apply(Command::SetShape { id: 5, shape }), Err(DocError::NotALevel(1))));
    assert!(matches!(d.apply(Command::SetSketch(Sketch { level: None, ..level(2, "0") })), Err(DocError::NotALevel(2))));
    let xz = Sketch { plane: principal(BasePlane::XZ, "0"), ..level(3, "0") };
    assert!(matches!(d.apply(Command::AddSketch(xz)), Err(DocError::BadPlane)));
}

#[test]
fn rooms_and_floors_only_on_levels() {
    let room_in = |sketch| Entity {
        id: 1,
        name: "r".into(),
        shape: Shape::Room(Room { seed: DVec2::new(1000.0, 1000.0), name: "Hall".into(), number: "1".into() }),
        layer: 0,
        sketch,
        construction: false,
        group: None,
        meta: Default::default(),
    };
    let slab_in = |sketch| Entity {
        id: 2,
        name: "s".into(),
        shape: Shape::Slab(Slab { outline: vec![DVec2::ZERO, DVec2::new(4000.0, 0.0), DVec2::new(4000.0, 3000.0)], thickness: 200.0 }),
        layer: 0,
        sketch,
        construction: false,
        group: None,
        meta: Default::default(),
    };
    let mut d = doc_with_sketch_1();
    assert!(matches!(d.apply(Command::Add(room_in(1))), Err(DocError::NotALevel(1))));
    assert!(matches!(d.apply(Command::Add(slab_in(1))), Err(DocError::NotALevel(1))));
    d.apply(Command::AddSketch(level(2, "0"))).unwrap();
    d.apply(Command::Add(room_in(2))).unwrap();
    d.apply(Command::Add(slab_in(2))).unwrap();
    assert!(matches!(d.apply(Command::SetSketch(Sketch { level: None, ..level(2, "0") })), Err(DocError::NotALevel(2))));
    let p = temp("rooms");
    d.save(&p).unwrap();
    let back = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    assert_eq!(back.get(1), d.get(1));
    assert_eq!(back.get(2), d.get(2));
}

#[test]
fn version_14_file_loads_without_levels() {
    let old = r#"{"version":14,"name":"x","next_id":1,"entities":{},"sketches":{"0":{"id":0,"name":"Sketch 0","plane":{"Principal":{"base":"XY","offset":"0"}}}}}"#;
    let d = load_text("v14", old).unwrap();
    assert_eq!(d.sketch(0).unwrap().level, None);
    let bad = r#"{"version":15,"name":"x","next_id":2,"entities":{"1":{"id":1,"name":"w","shape":{"Wall":{"a":[0.0,0.0],"b":[1000.0,0.0],"thickness":200.0,"height":2500.0,"align":"Center"}}}}}"#;
    assert!(matches!(load_text("v15bad", bad), Err(DocError::NotALevel(0))));
}

#[test]
fn levels_and_walls_roundtrip() {
    let mut d = Document::new("lv");
    d.apply(Command::AddSketch(level(1, "3000 mm"))).unwrap();
    d.apply(Command::Add(wall_in(1, 1))).unwrap();
    let p = temp("levels");
    d.save(&p).unwrap();
    let back = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    assert_eq!(back.sketches().cloned().collect::<Vec<_>>(), d.sketches().cloned().collect::<Vec<_>>());
    assert_eq!(back.get(1), d.get(1));
}

#[test]
fn opening_without_its_wall_does_not_load() {
    let text = r#"{"version":16,"name":"x","next_id":3,"entities":{"2":{"id":2,"name":"d","sketch":1,"shape":{"Opening":{"host":1,"wall":{"a":[0.0,0.0],"b":[4000.0,0.0],"thickness":200.0,"height":2500.0,"align":"Center"},"at":1000.0,"width":900.0,"height":2100.0,"sill":0.0,"kind":"Door","hinge_b":false,"swing_left":true}}}},"sketches":{"0":{"id":0,"name":"Sketch 0","plane":{"Principal":{"base":"XY","offset":"0"}}},"1":{"id":1,"name":"L1","plane":{"Principal":{"base":"XY","offset":"0"}},"level":{"height":"3000 mm"}}}}"#;
    assert!(matches!(load_text("noh", text), Err(DocError::NotAHost(1))));
}

#[test]
fn openings_roundtrip() {
    let mut d = Document::new("op");
    d.apply(Command::AddSketch(level(1, "0"))).unwrap();
    d.apply(Command::Add(wall_in(1, 1))).unwrap();
    let Shape::Wall(wall) = d.get(1).unwrap().shape.clone() else { panic!("not a wall") };
    let o = Opening { host: 1, wall, at: 1000.0, width: 1200.0, height: 1200.0, sill: 900.0, kind: OpeningKind::Window, hinge_b: false, swing_left: true };
    let e = Entity { id: 2, name: "win".into(), shape: Shape::Opening(o), layer: 0, sketch: 1, construction: false, group: None, meta: Default::default() };
    d.apply(Command::Add(e)).unwrap();
    let p = temp("openings");
    d.save(&p).unwrap();
    let back = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    assert_eq!(back.get(2), d.get(2));
}
