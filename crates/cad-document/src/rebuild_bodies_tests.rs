use crate::*;
use cad_geometry::profile::Profile;
use cad_geometry::{DVec2, DVec3, Shape};
use cad_modeler::Plane3;
use std::f64::consts::TAU;

fn circle(cx: f64, cy: f64, r: f64) -> Shape {
    Shape::Circle { center: DVec2::new(cx, cy), radius: r }
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Shape {
    Shape::rect(DVec2::new(x0, y0), DVec2::new(x1, y1))
}

fn put(d: &mut Document, id: EntityId, sketch: SketchId, shape: Shape) {
    d.apply(Command::Add(Entity { id, name: format!("e{id}"), shape, layer: 0, sketch, construction: false, group: None, meta: Default::default() })).unwrap();
}

fn sketch_at(d: &mut Document, id: SketchId, base: BasePlane, offset: &str) {
    let plane = PlaneDef::Principal { base, offset: offset.into() };
    d.apply(Command::AddSketch(Sketch { id, name: format!("S{id}"), plane, level: None })).unwrap();
}

struct Ex<'a> {
    distance: &'a str,
    offset: &'a str,
    op: BodyOp,
    flip: bool,
    target: Target,
}

fn ex(distance: &str, op: BodyOp) -> Ex<'_> {
    Ex { distance, offset: "0", op, flip: false, target: Target::Auto }
}

fn push(d: &mut Document, id: FeatureId, profile: EntityId, e: Ex) {
    let kind = FeatureKind::Extrude {
        profile: vec![profile],
        distance: e.distance.into(),
        offset: e.offset.into(),
        taper: "0 deg".into(),
        op: e.op,
        flip: e.flip,
        target: e.target,
    };
    let index = d.features().len();
    d.apply(Command::AddFeature { index, feature: Feature { id, name: format!("f{id}"), suppressed: false, kind } }).unwrap();
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs(), "{a} vs {b}");
}

fn ids(r: &RebuildResult) -> Vec<FeatureId> {
    r.bodies.iter().map(|(id, _)| *id).collect()
}

/// Volume of the faceted circle of radius 4 at (20, 10), per unit height.
fn faceted_area() -> f64 {
    let shape = circle(20.0, 10.0, 4.0);
    let disc = cad_modeler::extrude(&Profile::from_shapes(&[&shape]).unwrap(), &Plane3::XY, 1.0).unwrap();
    Body::from_brep(&disc, BODY_TOLERANCE).unwrap().volume()
}

/// A 40 x 20 plate of thickness t = 5 mm on sketch 0 (feature 1), and a circle of radius 4 on sketch 1 at z = t (entity 2).
fn plate_and_circle() -> Document {
    let mut d = Document::new("boss");
    d.apply(Command::SetParam { name: "t".into(), expr: Some("5 mm".into()) }).unwrap();
    sketch_at(&mut d, 1, BasePlane::XY, "t");
    put(&mut d, 1, 0, rect(0.0, 0.0, 40.0, 20.0));
    put(&mut d, 2, 1, circle(20.0, 10.0, 4.0));
    push(&mut d, 1, 1, ex("t", BodyOp::Add));
    d
}

#[test]
fn boss_on_top_follows_the_parameter() {
    let mut d = plate_and_circle();
    push(&mut d, 2, 2, ex("3 mm", BodyOp::Add));
    let mut rb = Rebuilder::default();
    let r = rb.rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    assert_eq!(ids(&r), [1]);
    close(r.total_volume(), 4000.0 + faceted_area() * 3.0, 1e-3);
    let (lo, hi) = r.body(1).unwrap().bbox();
    assert_eq!((lo.z, hi.z), (0.0, 8.0));
    d.apply(Command::SetParam { name: "t".into(), expr: Some("7 mm".into()) }).unwrap();
    let (_, hi) = rb.rebuild(&d).body(1).unwrap().bbox();
    close(hi.z, 10.0, 1e-9);
}

