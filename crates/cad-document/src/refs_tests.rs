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

/// Cylinder of radius r about the Z axis, z 0..h.
fn cyl(r: f64, h: f64) -> Body {
    let c = Shape::Circle { center: DVec2::ZERO, radius: r };
    Body::from_brep(&extrude(&Profile::from_shapes(&[&c]).unwrap(), &Plane3::XY, h).unwrap(), BODY_TOLERANCE).unwrap()
}

/// z of the plane of face i (its `origin` lies on the plane).
fn plane_z(b: &Body, i: usize) -> f64 {
    let Surface::Plane { origin, .. } = b.faces[i].surface else { panic!("face {i} is not a plane") };
    origin.z
}

#[test]
fn face_refs_name_faces_again() {
    let b = block(0.0, 0.0, 40.0, 20.0, 10.0);
    let top = (0..b.faces.len()).find(|&i| FaceKey::of(&b.faces[i]).matches(&FaceKey::Plane(DVec3::Z))).unwrap();
    let r = FaceRef::of(&b, top).unwrap();
    assert!(r.key.matches(&FaceKey::Plane(DVec3::Z)));
    close(r.at.z, 10.0);
    assert!((0.0..=40.0).contains(&r.at.x) && (0.0..=20.0).contains(&r.at.y));
    assert_eq!(FaceRef::of(&b, 99), None);
    let taller = block(0.0, 0.0, 40.0, 20.0, 15.0);
    close(plane_z(&taller, find_face(&taller, &r).unwrap()), 15.0);
    let s = step();
    let up = |x: f64| FaceRef { key: FaceKey::Plane(DVec3::Z), at: DVec3::new(x, 10.0, 10.0) };
    close(plane_z(&s, find_face(&s, &up(30.0)).unwrap()), 10.0);
    close(plane_z(&s, find_face(&s, &up(10.0)).unwrap()), 15.0);
    assert_eq!(find_face(&b, &FaceRef { key: FaceKey::Plane(-DVec3::Z), at: DVec3::new(100.0, 100.0, 0.0) }), None);
    assert_eq!(find_face(&b, &FaceRef { key: FaceKey::Other, at: DVec3::ZERO }), None);
}

#[test]
fn cylinder_refs_follow_a_new_radius() {
    let r = FaceRef { key: FaceKey::Cylinder(DVec3::Z), at: DVec3::new(5.0, 0.0, 5.0) };
    let c = cyl(6.0, 12.0);
    let i = find_face(&c, &r).unwrap();
    assert!(matches!(c.faces[i].surface, Surface::Cylinder { radius, .. } if (radius - 6.0).abs() <= 1e-9));
    assert!(FaceRef::of(&c, i).unwrap().key.matches(&FaceKey::Cylinder(-DVec3::Z)));
    let top = FaceRef { key: FaceKey::Plane(DVec3::Z), at: DVec3::new(1.0, 1.0, 10.0) };
    close(plane_z(&c, find_face(&c, &top).unwrap()), 12.0);
}
