use crate::rebuild_pattern::tests::{extrude, push, put, rect};
use crate::*;
use cad_geometry::DVec3;

fn face(n: DVec3, at: [f64; 3]) -> FaceRef {
    FaceRef { key: FaceKey::Plane(n), at: DVec3::from(at) }
}

fn draft(id: FeatureId, body: FeatureId, faces: Vec<FaceRef>, sketch: SketchId, angle: &str) -> Feature {
    Feature { id, name: format!("draft{id}"), suppressed: false, kind: FeatureKind::Draft { body, faces, sketch, angle: angle.into() } }
}

/// Block (0, 0, 0)-(40, 20, h) with h = 10 mm: entity 1 on sketch 0, feature 1.
fn block() -> Document {
    let mut d = Document::new("draft");
    d.apply(Command::SetParam { name: "h".into(), expr: Some("10 mm".into()) }).unwrap();
    put(&mut d, 1, 0, rect(0.0, 0.0, 40.0, 20.0));
    push(&mut d, extrude(1, 1, "h", BodyOp::New));
    d
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs(), "{a} vs {b}");
}

fn set(d: &mut Document, f: Feature) {
    d.apply(Command::SetFeature(f)).unwrap();
}

fn add(d: &mut Document, feature: Feature) -> Result<Command, DocError> {
    let index = d.features().len();
    d.apply(Command::AddFeature { index, feature })
}

fn px() -> FaceRef {
    face(DVec3::X, [40.0, 10.0, 5.0])
}

#[test]
fn draft_follows_its_face() {
    let mut d = block();
    push(&mut d, draft(2, 1, vec![px()], 0, "5 deg"));
    let mut rb = Rebuilder::default();
    let r = rb.rebuild(&d);
    assert_eq!((&r.states[&1], &r.states[&2]), (&FeatureState::Ok, &FeatureState::Ok));
    let b = r.body(1).unwrap();
    close(b.volume(), 7912.511336474076, 1e-9);
    let x = b.mesh.positions.iter().filter(|p| (p.z - 10.0).abs() <= 1e-9).map(|p| p.x).fold(f64::MIN, f64::max);
    assert!((x - 39.12511336474076).abs() <= 1e-9, "{x}");
    assert_eq!(r.depends[&2], vec![1]);

    d.apply(Command::SetShape { id: 1, shape: rect(0.0, 0.0, 50.0, 20.0) }).unwrap();
    let r = rb.rebuild(&d);
    assert_eq!(r.recomputed, 2);
    close(r.body(1).unwrap().volume(), 9912.511336474076, 1e-9);

    d.apply(Command::SetParam { name: "h".into(), expr: Some("20 mm".into()) }).unwrap();
    let r = rb.rebuild(&d);
    close(r.body(1).unwrap().volume(), 19650.045345896306, 1e-9);
}

#[test]
fn four_sides_and_a_neutral_sketch() {
    let mut d = block();
    let sides = vec![px(), face(-DVec3::X, [0.0, 10.0, 5.0]), face(DVec3::Y, [20.0, 20.0, 5.0]), face(-DVec3::Y, [20.0, 0.0, 5.0])];
    push(&mut d, draft(2, 1, sides, 0, "5 deg"));
    let r = Rebuilder::default().rebuild(&d);
    close(r.body(1).unwrap().volume(), 7485.273707171859, 1e-9);

    let plane = PlaneDef::Principal { base: BasePlane::XY, offset: "5 mm".into() };
    d.apply(Command::AddSketch(Sketch { id: 1, name: "mid".into(), plane, level: None })).unwrap();
    set(&mut d, draft(2, 1, vec![px()], 1, "5 deg"));
    let r = Rebuilder::default().rebuild(&d);
    let b = r.body(1).unwrap();
    close(b.volume(), 8000.0, 1e-9);
    close(b.bbox().1.x, 40.43744331762962, 1e-9);

    set(&mut d, draft(2, 1, vec![px()], 0, "-5 deg"));
    close(Rebuilder::default().rebuild(&d).body(1).unwrap().volume(), 8087.488663525924, 1e-9);
}

#[test]
fn bad_drafts() {
    let mut d = block();
    push(&mut d, draft(2, 1, vec![px()], 0, "5 deg"));
    let cases = [
        (vec![face(DVec3::Z, [20.0, 10.0, 10.0])], 1, "5 deg", "the face must be parallel to the pull direction"),
        (vec![face(DVec3::X, [100.0, 100.0, 5.0])], 1, "5 deg", "face not found; pick it again"),
        (vec![px()], 7, "5 deg", "body not found"),
        (vec![px()], 1, "80 deg", "draft angle too large for the face"),
        (vec![px()], 1, "5 mm", "expected an angle"),
    ];
    for (faces, body, angle, msg) in cases {
        set(&mut d, draft(2, body, faces, 0, angle));
        let r = Rebuilder::default().rebuild(&d);
        assert_eq!(r.states[&2], FeatureState::Error(msg.into()));
        close(r.body(1).unwrap().volume(), 8000.0, 1e-9);
    }
    assert!(matches!(add(&mut d, draft(3, 1, vec![], 0, "5 deg")), Err(DocError::BadFeature("no faces"))));
    assert!(matches!(add(&mut d, draft(3, 1, vec![px()], 9, "5 deg")), Err(DocError::SketchNotFound(9))));
}

#[test]
fn draft_saves_and_loads() {
    let mut d = block();
    push(&mut d, draft(2, 1, vec![px()], 0, "5 deg"));
    let p = std::env::temp_dir().join(format!("cadtest_{}_draft.cadproj", std::process::id()));
    d.save(&p).unwrap();
    let loaded = Document::load(&p).unwrap();
    std::fs::remove_file(&p).ok();
    assert_eq!(loaded.features(), d.features());
    assert_eq!(FORMAT_VERSION, 24);
}
