use crate::rebuild_pattern::tests::{extrude, push, put, rect};
use crate::*;
use cad_geometry::DVec3;

const O: [&str; 3] = ["0", "0", "0"];
const Z: [&str; 3] = ["0", "0", "1"];

/// Body 1: the box (0,0,0)-(10,20,5), an extruded rectangle.
pub(crate) fn boxed() -> Document {
    let mut d = Document::new("asm");
    put(&mut d, 1, 0, rect(0.0, 0.0, 10.0, 20.0));
    push(&mut d, extrude(1, 1, "5 mm", BodyOp::New));
    d
}

pub(crate) fn comp(id: ComponentId, at: [&str; 3], grounded: bool) -> Component {
    let zero = || "0".to_string();
    Component { id, name: format!("c{id}"), body: 1, at: at.map(String::from), angles: [zero(), zero(), zero()], meta: Default::default(), grounded }
}

/// Parent frame at `parent_origin` along `parent_axis`; child frame at the child's origin along -Z.
pub(crate) fn joint(id: JointId, kind: JointKind, parent: ComponentId, child: ComponentId, parent_origin: [&str; 3], parent_axis: [&str; 3], value: &str) -> Joint {
    let parent_frame = Frame { origin: parent_origin.map(String::from), axis: parent_axis.map(String::from), face: None };
    Joint { id, kind, parent, child, parent_frame, child_frame: Frame::from_vectors(DVec3::ZERO, -DVec3::Z), value: value.into(), min: None, max: None, closes_loop: false, free: false }
}

pub(crate) fn ok(d: &mut Document, cmd: Command) -> Command {
    d.apply(cmd).unwrap()
}

fn strings(a: [&str; 3]) -> [String; 3] {
    a.map(String::from)
}

#[test]
fn frames_from_vectors_and_faces() {
    let f = Frame::from_vectors(DVec3::new(1.0 / 3.0, 0.0, -1e-9), DVec3::new(0.0, 0.6, -0.8));
    assert_eq!(f, Frame { origin: strings(["0.333 mm", "0 mm", "0 mm"]), axis: strings(["0", "0.6", "-0.8"]), face: None });
    assert_eq!(Frame::from_vectors(DVec3::ZERO, -DVec3::Z).axis, strings(["0", "0", "-1"]));
    let r = Rebuilder::default().rebuild(&boxed());
    let body = r.body(1).unwrap();
    let frames: Vec<Frame> = (0..body.faces.len()).filter_map(|i| Frame::from_face(body, i)).collect();
    assert_eq!(frames.len(), body.faces.len());
    let origin = |axis: [&str; 3]| frames.iter().find(|f| f.axis == strings(axis)).unwrap().origin.clone();
    assert_eq!(origin(["0", "0", "1"]), strings(["5 mm", "10 mm", "5 mm"]));
    assert_eq!(origin(["0", "0", "-1"]), strings(["5 mm", "10 mm", "0 mm"]));
    assert_eq!(origin(["1", "0", "0"]), strings(["10 mm", "10 mm", "2.5 mm"]));
}

