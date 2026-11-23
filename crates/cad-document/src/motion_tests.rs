use crate::joint::tests::{boxed, comp, joint, ok};
use crate::*;
use cad_geometry::DVec3;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_3, FRAC_PI_6};

const O: [&str; 3] = ["0", "0", "0"];
const Z: [&str; 3] = ["0", "0", "1"];

/// Body 1 the box (0,0,0)-(10,20,5); component 1 grounded at the origin; component 2 on revolute joint 1 about Z
/// through (10, 0, 0) at 0 deg (it lies at x 10..20); component 3 fixed on component 2 by joint 2, 5 mm up.
fn arm() -> Document {
    let mut d = boxed();
    for c in [comp(1, O, true), comp(2, O, false), comp(3, O, false)] {
        ok(&mut d, Command::AddComponent(c));
    }
    ok(&mut d, Command::AddJoint(joint(1, JointKind::Revolute, 1, 2, ["10 mm", "0", "0"], Z, "0 deg")));
    ok(&mut d, Command::AddJoint(joint(2, JointKind::Fixed, 2, 3, ["0", "0", "5 mm"], Z, "0")));
    d
}

fn study(joint: JointId, start: &str, end: &str, steps: usize, trace: ComponentId, point: [&str; 3]) -> MotionStudy {
    MotionStudy { joint, start: start.into(), end: end.into(), steps, trace, point: point.map(String::from) }
}

fn near(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} vs {b}");
}

fn near3(a: DVec3, b: [f64; 3], t: f64) {
    assert!(a.distance(DVec3::from(b)) <= t, "{a} vs {b:?}");
}

#[test]
fn revolute_drive_samples_and_traces() {
    let d = arm();
    let r = Rebuilder::default().rebuild(&d);
    let m = d.drive(&r, &study(1, "0", "90 deg", 3, 2, ["10 mm", "0", "0"])).unwrap();
    assert_eq!(m.joint, 1);
    assert!(m.angle);
    assert_eq!(m.moving, vec![2, 3]);
    assert_eq!(m.values.len(), 4);
    for (v, e) in m.values.iter().zip([0.0, FRAC_PI_6, FRAC_PI_3, FRAC_PI_2]) {
        near(*v, e, 1e-12);
    }
    let expected = [[20.0, 0.0, 0.0], [18.660254037844387, 5.0, 0.0], [15.0, 8.660254037844386, 0.0], [10.0, 10.0, 0.0]];
    for (p, e) in m.trace.iter().zip(expected) {
        near3(*p, e, 1e-9);
    }
    near3(m.moves[0][&2].transform_point3(DVec3::new(1.0, 2.0, 3.0)), [1.0, 2.0, 3.0], 1e-9);
    near3(m.moves[3][&3].transform_point3(DVec3::new(20.0, 0.0, 5.0)), [10.0, 10.0, 5.0], 1e-9);
}

#[test]
fn first_collision_of_the_arm() {
    let d = arm();
    let r = Rebuilder::default().rebuild(&d);
    let m = d.drive(&r, &study(1, "0", "90 deg", 3, 2, ["10 mm", "0", "0"])).unwrap();
    let c = d.first_collision(&r, &m).unwrap().unwrap();
    assert_eq!((c.sample, c.moving, c.other), (1, 2, 1));
    near(c.volume, 535.8983848622456, 1e-6);
    near(c.region.volume(), 535.8983848622456, 1e-6);
    let m = d.drive(&r, &study(1, "0", "-90 deg", 3, 2, O)).unwrap();
    assert_eq!(d.first_collision(&r, &m), Ok(None));
}

