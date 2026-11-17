use crate::fixtures::{box_body, plate_with_hole};
use glam::DVec3;
use std::f64::consts::PI;

fn area(t: &[[DVec3; 3]]) -> f64 {
    t.iter().map(|[a, b, c]| 0.5 * (b - a).cross(c - a).length()).sum()
}

#[test]
fn box_cut_is_full_rectangle() {
    let cap = box_body([0.0; 3], [40.0, 20.0, 10.0]).section_cap(2, 5.0);
    assert!((area(&cap) - 800.0).abs() < 1e-9);
    assert!(cap.iter().flatten().all(|p| (p.z - 5.0).abs() < 1e-12));
}

#[test]
fn hole_is_left_open() {
    let a = area(&plate_with_hole(5.0).section_cap(2, 5.0));
    assert!((a - (800.0 - PI * 25.0)).abs() < 0.05, "{a}");
}

#[test]
fn cut_through_hole_gives_two_pieces() {
    let a = area(&plate_with_hole(5.0).section_cap(0, 20.0));
    assert!((a - 100.0).abs() < 0.05, "{a}");
}

#[test]
fn plane_outside_is_empty() {
    assert!(box_body([0.0; 3], [40.0, 20.0, 10.0]).section_cap(2, 50.0).is_empty());
}
