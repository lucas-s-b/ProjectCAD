use crate::*;
use cad_geometry::{DVec2, DVec3, Shape};
use crate::FaceKey;
use std::f64::consts::PI;

const EDGE_ERROR: &str = "edge not found; pick it again";

fn close(a: f64, b: f64, abs: f64) {
    assert!((a - b).abs() <= abs, "{a} vs {b}");
}

fn feature(id: FeatureId, kind: FeatureKind) -> Feature {
    Feature { id, name: format!("f{id}"), suppressed: false, kind }
}

fn push(d: &mut Document, f: Feature) {
    let index = d.features().len();
    d.apply(Command::AddFeature { index, feature: f }).unwrap();
}

fn fillet(id: FeatureId, edges: &[DVec3], radius: &str) -> Feature {
    feature(id, FeatureKind::Fillet { edges: edges.to_vec(), faces: Vec::new(), radius: radius.into() })
}

fn param(d: &mut Document, name: &str, expr: &str) {
    d.apply(Command::SetParam { name: name.into(), expr: Some(expr.into()) }).unwrap();
}

/// A 40 x 20 plate of height h = 10 mm (feature 1) and r = 3 mm.
fn plate() -> Document {
    let mut d = Document::new("plate");
    d.apply(Command::Add(Entity { id: 1, name: "r".into(), shape: Shape::rect(DVec2::ZERO, DVec2::new(40.0, 20.0)), layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
    param(&mut d, "h", "10 mm");
    param(&mut d, "r", "3 mm");
    let kind = FeatureKind::Extrude { profile: vec![1], distance: "h".into(), offset: "0".into(), taper: "0 deg".into(), op: BodyOp::Add, flip: false, target: Target::Auto };
    push(&mut d, feature(1, kind));
    d
}

fn corner_edge() -> DVec3 {
    DVec3::new(0.0, 0.0, 5.0)
}

fn rounded(n: f64) -> f64 {
    8000.0 - n * (9.0 - 9.0 * PI / 4.0) * 10.0
}

#[test]
fn fillet_follows_its_radius_and_recomputes_only_itself() {
    let mut d = plate();
    push(&mut d, fillet(2, &[corner_edge()], "r"));
    let mut rb = Rebuilder::default();
    let r = rb.rebuild(&d);
    assert_eq!((&r.states[&2], r.recomputed), (&FeatureState::Ok, 2));
    close(r.total_volume(), rounded(1.0), 0.05);
    param(&mut d, "r", "2 mm");
    let r = rb.rebuild(&d);
    assert_eq!((&r.states[&2], r.recomputed), (&FeatureState::Ok, 1));
    close(r.total_volume(), 8000.0 - (4.0 - PI) * 10.0, 0.05);
}

#[test]
fn two_edges_in_one_feature() {
    let mut d = plate();
    push(&mut d, fillet(2, &[corner_edge(), DVec3::new(40.0, 0.0, 5.0)], "r"));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    close(r.total_volume(), rounded(2.0), 0.1);
}

#[test]
fn a_point_off_every_edge_fails_and_keeps_the_body() {
    let mut d = plate();
    // The second point misses, so the first blend is not kept either.
    push(&mut d, fillet(2, &[corner_edge(), DVec3::new(20.0, 10.0, 5.0)], "r"));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Error(EDGE_ERROR.into()));
    close(r.total_volume(), 8000.0, 1e-9);
}

#[test]
fn a_moved_edge_is_not_found_until_it_is_back() {
    let mut d = plate();
    push(&mut d, fillet(2, &[corner_edge()], "r"));
    let mut rb = Rebuilder::default();
    assert_eq!(rb.rebuild(&d).states[&2], FeatureState::Ok);
    param(&mut d, "h", "4 mm");
    let r = rb.rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Error(EDGE_ERROR.into()));
    close(r.total_volume(), 3200.0, 1e-9);
    param(&mut d, "h", "10 mm");
    let r = rb.rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    close(r.total_volume(), rounded(1.0), 0.05);
}

#[test]
fn a_blend_that_is_too_large_reports_the_kernel_error() {
    let mut d = plate();
    push(&mut d, feature(2, FeatureKind::Chamfer { faces: Vec::new(), edges: vec![corner_edge()], distance: "0 mm".into() }));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Error("blend size must be positive".into()));
    close(r.total_volume(), 8000.0, 1e-9);
}

#[test]
fn chamfer_of_a_top_edge() {
    let mut d = plate();
    push(&mut d, feature(2, FeatureKind::Chamfer { faces: Vec::new(), edges: vec![DVec3::new(20.0, 0.0, 10.0)], distance: "2 mm".into() }));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    close(r.total_volume(), 8000.0 - 2.0 * 40.0, 1e-6);
}

