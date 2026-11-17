use crate::*;
use cad_geometry::profile::Profile;
use cad_geometry::{DVec2, Shape};
use cad_modeler::Plane3;
use std::f64::consts::{PI, TAU};

fn entity(id: EntityId, shape: Shape) -> Command {
    Command::Add(Entity { id, name: format!("e{id}"), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })
}

fn circle(cx: f64, cy: f64, r: f64) -> Shape {
    Shape::Circle { center: DVec2::new(cx, cy), radius: r }
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Shape {
    Shape::rect(DVec2::new(x0, y0), DVec2::new(x1, y1))
}

fn extrude(id: FeatureId, profile: EntityId, distance: &str, op: BodyOp) -> Feature {
    let kind = FeatureKind::Extrude { profile: vec![profile], distance: distance.into(), offset: "0".into(), op, flip: false, target: Target::Auto };
    Feature { id, name: format!("f{id}"), suppressed: false, kind }
}

fn set_param(d: &mut Document, name: &str, expr: &str) {
    d.apply(Command::SetParam { name: name.into(), expr: Some(expr.into()) }).unwrap();
}

/// A 40 x 20 plate (entity 1) of thickness `t` with a hole of radius 4 (entity 2) cut through it.
fn plate() -> Document {
    let mut d = Document::new("plate");
    for (n, v) in [("w", "40 mm"), ("h", "20 mm"), ("t", "5 mm"), ("r", "4 mm")] {
        set_param(&mut d, n, v);
    }
    d.apply(entity(1, rect(0.0, 0.0, 40.0, 20.0))).unwrap();
    d.apply(entity(2, circle(20.0, 10.0, 4.0))).unwrap();
    for (i, f) in [extrude(1, 1, "t", BodyOp::Add), extrude(2, 2, "t", BodyOp::Cut)].into_iter().enumerate() {
        d.apply(Command::AddFeature { index: i, feature: f }).unwrap();
    }
    d
}

fn volume(r: &RebuildResult) -> f64 {
    assert_eq!(r.bodies.len(), 1);
    r.total_volume()
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs(), "{a} vs {b}");
}

fn state(r: &RebuildResult, id: FeatureId) -> &FeatureState {
    &r.states[&id]
}

#[test]
fn plate_with_a_hole() {
    let d = plate();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!((state(&r, 1), state(&r, 2)), (&FeatureState::Ok, &FeatureState::Ok));
    assert!(r.bodies[0].1.mesh.is_closed());
    // The hole is the tessellated circle, not the exact one.
    let shape = circle(20.0, 10.0, 4.0);
    let disc = cad_modeler::extrude(&Profile::from_shapes(&[&shape]).unwrap(), &Plane3::XY, 1.0).unwrap();
    let area = Body::from_brep(&disc, BODY_TOLERANCE).unwrap().volume();
    close(volume(&r), (800.0 - area) * 5.0, 1e-3);
    assert_eq!(r.recomputed, 2);
}

#[test]
fn parameters_drive_geometry() {
    let mut d = plate();
    let mut rb = Rebuilder::default();
    let before = volume(&rb.rebuild(&d));
    set_param(&mut d, "t", "10 mm");
    close(volume(&rb.rebuild(&d)), 2.0 * before, 1e-9);
}

#[test]
fn only_what_changed_is_recomputed() {
    let mut d = plate();
    let mut rb = Rebuilder::default();
    assert_eq!(rb.rebuild(&d).recomputed, 2);
    assert_eq!(rb.rebuild(&d).recomputed, 0);

    d.apply(Command::SetFeature(extrude(2, 2, "t / 2", BodyOp::Cut))).unwrap();
    assert_eq!(rb.rebuild(&d).recomputed, 1);

    set_param(&mut d, "t", "6 mm");
    assert_eq!(rb.rebuild(&d).recomputed, 2);

    set_param(&mut d, "w", "50 mm");
    assert_eq!(rb.rebuild(&d).recomputed, 0);

    d.apply(Command::SetShape { id: 1, shape: rect(1.0, 0.0, 41.0, 20.0) }).unwrap();
    assert_eq!(rb.rebuild(&d).recomputed, 2);
}

#[test]
fn suppression_and_back() {
    let mut d = plate();
    let mut rb = Rebuilder::default();
    let full = volume(&rb.rebuild(&d));
    let cut = d.feature(2).unwrap().clone();
    d.apply(Command::SetFeature(Feature { suppressed: true, ..cut.clone() })).unwrap();
    let r = rb.rebuild(&d);
    assert_eq!(*state(&r, 2), FeatureState::Suppressed);
    close(volume(&r), 800.0 * 5.0, 1e-9);
    d.apply(Command::SetFeature(cut)).unwrap();
    let r = rb.rebuild(&d);
    assert!(r.recomputed <= 1);
    close(volume(&r), full, 1e-12);
    assert_eq!(*state(&r, 2), FeatureState::Ok);
}