#[test]
fn joint_commands_check_and_undo() {
    let mut d = boxed();
    ok(&mut d, Command::AddComponent(comp(1, O, true)));
    ok(&mut d, Command::AddComponent(comp(2, O, false)));
    ok(&mut d, Command::AddComponent(comp(3, O, false)));
    let j = |id, parent, child| joint(id, JointKind::Fixed, parent, child, O, Z, "0");
    // 1
    assert!(matches!(d.apply(Command::AddJoint(j(1, 1, 2))), Ok(Command::RemoveJoint(1))));
    assert!(matches!(d.apply(Command::AddJoint(j(1, 1, 2))), Err(DocError::JointExists(1))));
    // 2
    assert!(matches!(d.apply(Command::AddJoint(j(2, 1, 9))), Err(DocError::ComponentNotFound(9))));
    assert!(matches!(d.apply(Command::AddJoint(j(2, 2, 1))), Err(DocError::BadJoint("the child is grounded"))));
    assert!(matches!(d.apply(Command::AddJoint(j(2, 3, 2))), Err(DocError::JointLoop(2))));
    // 3
    assert!(d.apply(Command::AddJoint(Joint { value: "1 +".into(), ..j(2, 2, 3) })).is_err());
    assert!(d.apply(Command::AddJoint(j(2, 2, 3))).is_ok());
    assert_eq!(d.alloc_joint_id(), 3);
    // 4
    ok(&mut d, Command::SetComponent(comp(1, O, false)));
    assert!(matches!(d.apply(Command::AddJoint(j(3, 3, 1))), Err(DocError::JointLoop(1))));
    assert!(matches!(d.apply(Command::AddJoint(j(3, 1, 1))), Err(DocError::JointLoop(1))));
    // 5
    assert!(matches!(d.apply(Command::SetComponent(comp(3, O, true))), Err(DocError::BadComponent("a joint child cannot be grounded"))));
    assert!(matches!(d.apply(Command::RemoveComponent(2)), Err(DocError::ComponentInUse(2))));
    // 6
    let Command::SetJoint(old) = ok(&mut d, Command::SetJoint(joint(1, JointKind::Revolute, 1, 2, O, Z, "30 deg"))) else { panic!("not SetJoint") };
    assert_eq!(old.kind, JointKind::Fixed);
    assert_eq!(d.joint(1).unwrap().value, "30 deg");
    assert!(matches!(d.apply(Command::SetJoint(j(7, 1, 2))), Err(DocError::JointNotFound(7))));
    assert!(matches!(d.apply(Command::RemoveJoint(7)), Err(DocError::JointNotFound(7))));
    // 7
    assert!(matches!(d.apply(Command::RemoveJoint(2)), Ok(Command::AddJoint(_))));
    ok(&mut d, Command::RemoveJoint(1));
    ok(&mut d, Command::RemoveComponent(2));
    assert_eq!(d.joints().count(), 0);
    assert!(d.parent_joint(3).is_none());
}

#[test]
fn explode_command() {
    let mut d = boxed();
    ok(&mut d, Command::AddComponent(comp(1, O, false)));
    let step = ExplodeStep { component: 1, direction: strings(Z), distance: "30 mm".into() };
    assert!(d.apply(Command::SetExplode(vec![ExplodeStep { distance: "1 +".into(), ..step.clone() }])).is_err());
    assert!(d.explode_steps().is_empty());
    let Command::SetExplode(v) = ok(&mut d, Command::SetExplode(vec![step.clone()])) else { panic!("not SetExplode") };
    assert!(v.is_empty());
    assert_eq!(d.explode_steps(), std::slice::from_ref(&step));
    let Command::SetExplode(v) = ok(&mut d, Command::SetExplode(vec![])) else { panic!("not SetExplode") };
    assert_eq!(v, vec![step]);
}

/// Components 1 (grounded, at x 100), 2 and 3; revolute joint 1 (1 > 2), slider joint 2 (2 > 3), one explode step.
fn jointed() -> Document {
    let mut d = boxed();
    ok(&mut d, Command::AddComponent(comp(1, ["100 mm", "0", "0"], true)));
    ok(&mut d, Command::AddComponent(comp(2, O, false)));
    ok(&mut d, Command::AddComponent(comp(3, O, false)));
    ok(&mut d, Command::AddJoint(joint(1, JointKind::Revolute, 1, 2, ["10 mm", "0", "0"], Z, "45 deg")));
    ok(&mut d, Command::AddJoint(joint(2, JointKind::Slider, 2, 3, O, Z, "3 mm")));
    ok(&mut d, Command::SetExplode(vec![ExplodeStep { component: 3, direction: strings(Z), distance: "50 mm".into() }]));
    d
}

