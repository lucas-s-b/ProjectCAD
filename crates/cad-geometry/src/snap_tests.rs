use super::*;
use std::f64::consts::PI;

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn line(a: (f64, f64), b: (f64, f64)) -> Shape {
    Shape::Line { a: v2(a.0, a.1), b: v2(b.0, b.1) }
}
fn circle(x: f64, y: f64, r: f64) -> Shape {
    Shape::Circle { center: v2(x, y), radius: r }
}

#[test]
fn snap_point_counts() {
    assert_eq!(snap_points(&line((0.0, 0.0), (2.0, 0.0))).len(), 3);
    assert_eq!(snap_points(&Shape::rect(v2(0.0, 0.0), v2(2.0, 2.0))).len(), 9);
    assert_eq!(snap_points(&circle(0.0, 0.0, 1.0)).len(), 5);
    assert_eq!(snap_points(&Shape::Arc { center: v2(0.0, 0.0), radius: 1.0, start: 0.0, sweep: 1.0 }).len(), 4);
    let pl = |closed| Shape::Polyline { points: vec![v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, 1.0)], closed };
    assert_eq!(snap_points(&pl(false)).len(), 5);
    assert_eq!(snap_points(&pl(true)).len(), 6);
    let b = Shape::Box { center: glam::DVec3::new(1.0, 2.0, 3.0), size: glam::DVec3::ONE };
    assert_eq!(snap_points(&b), vec![Snap { point: v2(1.0, 2.0), kind: SnapKind::Center }]);
}

#[test]
fn segment_intersections() {
    let p = intersections(&line((0.0, 0.0), (2.0, 2.0)), &line((0.0, 2.0), (2.0, 0.0)));
    assert_eq!(p.len(), 1);
    assert!(p[0].distance(v2(1.0, 1.0)) < 1e-9);
    assert!(intersections(&line((0.0, 0.0), (1.0, 0.0)), &line((0.0, 1.0), (1.0, 1.0))).is_empty());
    assert!(intersections(&line((0.0, 0.0), (1.0, 0.0)), &line((2.0, -1.0), (2.0, 1.0))).is_empty());
}

#[test]
fn line_circle() {
    let c = circle(0.0, 0.0, 1.0);
    assert_eq!(intersections(&line((-2.0, 0.0), (2.0, 0.0)), &c).len(), 2);
    assert_eq!(intersections(&line((-2.0, 1.0), (2.0, 1.0)), &c).len(), 1);
    assert!(intersections(&line((-2.0, 3.0), (2.0, 3.0)), &c).is_empty());
    let half = Shape::Arc { center: v2(0.0, 0.0), radius: 1.0, start: 0.0, sweep: PI };
    assert_eq!(intersections(&line((-2.0, 0.5), (2.0, 0.5)), &half).len(), 2);
    assert_eq!(intersections(&line((0.0, -2.0), (0.0, 2.0)), &half).len(), 1);
}

#[test]
fn circle_circle_and_rect() {
    assert_eq!(intersections(&circle(0.0, 0.0, 1.0), &circle(1.0, 0.0, 1.0)).len(), 2);
    assert!(intersections(&circle(0.0, 0.0, 1.0), &circle(0.0, 0.0, 2.0)).is_empty());
    let r = Shape::rect(v2(0.0, 0.0), v2(2.0, 2.0));
    assert_eq!(intersections(&r, &line((-1.0, 1.0), (3.0, 1.0))).len(), 2);
}

#[test]
fn object_snap_cases() {
    let l = line((0.0, 0.0), (10.0, 0.0));
    assert_eq!(object_snap(v2(0.1, 0.1), 1.0, None, [&l]), Some(Snap { point: v2(0.0, 0.0), kind: SnapKind::Endpoint }));
    assert_eq!(object_snap(v2(5.0, 5.0), 1.0, None, [&l]), None);
    let m = line((5.0, -5.0), (5.0, 5.0));
    let hit = object_snap(v2(5.2, 0.2), 1.0, None, [&l, &m]).unwrap();
    assert_eq!(hit.kind, SnapKind::Intersection);
    assert!(hit.point.distance(v2(5.0, 0.0)) < 1e-9);
    // line (0,0)-(2,0): Midpoint (1,0) and Endpoint (1,0) of a second line tie
    let a = line((0.0, 0.0), (2.0, 0.0));
    let b = line((1.0, 3.0), (1.0, 0.0));
    let pick = object_snap(v2(1.0, 0.0), 0.5, None, [&a, &b]).unwrap();
    assert_eq!(pick.kind, SnapKind::Endpoint);
}

