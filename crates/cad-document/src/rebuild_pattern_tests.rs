use crate::*;
use cad_geometry::profile::Profile;
use cad_geometry::{DVec2, DVec3, Shape};
use cad_modeler::Plane3;

const COUNT_ERROR: &str = "count must be a whole number from 1 to 500";

pub(crate) fn circle(cx: f64, cy: f64, r: f64) -> Shape {
    Shape::Circle { center: DVec2::new(cx, cy), radius: r }
}

pub(crate) fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Shape {
    Shape::rect(DVec2::new(x0, y0), DVec2::new(x1, y1))
}

pub(crate) fn put(d: &mut Document, id: EntityId, sketch: SketchId, shape: Shape) {
    d.apply(Command::Add(Entity { id, name: format!("e{id}"), shape, layer: 0, sketch, construction: false, group: None, meta: Default::default() })).unwrap();
}

pub(crate) fn extrude(id: FeatureId, profile: EntityId, distance: &str, op: BodyOp) -> Feature {
    let kind = FeatureKind::Extrude { profile: vec![profile], distance: distance.into(), offset: "0".into(), taper: "0 deg".into(), op, flip: false, target: Target::Auto };
    Feature { id, name: format!("f{id}"), suppressed: false, kind }
}

pub(crate) fn linear(id: FeatureId, source: FeatureId, count: &str, step: [&str; 3]) -> Feature {
    let [dx, dy, dz] = step.map(String::from);
    let kind = FeatureKind::LinearPattern { source, count: count.into(), dx, dy, dz };
    Feature { id, name: format!("p{id}"), suppressed: false, kind }
}

fn circular(id: FeatureId, source: FeatureId, count: &str, angle: &str) -> Feature {
    let kind = FeatureKind::CircularPattern {
        source,
        count: count.into(),
        angle: angle.into(),
        axis: Axis3::Z,
        cx: "0".into(),
        cy: "0".into(),
        cz: "0".into(),
    };
    Feature { id, name: format!("c{id}"), suppressed: false, kind }
}

pub(crate) fn mirror(id: FeatureId, source: MirrorSource, plane: BasePlane, offset: &str) -> Feature {
    Feature { id, name: format!("m{id}"), suppressed: false, kind: FeatureKind::Mirror { source, plane, offset: offset.into() } }
}

pub(crate) fn push(d: &mut Document, f: Feature) {
    let index = d.features().len();
    d.apply(Command::AddFeature { index, feature: f }).unwrap();
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs().max(1.0), "{a} vs {b}");
}

/// Volume of the faceted circle of radius `r` per unit height.
fn faceted_area(r: f64) -> f64 {
    let shape = circle(0.0, 0.0, r);
    let disc = cad_modeler::extrude(&Profile::from_shapes(&[&shape]).unwrap(), &Plane3::XY, 1.0).unwrap();
    Body::from_brep(&disc, BODY_TOLERANCE).unwrap().volume()
}

/// A 100 x 60 x 5 plate (feature 1) with a hole of radius 3 at (10, 10) cut through it (feature 2).
fn holed_plate() -> Document {
    let mut d = Document::new("plate");
    put(&mut d, 1, 0, rect(0.0, 0.0, 100.0, 60.0));
    put(&mut d, 2, 0, circle(10.0, 10.0, 3.0));
    push(&mut d, extrude(1, 1, "5 mm", BodyOp::Add));
    push(&mut d, extrude(2, 2, "5 mm", BodyOp::Cut));
    d
}

fn volume(d: &Document) -> f64 {
    let r = Rebuilder::default().rebuild(d);
    assert!(r.states.values().all(|s| *s == FeatureState::Ok), "{:?}", r.states);
    r.total_volume()
}

/// Volume one hole removes from the plate.
fn one_hole() -> f64 {
    30000.0 - volume(&holed_plate())
}

#[test]
fn linear_pattern_of_holes_and_a_pattern_of_a_pattern() {
    let removed = one_hole();
    close(removed, faceted_area(3.0) * 5.0, 1e-9);
    let mut d = holed_plate();
    push(&mut d, linear(3, 2, "4", ["20 mm", "0", "0"]));
    close(30000.0 - volume(&d), 4.0 * removed, 1e-6);
    push(&mut d, linear(4, 3, "3", ["0", "15 mm", "0"]));
    close(30000.0 - volume(&d), 12.0 * removed, 1e-6);
}

