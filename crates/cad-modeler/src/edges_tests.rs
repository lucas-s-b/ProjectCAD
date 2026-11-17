use crate::fixtures::*;
use crate::{BodyEdge, BoolOp, Convexity, EdgeGeom, Plane3, boolean, extrude};
use glam::DVec3;
use std::f64::consts::{PI, TAU};

fn circles(edges: &[BodyEdge]) -> Vec<&BodyEdge> {
    edges.iter().filter(|e| matches!(e.geom, EdgeGeom::Circle { .. })).collect()
}

fn circle_at(e: &BodyEdge) -> (DVec3, f64) {
    let EdgeGeom::Circle { center, radius, .. } = e.geom else { panic!("not a circle: {:?}", e.geom) };
    (center, radius)
}

#[test]
fn box_has_twelve_convex_lines() {
    let edges = box_body([0.0; 3], [40.0, 20.0, 10.0]).edges();
    assert_eq!(edges.len(), 12);
    assert!(edges.iter().all(|e| matches!(e.geom, EdgeGeom::Line { .. }) && e.convexity == Convexity::Convex && !e.closed));
}

#[test]
fn cylinder_has_two_closed_circles() {
    let edges = cylinder_body(0.0, 0.0, 5.0, 0.0, 10.0).edges();
    assert_eq!(edges.len(), 2);
    for e in &edges {
        let EdgeGeom::Circle { radius, sweep, .. } = e.geom else { panic!("{:?}", e.geom) };
        assert!(e.closed && e.convexity == Convexity::Convex);
        assert_eq!((radius, sweep), (5.0, TAU));
    }
}

#[test]
fn l_prism_has_one_concave_edge() {
    let edges = l_prism(10.0).edges();
    let concave: Vec<_> = edges.iter().filter(|e| e.convexity == Convexity::Concave).collect();
    assert_eq!(concave.len(), 1);
    let EdgeGeom::Line { a, b } = concave[0].geom else { panic!() };
    assert!(a.truncate().distance(glam::DVec2::new(10.0, 10.0)) < 1e-9 && b.truncate() == a.truncate());
}

#[test]
fn hole_rims_are_convex_circles() {
    let edges = plate_with_hole(4.0).edges();
    let rims = circles(&edges);
    assert_eq!(rims.len(), 2);
    for e in rims {
        let (c, r) = circle_at(e);
        assert!(e.closed && e.convexity == Convexity::Convex);
        assert!((r - 4.0).abs() < 1e-9 && c.truncate().distance(glam::DVec2::new(20.0, 10.0)) < 1e-9);
    }
}

#[test]
fn boss_foot_is_concave_and_top_rim_convex() {
    let edges = plate_with_boss(4.0).edges();
    let rims = circles(&edges);
    assert_eq!(rims.len(), 2);
    for e in rims {
        let (c, r) = circle_at(e);
        assert!((r - 4.0).abs() < 1e-9);
        let expected = if (c.z - 10.0).abs() < 1e-9 { Convexity::Concave } else { Convexity::Convex };
        assert!(c.z == 10.0 || c.z == 20.0, "{c:?}");
        assert_eq!(e.convexity, expected);
    }
}

#[test]
fn crossing_cylinders_meet_in_other_edges() {
    let across = Plane3 { origin: DVec3::new(-10.0, 0.0, 0.0), x: DVec3::Y, y: DVec3::Z };
    let beam = crate::Body::from_brep(&extrude(&make(&[circle(0.0, 0.0, 3.0)]), &across, 20.0).unwrap(), 0.01).unwrap();
    let u = boolean(&cylinder_body(0.0, 0.0, 5.0, -10.0, 10.0), &beam, BoolOp::Union).unwrap();
    let seams: Vec<_> = u.edges().into_iter().filter(|e| e.faces.iter().all(|&f| matches!(u.faces[f as usize].surface, crate::Surface::Cylinder { .. }))).collect();
    assert!(!seams.is_empty());
    assert!(seams.iter().all(|e| e.geom == EdgeGeom::Other));
}