#[test]
fn undo_and_redo_of_blend_commands() {
    let mut d = plate();
    let mut h = History::default();
    let f = fillet(2, &[corner_edge()], "r");
    h.execute(&mut d, vec![Command::AddFeature { index: 1, feature: f.clone() }]).unwrap();
    let g = fillet(2, &[corner_edge(), DVec3::new(40.0, 0.0, 5.0)], "2 mm");
    h.execute(&mut d, vec![Command::SetFeature(g.clone())]).unwrap();
    let volume = |d: &Document| Rebuilder::default().rebuild(d).total_volume();
    close(volume(&d), 8000.0 - 2.0 * (4.0 - PI) * 10.0, 0.1);
    assert!(h.undo(&mut d));
    assert_eq!(d.features()[1], f);
    close(volume(&d), rounded(1.0), 0.05);
    assert!(h.undo(&mut d));
    assert_eq!(d.features().len(), 1);
    assert!(h.redo(&mut d) && h.redo(&mut d));
    assert_eq!(d.features()[1], g);
}

#[test]
fn blends_are_validated_and_survive_save_and_load() {
    let mut d = plate();
    let bad = |edges: &[DVec3], size: &str| d.clone().apply(Command::AddFeature { index: 1, feature: fillet(2, edges, size) });
    assert!(matches!(bad(&[], "r"), Err(DocError::BadFeature("no edges"))));
    assert!(matches!(bad(&[DVec3::new(f64::NAN, 0.0, 0.0)], "r"), Err(DocError::BadFeature("edge point is not finite"))));
    assert!(bad(&[corner_edge()], "3 +").is_err());
    let fillet_f = fillet(2, &[corner_edge()], "r");
    let chamfer_f = feature(3, FeatureKind::Chamfer { faces: Vec::new(), edges: vec![DVec3::new(20.0, 0.0, 10.0)], distance: "2 mm".into() });
    assert!(fillet_f.entities().is_empty() && fillet_f.expressions() == ["r"] && chamfer_f.expressions() == ["2 mm"]);
    push(&mut d, fillet_f.clone());
    push(&mut d, chamfer_f.clone());
    assert_eq!(d.features_using_param("r"), [2]);
    let path = std::env::temp_dir().join(format!("projectcad_blend_{}.cad", std::process::id()));
    d.save(&path).unwrap();
    let loaded = Document::load(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(loaded.features()[1..], [fillet_f, chamfer_f]);
}

fn top_front() -> [FaceKey; 2] {
    [FaceKey::Plane(DVec3::Z), FaceKey::Plane(-DVec3::Y)]
}

fn keyed(id: FeatureId, edges: &[DVec3], faces: Vec<[FaceKey; 2]>, radius: &str) -> Feature {
    feature(id, FeatureKind::Fillet { edges: edges.to_vec(), faces, radius: radius.into() })
}

#[test]
fn keyed_chamfer_follows_a_taller_plate() {
    let mut d = plate();
    let kind = FeatureKind::Chamfer { edges: vec![DVec3::new(20.0, 0.0, 10.0)], faces: vec![top_front()], distance: "2 mm".into() };
    push(&mut d, feature(2, kind));
    let mut rb = Rebuilder::default();
    let r = rb.rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    close(r.total_volume(), 7920.0, 1e-6);
    param(&mut d, "h", "15 mm");
    let r = rb.rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    close(r.total_volume(), 11920.0, 1e-6);
    assert_eq!(r.recomputed, 2);
}

#[test]
fn keyed_fillet_finds_a_shorter_corner() {
    let mut d = plate();
    push(&mut d, keyed(2, &[corner_edge()], vec![[FaceKey::Plane(-DVec3::X), FaceKey::Plane(-DVec3::Y)]], "r"));
    param(&mut d, "h", "4 mm");
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    close(r.total_volume(), 3200.0 - (9.0 - 9.0 * PI / 4.0) * 4.0, 0.05);
}

#[test]
fn keyed_edge_without_a_match_fails() {
    let mut d = plate();
    push(&mut d, keyed(2, &[corner_edge()], vec![[FaceKey::Plane(DVec3::Z), FaceKey::Plane(-DVec3::Z)]], "r"));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Error(EDGE_ERROR.into()));
    close(r.total_volume(), 8000.0, 1e-9);
}

#[test]
fn face_pairs_must_match_the_edges() {
    let d = plate();
    let f = keyed(2, &[corner_edge()], vec![top_front(), top_front()], "r");
    assert!(matches!(d.clone().apply(Command::AddFeature { index: 1, feature: f }), Err(DocError::BadFeature("one face pair per edge"))));
}

#[test]
fn keyed_blends_survive_save_and_load() {
    let mut d = plate();
    let f = feature(2, FeatureKind::Chamfer { edges: vec![DVec3::new(20.0, 0.0, 10.0)], faces: vec![top_front()], distance: "2 mm".into() });
    push(&mut d, f.clone());
    let path = std::env::temp_dir().join(format!("projectcad_keyed_blend_{}.cad", std::process::id()));
    d.save(&path).unwrap();
    let loaded = Document::load(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(loaded.features()[1], f);
}
