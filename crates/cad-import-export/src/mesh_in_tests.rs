use super::*;
use crate::{to_obj, to_stl};
use cad_geometry::{DVec2, DVec3, Shape, profile::Profile};
use cad_modeler::{Body, Plane3, extrude};

fn body(shape: Shape, z0: f64, height: f64) -> Body {
    let profile = Profile::from_shapes(&[&shape]).unwrap();
    let plane = Plane3 { origin: DVec3::new(0.0, 0.0, z0), ..Plane3::XY };
    Body::from_brep(&extrude(&profile, &plane, height).unwrap(), 0.01).unwrap()
}

fn cuboid() -> Body {
    body(Shape::rect(DVec2::ZERO, DVec2::new(40.0, 20.0)), 0.0, 10.0)
}

fn cylinder() -> Body {
    body(Shape::Circle { center: DVec2::new(5.0, 5.0), radius: 7.0 }, 0.0, 12.0)
}

fn volume(m: &MeshData) -> f64 {
    let p = |i: u32| DVec3::from(m.0[i as usize].map(f64::from));
    m.1.iter().map(|t| p(t[0]).dot(p(t[1]).cross(p(t[2]))) / 6.0).sum()
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs(), "{a} vs {b}");
}

#[test]
fn stl_round_trip_keeps_the_volume() {
    let b = cylinder();
    let m = from_stl(&to_stl(&[&b])).unwrap();
    close(volume(&m), b.volume(), 1e-5);
    assert_eq!(m.0.len(), b.mesh.positions.len());
    assert_eq!(m.1.len(), b.mesh.triangle_count());
    let positions = m.0.iter().map(|q| DVec3::from(q.map(f64::from))).collect();
    assert!(Body::from_mesh(positions, &m.1).is_ok());
}

#[test]
fn ascii_stl_is_read() {
    let corners = [[0, 0, 0], [10, 0, 0], [0, 10, 0], [0, 0, 10]];
    let facets = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
    let mut text = String::from("solid t\n");
    for f in facets {
        text += "facet normal 0 0 0\nouter loop\n";
        for k in f {
            let c = corners[k];
            text += &format!("vertex {} {} {}\n", c[0], c[1], c[2]);
        }
        text += "endloop\nendfacet\n";
    }
    text += "endsolid t\n";
    let m = from_stl(text.as_bytes()).unwrap();
    assert_eq!((m.0.len(), m.1.len()), (4, 4));
    close(volume(&m), 1000.0 / 6.0, 1e-9);
}

#[test]
fn obj_round_trip_keeps_the_volume() {
    let (a, b) = (cuboid(), cylinder());
    let m = from_obj(&to_obj(&[("a", &a), ("b", &b)])).unwrap();
    close(volume(&m), a.volume() + b.volume(), 1e-5);
}

#[test]
fn obj_faces_with_slashes_negative_indices_and_quads() {
    let mut text = String::new();
    for i in 0..8 {
        text += &format!("v {} {} {}\n", i & 1, (i >> 1) & 1, (i >> 2) & 1);
    }
    text += "vt 0 0\nvn 0 0 1\n";
    text += "f 1/1/1 3/2/1 4/3/1 2/4/1\nf 5/1/1 6/2/1 8/3/1 7/4/1\nf 1//1 2//1 6//1 5//1\n";
    text += "f 3 7 8 4\nf 1 5 7 3\nf -7 -5 -1 -3\n";
    let m = from_obj(&text).unwrap();
    assert_eq!((m.0.len(), m.1.len()), (8, 12));
    close(volume(&m), 1.0, 1e-9);
}

#[test]
fn bad_files_are_refused() {
    assert_eq!(from_stl(b""), Err("STL is empty or truncated".to_string()));
    assert!(from_obj("v 0 0 0\nf 1 2 3\n").unwrap_err().contains("point 2 of 1"));
}