#[test]
fn grid_rounds() {
    let g = grid_snap(v2(3.4, 7.6), 5.0);
    assert_eq!(g, Snap { point: v2(5.0, 10.0), kind: SnapKind::Grid });
}

#[test]
fn perpendicular_foot_on_segment() {
    let l = line((0.0, 0.0), (20.0, 0.0));
    let hit = object_snap(v2(5.2, 0.3), 1.0, Some(v2(5.0, 10.0)), [&l]).unwrap();
    assert_eq!(hit.kind, SnapKind::Perpendicular);
    assert!(hit.point.distance(v2(5.0, 0.0)) < 1e-9);
    // foot (30, 0) lies beyond the segment end
    assert_eq!(object_snap(v2(30.0, 0.2), 1.0, Some(v2(30.0, 10.0)), [&l]), None);
    // the same cursor without `from` finds nothing either
    assert_eq!(object_snap(v2(5.2, 0.3), 1.0, None, [&l]), None);
}

#[test]
fn perpendicular_points_on_circle() {
    let c = circle(0.0, 0.0, 5.0);
    let r = 5.0 / 2f64.sqrt();
    let from = Some(v2(10.0, 10.0));
    let hit = object_snap(v2(r + 0.1, r), 1.0, from, [&c]).unwrap();
    assert_eq!(hit.kind, SnapKind::Perpendicular);
    assert!(hit.point.distance(v2(r, r)) < 1e-9);
    let far = object_snap(v2(-r, -r + 0.1), 1.0, from, [&c]).unwrap();
    assert!(far.kind == SnapKind::Perpendicular && far.point.distance(v2(-r, -r)) < 1e-9);
    // from the centre there is no perpendicular
    let centre = object_snap(v2(r, r), 0.5, Some(v2(0.0, 0.0)), [&c]);
    assert!(centre.is_none_or(|s| s.kind != SnapKind::Perpendicular));
}

#[test]
fn tangent_points_from_outside_only() {
    let c = circle(0.0, 0.0, 5.0);
    let f = v2(13.0, 0.0);
    for sign in [1.0, -1.0] {
        let want = v2(25.0 / 13.0, sign * 60.0 / 13.0);
        let hit = object_snap(want + v2(0.1, 0.1), 1.0, Some(f), [&c]).unwrap();
        assert_eq!(hit.kind, SnapKind::Tangent);
        assert!(hit.point.distance(want) < 1e-9, "{hit:?}");
    }
    // inside the circle: no tangent anywhere near the rim
    let inside = object_snap(v2(25.0 / 13.0, 60.0 / 13.0), 0.5, Some(v2(1.0, 0.0)), [&c]);
    assert!(inside.is_none_or(|s| s.kind != SnapKind::Tangent));
}

#[test]
fn arc_sweep_filters_perpendicular_and_tangent() {
    let arc = Shape::Arc { center: v2(0.0, 0.0), radius: 5.0, start: 0.1, sweep: 1.5 };
    let f = v2(13.0, 0.0);
    let up = v2(25.0 / 13.0, 60.0 / 13.0);
    let down = v2(25.0 / 13.0, -60.0 / 13.0);
    assert_eq!(object_snap(up, 0.5, Some(f), [&arc]).map(|s| s.kind), Some(SnapKind::Tangent));
    assert_eq!(object_snap(down, 0.5, Some(f), [&arc]), None);
    // the perpendicular point at angle 0 is outside the sweep
    assert_eq!(object_snap(v2(5.0, 0.0), 0.3, Some(f), [&arc]), None);
}

#[test]
fn kind_order_puts_new_kinds_before_grid() {
    assert!(SnapKind::Center < SnapKind::Perpendicular);
    assert!(SnapKind::Perpendicular < SnapKind::Tangent && SnapKind::Tangent < SnapKind::Grid);
}