#[test]
fn joints_save_and_load() {
    let d = jointed();
    let p = std::env::temp_dir().join(format!("cadtest_{}_joints.cadproj", std::process::id()));
    d.save(&p).unwrap();
    let loaded = Document::load(&p);
    std::fs::remove_file(&p).ok();
    let mut loaded = loaded.unwrap();
    assert_eq!(loaded.joints().cloned().collect::<Vec<_>>(), d.joints().cloned().collect::<Vec<_>>());
    assert_eq!(loaded.components().cloned().collect::<Vec<_>>(), d.components().cloned().collect::<Vec<_>>());
    assert_eq!(loaded.explode_steps(), d.explode_steps());
    assert!(loaded.component(1).unwrap().grounded);
    assert_eq!(loaded.alloc_joint_id(), 3);
    assert_eq!(FORMAT_VERSION, 27);
}

#[test]
fn old_and_broken_joint_files() {
    let d = jointed();
    let p = std::env::temp_dir().join(format!("cadtest_{}_joints_old.cadproj", std::process::id()));
    let full = serde_json::to_value(&d).unwrap();
    let load = |v: &serde_json::Value| {
        std::fs::write(&p, serde_json::to_vec(v).unwrap()).unwrap();
        let r = Document::load(&p);
        std::fs::remove_file(&p).ok();
        r
    };
    // (a)
    let mut v = full.clone();
    let o = v.as_object_mut().unwrap();
    o.remove("joints");
    o.remove("next_joint_id");
    o.remove("explode");
    for c in v["components"].as_object_mut().unwrap().values_mut() {
        c.as_object_mut().unwrap().remove("grounded");
    }
    v["version"] = 20.into();
    let mut loaded = load(&v).unwrap();
    assert_eq!(loaded.joints().count(), 0);
    assert!(loaded.explode_steps().is_empty());
    assert!(!loaded.component(1).unwrap().grounded);
    assert_eq!(loaded.alloc_joint_id(), 1);
    // (b)
    let mut v = full.clone();
    v["components"].as_object_mut().unwrap().remove("3");
    assert!(matches!(load(&v), Err(DocError::ComponentNotFound(3))));
    // (c)
    let mut v = full;
    v["components"]["2"]["grounded"] = true.into();
    assert!(matches!(load(&v), Err(DocError::BadJoint("the child is grounded"))));
}

pub(crate) fn rebuilt(d: &Document) -> RebuildResult {
    Rebuilder::default().rebuild(d)
}

fn bbox(r: &RebuildResult, id: ComponentId) -> (DVec3, DVec3) {
    r.component(id).unwrap().bbox()
}

fn bbox_is(r: &RebuildResult, id: ComponentId, lo: [f64; 3], hi: [f64; 3]) {
    let (l, h) = bbox(r, id);
    assert!(l.distance(DVec3::from(lo)) <= 1e-9 && h.distance(DVec3::from(hi)) <= 1e-9, "component {id}: {l} {h}");
}

fn set_param(d: &mut Document, name: &str, expr: &str) {
    ok(d, Command::SetParam { name: name.into(), expr: Some(expr.into()) });
}

#[test]
fn fixed_joint_stacks_boxes() {
    let mut d = boxed();
    ok(&mut d, Command::AddComponent(comp(1, ["100 mm", "0", "0"], true)));
    ok(&mut d, Command::AddComponent(comp(2, O, false)));
    bbox_is(&rebuilt(&d), 2, [0.0, 0.0, 0.0], [10.0, 20.0, 5.0]);
    let j = Joint { child_frame: Frame::from_vectors(DVec3::new(5.0, 10.0, 0.0), -DVec3::Z), ..joint(1, JointKind::Fixed, 1, 2, ["5 mm", "10 mm", "5 mm"], Z, "0") };
    ok(&mut d, Command::AddJoint(j));
    let r = rebuilt(&d);
    bbox_is(&r, 2, [100.0, 0.0, 5.0], [110.0, 20.0, 10.0]);
    assert!((r.component(2).unwrap().volume() - 1000.0).abs() <= 1e-9);
    bbox_is(&r, 1, [100.0, 0.0, 0.0], [110.0, 20.0, 5.0]);
}

