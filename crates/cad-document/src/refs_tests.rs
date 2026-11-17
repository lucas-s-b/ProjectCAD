use super::*;
use crate::feature::BODY_TOLERANCE;
use cad_geometry::profile::Profile;
use cad_geometry::{DVec2, Shape};
use cad_modeler::{BoolOp, boolean, extrude};

/// Block [x0, x1] x [y0, y1] x [0, h].
fn block(x0: f64, y0: f64, x1: f64, y1: f64, h: f64) -> Body {
    let rect = Shape::rect(DVec2::new(x0, y0), DVec2::new(x1, y1));
    let profile = Profile::from_shapes(&[&rect]).unwrap();
    Body::from_brep(&extrude(&profile, &Plane3::XY, h).unwrap(), BODY_TOLERANCE).unwrap()
}

fn top(z: f64) -> Plane3 {
    Plane3 { origin: DVec3::new(0.0, 0.0, z), ..Plane3::XY }
}

fn step() -> Body {
    boolean(&block(0.0, 0.0, 40.0, 20.0, 10.0), &block(0.0, 0.0, 20.0, 20.0, 15.0), BoolOp::Union).unwrap()
}

fn z_of(p: Result<Plane3, String>) -> f64 {
    p.unwrap().origin.z
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9, "{a} vs {b}");
}

#[test]
fn face_keys_match_by_direction() {
    let z = DVec3::Z;
    assert!(FaceKey::Plane(z).matches(&FaceKey::Plane((z + DVec3::X * 1e-8).normalize())));
    assert!(!FaceKey::Plane(z).matches(&FaceKey::Plane(-z)));
    assert!(FaceKey::Cylinder(z).matches(&FaceKey::Cylinder(-z)));
    assert!(!FaceKey::Plane(z).matches(&FaceKey::Cylinder(z)));
    assert!(FaceKey::Other.matches(&FaceKey::Other));

    let b = block(0.0, 0.0, 40.0, 20.0, 10.0);
    let bottom = b
        .faces
        .iter()
        .enumerate()
        .find(|(fi, f)| {
            matches!(f.surface, Surface::Plane { .. })
                && b.mesh.indices.as_chunks::<3>().0.iter().zip(&b.tri_face).any(|(t, &g)| {
                    g as usize == *fi && t.iter().any(|&i| b.mesh.positions[i as usize].z == 0.0)
                })
        })
        .map(|(_, f)| f)
        .unwrap();
    let FaceKey::Plane(n) = FaceKey::of(bottom) else { panic!("not a plane key") };
    assert!(n.distance(-DVec3::Z) < 1e-12, "{n}");
}

#[test]
fn plane_face_follows_a_taller_block() {
    let bodies: Bodies = vec![(1, Arc::new(block(0.0, 0.0, 40.0, 20.0, 15.0)))];
    let p = face_plane(top(10.0), DVec3::new(20.0, 10.0, 10.0), 1, &bodies).unwrap();
    close(p.origin.z, 15.0);
    assert_eq!((p.x, p.y), (DVec3::X, DVec3::Y));
}

#[test]
fn plane_face_takes_the_step_under_the_point() {
    let bodies: Bodies = vec![(1, Arc::new(step()))];
    close(z_of(face_plane(top(10.0), DVec3::new(30.0, 10.0, 10.0), 1, &bodies)), 10.0);
    close(z_of(face_plane(top(15.0), DVec3::new(10.0, 10.0, 15.0), 1, &bodies)), 15.0);
    let small: Bodies = vec![(1, Arc::new(block(0.0, 0.0, 20.0, 20.0, 15.0)))];
    assert_eq!(face_plane(top(10.0), DVec3::new(30.0, 10.0, 10.0), 1, &small), Err(FACE_ERROR.to_string()));
}

#[test]
fn plane_face_prefers_its_own_body() {
    let bodies: Bodies = vec![(1, Arc::new(block(0.0, 0.0, 40.0, 20.0, 10.0))), (2, Arc::new(block(0.0, 0.0, 40.0, 20.0, 35.0)))];
    let at = DVec3::new(20.0, 10.0, 20.0);
    close(z_of(face_plane(top(20.0), at, 2, &bodies)), 35.0);
    close(z_of(face_plane(top(20.0), at, 3, &bodies)), 10.0);
}

#[test]
fn edge_keys_find_the_moved_top_edge() {
    let b = block(0.0, 0.0, 40.0, 20.0, 10.0);
    let at = DVec3::new(20.0, 20.0, 10.0);
    let e = b.nearest_edge(at, 0.05).unwrap();
    let keys = edge_keys(&b, &e);
    let (z, y) = (FaceKey::Plane(DVec3::Z), FaceKey::Plane(DVec3::Y));
    assert!((keys[0].matches(&z) && keys[1].matches(&y)) || (keys[0].matches(&y) && keys[1].matches(&z)));
    let (found, d) = find_edge(&block(0.0, 0.0, 40.0, 20.0, 15.0), &keys, at).unwrap();
    assert!(found.points.iter().all(|p| (p.y - 20.0).abs() < 1e-9 && (p.z - 15.0).abs() < 1e-9));
    close(d, 5.0);
}

#[test]
fn find_edge_needs_matching_faces() {
    let b = block(0.0, 0.0, 40.0, 20.0, 10.0);
    let keys = [FaceKey::Plane(DVec3::Z), FaceKey::Plane(-DVec3::Z)];
    assert!(find_edge(&b, &keys, DVec3::new(20.0, 20.0, 10.0)).is_none());
}