#[test]
fn blind_hole_needs_flip_to_cut_downward() {
    let mut d = plate_and_circle();
    push(&mut d, 2, 2, Ex { flip: true, ..ex("2 mm", BodyOp::Cut) });
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    close(r.total_volume(), 4000.0 - faceted_area() * 2.0, 1e-3);

    let kind = FeatureKind::Extrude {
        profile: vec![2],
        distance: "2 mm".into(),
        offset: "0".into(),
        taper: "0 deg".into(),
        op: BodyOp::Cut,
        flip: false,
        target: Target::Auto,
    };
    d.apply(Command::SetFeature(Feature { id: 2, name: "f2".into(), suppressed: false, kind })).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    close(r.total_volume(), 4000.0, 1e-9);
}

#[test]
fn xz_sketch_extrudes_along_minus_y() {
    let mut d = Document::new("xz");
    sketch_at(&mut d, 1, BasePlane::XZ, "0");
    put(&mut d, 1, 1, rect(0.0, 0.0, 10.0, 10.0));
    push(&mut d, 1, 1, ex("5 mm", BodyOp::Add));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&1], FeatureState::Ok);
    let (lo, hi) = r.body(1).unwrap().bbox();
    assert_eq!((lo, hi), (DVec3::new(0.0, -5.0, 0.0), DVec3::new(10.0, 0.0, 10.0)));
}

#[test]
fn revolve_uses_the_sketch_plane() {
    let mut d = Document::new("rev");
    sketch_at(&mut d, 1, BasePlane::XZ, "0");
    put(&mut d, 1, 1, rect(10.0, 0.0, 20.0, 5.0));
    let kind = FeatureKind::Revolve {
        profile: vec![1],
        axis: RevolveAxis::Y,
        angle: "360 deg".into(),
        op: BodyOp::Add,
        target: Target::Auto,
    };
    d.apply(Command::AddFeature { index: 0, feature: Feature { id: 1, name: "r".into(), suppressed: false, kind } }).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&1], FeatureState::Ok);
    close(r.total_volume(), 50.0 * TAU * 15.0, 3e-3);
    // The sketch's Y axis is world Z, so the solid spans z from 0 to 5 around the Z axis.
    let (lo, hi) = r.body(1).unwrap().bbox();
    close(hi.z - lo.z, 5.0, 1e-9);
}

#[test]
fn fixed_plane_on_a_side_face() {
    let mut d = Document::new("fixed");
    let plane = PlaneDef::Fixed { origin: DVec3::new(40.0, 0.0, 0.0), x: DVec3::Y, y: DVec3::Z };
    d.apply(Command::AddSketch(Sketch { id: 1, name: "Side".into(), plane, level: None })).unwrap();
    put(&mut d, 1, 0, rect(0.0, 0.0, 40.0, 20.0));
    put(&mut d, 2, 1, circle(10.0, 2.5, 2.0));
    push(&mut d, 1, 1, ex("5 mm", BodyOp::Add));
    push(&mut d, 2, 2, ex("6 mm", BodyOp::Add));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    assert_eq!(ids(&r), [1]);
    close(r.body(1).unwrap().bbox().1.x, 46.0, 1e-9);
}

/// Bodies 1 and 2: 10 x 10 x 10 blocks at x 0..10 and x 30..40.
fn two_blocks() -> Document {
    let mut d = Document::new("blocks");
    put(&mut d, 1, 0, rect(0.0, 0.0, 10.0, 10.0));
    put(&mut d, 2, 0, rect(30.0, 0.0, 40.0, 10.0));
    push(&mut d, 1, 1, ex("10 mm", BodyOp::New));
    push(&mut d, 2, 2, ex("10 mm", BodyOp::New));
    d
}

fn volumes(r: &RebuildResult) -> Vec<f64> {
    r.bodies.iter().map(|(_, b)| b.volume()).collect()
}

fn through(d: &mut Document, id: FeatureId, op: BodyOp, target: Target) {
    put(d, 100 + id, 0, rect(-5.0, 4.0, 45.0, 6.0));
    push(d, id, 100 + id, Ex { offset: "-5 mm", target, ..ex("20 mm", op) });
}