#[test]
fn circular_pattern_around_the_origin() {
    let mut d = Document::new("disc");
    put(&mut d, 1, 0, circle(0.0, 0.0, 50.0));
    put(&mut d, 2, 0, circle(30.0, 0.0, 3.0));
    push(&mut d, extrude(1, 1, "5 mm", BodyOp::Add));
    push(&mut d, extrude(2, 2, "5 mm", BodyOp::Cut));
    let (disc, one) = (volume(&Document { features: d.features()[..1].to_vec(), ..d.clone() }), volume(&d));
    let removed = disc - one;
    push(&mut d, circular(3, 2, "6", "360 deg"));
    close(volume(&d), disc - 6.0 * removed, 1e-6);

    d.apply(Command::SetFeature(circular(3, 2, "3", "90 deg"))).unwrap();
    close(volume(&d), disc - 3.0 * removed, 1e-6);
    let body = Rebuilder::default().rebuild(&d).bodies[0].1.clone();
    let rim = |deg: f64| DVec3::new(33.0 * deg.to_radians().cos(), 33.0 * deg.to_radians().sin(), 5.0);
    for deg in [0.0, 45.0, 90.0] {
        assert!(body.nearest_edge(rim(deg), 0.05).is_some(), "no hole at {deg} degrees");
    }
    assert!(body.nearest_edge(rim(22.5), 0.05).is_none() && body.nearest_edge(rim(135.0), 0.05).is_none());
}

#[test]
fn count_is_an_expression_and_is_checked() {
    let removed = one_hole();
    let mut d = holed_plate();
    d.apply(Command::SetParam { name: "n".into(), expr: Some("5".into()) }).unwrap();
    push(&mut d, linear(3, 2, "n", ["20 mm", "0", "0"]));
    let mut rb = Rebuilder::default();
    let r = rb.rebuild(&d);
    close(30000.0 - r.total_volume(), 5.0 * removed, 1e-6);
    d.apply(Command::SetParam { name: "n".into(), expr: Some("2".into()) }).unwrap();
    let r = rb.rebuild(&d);
    assert_eq!(r.recomputed, 1);
    close(30000.0 - r.total_volume(), 2.0 * removed, 1e-6);
    for bad in ["2.5", "0", "501", "3 mm", "-1"] {
        d.apply(Command::SetFeature(linear(3, 2, bad, ["20 mm", "0", "0"]))).unwrap();
        let r = Rebuilder::default().rebuild(&d);
        assert_eq!(r.states[&3], FeatureState::Error(COUNT_ERROR.into()), "count {bad}");
        close(r.total_volume(), 30000.0 - removed, 1e-9);
    }
    d.apply(Command::SetFeature(linear(3, 2, "1", ["20 mm", "0", "0"]))).unwrap();
    close(volume(&d), 30000.0 - removed, 1e-9);
    d.apply(Command::SetFeature(linear(3, 2, "2", ["20 deg", "0", "0"]))).unwrap();
    assert_eq!(Rebuilder::default().rebuild(&d).states[&3], FeatureState::Error("expected a length".into()));
}

/// Plate 100 x 60 x 5 (feature 1) with a boss of radius 3 at (10, 10) on its top (feature 3, sketch 1).
fn bossed_plate(op: BodyOp, at_x: f64) -> Document {
    let mut d = Document::new("boss");
    let plane = PlaneDef::Principal { base: BasePlane::XY, offset: "5 mm".into() };
    d.apply(Command::AddSketch(Sketch { id: 1, name: "top".into(), plane })).unwrap();
    put(&mut d, 1, 0, rect(-50.0, -30.0, 50.0, 30.0));
    put(&mut d, 2, 1, circle(at_x, 10.0, 3.0));
    push(&mut d, extrude(1, 1, "5 mm", BodyOp::Add));
    push(&mut d, extrude(2, 2, "5 mm", op));
    d
}

