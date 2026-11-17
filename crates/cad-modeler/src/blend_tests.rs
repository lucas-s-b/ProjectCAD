use crate::fixtures::*;
use crate::{BodyEdge, BoolOp, Body, ModelError, Plane3, Surface, boolean, chamfer_edge, extrude, fillet_edge};
use glam::DVec3;
use std::f64::consts::PI;

const TOL: f64 = 0.01;

fn edge_at(b: &Body, x: f64, y: f64, z: f64) -> BodyEdge {
    b.nearest_edge(DVec3::new(x, y, z), 0.05).expect("an edge near the point")
}

fn plate() -> Body {
    box_body([0.0; 3], [40.0, 20.0, 10.0])
}

fn close(got: f64, exact: f64, rel: f64) {
    assert!((got - exact).abs() <= rel * exact.abs(), "got {got}, exact {exact}, relative error {}", (got - exact) / exact);
}

/// Area of the fillet section in a wedge of angle `theta`.
fn fillet_area(r: f64, theta: f64) -> f64 {
    r * r / (theta / 2.0).tan() - (PI - theta) / 2.0 * r * r
}

/// Section area of a fillet on a right angle, and the distance of its centroid from the corner along each side.
fn quarter() -> (f64, f64) {
    (1.0 - PI / 4.0, (10.0 - 3.0 * PI) / (3.0 * (4.0 - PI)))
}

fn fails_with(r: Result<Body, ModelError>, msg: &'static str) {
    assert_eq!(r.err(), Some(ModelError::Invalid(msg)));
}

const POSITIVE: &str = "blend size must be positive";
const NOT_YET: &str = "this edge cannot be blended yet";
const TOO_LARGE: &str = "blend size too large for this edge";

#[test]
fn fillet_of_a_vertical_box_edge() {
    let b = plate();
    let r = fillet_edge(&b, &edge_at(&b, 0.0, 0.0, 5.0), 3.0, TOL).unwrap();
    assert!(r.mesh.is_closed());
    let exact = 8000.0 - (9.0 - 9.0 * PI / 4.0) * 10.0;
    assert!((r.volume() - exact).abs() < 0.02, "{} vs {exact}", r.volume());
    assert!(r.faces.iter().any(|f| matches!(f.surface, Surface::Cylinder { radius, .. } if (radius - 3.0).abs() < 1e-9) && f.outward));
}

#[test]
fn chamfer_of_a_top_edge() {
    let b = plate();
    let r = chamfer_edge(&b, &edge_at(&b, 20.0, 0.0, 10.0), 2.0, TOL).unwrap();
    assert!(r.mesh.is_closed());
    close(r.volume(), 8000.0 - 2.0 * 40.0, 1e-6);
}

#[test]
fn fillet_of_a_concave_edge_adds_material() {
    let l = l_prism(10.0);
    let r = fillet_edge(&l, &edge_at(&l, 10.0, 10.0, 5.0), 2.0, TOL).unwrap();
    assert!(r.mesh.is_closed());
    assert!((r.volume() - l.volume() - (4.0 - PI) * 10.0).abs() < 0.02);
}

#[test]
fn fillet_of_a_sixty_degree_edge() {
    let t = triangle_prism(10.0);
    let r = fillet_edge(&t, &edge_at(&t, 0.0, 0.0, 5.0), 1.0, TOL).unwrap();
    assert!(r.mesh.is_closed());
    close(t.volume() - r.volume(), fillet_area(1.0, PI / 3.0) * 10.0, 0.005);
}

#[test]
fn rims_of_a_hole() {
    let (a, xbar) = quarter();
    let h = plate_with_hole(4.0);
    let rim = edge_at(&h, 24.0, 10.0, 10.0);
    let r = fillet_edge(&h, &rim, 1.0, TOL).unwrap();
    assert!(r.mesh.is_closed());
    assert!(r.faces.iter().any(|f| matches!(f.surface, Surface::Torus { .. })));
    close(h.volume() - r.volume(), a * 2.0 * PI * (4.0 + xbar), 0.01);
    let c = chamfer_edge(&h, &rim, 1.0, TOL).unwrap();
    assert!(c.mesh.is_closed());
    close(h.volume() - c.volume(), 0.5 * 2.0 * PI * (4.0 + 1.0 / 3.0), 0.01);
}

