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
    assert_eq!(GizmoDrag { axis: 2, start: 1.0, now: 4.5 }.delta(), DVec3::new(0.0, 0.0, 3.5));
}