#[test]
fn revolute_joint_turns_the_subtree() {
    let mut d = boxed();
    set_param(&mut d, "ang", "0 deg");
    for c in [comp(1, O, true), comp(2, O, false), comp(3, O, false)] {
        ok(&mut d, Command::AddComponent(c));
    }
    ok(&mut d, Command::AddJoint(joint(1, JointKind::Revolute, 1, 2, ["10 mm", "0", "0"], Z, "ang")));
    ok(&mut d, Command::AddJoint(joint(2, JointKind::Fixed, 2, 3, ["0", "0", "5 mm"], Z, "0")));
    let mut rebuilder = Rebuilder::default();
    let r = rebuilder.rebuild(&d);
    bbox_is(&r, 2, [10.0, 0.0, 0.0], [20.0, 20.0, 5.0]);
    bbox_is(&r, 3, [10.0, 0.0, 5.0], [20.0, 20.0, 10.0]);
    set_param(&mut d, "ang", "90 deg");
    let r = rebuilder.rebuild(&d);
    bbox_is(&r, 2, [-10.0, 0.0, 0.0], [10.0, 10.0, 5.0]);
    bbox_is(&r, 3, [-10.0, 0.0, 5.0], [10.0, 10.0, 10.0]);
    assert_eq!(r.recomputed, 0);
    ok(&mut d, Command::SetJoint(joint(1, JointKind::Revolute, 1, 2, ["10 mm", "0", "0"], Z, "5 mm")));
    let r = rebuilder.rebuild(&d);
    let expected = "joint 1: expected an angle".to_string();
    assert_eq!(r.components[1].1.as_ref().err(), Some(&expected));
    assert_eq!(r.components[2].1.as_ref().err(), Some(&expected));
    assert!(r.components[0].1.is_ok());
}

#[test]
fn slider_joint_along_x() {
    let mut d = boxed();
    set_param(&mut d, "dx", "2 mm");
    ok(&mut d, Command::AddComponent(comp(1, O, true)));
    ok(&mut d, Command::AddComponent(comp(2, O, false)));
    let j = Joint { child_frame: Frame::from_vectors(DVec3::ZERO, -DVec3::X), ..joint(1, JointKind::Slider, 1, 2, ["10 mm", "0", "0"], ["1", "0", "0"], "dx") };
    ok(&mut d, Command::AddJoint(j.clone()));
    bbox_is(&rebuilt(&d), 2, [12.0, 0.0, 0.0], [22.0, 20.0, 5.0]);
    set_param(&mut d, "dx", "7 mm");
    bbox_is(&rebuilt(&d), 2, [17.0, 0.0, 0.0], [27.0, 20.0, 5.0]);
    ok(&mut d, Command::SetJoint(Joint { parent_frame: Frame { origin: strings(["10 mm", "0", "0"]), axis: strings(O), face: None }, ..j }));
    assert_eq!(rebuilt(&d).components[1].1.as_ref().err(), Some(&"joint 1: zero-length axis".to_string()));
}

#[test]
fn explode_offsets_run_in_steps() {
    let s = |component, direction: [&str; 3], distance: &str| ExplodeStep { component, direction: strings(direction), distance: distance.into() };
    let mut d = boxed();
    ok(&mut d, Command::AddComponent(comp(1, O, false)));
    ok(&mut d, Command::AddComponent(comp(2, O, false)));
    ok(&mut d, Command::SetExplode(vec![s(1, Z, "30 mm"), s(2, ["0", "3", "4"], "10 mm"), s(1, ["2", "0", "0"], "10 mm")]));
    let at = |factor: f64| {
        let o = d.explode_offsets(factor);
        assert_eq!(o.len(), 2);
        (o[&1], o[&2])
    };
    let near = |(a, b): (DVec3, DVec3), (c, e): (DVec3, DVec3)| assert!(a.distance(c) <= 1e-12 && b.distance(e) <= 1e-12, "{a} {b}");
    near(at(1.0), (DVec3::new(10.0, 0.0, 30.0), DVec3::new(0.0, 6.0, 8.0)));
    near(at(0.5), (DVec3::new(0.0, 0.0, 30.0), DVec3::new(0.0, 3.0, 4.0)));
    near(at(0.0), (DVec3::ZERO, DVec3::ZERO));
    ok(&mut d, Command::SetExplode(vec![s(1, Z, "30 mm"), s(2, O, "5 mm"), s(9, Z, "5 mm")]));
    let o = d.explode_offsets(1.0);
    assert_eq!(o.len(), 1);
    assert!(o[&1].distance(DVec3::new(0.0, 0.0, 30.0)) <= 1e-12);
    assert_eq!(d.explode_vector(&s(2, O, "5 mm")), Err("zero-length direction".to_string()));
    assert_eq!(d.explode_vector(&s(1, Z, "1 deg")), Err("expected a length".to_string()));
    bbox_is(&rebuilt(&d), 1, [0.0, 0.0, 0.0], [10.0, 20.0, 5.0]);
}