#[test]
fn new_creates_separate_bodies() {
    let r = Rebuilder::default().rebuild(&two_blocks());
    assert_eq!(ids(&r), [1, 2]);
    assert_eq!(volumes(&r).len(), 2);
    close(r.total_volume(), 2000.0, 1e-9);
    assert!(r.body(2).is_some() && r.body(3).is_none());
}

#[test]
fn cut_auto_goes_through_every_body() {
    let mut d = two_blocks();
    through(&mut d, 3, BodyOp::Cut, Target::Auto);
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&3], FeatureState::Ok);
    assert_eq!(ids(&r), [1, 2]);
    for v in volumes(&r) {
        close(v, 800.0, 1e-9);
    }
}

#[test]
fn cut_by_target_changes_one_body() {
    let mut d = two_blocks();
    through(&mut d, 3, BodyOp::Cut, Target::Body(2));
    let r = Rebuilder::default().rebuild(&d);
    let v = volumes(&r);
    close(v[0], 1000.0, 1e-9);
    close(v[1], 800.0, 1e-9);
}

fn bridge(d: &mut Document, target: Target) {
    put(d, 50, 0, rect(5.0, 2.0, 35.0, 8.0));
    push(d, 3, 50, Ex { target, ..ex("5 mm", BodyOp::Add) });
}

#[test]
fn add_auto_merges_with_the_last_body_only() {
    let mut d = two_blocks();
    bridge(&mut d, Target::Auto);
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&3], FeatureState::Ok);
    assert_eq!(ids(&r), [1, 2]);
    let v = volumes(&r);
    close(v[0], 1000.0, 1e-9);
    close(v[1], 1000.0 + 900.0 - 150.0, 1e-9);
}

#[test]
fn add_all_merges_everything_into_the_first_body() {
    let mut d = two_blocks();
    bridge(&mut d, Target::All);
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&3], FeatureState::Ok);
    assert_eq!(ids(&r), [1]);
    close(r.total_volume(), 2000.0 + 900.0 - 300.0, 1e-9);
}

#[test]
fn missing_target_is_an_error_and_keeps_the_bodies() {
    let mut d = two_blocks();
    bridge(&mut d, Target::Body(99));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&3], FeatureState::Error("target body not found".into()));
    assert_eq!(ids(&r), [1, 2]);
    close(r.total_volume(), 2000.0, 1e-9);
}

#[test]
fn a_cut_that_swallows_a_body_removes_it() {
    let mut d = two_blocks();
    put(&mut d, 60, 0, rect(-5.0, -5.0, 15.0, 15.0));
    push(&mut d, 3, 60, Ex { offset: "-5 mm", target: Target::Body(1), ..ex("20 mm", BodyOp::Cut) });
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&3], FeatureState::Ok);
    assert_eq!(ids(&r), [2]);
    close(r.total_volume(), 1000.0, 1e-9);
}

#[test]
fn cache_keeps_every_body_list() {
    let mut d = two_blocks();
    through(&mut d, 3, BodyOp::Cut, Target::Auto);
    let mut rb = Rebuilder::default();
    assert_eq!(rb.rebuild(&d).recomputed, 3);
    let r = rb.rebuild(&d);
    assert_eq!((r.recomputed, ids(&r)), (0, vec![1, 2]));
    close(r.total_volume(), 1600.0, 1e-9);
    let kind = FeatureKind::Extrude {
        profile: vec![103],
        distance: "20 mm".into(),
        offset: "-5 mm".into(),
        taper: "0 deg".into(),
        op: BodyOp::Cut,
        flip: false,
        target: Target::Body(1),
    };
    d.apply(Command::SetFeature(Feature { id: 3, name: "f3".into(), suppressed: false, kind })).unwrap();
    let r = rb.rebuild(&d);
    assert_eq!(r.recomputed, 1);
    close(r.total_volume(), 1800.0, 1e-9);
}