#[test]
fn boss_pattern_grows_the_body_and_follows_the_source() {
    let boss = faceted_area(3.0) * 5.0;
    let mut d = bossed_plate(BodyOp::Add, 10.0);
    push(&mut d, linear(3, 2, "3", ["10 mm", "0", "0"]));
    let mut rb = Rebuilder::default();
    close(rb.rebuild(&d).total_volume(), 30000.0 + 3.0 * boss, 1e-6);
    // Editing the source recomputes the pattern and changes every copy.
    d.apply(Command::SetFeature(extrude(2, 2, "8 mm", BodyOp::Add))).unwrap();
    let r = rb.rebuild(&d);
    assert_eq!(r.recomputed, 2);
    close(r.total_volume(), 30000.0 + 3.0 * boss * 8.0 / 5.0, 1e-6);
}

#[test]
fn pattern_of_a_new_body_is_one_extra_body() {
    let boss = faceted_area(3.0) * 5.0;
    let mut d = bossed_plate(BodyOp::New, 10.0);
    push(&mut d, linear(3, 2, "3", ["10 mm", "0", "0"]));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.bodies.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [1, 2, 3]);
    close(r.body(3).unwrap().volume(), 2.0 * boss, 1e-6);
    close(r.body(2).unwrap().volume(), boss, 1e-6);
}

#[test]
fn mirror_of_a_boss_is_symmetric() {
    let boss = faceted_area(3.0) * 5.0;
    let mut d = bossed_plate(BodyOp::Add, 20.0);
    push(&mut d, mirror(3, MirrorSource::Feature(2), BasePlane::YZ, "0"));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&3], FeatureState::Ok);
    close(r.total_volume(), 30000.0 + 2.0 * boss, 1e-6);
    // The boss at x = 20 sits at y = 10 on both sides, so only the x moment cancels.
    assert!(r.bodies[0].1.centroid().x.abs() < 1e-9);
}

#[test]
fn mirror_of_bodies_doubles_a_block() {
    let mut d = Document::new("block");
    put(&mut d, 1, 0, rect(0.0, 0.0, 10.0, 10.0));
    push(&mut d, extrude(1, 1, "10 mm", BodyOp::Add));
    push(&mut d, mirror(2, MirrorSource::Bodies, BasePlane::YZ, "0"));
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.states[&2], FeatureState::Ok);
    close(r.total_volume(), 2000.0, 1e-9);
    let (lo, hi) = r.bodies[0].1.bbox();
    assert_eq!((lo, hi), (DVec3::new(-10.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0)));
    // XZ has the normal -Y, so a positive offset moves the plane to negative y.
    d.apply(Command::SetFeature(mirror(2, MirrorSource::Bodies, BasePlane::XZ, "5 mm"))).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    let (lo, hi) = r.bodies[0].1.bbox();
    close(r.total_volume(), 2000.0, 1e-9);
    assert_eq!((lo.y, hi.y), (-20.0, 10.0));
}

#[test]
fn bad_sources_report_errors_and_keep_the_bodies() {
    let removed = one_hole();
    let state = |d: &Document| Rebuilder::default().rebuild(d).states[&3].clone();
    let err = |m: &str| FeatureState::Error(m.into());

    let mut d = holed_plate();
    push(&mut d, linear(3, 2, "4", ["20 mm", "0", "0"]));
    d.apply(Command::MoveFeature { id: 3, index: 1 }).unwrap();
    assert_eq!(state(&d), err("source feature must come before this feature"));
    // The plate is cut by the hole only; the pattern changed nothing.
    close(Rebuilder::default().rebuild(&d).total_volume(), 30000.0 - removed, 1e-9);

    let mut d = holed_plate();
    push(&mut d, linear(3, 2, "4", ["20 mm", "0", "0"]));
    let hole = Feature { suppressed: true, ..d.feature(2).unwrap().clone() };
    d.apply(Command::SetFeature(hole)).unwrap();
    assert_eq!(state(&d), err("source feature is not active"));

    let mut d = holed_plate();
    let kind = FeatureKind::Fillet { edges: vec![DVec3::new(0.0, 0.0, 2.5)], faces: Vec::new(), radius: "1 mm".into() };
    push(&mut d, Feature { id: 3, name: "fil".into(), suppressed: false, kind });
    push(&mut d, linear(4, 3, "2", ["20 mm", "0", "0"]));
    assert_eq!(Rebuilder::default().rebuild(&d).states[&4], err("this feature cannot be used as a source"));
}