#[test]
fn frame_on_a_face_follows_the_part() {
    let mut d = Document::new("asm");
    set_param(&mut d, "h", "5 mm");
    put(&mut d, 1, 0, rect(0.0, 0.0, 10.0, 20.0));
    push(&mut d, extrude(1, 1, "h", BodyOp::New));
    ok(&mut d, Command::AddComponent(comp(1, O, true)));
    ok(&mut d, Command::AddComponent(comp(2, O, false)));
    let body = rebuilt(&d).body(1).unwrap().clone();
    let top = (0..body.faces.len()).find(|&i| Frame::from_face(&body, i).is_some_and(|f| f.axis == strings(Z))).unwrap();
    let parent_frame = Frame::from_face(&body, top).unwrap();
    assert!(matches!(parent_frame.face, Some(FaceRef { key: FaceKey::Plane(n), .. }) if n.distance(DVec3::Z) <= 1e-12));
    let j = Joint { parent_frame, child_frame: Frame::from_vectors(DVec3::new(5.0, 10.0, 0.0), -DVec3::Z), ..joint(1, JointKind::Fixed, 1, 2, O, Z, "0") };
    ok(&mut d, Command::AddJoint(j.clone()));
    bbox_is(&rebuilt(&d), 2, [0.0, 0.0, 5.0], [10.0, 20.0, 10.0]);
    set_param(&mut d, "h", "8 mm");
    bbox_is(&rebuilt(&d), 2, [0.0, 0.0, 8.0], [10.0, 20.0, 16.0]);
    ok(&mut d, Command::SetJoint(Joint { parent_frame: Frame { face: None, ..j.parent_frame.clone() }, ..j.clone() }));
    bbox_is(&rebuilt(&d), 2, [0.0, 0.0, 5.0], [10.0, 20.0, 13.0]);
    let lost = Frame { face: Some(FaceRef { key: FaceKey::Plane(DVec3::X), at: DVec3::new(100.0, 100.0, 100.0) }), ..j.parent_frame.clone() };
    ok(&mut d, Command::SetJoint(Joint { parent_frame: lost, ..j }));
    assert_eq!(rebuilt(&d).components[1].1.as_ref().err(), Some(&"joint 1: face not found; pick it again".to_string()));
}

