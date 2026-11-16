use super::*;
use crate::fixtures::*;
use std::f64::consts::PI;
use std::time::Instant;

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs(), "{a} vs {b}");
}

/// The Minkowski difference of a 40 x 20 x 10 box and the tool of radius 2 is the 36 x 16 x 6 box.
#[test]
fn erosion_of_a_box_by_the_tool() {
    let start = Instant::now();
    let inner = inner_body(&box_body([0.0; 3], [40.0, 20.0, 10.0]), 2.0, 0.02).unwrap();
    println!("erosion took {:?}", start.elapsed());
    assert!(inner.mesh.is_closed());
    close(inner.volume(), 36.0 * 16.0 * 6.0, 1e-6);
    let (lo, hi) = inner.bbox();
    assert!(lo.distance(DVec3::new(2.0, 2.0, 2.0)) < 1e-9 && hi.distance(DVec3::new(38.0, 18.0, 8.0)) < 1e-9, "{lo} {hi}");
}

const TOL: f64 = 0.02;

fn plain_box() -> Body {
    box_body([0.0; 3], [40.0, 20.0, 10.0])
}

fn all_planes(b: &Body) -> bool {
    b.faces.iter().all(|f| matches!(f.surface, Surface::Plane { .. }))
}

fn invalid(r: Result<Body, ModelError>) -> &'static str {
    match r {
        Err(ModelError::Invalid(m)) => m,
        other => panic!("expected an Invalid error, got {:?}", other.map(|b| b.volume())),
    }
}

#[test]
fn closed_shell_of_a_box() {
    let r = shell(&plain_box(), 2.0, None, TOL).unwrap();
    assert!(r.mesh.is_closed());
    assert_eq!(components(&r.mesh), 2);
    close(r.volume(), 8000.0 - 36.0 * 16.0 * 6.0, 1e-6);
    // Six outer and six inner planes; the cavity faces face into the wall.
    assert!(r.faces.len() == 12 && all_planes(&r));
    assert_eq!(r.faces.iter().filter(|f| !f.outward).count(), 6);
}

#[test]
fn open_shell_of_a_box() {
    let r = shell(&plain_box(), 2.0, Some(DVec3::new(20.0, 10.0, 10.0)), TOL).unwrap();
    assert!(r.mesh.is_closed());
    assert_eq!(components(&r.mesh), 1);
    close(r.volume(), 8000.0 - 36.0 * 16.0 * 8.0, 1e-6);
    assert!(r.faces.len() == 11 && all_planes(&r));
    for t in r.mesh.indices.as_chunks::<3>().0 {
        let p = t.map(|i| r.mesh.positions[i as usize]);
        let on_top = p.iter().all(|q| (q.z - 10.0).abs() < 1e-9);
        let c = (p[0] + p[1] + p[2]) / 3.0;
        assert!(!(on_top && c.x > 2.0 && c.x < 38.0 && c.y > 2.0 && c.y < 18.0), "triangle left in the opening at {c}");
    }
}

#[test]
fn open_shell_of_an_l_prism() {
    let r = shell(&l_prism(10.0), 2.0, Some(DVec3::new(5.0, 5.0, 10.0)), TOL).unwrap();
    assert!(r.mesh.is_closed());
    assert_eq!(components(&r.mesh), 1);
    // The cavity footprint is the L inset by 2 (area 156) plus the corner square next to the reflex vertex without its quarter disc.
    let cavity = (156.0 + 4.0 - PI) * 8.0;
    close(r.volume(), 300.0 * 10.0 - cavity, 0.01);
}

#[test]
fn open_shell_of_a_cylinder() {
    let r = shell(&cylinder_body(0.0, 0.0, 10.0, 0.0, 20.0), 2.0, Some(DVec3::new(0.0, 0.0, 20.0)), TOL).unwrap();
    assert!(r.mesh.is_closed());
    assert_eq!(components(&r.mesh), 1);
    close(r.volume(), PI * (100.0 - 64.0) * 18.0 + PI * 100.0 * 2.0, 0.03);
}

#[test]
fn errors() {
    let b = plain_box();
    assert_eq!(invalid(shell(&b, 6.0, None, TOL)), "shell thickness is too large for this body");
    for t in [0.0, -1.0, 1e-7, f64::NAN, f64::INFINITY] {
        assert_eq!(invalid(shell(&b, t, None, TOL)), "shell thickness must be positive");
    }
    assert_eq!(invalid(shell(&b, 2.0, Some(DVec3::new(20.0, 10.0, 12.0)), TOL)), "face not found; pick it again");
    let cyl = cylinder_body(0.0, 0.0, 10.0, 0.0, 20.0);
    assert_eq!(invalid(shell(&cyl, 2.0, Some(DVec3::new(10.0, 0.0, 10.0)), TOL)), "only a planar face can be opened");
    // The wall reaches the middle of the box: the inner solid is too thin for the cutter to meet its copy.
    assert_eq!(invalid(shell(&b, 4.0, Some(DVec3::new(20.0, 10.0, 10.0)), TOL)), "shell thickness is too large to open this face");
}
