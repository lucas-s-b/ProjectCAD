use crate::rebuild_pattern::tests::{circle, extrude, linear, push, put, rect};
use crate::*;
use cad_geometry::{DVec2, Shape};
use std::f64::consts::PI;

fn line(a: (f64, f64), b: (f64, f64)) -> Shape {
    Shape::Line { a: DVec2::new(a.0, a.1), b: DVec2::new(b.0, b.1) }
}

fn sketch(d: &mut Document, id: SketchId, base: BasePlane, offset: &str) {
    let plane = PlaneDef::Principal { base, offset: offset.into() };
    d.apply(Command::AddSketch(Sketch { id, name: format!("s{id}"), plane, level: None })).unwrap();
}

fn sweep(id: FeatureId, profile: Vec<EntityId>, path: Vec<EntityId>, op: BodyOp) -> Feature {
    Feature { id, name: format!("sweep{id}"), suppressed: false, kind: FeatureKind::Sweep { profile, path, op, target: Target::Auto } }
}

fn loft(id: FeatureId, sections: Vec<Vec<EntityId>>, op: BodyOp) -> Feature {
    Feature { id, name: format!("loft{id}"), suppressed: false, kind: FeatureKind::Loft { sections, op, target: Target::Auto, smooth: false } }
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs().max(1.0), "{a} vs {b}");
}

/// Sketch 0 (XY) holds an L path (entities 1, 2); sketch 1 (YZ) holds a circle of radius 2 at the path start (entity 3).
fn pipe_doc() -> Document {
    let mut d = Document::new("pipe");
    sketch(&mut d, 1, BasePlane::YZ, "0");
    put(&mut d, 1, 0, line((0.0, 0.0), (30.0, 0.0)));
    put(&mut d, 2, 0, line((30.0, 0.0), (30.0, 20.0)));
    put(&mut d, 3, 1, circle(0.0, 0.0, 2.0));
    d
}

/// Area of the polygon the circle of radius `r` is tessellated as, found through an extrusion of height 1.
fn faceted_area(r: f64) -> f64 {
    let profile = cad_geometry::profile::Profile::from_shapes(&[&circle(0.0, 0.0, r)]).unwrap();
    Body::from_brep(&cad_modeler::extrude(&profile, &cad_modeler::Plane3::XY, 1.0).unwrap(), BODY_TOLERANCE).unwrap().volume()
}

fn rebuilt(d: &Document) -> RebuildResult {
    Rebuilder::default().rebuild(d)
}

#[test]
fn sweep_makes_a_pipe() {
    let mut d = pipe_doc();
    push(&mut d, sweep(1, vec![3], vec![1, 2], BodyOp::Add));
    let r = rebuilt(&d);
    assert_eq!(r.states[&1], FeatureState::Ok);
    assert_eq!(r.bodies.len(), 1);
    let body = r.body(1).unwrap();
    assert!(body.mesh.is_closed());
    // The path is straight lines, so the section is exact: area times the length of the path.
    close(body.volume(), faceted_area(2.0) * 50.0, 1e-9);
    assert!((body.volume() / (PI * 4.0 * 50.0) - 1.0).abs() < 0.02);
}

#[test]
fn sweep_path_may_be_given_from_its_far_end() {
    let mut d = Document::new("far");
    sketch(&mut d, 1, BasePlane::YZ, "0");
    put(&mut d, 1, 0, line((30.0, 20.0), (30.0, 0.0)));
    put(&mut d, 2, 0, line((30.0, 0.0), (0.0, 0.0)));
    put(&mut d, 3, 1, circle(0.0, 0.0, 2.0));
    push(&mut d, sweep(1, vec![3], vec![1, 2], BodyOp::Add));
    let r = rebuilt(&d);
    assert_eq!(r.states[&1], FeatureState::Ok);
    close(r.total_volume(), faceted_area(2.0) * 50.0, 1e-9);
}

