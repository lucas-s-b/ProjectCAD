use crate::rebuild_pattern::tests::{extrude, push, put, rect};
use crate::*;
use cad_geometry::DVec3;

fn near3(a: DVec3, b: DVec3, tol: f64) {
    assert!(a.distance(b) <= tol, "{a} vs {b}");
}

/// Body 1: the box (0,0,0)-(10,20,5), an extruded rectangle.
fn boxed() -> Document {
    let mut d = Document::new("asm");
    put(&mut d, 1, 0, rect(0.0, 0.0, 10.0, 20.0));
    push(&mut d, extrude(1, 1, "5 mm", BodyOp::New));
    d
}

fn comp(id: ComponentId, body: FeatureId, at: [&str; 3], angles: [&str; 3]) -> Component {
    Component { id, name: format!("c{id}"), body, at: at.map(String::from), angles: angles.map(String::from), meta: Default::default(), grounded: false }
}

#[test]
fn placement_turns_about_x_then_y_then_z_then_moves() {
    let d = boxed();
    let p = DVec3::new(10.0, 20.0, 5.0);
    let at = |at: [&str; 3], angles: [&str; 3]| d.placement(&comp(1, 1, at, angles)).unwrap().transform_point3(p);
    near3(at(["100 mm", "0", "0"], ["0", "0", "90 deg"]), DVec3::new(80.0, 10.0, 5.0), 1e-9);
    near3(at(["0"; 3], ["90 deg", "0", "90 deg"]), DVec3::new(5.0, 10.0, 20.0), 1e-9);
    near3(at(["0"; 3], ["0", "90 deg", "0"]), DVec3::new(5.0, 20.0, -10.0), 1e-9);
    near3(at(["100", "0", "0"], ["0", "0", "90"]), DVec3::new(80.0, 10.0, 5.0), 1e-9);
    assert_eq!(d.placement(&comp(1, 1, ["1 deg", "0", "0"], ["0"; 3])).err(), Some("expected a length".to_string()));
    assert_eq!(d.placement(&comp(1, 1, ["0"; 3], ["5 mm", "0", "0"])).err(), Some("expected an angle".to_string()));
}

#[test]
fn component_commands_check_and_undo() {
    let mut d = boxed();
    let z = ["0"; 3];
    let undo = d.apply(Command::AddComponent(comp(1, 1, z, z))).unwrap();
    assert!(matches!(undo, Command::RemoveComponent(1)));
    assert!(matches!(d.apply(Command::AddComponent(comp(1, 1, z, z))), Err(DocError::ComponentExists(1))));
    assert!(matches!(d.apply(Command::AddComponent(comp(2, 9, z, z))), Err(DocError::FeatureNotFound(9))));
    let blank = Component { name: "  ".into(), ..comp(2, 1, z, z) };
    assert!(matches!(d.apply(Command::AddComponent(blank)), Err(DocError::BadComponent("empty name"))));
    assert!(d.apply(Command::AddComponent(comp(2, 1, ["1 +", "0", "0"], z))).is_err());
    assert!(matches!(d.apply(Command::SetComponent(comp(7, 1, z, z))), Err(DocError::ComponentNotFound(7))));
    assert!(matches!(d.apply(Command::RemoveComponent(7)), Err(DocError::ComponentNotFound(7))));
    assert!(matches!(d.apply(Command::RemoveFeature(1)), Err(DocError::BodyInUse(1))));
    assert_eq!(d.alloc_component_id(), 2);
    d.apply(Command::AddComponent(comp(5, 1, z, z))).unwrap();
    assert_eq!(d.alloc_component_id(), 6);
    let renamed = Component { name: "gear".into(), ..comp(1, 1, z, z) };
    let Ok(Command::SetComponent(old)) = d.apply(Command::SetComponent(renamed)) else { panic!("SetComponent") };
    assert_eq!(old.name, "c1");
    assert_eq!(d.component(1).unwrap().name, "gear");
    d.apply(undo).unwrap();
    assert!(d.component(1).is_none());
    assert!(matches!(d.apply(Command::RemoveFeature(1)), Err(DocError::BodyInUse(1))));
    d.apply(Command::RemoveComponent(5)).unwrap();
    d.apply(Command::RemoveFeature(1)).unwrap();
    assert_eq!(d.components().count(), 0);
}

