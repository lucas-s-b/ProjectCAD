use crate::fixtures::*;
use crate::{ModelError, Plane3, TriMesh, extrude, prism_volume, tessellate};
use cad_geometry::profile::Profile;
use glam::DVec3;
use std::f64::consts::PI;

fn mesh_of(p: &Profile, plane: &Plane3, d: f64, tol: f64) -> TriMesh {
    let m = tessellate(&extrude(p, plane, d).unwrap(), tol).unwrap();
    assert!(m.is_closed());
    assert_eq!(m.normals.len(), m.indices.len());
    assert!(m.normals.iter().all(|n| (n.length() - 1.0).abs() < 1e-9));
    m
}

#[test]
fn box_mesh() {
    let m = mesh_of(&make(&[rect(0.0, 0.0, 40.0, 20.0)]), &Plane3::XY, 10.0, 0.01);
    assert_eq!(m.triangle_count(), 12);
    assert!((m.volume() - 8000.0).abs() < 1e-9);
    assert!(m.centroid().distance(DVec3::new(20.0, 10.0, 5.0)) < 1e-9);
    assert!((m.area() - 2.0 * (800.0 + 400.0 + 200.0)).abs() < 1e-9);
    // Corner normals are the face normals, pointing out.
    for (t, n) in m.indices.as_chunks::<3>().0.iter().zip(m.normals.as_chunks::<3>().0) {
        let (a, b, c) = (m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]);
        let face = (b - a).cross(c - a).normalize();
        assert!(n.iter().all(|x| x.distance(face) < 1e-9), "{n:?} vs {face:?}");
    }
}

#[test]
fn cylinder_error_follows_the_sagitta() {
    let p = make(&[circle(0.0, 0.0, 10.0)]);
    let exact = PI * 100.0 * 30.0;
    for tol in [0.01, 0.001] {
        let m = mesh_of(&p, &Plane3::XY, 30.0, tol);
        let err = (exact - m.volume()) / exact;
        // Inscribed polygon: area error is about 4 tol / (3 r), so the plan's 0.1 % and 0.01 % cannot hold.
        println!("circle tol {tol}: relative volume error {err:.3e}");
        assert!(err > 0.0 && err < 4.0 * tol / (3.0 * 10.0), "{err}");
    }
}

#[test]
fn rounded_and_holed_volumes() {
    for p in [rounded_rect(40.0, 20.0, 3.0), holed_rect()] {
        let m = mesh_of(&p, &Plane3::XY, 7.0, 0.01);
        let exact = prism_volume(&p, 7.0);
        assert!((m.volume() - exact).abs() / exact < 1e-3, "{} vs {exact}", m.volume());
    }
}

#[test]
fn negative_distance_is_outward() {
    let p = holed_rect();
    let m = mesh_of(&p, &Plane3::XY, -7.0, 0.01);
    assert!(m.volume() > 0.0);
    assert!((m.volume() - prism_volume(&p, -7.0)).abs() / prism_volume(&p, -7.0) < 1e-3);
    let a = mesh_of(&p, &tilted(), 7.0, 0.01);
    let b = mesh_of(&p, &tilted(), -7.0, 0.01);
    assert!((a.volume() - b.volume()).abs() < 1e-6 && a.volume() > 0.0);
}

#[test]
fn bad_tolerance_errors() {
    let b = extrude(&make(&[rect(0.0, 0.0, 1.0, 1.0)]), &Plane3::XY, 1.0).unwrap();
    for tol in [1e-9, 0.0, -1.0, f64::NAN] {
        assert_eq!(tessellate(&b, tol), Err(ModelError::Invalid("tessellation tolerance too small")));
    }
}


#[test]
fn collinear_boundary_point_keeps_the_mesh_closed() {
    let poly = cad_geometry::shape::Shape::Polyline {
        points: vec![v2(0.0, 0.0), v2(2.0, 0.0), v2(4.0, 0.0), v2(4.0, 4.0), v2(0.0, 4.0)],
        closed: true,
    };
    let m = mesh_of(&make(&[poly]), &Plane3::XY, 1.0, 0.01);
    assert!((m.volume() - 16.0).abs() < 1e-9);
}
