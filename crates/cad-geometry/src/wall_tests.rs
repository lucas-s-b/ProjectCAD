use super::*;
use crate::shape::{GeomError, Shape};
use crate::snap::{SnapKind, snap_points};
use crate::xform::mirror_across;
use crate::{Tolerance, distance_2d};

fn wall(align: WallAlign) -> Wall {
    Wall { a: DVec2::ZERO, b: DVec2::new(4000.0, 0.0), thickness: 200.0, height: 2500.0, align }
}

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

#[test]
fn faces_follow_alignment() {
    assert_eq!(wall(WallAlign::Center).faces(), (100.0, -100.0));
    assert_eq!(wall(WallAlign::Left).faces(), (200.0, 0.0));
    assert_eq!(wall(WallAlign::Right).faces(), (0.0, -200.0));
}

#[test]
fn corners_are_counter_clockwise() {
    assert_eq!(wall(WallAlign::Center).corners(), [v(0.0, -100.0), v(4000.0, -100.0), v(4000.0, 100.0), v(0.0, 100.0)]);
    assert_eq!(wall(WallAlign::Left).corners(), [v(0.0, 0.0), v(4000.0, 0.0), v(4000.0, 200.0), v(0.0, 200.0)]);
    assert_eq!(wall(WallAlign::Right).corners(), [v(0.0, -200.0), v(4000.0, -200.0), v(4000.0, 0.0), v(0.0, 0.0)]);
}

#[test]
fn validate_refuses_bad_walls() {
    let tol = Tolerance::default();
    let mut w = wall(WallAlign::Center);
    w.b = w.a;
    assert_eq!(Shape::Wall(w).validate(&tol), Err(GeomError::Degenerate("wall has zero length")));
    let mut w = wall(WallAlign::Center);
    w.thickness = 0.0;
    assert_eq!(Shape::Wall(w).validate(&tol), Err(GeomError::Degenerate("wall thickness and height must be positive")));
    let mut w = wall(WallAlign::Center);
    w.a.x = f64::NAN;
    assert_eq!(Shape::Wall(w).validate(&tol), Err(GeomError::NonFinite));
}

#[test]
fn mirror_swaps_the_side() {
    let w = Wall { a: v(0.0, 0.0), b: v(10.0, 0.0), thickness: 2.0, height: 2500.0, align: WallAlign::Left };
    let Shape::Wall(m) = Shape::Wall(w).transformed(&mirror_across(DVec2::ZERO, DVec2::Y)).unwrap() else { panic!() };
    assert!(m.a.distance(v(0.0, 0.0)) < 1e-12 && m.b.distance(v(-10.0, 0.0)) < 1e-12);
    assert_eq!(m.align, WallAlign::Right);
    let want = [v(0.0, 2.0), v(-10.0, 2.0), v(-10.0, 0.0), v(0.0, 0.0)];
    for (c, w) in m.corners().iter().zip(want) {
        assert!(c.distance(w) < 1e-12, "{c:?} vs {w:?}");
    }
}

#[test]
fn snaps_and_picks() {
    let s = Shape::Wall(wall(WallAlign::Center));
    let snaps = snap_points(&s);
    assert!(snaps.iter().any(|p| p.kind == SnapKind::Midpoint && p.point.distance(v(2000.0, 0.0)) < 1e-9));
    assert!(snaps.iter().any(|p| p.kind == SnapKind::Endpoint && p.point.distance(v(0.0, 0.0)) < 1e-9));
    assert!(snaps.iter().any(|p| p.kind == SnapKind::Endpoint && p.point.distance(v(4000.0, 0.0)) < 1e-9));
    assert!(distance_2d(&s, v(2000.0, 50.0)).abs() < 1e-9);
    assert!((distance_2d(&s, v(2000.0, 300.0)) - 200.0).abs() < 1e-9);
}