#[test]
fn component_moved_writes_millimetres() {
    let mut d = boxed();
    d.apply(Command::SetParam { name: "dx".into(), expr: Some("2.5 mm".into()) }).unwrap();
    let c = comp(1, 1, ["dx", "0", "0"], ["0", "0", "90 deg"]);
    let m = d.component_moved(&c, DVec3::new(1.0, -2.0, 0.25)).unwrap();
    assert_eq!(m.at, ["3.5 mm", "-2 mm", "0.25 mm"].map(String::from));
    assert_eq!((m.angles, m.id, m.name), (c.angles, c.id, c.name));
    assert_eq!(mm(54.0), "54 mm");
    assert_eq!(mm(1.0 / 3.0), "0.333 mm");
}

fn two_components() -> Document {
    let mut d = boxed();
    let mut c1 = comp(1, 1, ["0"; 3], ["0"; 3]);
    c1.meta.insert("material".into(), "Steel".into());
    d.apply(Command::AddComponent(c1)).unwrap();
    d.apply(Command::AddComponent(comp(2, 1, ["10 mm", "0", "0"], ["0"; 3]))).unwrap();
    d
}

#[test]
fn components_save_and_load() {
    let d = two_components();
    let p = std::env::temp_dir().join(format!("cadtest_{}_components.cadproj", std::process::id()));
    d.save(&p).unwrap();
    let loaded = Document::load(&p);
    std::fs::remove_file(&p).ok();
    let mut loaded = loaded.unwrap();
    assert_eq!(loaded.components().cloned().collect::<Vec<_>>(), d.components().cloned().collect::<Vec<_>>());
    assert_eq!(loaded.alloc_component_id(), 3);
    assert_eq!(FORMAT_VERSION, 26);
}

#[test]
fn old_and_broken_files() {
    let d = two_components();
    let p = std::env::temp_dir().join(format!("cadtest_{}_components_old.cadproj", std::process::id()));
    let full = serde_json::to_value(&d).unwrap();
    let mut v = full.clone();
    let o = v.as_object_mut().unwrap();
    o.remove("components");
    o.remove("next_component_id");
    v["version"] = 19.into();
    std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
    let loaded = Document::load(&p);
    std::fs::remove_file(&p).ok();
    let mut loaded = loaded.unwrap();
    assert_eq!(loaded.components().count(), 0);
    assert_eq!(loaded.alloc_component_id(), 1);
    let mut v = full;
    v["features"] = serde_json::json!([]);
    std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
    let loaded = Document::load(&p);
    std::fs::remove_file(&p).ok();
    assert!(matches!(loaded, Err(DocError::FeatureNotFound(1))));
}

fn rebuilt(d: &Document) -> RebuildResult {
    Rebuilder::default().rebuild(d)
}

fn bbox_is(b: &Body, lo: [f64; 3], hi: [f64; 3]) {
    let (l, h) = b.bbox();
    near3(l, DVec3::from(lo), 1e-9);
    near3(h, DVec3::from(hi), 1e-9);
}

#[test]
fn components_place_their_body() {
    let mut d = boxed();
    for c in [
        comp(1, 1, ["100 mm", "0", "0"], ["0", "0", "90 deg"]),
        comp(2, 1, ["0"; 3], ["90 deg", "0", "90 deg"]),
        comp(3, 1, ["0"; 3], ["0", "90 deg", "0"]),
    ] {
        d.apply(Command::AddComponent(c)).unwrap();
    }
    let r = rebuilt(&d);
    assert_eq!(r.components.len(), 3);
    bbox_is(r.component(1).unwrap(), [80.0, 0.0, 0.0], [100.0, 10.0, 5.0]);
    bbox_is(r.component(2).unwrap(), [0.0, 0.0, 0.0], [5.0, 10.0, 20.0]);
    bbox_is(r.component(3).unwrap(), [0.0, 0.0, -10.0], [5.0, 20.0, 0.0]);
    for id in 1..=3 {
        assert!((r.component(id).unwrap().volume() - 1000.0).abs() <= 1e-9);
    }
    assert_eq!(r.bodies.len(), 1);
    bbox_is(r.body(1).unwrap(), [0.0, 0.0, 0.0], [10.0, 20.0, 5.0]);
}