#[test]
fn frame_face_serialisation() {
    assert!(!serde_json::to_string(&Frame::from_vectors(DVec3::ZERO, DVec3::Z)).unwrap().contains("\"face\""));
    let old: Frame = serde_json::from_str(r#"{"origin":["0","0","0"],"axis":["0","0","1"]}"#).unwrap();
    assert_eq!(old.face, None);
    let f = Frame { face: Some(FaceRef { key: FaceKey::Cylinder(DVec3::Z), at: DVec3::new(1.0, 2.0, 3.0) }), ..old };
    let back: Frame = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
    assert_eq!(back, f);
}

fn lim(j: Joint, min: Option<&str>, max: Option<&str>) -> Joint {
    Joint { min: min.map(String::from), max: max.map(String::from), ..j }
}

fn err(d: &Document, id: ComponentId) -> Option<String> {
    rebuilt(d).components.iter().find(|(i, _)| *i == id).unwrap().1.clone().err()
}

#[test]
fn revolute_limits_refuse_values_outside() {
    let mut d = boxed();
    ok(&mut d, Command::AddComponent(comp(1, O, true)));
    ok(&mut d, Command::AddComponent(comp(2, O, false)));
    let base = joint(1, JointKind::Revolute, 1, 2, ["10 mm", "0", "0"], Z, "60 deg");
    ok(&mut d, Command::AddJoint(lim(base.clone(), Some("-45 deg"), Some("45 deg"))));
    assert_eq!(err(&d, 2), Some("joint 1: 60 deg is above the maximum 45 deg".to_string()));
    let set = |d: &mut Document, value: &str, min: Option<&str>, max: Option<&str>| {
        ok(d, Command::SetJoint(lim(Joint { value: value.into(), ..base.clone() }, min, max)));
    };
    set(&mut d, "-60 deg", Some("-45 deg"), Some("45 deg"));
    assert_eq!(err(&d, 2), Some("joint 1: -60 deg is below the minimum -45 deg".to_string()));
    set(&mut d, "45 deg", Some("-45 deg"), Some("45 deg"));
    assert_eq!(err(&d, 2), None);
    bbox_is(&rebuilt(&d), 2, [-4.142135623730951, 0.0, 0.0], [17.071067811865476, 21.213203435596427, 5.0]);
    set(&mut d, "0 deg", Some("50 deg"), Some("45 deg"));
    assert_eq!(err(&d, 2), Some("joint 1: the minimum is above the maximum".to_string()));
    set(&mut d, "0 deg", Some("-45 deg"), Some("5 mm"));
    assert_eq!(err(&d, 2), Some("joint 1: expected an angle".to_string()));
    set(&mut d, "40 deg", None, Some("30"));
    assert_eq!(err(&d, 2), Some("joint 1: 40 deg is above the maximum 30 deg".to_string()));
}

#[test]
fn slider_limits_and_fixed_joints() {
    let mut d = boxed();
    for c in [comp(1, O, true), comp(2, O, false), comp(3, O, false)] {
        ok(&mut d, Command::AddComponent(c));
    }
    let s = Joint { child_frame: Frame::from_vectors(DVec3::ZERO, -DVec3::X), ..joint(1, JointKind::Slider, 1, 2, ["10 mm", "0", "0"], ["1", "0", "0"], "25 mm") };
    let s = lim(s, Some("0 mm"), Some("20 mm"));
    ok(&mut d, Command::AddJoint(s.clone()));
    assert_eq!(err(&d, 2), Some("joint 1: 25 mm is above the maximum 20 mm".to_string()));
    ok(&mut d, Command::SetJoint(Joint { value: "-1 mm".into(), ..s.clone() }));
    assert_eq!(err(&d, 2), Some("joint 1: -1 mm is below the minimum 0 mm".to_string()));
    ok(&mut d, Command::SetJoint(Joint { value: "20 mm".into(), ..s }));
    assert_eq!(err(&d, 2), None);
    // A fixed joint ignores its limits.
    let f = lim(joint(2, JointKind::Fixed, 2, 3, O, Z, "0"), Some("10 mm"), Some("5 mm"));
    ok(&mut d, Command::AddJoint(f));
    assert_eq!(err(&d, 3), None);
    assert!(d.apply(Command::SetJoint(lim(joint(2, JointKind::Fixed, 2, 3, O, Z, "0"), Some("1 +"), None))).is_err());
}

#[test]
fn joint_limits_save_and_load() {
    let mut d = jointed();
    let j1 = lim(d.joint(1).unwrap().clone(), Some("-90 deg"), Some("90 deg"));
    ok(&mut d, Command::SetJoint(j1));
    let p = std::env::temp_dir().join(format!("cadtest_{}_joint_limits.cadproj", std::process::id()));
    let load = |d: &Document, version: Option<u32>| {
        let mut v = serde_json::to_value(d).unwrap();
        if let Some(n) = version {
            v["version"] = n.into();
        }
        std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
        let r = Document::load(&p);
        std::fs::remove_file(&p).ok();
        r.unwrap()
    };
    d.save(&p).unwrap();
    let loaded = Document::load(&p).unwrap();
    std::fs::remove_file(&p).ok();
    assert_eq!(loaded.joint(1), d.joint(1));
    assert_eq!(loaded.joint(2).unwrap().min, None);
    assert!(!serde_json::to_string(d.joint(2).unwrap()).unwrap().contains("\"min\""));
    let old = load(&jointed(), Some(23));
    assert_eq!(old.joint(1).unwrap().max, None);
}

/// Components 1 (grounded), 2 and 3; fixed tree joints 1 (1 > 2) and 2 (2 > 3); joint 3 closes 1 > 3.
fn looped() -> Document {
    let mut d = boxed();
    for c in [comp(1, O, true), comp(2, O, false), comp(3, O, false)] {
        ok(&mut d, Command::AddComponent(c));
    }
    ok(&mut d, Command::AddJoint(joint(1, JointKind::Fixed, 1, 2, O, Z, "0")));
    ok(&mut d, Command::AddJoint(joint(2, JointKind::Fixed, 2, 3, O, Z, "0")));
    ok(&mut d, Command::AddJoint(Joint { closes_loop: true, ..joint(3, JointKind::Fixed, 1, 3, O, Z, "0") }));
    d
}

#[test]
fn closing_joints_skip_the_tree_checks() {
    let mut d = looped();
    assert_eq!(d.parent_joint(3).unwrap().id, 2);
    let fixed = |id, parent, child| joint(id, JointKind::Fixed, parent, child, O, Z, "0");
    assert!(matches!(d.apply(Command::AddJoint(Joint { closes_loop: true, ..fixed(4, 2, 2) })), Err(DocError::JointLoop(2))));
    assert!(matches!(d.apply(Command::AddJoint(fixed(4, 3, 1))), Err(DocError::BadJoint("the child is grounded"))));
    ok(&mut d, Command::AddJoint(Joint { closes_loop: true, ..fixed(4, 3, 1) }));
    assert!(matches!(d.apply(Command::SetJoint(fixed(3, 1, 3))), Err(DocError::JointLoop(3))));
}

#[test]
fn value_text_and_free() {
    let rev = joint(1, JointKind::Revolute, 1, 2, O, Z, "0");
    assert_eq!(rev.value_text(std::f64::consts::FRAC_PI_2), "90 deg");
    assert_eq!(joint(1, JointKind::Slider, 1, 2, O, Z, "0").value_text(12.5), "12.5 mm");
    assert!(Joint { free: true, ..rev.clone() }.is_free());
    assert!(!rev.is_free());
    assert!(!Joint { free: true, ..joint(1, JointKind::Fixed, 1, 2, O, Z, "0") }.is_free());
    assert!(!Joint { free: true, closes_loop: true, ..rev }.is_free());
}

#[test]
fn loop_flags_save_and_load() {
    let mut d = looped();
    let j2 = Joint { free: true, kind: JointKind::Revolute, ..d.joint(2).unwrap().clone() };
    ok(&mut d, Command::SetJoint(j2));
    let p = std::env::temp_dir().join(format!("cadtest_{}_joint_loops.cadproj", std::process::id()));
    d.save(&p).unwrap();
    let loaded = Document::load(&p);
    std::fs::remove_file(&p).ok();
    assert_eq!(loaded.unwrap().joints().cloned().collect::<Vec<_>>(), d.joints().cloned().collect::<Vec<_>>());
    let text = |id| serde_json::to_string(d.joint(id).unwrap()).unwrap();
    assert!(!text(1).contains("closes_loop") && !text(1).contains("\"free\""));
    assert!(text(3).contains("\"closes_loop\":true"));
    assert!(text(2).contains("\"free\":true"));
    let mut v = serde_json::to_value(jointed()).unwrap();
    v["version"] = 24.into();
    std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
    let old = Document::load(&p);
    std::fs::remove_file(&p).ok();
    let old = old.unwrap();
    assert!(!old.joint(1).unwrap().closes_loop && !old.joint(1).unwrap().free);
}