#[test]
fn failures_do_not_stop_later_features() {
    let mut d = plate();
    let mut rb = Rebuilder::default();
    let good = volume(&rb.rebuild(&d));
    d.apply(Command::AddFeature { index: 1, feature: extrude(3, 1, "q", BodyOp::Add) }).unwrap();
    let r = rb.rebuild(&d);
    assert_eq!(*state(&r, 3), FeatureState::Error("unknown name 'q'".into()));
    assert_eq!((state(&r, 1), state(&r, 2)), (&FeatureState::Ok, &FeatureState::Ok));
    close(volume(&r), good, 1e-12);
    set_param(&mut d, "q", "5 mm");
    let r = rb.rebuild(&d);
    assert_eq!(*state(&r, 3), FeatureState::Ok);
    assert!(r.states.values().all(|s| *s == FeatureState::Ok));
}

#[test]
fn wrong_dimension_and_missing_body_and_open_profile() {
    let mut d = Document::new("e");
    d.apply(entity(1, rect(0.0, 0.0, 10.0, 10.0))).unwrap();
    d.apply(entity(2, Shape::Line { a: DVec2::ZERO, b: DVec2::new(5.0, 0.0) })).unwrap();
    let features = [
        extrude(1, 1, "5", BodyOp::Cut),
        extrude(2, 1, "5", BodyOp::Intersect),
        extrude(3, 1, "30 deg", BodyOp::Add),
        extrude(4, 2, "5", BodyOp::Add),
        extrude(5, 1, "5", BodyOp::Add),
    ];
    for (i, f) in features.into_iter().enumerate() {
        d.apply(Command::AddFeature { index: i, feature: f }).unwrap();
    }
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(*state(&r, 1), FeatureState::Error("nothing to cut from".into()));
    assert_eq!(*state(&r, 2), FeatureState::Error("nothing to intersect with".into()));
    assert_eq!(*state(&r, 3), FeatureState::Error("expected a length".into()));
    assert!(matches!(state(&r, 4), FeatureState::Error(_)));
    assert_eq!(*state(&r, 5), FeatureState::Ok);
    close(volume(&r), 500.0, 1e-9);
}

fn revolve(id: FeatureId, axis: RevolveAxis, angle: &str) -> Feature {
    let kind = FeatureKind::Revolve { profile: vec![1], axis, angle: angle.into(), op: BodyOp::Add, target: Target::Auto };
    Feature { id, name: format!("rev{id}"), suppressed: false, kind }
}

#[test]
fn revolve_matches_pappus() {
    let mut d = Document::new("rev");
    d.apply(entity(1, rect(10.0, 0.0, 20.0, 5.0))).unwrap();
    d.apply(entity(2, Shape::Line { a: DVec2::new(0.0, -3.0), b: DVec2::new(0.0, 8.0) })).unwrap();
    let full = 50.0 * TAU * 15.0;
    for (axis, angle, expected) in [
        (RevolveAxis::Y, "360 deg", full),
        (RevolveAxis::Line(2), "360 deg", full),
        (RevolveAxis::Y, "90 deg", full / 4.0),
        (RevolveAxis::Y, &format!("{} rad", PI / 2.0)[..], full / 4.0),
    ] {
        d.apply(Command::AddFeature { index: 0, feature: revolve(9, axis, angle) }).unwrap();
        let r = Rebuilder::default().rebuild(&d);
        assert_eq!(*state(&r, 9), FeatureState::Ok);
        close(volume(&r), expected, 3e-3);
        d.apply(Command::RemoveFeature(9)).unwrap();
    }
    d.apply(Command::AddFeature { index: 0, feature: revolve(9, RevolveAxis::Y, "5 mm") }).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(*state(&r, 9), FeatureState::Error("expected an angle".into()));
    d.apply(Command::SetFeature(revolve(9, RevolveAxis::Line(1), "90 deg"))).unwrap();
    assert_eq!(*state(&Rebuilder::default().rebuild(&d), 9), FeatureState::Error("axis is not a line".into()));
}

#[test]
fn removed_features_leave_the_cache() {
    let mut d = plate();
    let mut rb = Rebuilder::default();
    rb.rebuild(&d);
    d.apply(Command::RemoveFeature(2)).unwrap();
    let r = rb.rebuild(&d);
    assert_eq!((r.recomputed, r.states.len()), (0, 1));
    close(volume(&r), 800.0 * 5.0, 1e-9);
}

#[test]
fn rebuild_until_stops_and_keeps_cache() {
    let d = plate();
    let mut rb = Rebuilder::default();
    let calls = std::cell::Cell::new(0usize);
    let stop = || {
        calls.set(calls.get() + 1);
        calls.get() > 1
    };
    assert!(rb.rebuild_until(&d, &stop).is_none());
    assert_eq!(rb.rebuild(&d).recomputed, 1);
}

#[test]
fn panic_message_reads_str_and_string() {
    let p: Box<dyn std::any::Any + Send> = Box::new("a");
    assert_eq!(panic_message(p.as_ref()), "a");
    let p: Box<dyn std::any::Any + Send> = Box::new(String::from("b"));
    assert_eq!(panic_message(p.as_ref()), "b");
    let p: Box<dyn std::any::Any + Send> = Box::new(3);
    assert_eq!(panic_message(p.as_ref()), "unknown panic");
}