#[test]
fn slider_drive_and_limits() {
    let mut d = boxed();
    ok(&mut d, Command::AddComponent(comp(1, O, true)));
    ok(&mut d, Command::AddComponent(comp(2, O, false)));
    let s = Joint {
        child_frame: Frame::from_vectors(DVec3::ZERO, -DVec3::X),
        min: Some("0 mm".into()),
        max: Some("20 mm".into()),
        ..joint(1, JointKind::Slider, 1, 2, ["10 mm", "0", "0"], ["1", "0", "0"], "0 mm")
    };
    ok(&mut d, Command::AddJoint(s.clone()));
    let r = Rebuilder::default().rebuild(&d);
    let m = d.drive(&r, &study(1, "0", "20 mm", 4, 2, O)).unwrap();
    assert!(!m.angle);
    assert_eq!(m.moving, vec![2]);
    for (k, (v, p)) in m.values.iter().zip(&m.trace).enumerate() {
        near(*v, 5.0 * k as f64, 1e-12);
        near3(*p, [10.0 + 5.0 * k as f64, 0.0, 0.0], 1e-9);
    }
    assert_eq!(d.first_collision(&r, &m), Ok(None));
    assert_eq!(d.drive(&r, &study(1, "0", "30 mm", 4, 2, O)).err(), Some("end: 30 mm is above the maximum 20 mm".to_string()));
    assert_eq!(d.drive(&r, &study(1, "-5 mm", "10 mm", 4, 2, O)).err(), Some("start: -5 mm is below the minimum 0 mm".to_string()));
    ok(&mut d, Command::SetJoint(Joint { min: None, max: None, ..s }));
    let r = Rebuilder::default().rebuild(&d);
    let m = d.drive(&r, &study(1, "0", "-5 mm", 2, 2, O)).unwrap();
    let c = d.first_collision(&r, &m).unwrap().unwrap();
    assert_eq!((c.sample, c.moving, c.other), (1, 2, 1));
    near(c.volume, 250.0, 1e-6);
}

#[test]
fn drive_input_errors() {
    let d = arm();
    let r = Rebuilder::default().rebuild(&d);
    let err = |s: MotionStudy| d.drive(&r, &s).err();
    let say = |s: &str| Some(s.to_string());
    assert_eq!(err(study(9, "0", "90", 3, 2, O)), say("joint 9 not found"));
    assert_eq!(err(study(2, "0", "90", 3, 2, O)), say("a fixed joint cannot be driven"));
    assert_eq!(err(study(1, "0", "90", 0, 2, O)), say("steps must be 1 to 360"));
    assert_eq!(err(study(1, "0", "90", 361, 2, O)), say("steps must be 1 to 360"));
    assert_eq!(err(study(1, "5 mm", "90", 3, 2, O)), say("start: expected an angle"));
    assert_eq!(err(study(1, "0", "90", 3, 2, ["1 deg", "0", "0"])), say("trace point: expected a length"));
    assert_eq!(err(study(1, "0", "90", 3, 9, O)), say("step 0: component 9 not found"));
    assert_eq!(MotionStudy::default().steps, 36);
}

#[test]
fn csv_of_the_arm() {
    let mut d = arm();
    let r = Rebuilder::default().rebuild(&d);
    let m = d.drive(&r, &study(1, "0", "90 deg", 3, 2, ["10 mm", "0", "0"])).unwrap();
    let c = d.first_collision(&r, &m).unwrap().unwrap();
    let csv = d.motion_csv(&m, Some(&c));
    assert_eq!(
        csv.lines().collect::<Vec<_>>(),
        [
            "\u{feff}Step,Value (deg),X (mm),Y (mm),Z (mm),Collision",
            "0,0.000000,20.000000,0.000000,0.000000,",
            "1,30.000000,18.660254,5.000000,0.000000,c2 / c1 interference 535.898 mm³",
            "2,60.000000,15.000000,8.660254,0.000000,",
            "3,90.000000,10.000000,10.000000,0.000000,",
        ]
    );
    let renamed = Component { name: "arm, left".into(), ..d.component(2).unwrap().clone() };
    ok(&mut d, Command::SetComponent(renamed));
    let csv = d.motion_csv(&m, Some(&c));
    assert_eq!(csv.lines().nth(2), Some("1,30.000000,18.660254,5.000000,0.000000,\"arm, left / c1 interference 535.898 mm³\""));
    assert!(d.motion_csv(&m, None).lines().nth(2).unwrap().ends_with("0.000000,"));
}
