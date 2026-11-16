use crate::fixtures::*;
use crate::{ModelError, Plane3, extrude};

#[test]
fn box_counts() {
    let b = extrude(&make(&[rect(0.0, 0.0, 40.0, 20.0)]), &Plane3::XY, 10.0).unwrap();
    assert_eq!((b.vertices.len(), b.edges.len(), b.faces.len()), (8, 12, 6));
    assert_eq!(b.euler(), 2);
    assert_eq!(b.validate(), Ok(()));
    let (lo, hi) = b.bbox();
    assert_eq!((lo, hi), (glam::DVec3::ZERO, glam::DVec3::new(40.0, 20.0, 10.0)));
}

#[test]
fn cylinder_counts() {
    let b = extrude(&make(&[circle(0.0, 0.0, 10.0)]), &Plane3::XY, 30.0).unwrap();
    assert_eq!((b.vertices.len(), b.edges.len(), b.faces.len()), (4, 6, 4));
    assert_eq!(b.euler(), 2);
    let (lo, hi) = b.bbox();
    assert!(lo.distance(glam::DVec3::new(-10.0, -10.0, 0.0)) < 1e-9, "{lo:?}");
    assert!(hi.distance(glam::DVec3::new(10.0, 10.0, 30.0)) < 1e-9, "{hi:?}");
}

#[test]
fn rounded_and_holed() {
    let b = extrude(&rounded_rect(40.0, 20.0, 3.0), &Plane3::XY, 5.0).unwrap();
    assert_eq!(b.faces.len(), 10);
    assert_eq!(b.euler(), 2);
    let h = extrude(&holed_rect(), &Plane3::XY, 5.0).unwrap();
    assert_eq!(h.faces.len(), 8);
    assert_eq!(h.faces[0].loops.len(), 2);
    assert_eq!(h.faces[1].loops.len(), 2);
    // V = 12, E = 18, F = 8, one inner loop on each cap.
    assert_eq!(h.euler(), 12 - 18 + 8 - 2);
    assert_eq!(h.euler(), 0);
}

#[test]
fn negative_distance_and_tilted_plane() {
    let p = make(&[rect(0.0, 0.0, 40.0, 20.0)]);
    let b = extrude(&p, &Plane3::XY, -10.0).unwrap();
    assert_eq!((b.faces.len(), b.euler()), (6, 2));
    assert_eq!(b.bbox().0.z, -10.0);
    for profile in [p, rounded_rect(40.0, 20.0, 3.0), holed_rect(), make(&[circle(1.0, 2.0, 4.0)])] {
        for d in [7.0, -7.0] {
            let a = extrude(&profile, &tilted(), d).unwrap();
            assert_eq!(a.validate(), Ok(()));
            let b = extrude(&profile, &Plane3::XY, d).unwrap();
            assert_eq!(a.faces.len(), b.faces.len());
        }
    }
}

#[test]
fn zero_distance_errors() {
    let p = make(&[rect(0.0, 0.0, 1.0, 1.0)]);
    for d in [0.0, 1e-8, f64::NAN, f64::INFINITY] {
        assert_eq!(extrude(&p, &Plane3::XY, d), Err(ModelError::Invalid("extrusion distance is zero")));
    }
}
