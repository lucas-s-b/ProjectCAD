use super::closure;
use crate::joint::tests::{boxed, comp, joint, ok, rebuilt};
use crate::*;
use cad_geometry::{DAffine3, DVec3};
use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, PI};

const O: [&str; 3] = ["0", "0", "0"];
const Z: [&str; 3] = ["0", "0", "1"];

/// Four-bar linkage on body 1 (only the joint frames matter): ground #1 (grounded); crank #2, a = 20 mm (joint 1,
/// revolute about Z at the origin, the input, 0 deg); coupler #3, b = 60 mm (joint 2 at (20, 0, 0) on the crank, free,
/// start 40 deg); rocker #4, c = 40 mm (joint 3 at (50, 0, 0) on the ground, so d = 50 mm, free, start 60 deg);
/// joint 4 closes the loop: (60, 0, 0) on the coupler meets (40, 0, 0) on the rocker. Child frames point along -Z, so
/// each mate is translate(parent origin) · Rz(value) · translate(-child origin) and everything stays in z = 0: joint 3's
/// value is the rocker angle θ4, joint 2's is θ3 − θ2.
pub(crate) fn four_bar() -> Document {
    let mut d = boxed();
    for c in [comp(1, O, true), comp(2, O, false), comp(3, O, false), comp(4, O, false)] {
        ok(&mut d, Command::AddComponent(c));
    }
    let free = |j: Joint| Joint { free: true, ..j };
    ok(&mut d, Command::AddJoint(joint(1, JointKind::Revolute, 1, 2, O, Z, "0 deg")));
    ok(&mut d, Command::AddJoint(free(joint(2, JointKind::Revolute, 2, 3, ["20 mm", "0", "0"], Z, "40 deg"))));
    ok(&mut d, Command::AddJoint(free(joint(3, JointKind::Revolute, 1, 4, ["50 mm", "0", "0"], Z, "60 deg"))));
    let close = Joint { child_frame: Frame::from_vectors(DVec3::new(40.0, 0.0, 0.0), -DVec3::Z), closes_loop: true, ..joint(4, JointKind::Revolute, 3, 4, ["60 mm", "0", "0"], Z, "0") };
    ok(&mut d, Command::AddJoint(close));
    d
}

/// The placement error of component `id` in the rebuild.
pub(crate) fn err(d: &Document, id: ComponentId) -> Option<String> {
    rebuilt(d).components.iter().find(|(i, _)| *i == id).unwrap().1.clone().err()
}

fn rows(m: DAffine3, kind: JointKind) -> Vec<f64> {
    let mut out = Vec::new();
    closure(kind, m, &mut out);
    out
}

fn same(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (a, b) in got.iter().zip(want) {
        assert!((a - b).abs() <= 1e-12, "{got:?} vs {want:?}");
    }
}

fn near3(p: DVec3, want: [f64; 3], tol: f64) {
    assert!(p.distance(DVec3::from_array(want)) <= tol, "{p:?} vs {want:?}");
}

fn set(d: &mut Document, f: impl FnOnce(Joint) -> Joint, id: JointId) {
    let j = f(d.joint(id).unwrap().clone());
    ok(d, Command::SetJoint(j));
}