#[test]
fn foot_and_top_of_a_boss() {
    let (a, xbar) = quarter();
    let b = plate_with_boss(4.0);
    let foot = fillet_edge(&b, &edge_at(&b, 24.0, 10.0, 10.0), 1.0, TOL).unwrap();
    assert!(foot.mesh.is_closed());
    close(foot.volume() - b.volume(), a * 2.0 * PI * (4.0 + xbar), 0.01);
    let top = fillet_edge(&b, &edge_at(&b, 24.0, 10.0, 20.0), 1.0, TOL).unwrap();
    assert!(top.mesh.is_closed());
    close(b.volume() - top.volume(), a * 2.0 * PI * (4.0 - xbar), 0.01);
}

#[test]
fn bad_requests() {
    let b = plate_with_boss(4.0);
    let top = edge_at(&b, 24.0, 10.0, 20.0);
    fails_with(fillet_edge(&b, &top, 5.0, TOL), TOO_LARGE);
    for size in [0.0, -1.0, f64::NAN, 1e-9] {
        fails_with(fillet_edge(&b, &top, size, TOL), POSITIVE);
        fails_with(chamfer_edge(&b, &top, size, TOL), POSITIVE);
    }
    let across = Plane3 { origin: DVec3::new(-10.0, 0.0, 0.0), x: DVec3::Y, y: DVec3::Z };
    let beam = Body::from_brep(&extrude(&make(&[circle(0.0, 0.0, 3.0)]), &across, 20.0).unwrap(), 0.01).unwrap();
    let u = boolean(&cylinder_body(0.0, 0.0, 5.0, -10.0, 10.0), &beam, BoolOp::Union).unwrap();
    let other = u.edges().into_iter().find(|e| e.geom == crate::EdgeGeom::Other).unwrap();
    fails_with(fillet_edge(&u, &other, 0.5, TOL), NOT_YET);
}

#[test]
fn rounding_all_vertical_edges_one_after_the_other() {
    let mut b = plate();
    for (x, y) in [(0.0, 0.0), (40.0, 0.0), (40.0, 20.0), (0.0, 20.0)] {
        b = fillet_edge(&b, &edge_at(&b, x, y, 5.0), 3.0, TOL).unwrap();
        assert!(b.mesh.is_closed());
    }
    let exact = 8000.0 - 4.0 * (9.0 - 9.0 * PI / 4.0) * 10.0;
    assert!((b.volume() - exact).abs() < 0.1, "{} vs {exact}", b.volume());
    let top = fillet_edge(&b, &edge_at(&b, 20.0, 0.0, 10.0), 1.0, TOL).unwrap();
    assert!(top.mesh.is_closed() && top.volume() < b.volume());
}

#[test]
fn an_open_rim_is_filleted_over_its_own_arc() {
    let (full, half) = (plate_with_hole(4.0), plate_half_hole(4.0));
    let all = full.volume() - fillet_edge(&full, &edge_at(&full, 24.0, 10.0, 10.0), 1.0, TOL).unwrap().volume();
    let r = fillet_edge(&half, &edge_at(&half, 36.0, 10.0, 10.0), 1.0, TOL).unwrap();
    assert!(r.mesh.is_closed());
    close(half.volume() - r.volume(), all / 2.0, 0.03);
}

#[test]
fn fillet_on_a_line_next_to_a_cylinder_is_refused() {
    let d = d_flat();
    let line = d.edges().into_iter().find(|e| matches!(e.geom, crate::EdgeGeom::Line { .. }) && !e.faces.iter().all(|&f| matches!(d.faces[f as usize].surface, Surface::Plane { .. }))).unwrap();
    fails_with(fillet_edge(&d, &line, 1.0, TOL), NOT_YET);
}