#[test]
fn version_6_feature_file_rebuilds() {
    let text = r#"{"version":6,"name":"x","next_id":2,"next_feature_id":2,"entities":{"1":{"id":1,"name":"r",
        "shape":{"Polyline":{"points":[[0.0,0.0],[10.0,0.0],[10.0,10.0],[0.0,10.0]],"closed":true}}}},
        "features":[{"id":1,"name":"e","suppressed":false,
        "kind":{"Extrude":{"profile":[1],"distance":"5","offset":"0","op":"Add"}}}]}"#;
    let p = std::env::temp_dir().join(format!("cadtest_{}_v6feature.cadproj", std::process::id()));
    std::fs::write(&p, text).unwrap();
    let d = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&1], FeatureState::Ok);
    close(r.total_volume(), 500.0, 1e-9);
}

fn param(d: &mut Document, name: &str, expr: &str) {
    d.apply(Command::SetParam { name: name.into(), expr: Some(expr.into()) }).unwrap();
}

/// Plate 40 x 20 x h (h = 10 mm, feature 1), sketch 1 on its top face picked at `at`, circle r 4 at (20, 10) in it (entity 2).
fn face_sketch(at: DVec3) -> Document {
    let mut d = Document::new("face");
    param(&mut d, "h", "10 mm");
    put(&mut d, 1, 0, rect(0.0, 0.0, 40.0, 20.0));
    push(&mut d, 1, 1, ex("h", BodyOp::Add));
    let plane = PlaneDef::Face { origin: DVec3::new(0.0, 0.0, 10.0), x: DVec3::X, y: DVec3::Y, at, body: 1 };
    d.apply(Command::AddSketch(Sketch { id: 1, name: "Top".into(), plane, level: None })).unwrap();
    put(&mut d, 2, 1, circle(20.0, 10.0, 4.0));
    d
}

#[test]
fn sketch_on_face_follows_the_face() {
    let fa = faceted_area();
    let mut d = face_sketch(DVec3::new(20.0, 10.0, 10.0));
    push(&mut d, 2, 2, ex("3 mm", BodyOp::Add));
    let mut rb = Rebuilder::default();
    let r = rb.rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    close(r.total_volume(), 8000.0 + 3.0 * fa, 1e-4);
    close(r.planes[&1].clone().unwrap().origin.z, 10.0, 1e-9);
    param(&mut d, "h", "15 mm");
    let r = rb.rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    close(r.total_volume(), 12000.0 + 3.0 * fa, 1e-4);
    close(r.planes[&1].clone().unwrap().origin.z, 15.0, 1e-9);
}

#[test]
fn sketch_on_a_lost_face_reports_it() {
    let mut d = face_sketch(DVec3::new(60.0, 10.0, 10.0));
    push(&mut d, 2, 2, ex("3 mm", BodyOp::Add));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Error("face not found; pick it again".into()));
    assert!(r.planes[&1].is_err());
    assert_eq!(ids(&r), [1]);
    close(r.total_volume(), 8000.0, 1e-9);
}

#[test]
fn face_sketch_is_placed_once_per_rebuild() {
    let fa = faceted_area();
    let mut d = face_sketch(DVec3::new(20.0, 10.0, 10.0));
    push(&mut d, 2, 2, ex("3 mm", BodyOp::Add));
    push(&mut d, 3, 2, ex("2 mm", BodyOp::Add));
    let r = Rebuilder::default().rebuild(&d);
    // Placed again on the boss top, feature 3 would add 5 fa in all.
    close(r.total_volume(), 8000.0 + 3.0 * fa, 1e-4);
}

#[test]
fn unused_face_sketch_follows_the_last_bodies() {
    let mut d = face_sketch(DVec3::new(20.0, 10.0, 10.0));
    param(&mut d, "h", "15 mm");
    let r = Rebuilder::default().rebuild(&d);
    close(r.planes[&1].clone().unwrap().origin.z, 15.0, 1e-9);
    close(d.resolve_plane(1).unwrap().origin.z, 10.0, 1e-9);
}