#[test]
fn closure_rows_by_kind() {
    let id = DAffine3::IDENTITY;
    same(&rows(id, JointKind::Revolute), &[0.0; 6]);
    same(&rows(id, JointKind::Slider), &[0.0; 8]);
    same(&rows(id, JointKind::Fixed), &[0.0; 9]);
    let moved = DAffine3::from_translation(DVec3::new(1.0, 2.0, 3.0)) * DAffine3::from_rotation_z(0.3);
    same(&rows(moved, JointKind::Revolute), &[1.0, 2.0, 3.0, 0.0, 0.0, 0.0]);
    same(&rows(DAffine3::from_rotation_x(PI), JointKind::Revolute), &[0.0, 0.0, 0.0, 0.0, 0.0, -2.0]);
    same(&rows(DAffine3::from_translation(DVec3::new(1.0, 0.0, 5.0)), JointKind::Slider), &[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    same(&rows(DAffine3::from_rotation_z(0.3), JointKind::Slider), &[0.0, 0.0, -0.04466351087439402, 0.29552020666133955, 0.0, 0.0, 0.0, 0.0]);
    same(&rows(DAffine3::from_rotation_x(FRAC_PI_2), JointKind::Fixed), &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, -1.0]);
}

#[test]
fn four_bar_closes_from_the_start_guess() {
    let d = four_bar();
    let r = rebuilt(&d);
    assert_eq!(r.joint_values.keys().copied().collect::<Vec<_>>(), vec![2, 3]);
    assert!((r.joint_values[&2].to_degrees() - 36.33605751461394).abs() <= 1e-6);
    assert!((r.joint_values[&3].to_degrees() - 62.72038726402191).abs() <= 1e-6);
    let placed = d.placements(&r.bodies);
    let c = [68.33333333333333, 35.551215012835904, 0.0];
    near3(placed[&4].as_ref().unwrap().transform_point3(DVec3::new(40.0, 0.0, 0.0)), c, 1e-6);
    near3(placed[&3].as_ref().unwrap().transform_point3(DVec3::new(60.0, 0.0, 0.0)), c, 1e-6);
    for c in 1..=4 {
        assert_eq!(err(&d, c), None);
    }
    assert_eq!(d.joint_value(&r, d.joint(3).unwrap()), Ok(r.joint_values[&3]));
    assert_eq!(d.joint_value(&r, d.joint(1).unwrap()), Ok(0.0));
}

#[test]
fn loop_errors() {
    // Over-constrained: no free joint left, the start values do not close the loop.
    let mut d = four_bar();
    for id in [2, 3] {
        set(&mut d, |j| Joint { free: false, ..j }, id);
    }
    assert_eq!(err(&d, 4), Some("joint 4: over-constrained: no free joint value can close the loop (residual 5.603e0)".to_string()));
    assert_eq!(err(&d, 3), None);
    // Under-constrained: the crank free too (three unknowns, rank 2).
    let mut d = four_bar();
    set(&mut d, |j| Joint { free: true, ..j }, 1);
    for c in [2, 3, 4] {
        assert_eq!(err(&d, c), Some("joint 4: under-constrained (degrees of freedom left: 1); mark fewer joints free".to_string()), "component {c}");
    }
    assert!(rebuilt(&d).joint_values.is_empty());
    // Not closing: the rocker pivot is out of reach.
    let mut d = four_bar();
    set(&mut d, |j| Joint { parent_frame: Frame { origin: ["200 mm", "0", "0"].map(String::from), ..j.parent_frame.clone() }, ..j }, 3);
    let e4 = err(&d, 4).unwrap();
    assert!(e4.starts_with("joint 4: the loop does not close (residual "), "{e4}");
    assert!(e4.ends_with("); check the free joints and their start values"), "{e4}");
    // Free joints without a loop.
    let mut d = four_bar();
    ok(&mut d, Command::RemoveJoint(4));
    let under = Some("joint 2: under-constrained (degrees of freedom left: 2); mark fewer joints free".to_string());
    assert_eq!(err(&d, 3), under);
    assert_eq!(err(&d, 4), under);
    assert_eq!(err(&d, 2), None);
    // Limits refuse the solved value.
    let mut d = four_bar();
    set(&mut d, |j| Joint { max: Some("60 deg".into()), ..j }, 3);
    assert_eq!(err(&d, 4), Some("joint 3: 62.72 deg is above the maximum 60 deg".to_string()));
    assert_eq!(err(&d, 3), None);
    // Drive replaces an unknown: joint 3 alone cannot meet two in-plane equations.
    let d = four_bar();
    let r = rebuilt(&d);
    let e = d.solve_joints(&r.bodies, Some((2, 0.0)), &BTreeMap::new()).err().unwrap();
    assert!(e.starts_with("joint 4: the loop does not close"), "{e}");
}

#[test]
fn closing_joint_checks_on_the_four_bar() {
    let mut d = four_bar();
    assert_eq!(d.parent_joint(4).unwrap().id, 3);
    let j = Joint { closes_loop: false, ..d.joint(4).unwrap().clone() };
    assert!(matches!(d.apply(Command::SetJoint(j)), Err(DocError::JointLoop(4))));
}
