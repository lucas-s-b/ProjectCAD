use super::*;
use eframe::egui::pos2;

#[test]
fn axis_param_finds_closest_point() {
    let s = axis_param(DVec3::ZERO, DVec3::X, DVec3::new(5.0, -10.0, 3.0), DVec3::Y).unwrap();
    assert!((s - 5.0).abs() < 1e-12);
    assert_eq!(axis_param(DVec3::ZERO, DVec3::X, DVec3::new(5.0, -10.0, 3.0), DVec3::X), None);
}

#[test]
fn hit_picks_nearest_arrow() {
    let tips = [pos2(160.0, 100.0), pos2(140.0, 60.0), pos2(100.0, 40.0)];
    assert_eq!(hit(pos2(100.0, 100.0), tips, pos2(130.0, 103.0), 8.0), Some(0));
    assert_eq!(hit(pos2(100.0, 100.0), tips, pos2(300.0, 300.0), 8.0), None);
}

#[test]
fn delta_is_along_axis() {
    assert_eq!(GizmoDrag { kind: GizmoKind::Move, axis: 2, start: 1.0, now: 4.5, snap: false }.delta(), DVec3::new(0.0, 0.0, 3.5));
}

fn near3(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-12, "{a} vs {b}");
}

#[test]
fn ring_angle_about_z() {
    let a = ring_angle(DVec3::ZERO, DVec3::Z, DVec3::X, DVec3::new(10.0, 10.0, 50.0), -DVec3::Z).unwrap();
    assert!((a - PI / 4.0).abs() < 1e-12);
    assert_eq!(ring_angle(DVec3::ZERO, DVec3::Z, DVec3::X, DVec3::new(10.0, 10.0, 50.0), DVec3::X), None);
    assert_eq!(ring_angle(DVec3::ZERO, DVec3::Z, DVec3::X, DVec3::new(10.0, 10.0, -50.0), -DVec3::Z), None);
    let p = ring_point(DVec3::ZERO, DVec3::X, DVec3::Y, 2.0, 1.0) + DVec3::X * 5.0;
    let a = ring_angle(DVec3::ZERO, DVec3::X, RING_U[0], p, -DVec3::X).unwrap();
    assert!((a - 1.0).abs() < 1e-12);
}

#[test]
fn ring_point_turns_counter_clockwise() {
    near3(ring_point(DVec3::ZERO, DVec3::Z, DVec3::X, 2.0, std::f64::consts::FRAC_PI_2), DVec3::new(0.0, 2.0, 0.0));
    near3(ring_point(DVec3::ZERO, DVec3::X, RING_U[0], 1.0, std::f64::consts::FRAC_PI_2), DVec3::new(0.0, 0.0, 1.0));
}

#[test]
fn turn_snaps_to_15_degrees() {
    let turn = |now: f64, snap: bool| GizmoDrag { kind: GizmoKind::Turn, axis: 2, start: 0.0, now, snap }.angle();
    assert!((turn(0.3, true) - 15f64.to_radians()).abs() < 1e-12);
    assert!((turn(0.5, true) - 30f64.to_radians()).abs() < 1e-12);
    assert_eq!(turn(0.5, false), 0.5);
}

#[test]
fn turn_motion_keeps_the_centre() {
    let g = GizmoDrag { kind: GizmoKind::Turn, axis: 2, start: 0.0, now: std::f64::consts::FRAC_PI_2, snap: false };
    let c = DVec3::new(10.0, 0.0, 0.0);
    near3(g.motion(c).transform_point3(DVec3::new(20.0, 0.0, 0.0)), DVec3::new(10.0, 10.0, 0.0));
    near3(g.motion(c).transform_point3(c), c);
    assert_eq!(g.delta(), DVec3::ZERO);
}

#[test]
fn unwrapped_turn_keeps_counting() {
    let now = 3.0;
    assert!((now + wrap_pi(-3.0 - now) - (TAU - 3.0)).abs() < 1e-12);
}

#[test]
fn hit_ring_picks_the_nearest() {
    let square = |s: f32| vec![pos2(100.0, 100.0), pos2(s, 100.0), pos2(s, s), pos2(100.0, s)];
    let rings = [square(200.0), square(300.0)];
    assert_eq!(hit_ring(&rings, pos2(150.0, 203.0), 8.0), Some(0));
    assert_eq!(hit_ring(&rings, pos2(250.0, 296.0), 8.0), Some(1));
    assert_eq!(hit_ring(&rings, pos2(150.0, 150.0), 8.0), None);
}

#[test]
fn joint_drag_shows_no_component_motion() {
    let g = |now: f64, snap: bool| GizmoDrag { kind: GizmoKind::Joint { joint: 1, v0: 0.5 }, axis: 0, start: 0.0, now, snap };
    assert_eq!(g(1.0, false).delta(), DVec3::ZERO);
    assert_eq!(g(1.0, false).motion(DVec3::ONE), DAffine3::IDENTITY);
    assert_eq!(g(1.0, false).angle(), 1.0);
    assert!((g(0.3, true).angle() - 15f64.to_radians()).abs() < 1e-12);
}