#[test]
fn nearest_edge_and_midpoint() {
    let b = box_body([0.0; 3], [40.0, 20.0, 10.0]);
    let e = b.nearest_edge(DVec3::new(20.0, 0.2, 9.9), 1.0).unwrap();
    let EdgeGeom::Line { a, b: end } = e.geom else { panic!() };
    assert!(a.y == 0.0 && end.y == 0.0 && a.z == 10.0 && end.z == 10.0 && (a.x - end.x).abs() == 40.0);
    assert!(e.midpoint().distance(DVec3::new(20.0, 0.0, 10.0)) < 1e-9);
    assert!(b.nearest_edge(DVec3::new(20.0, 10.0, 5.0), 1.0).is_none());

    let rim = plate_with_hole(4.0).nearest_edge(DVec3::new(24.0, 10.0, 10.0), 0.5).unwrap();
    let (c, r) = circle_at(&rim);
    assert!((rim.midpoint().distance(c) - r).abs() < 0.02);
}

#[test]
fn half_rim_is_an_open_arc_running_counter_clockwise() {
    let edges = plate_half_hole(4.0).edges();
    let rims: Vec<_> = circles(&edges).into_iter().filter(|e| circle_at(e).0.z == 10.0).collect();
    assert_eq!(rims.len(), 1);
    let e = rims[0];
    let EdgeGeom::Circle { center, axis, start, sweep, .. } = e.geom else { panic!() };
    assert!(!e.closed && e.convexity == Convexity::Convex);
    assert!((sweep - PI).abs() < 0.02, "{sweep}");
    let end = center + glam::DQuat::from_axis_angle(axis, sweep) * (start - center);
    assert!(end.distance(*e.points.last().unwrap()) < 1e-6);
}

#[test]
fn flat_parallel_to_the_axis_meets_the_cylinder_in_lines() {
    let d = d_flat();
    let is_cyl = |f: u32| matches!(d.faces[f as usize].surface, crate::Surface::Cylinder { .. });
    let is_x_plane = |f: u32| matches!(d.faces[f as usize].surface, crate::Surface::Plane { normal, .. } if normal.x.abs() > 0.999);
    let lines: Vec<_> = d.edges().into_iter().filter(|e| e.faces.iter().any(|&f| is_x_plane(f)) && e.faces.iter().any(|&f| is_cyl(f))).collect();
    assert_eq!(lines.len(), 2);
    for e in lines {
        let EdgeGeom::Line { a, b } = e.geom else { panic!("{:?}", e.geom) };
        assert!(((a.z - b.z).abs() - 10.0).abs() < 1e-9);
        assert!((a.x - 3.0).abs() < 1e-9 && (b.x - 3.0).abs() < 1e-9);
        assert!((a.y.abs() - 4.0).abs() < 0.05);
    }
}

#[test]
fn oblique_plane_cuts_the_cylinder_in_an_ellipse() {
    let cut = oblique_cut();
    let edges = cut.edges();
    let ellipses: Vec<_> = edges.iter().filter(|e| matches!(e.geom, EdgeGeom::Ellipse { .. })).collect();
    assert_eq!(ellipses.len(), 1);
    let e = ellipses[0];
    let EdgeGeom::Ellipse { center, major_dir, major, minor, .. } = e.geom else { unreachable!() };
    assert!(e.closed);
    assert!(center.distance(DVec3::new(0.0, 0.0, 10.0)) < 1e-9);
    assert!((minor - 5.0).abs() < 1e-9 && (major - 5.773502692).abs() < 1e-6);
    // The major axis runs up the tilted plane: (0, cos 30, sin 30) up to sign.
    assert!(major_dir.x.abs() < 1e-6 && (major_dir.y.abs() - 30f64.to_radians().cos()).abs() < 1e-6);
    let rings: Vec<_> = circles(&edges);
    assert_eq!(rings.len(), 1);
    assert!((circle_at(rings[0]).1 - 5.0).abs() < 1e-9);
}