#[test]
fn component_errors_are_reported() {
    let mut d = boxed();
    for c in [
        comp(1, 1, ["1 deg", "0", "0"], ["0"; 3]),
        comp(2, 1, ["0"; 3], ["5 mm", "0", "0"]),
        comp(3, 1, ["0"; 3], ["0"; 3]),
    ] {
        d.apply(Command::AddComponent(c)).unwrap();
    }
    let r = rebuilt(&d);
    assert_eq!(r.components[0].1.as_ref().err(), Some(&"expected a length".to_string()));
    assert_eq!(r.components[1].1.as_ref().err(), Some(&"expected an angle".to_string()));
    assert!(r.component(3).is_some());
    let mut f = d.feature(1).unwrap().clone();
    f.suppressed = true;
    d.apply(Command::SetFeature(f)).unwrap();
    let r = rebuilt(&d);
    assert_eq!(r.components[2].1.as_ref().err(), Some(&"body 1 not found".to_string()));
    assert!(r.component(3).is_none());
}

#[test]
fn parameters_move_components() {
    let mut d = boxed();
    d.apply(Command::SetParam { name: "dx".into(), expr: Some("50 mm".into()) }).unwrap();
    d.apply(Command::AddComponent(comp(1, 1, ["dx", "0", "0"], ["0"; 3]))).unwrap();
    let mut rebuilder = Rebuilder::default();
    let r = rebuilder.rebuild(&d);
    assert!((r.component(1).unwrap().bbox().0.x - 50.0).abs() <= 1e-9);
    d.apply(Command::SetParam { name: "dx".into(), expr: Some("70 mm".into()) }).unwrap();
    let r = rebuilder.rebuild(&d);
    assert!((r.component(1).unwrap().bbox().0.x - 70.0).abs() <= 1e-9);
    assert_eq!(r.recomputed, 0);
}

#[test]
fn check_lists_interference_contact_and_clearance() {
    let mut d = boxed();
    for c in [comp(1, 1, ["0"; 3], ["0"; 3]), comp(2, 1, ["5 mm", "0", "0"], ["0"; 3]), comp(3, 1, ["15 mm", "0", "0"], ["0"; 3])] {
        d.apply(Command::AddComponent(c)).unwrap();
    }
    let r = Rebuilder::default().rebuild(&d);
    let c = d.check(&r, "6 mm").unwrap();
    assert_eq!(c.meetings.len(), 2);
    assert_eq!((c.meetings[0].a, c.meetings[0].b), (1, 2));
    let Ok(Overlap::Interference { volume, .. }) = &c.meetings[0].kind else { panic!("{:?}", c.meetings[0]) };
    assert!((volume - 500.0).abs() <= 1e-6, "{volume}");
    assert_eq!((c.meetings[1].a, c.meetings[1].b), (2, 3));
    assert!(matches!(c.meetings[1].kind, Ok(Overlap::Contact)));
    assert_eq!(c.clearances.len(), 1);
    let (a, b, k) = &c.clearances[0];
    assert_eq!((*a, *b), (1, 3));
    assert!((k.distance - 5.0).abs() <= 1e-6);
    let (p, q) = k.points.unwrap();
    assert!((p.x - 10.0).abs() <= 1e-6 && (q.x - 15.0).abs() <= 1e-6);
    assert!(d.check(&r, "4 mm").unwrap().clearances.is_empty());
    assert_eq!(d.check(&r, "1 deg"), Err("expected a length".to_string()));
}
