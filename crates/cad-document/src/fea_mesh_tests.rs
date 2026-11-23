use crate::mass::tests::{boxed, with_shaft};
use crate::*;
use cad_geometry::DVec3;

fn near(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} vs {b}");
}

#[test]
fn mesh_of_the_box_body() {
    let d = boxed();
    let r = Rebuilder::default().rebuild(&d);
    let rep = d.mesh_body(&r, 1, "5 mm").unwrap();
    assert_eq!(rep.h, 5.0);
    assert_eq!((rep.stats.nodes, rep.stats.elements, rep.mesh.unsnapped, rep.stats.inverted), (765, 384, 0, 0));
    near(rep.stats.volume, 8000.0, 1e-9);
    near(rep.body_volume, 8000.0, 1e-9);
    assert!(rep.volume_error().abs() <= 1e-12);
    near(rep.stats.min_ratio, 0.7174389352143, 1e-12);
    assert_eq!(rep.accuracy.to_string(), "approximate (mesh)");
}

#[test]
fn face_nodes_follow_face_refs() {
    let d = boxed();
    let r = Rebuilder::default().rebuild(&d);
    let rep = d.mesh_body(&r, 1, "5 mm").unwrap();
    let body = &r.bodies[0].1;
    let face = |n: DVec3| FaceRef::of(body, (0..body.faces.len()).find(|&i| FaceKey::of(&body.faces[i]).matches(&FaceKey::Plane(n))).unwrap()).unwrap();
    let top = d.face_nodes(&r, &rep, &face(DVec3::Z)).unwrap();
    assert_eq!(top.len(), 153);
    assert!(top.iter().all(|&v| (rep.mesh.nodes[v as usize].z - 10.0).abs() <= 1e-9));
    let side = d.face_nodes(&r, &rep, &face(-DVec3::X)).unwrap();
    assert_eq!(side.len(), 45);
    assert!(side.iter().all(|&v| rep.mesh.nodes[v as usize].x.abs() <= 1e-9));
    let none = FaceRef { key: FaceKey::Plane(DVec3::new(1.0, 1.0, 0.0).normalize()), at: DVec3::ZERO };
    assert_eq!(d.face_nodes(&r, &rep, &none), Err("face not found; pick it again".to_string()));
}

#[test]
fn mesh_refusals() {
    let d = boxed();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(d.mesh_body(&r, 9, "5 mm"), Err("body 9 not found".to_string()));
    assert_eq!(d.mesh_body(&r, 1, "0 mm"), Err("element size must be positive".to_string()));
    assert_eq!(d.mesh_body(&r, 1, "0.1 mm"), Err("element size 0.1 mm gives 8000000 grid cells (at most 50000); use a larger element size".to_string()));
    assert!(d.mesh_body(&r, 1, "nonsense").is_err());
}

#[test]
fn mesh_of_the_shaft() {
    let mut d = boxed();
    with_shaft(&mut d);
    let r = Rebuilder::default().rebuild(&d);
    let rep = d.mesh_body(&r, 2, "2 mm").unwrap();
    assert_eq!(rep.stats.inverted, 0);
    assert!(rep.stats.min_ratio >= 0.15, "{}", rep.stats.min_ratio);
    assert!(rep.stats.elements > 10_000, "{}", rep.stats.elements);
    assert!(rep.volume_error().abs() <= 0.01, "{}", rep.volume_error());
    near(rep.body_volume, 19372.340134589307, 0.01);
}