#[test]
fn cut_sweep_removes_material() {
    let mut d = pipe_doc();
    put(&mut d, 4, 0, rect(-5.0, -5.0, 40.0, 30.0));
    push(&mut d, extrude(1, 4, "10 mm", BodyOp::Add));
    let block = rebuilt(&d).total_volume();
    push(&mut d, sweep(2, vec![3], vec![1, 2], BodyOp::Cut));
    let r = rebuilt(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    // The pipe is centred on the block's bottom plane: half of it is inside.
    close(block - r.total_volume(), faceted_area(2.0) * 50.0 / 2.0, 1e-6);
}

fn frustum_doc() -> Document {
    let mut d = Document::new("loft");
    d.apply(Command::SetParam { name: "h".into(), expr: Some("15 mm".into()) }).unwrap();
    sketch(&mut d, 1, BasePlane::XY, "h");
    put(&mut d, 1, 0, rect(-10.0, -10.0, 10.0, 10.0));
    put(&mut d, 2, 1, rect(-5.0, -5.0, 5.0, 5.0));
    push(&mut d, loft(1, vec![vec![1], vec![2]], BodyOp::Add));
    d
}

#[test]
fn loft_makes_a_frustum_that_follows_its_parameter() {
    let mut d = frustum_doc();
    let r = rebuilt(&d);
    assert_eq!(r.states[&1], FeatureState::Ok);
    close(r.total_volume(), 15.0 / 3.0 * (400.0 + 100.0 + 200.0), 1e-9);
    d.apply(Command::SetParam { name: "h".into(), expr: Some("20 mm".into()) }).unwrap();
    close(rebuilt(&d).total_volume(), 20.0 / 3.0 * 700.0, 1e-9);
    assert_eq!(d.features_using_entity(2), [1]);
}

#[test]
fn linear_pattern_of_a_sweep() {
    let mut d = pipe_doc();
    push(&mut d, sweep(1, vec![3], vec![1, 2], BodyOp::New));
    push(&mut d, linear(2, 1, "3", ["60 mm", "0", "0"]));
    let r = rebuilt(&d);
    assert!(r.states.values().all(|s| *s == FeatureState::Ok), "{:?}", r.states);
    close(r.total_volume(), 3.0 * faceted_area(2.0) * 50.0, 1e-9);
}

#[test]
fn bad_inputs_become_error_states() {
    let error = |d: &Document, id| match rebuilt(d).states.remove(&id) {
        Some(FeatureState::Error(m)) => m,
        other => panic!("{other:?}"),
    };
    // Open profile: a single line.
    let mut d = pipe_doc();
    put(&mut d, 4, 1, line((0.0, 0.0), (1.0, 1.0)));
    push(&mut d, sweep(1, vec![4], vec![1, 2], BodyOp::Add));
    assert!(error(&d, 1).contains("closed loops"));
    // A branch in the path.
    let mut d = pipe_doc();
    put(&mut d, 4, 0, line((30.0, 0.0), (40.0, -5.0)));
    push(&mut d, sweep(1, vec![3], vec![1, 2, 4], BodyOp::Add));
    assert!(error(&d, 1).contains("branches"));
    // A loft section with a hole.
    let mut d = frustum_doc();
    put(&mut d, 3, 1, circle(0.0, 0.0, 1.0));
    d.apply(Command::SetFeature(loft(1, vec![vec![1], vec![2, 3]], BodyOp::Add))).unwrap();
    assert!(error(&d, 1).contains("without holes"));
}

#[test]
fn feature_rules_are_checked_when_added() {
    let mut d = pipe_doc();
    put(&mut d, 4, 1, circle(5.0, 5.0, 1.0));
    let add = |f: Feature| Command::AddFeature { index: 0, feature: f };
    assert!(matches!(d.apply(add(sweep(1, vec![], vec![1], BodyOp::Add))), Err(DocError::BadFeature(_))));
    assert!(matches!(d.apply(add(sweep(1, vec![3], vec![], BodyOp::Add))), Err(DocError::BadFeature(_))));
    // Profile and path may be in different sketches, but one group may not span two.
    assert!(matches!(d.apply(add(sweep(1, vec![3, 1], vec![2], BodyOp::Add))), Err(DocError::MixedSketches)));
    assert!(matches!(d.apply(add(sweep(1, vec![3], vec![1, 4], BodyOp::Add))), Err(DocError::MixedSketches)));
    assert!(matches!(d.apply(add(loft(1, vec![vec![3]], BodyOp::Add))), Err(DocError::BadFeature("loft needs at least two sections"))));
    assert!(matches!(d.apply(add(loft(1, vec![vec![3], vec![]], BodyOp::Add))), Err(DocError::BadFeature(_))));
    assert!(matches!(d.apply(add(loft(1, vec![vec![3], vec![9]], BodyOp::Add))), Err(DocError::NotFound(9))));
    assert!(d.features().is_empty());
}

#[test]
fn path_and_section_entities_cannot_be_removed() {
    let mut d = pipe_doc();
    push(&mut d, sweep(1, vec![3], vec![1, 2], BodyOp::Add));
    for id in [1, 2, 3] {
        assert_eq!(d.features_using_entity(id), [1]);
        assert!(matches!(d.apply(Command::Remove(id)), Err(DocError::EntityInFeature(e)) if e == id));
    }
    let mut d = frustum_doc();
    assert!(matches!(d.apply(Command::Remove(2)), Err(DocError::EntityInFeature(2))));
}

#[test]
fn undo_redo_and_save_load() {
    let mut d = pipe_doc();
    let mut h = History::default();
    h.execute(&mut d, vec![Command::AddFeature { index: 0, feature: sweep(1, vec![3], vec![1, 2], BodyOp::Add) }]).unwrap();
    h.execute(&mut d, vec![Command::AddFeature { index: 1, feature: loft(2, vec![vec![3], vec![3]], BodyOp::New) }]).unwrap();
    let after = d.features().to_vec();
    assert!(h.undo(&mut d) && h.undo(&mut d));
    assert!(d.features().is_empty());
    assert!(h.redo(&mut d) && h.redo(&mut d));
    assert_eq!(d.features(), after);
    let p = std::env::temp_dir().join(format!("cadtest_{}_sweep.cadproj", std::process::id()));
    d.save(&p).unwrap();
    let loaded = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    assert_eq!(loaded.features(), after);
    assert_eq!(FORMAT_VERSION, 24);
}

#[test]
fn version_8_file_loads() {
    let entity = r#""1":{"id":1,"name":"c","shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}}"#;
    let extrude = r#"{"id":1,"name":"e","suppressed":false,"kind":{"Extrude":{"profile":[1],"distance":"5 mm","offset":"0","op":"Add"}}}"#;
    let text = format!(r#"{{"version":8,"name":"x","next_id":2,"next_feature_id":2,"entities":{{{entity}}},"features":[{extrude}]}}"#);
    let p = std::env::temp_dir().join(format!("cadtest_{}_v8.cadproj", std::process::id()));
    std::fs::write(&p, text).unwrap();
    let d = Document::load(&p);
    std::fs::remove_file(&p).unwrap();
    let d = d.unwrap();
    assert_eq!(d.features().len(), 1);
    assert!(rebuilt(&d).total_volume() > 0.0);
}

#[test]
fn smooth_loft_through_three_squares() {
    let mut d = Document::new("smooth");
    sketch(&mut d, 1, BasePlane::XY, "10 mm");
    sketch(&mut d, 2, BasePlane::XY, "20 mm");
    put(&mut d, 1, 0, rect(-5.0, -5.0, 5.0, 5.0));
    put(&mut d, 2, 1, rect(-10.0, -10.0, 10.0, 10.0));
    put(&mut d, 3, 2, rect(-5.0, -5.0, 5.0, 5.0));
    let mut f = loft(1, vec![vec![1], vec![2], vec![3]], BodyOp::Add);
    push(&mut d, f.clone());
    close(rebuilt(&d).total_volume(), 14000.0 / 3.0, 1e-9);
    if let FeatureKind::Loft { smooth, .. } = &mut f.kind {
        *smooth = true;
    }
    d.apply(Command::SetFeature(f)).unwrap();
    close(rebuilt(&d).total_volume(), 5207.979838053386, 1e-9);
}

#[test]
fn version_13_file_loads_without_taper_or_smooth() {
    let entity = r#""1":{"id":1,"name":"c","shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}}"#;
    let extrude = r#"{"id":1,"name":"e","suppressed":false,"kind":{"Extrude":{"profile":[1],"distance":"5 mm","offset":"0","op":"Add"}}}"#;
    let loft = r#"{"id":2,"name":"l","suppressed":false,"kind":{"Loft":{"sections":[[1],[1]],"op":"New"}}}"#;
    let text = format!(r#"{{"version":13,"name":"x","next_id":2,"next_feature_id":3,"entities":{{{entity}}},"features":[{extrude},{loft}]}}"#);
    let p = std::env::temp_dir().join(format!("cadtest_{}_v13.cadproj", std::process::id()));
    std::fs::write(&p, text).unwrap();
    let d = Document::load(&p);
    std::fs::remove_file(&p).unwrap();
    let d = d.unwrap();
    assert!(matches!(&d.features()[0].kind, FeatureKind::Extrude { taper, .. } if taper == "0 deg"));
    assert!(matches!(&d.features()[1].kind, FeatureKind::Loft { smooth: false, .. }));
}
