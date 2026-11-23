use crate::fixtures::*;
use crate::{Body, BodyFace, BoolOp, ModelError, Surface, TriMesh, boolean, union_all};
use glam::{DAffine3, DQuat, DVec3};
use std::f64::consts::FRAC_PI_2;

#[test]
fn box_body_properties() {
    let b = box_body([0.0, 0.0, 0.0], [40.0, 20.0, 10.0]);
    assert_eq!(b.tri_face.len(), b.mesh.triangle_count());
    assert_eq!(b.faces.len(), 6);
    assert!(b.mesh.is_closed());
    assert!((b.volume() - 8000.0).abs() < 1e-9);
    assert!((b.area() - 2800.0).abs() < 1e-9);
    assert!(b.centroid().distance(DVec3::new(20.0, 10.0, 5.0)) < 1e-9);
    assert_eq!(b.bbox(), (DVec3::ZERO, DVec3::new(40.0, 20.0, 10.0)));
    assert_eq!(b.feature_edges().len(), 12);
}

#[test]
fn cylinder_feature_edges_are_the_two_rims() {
    let b = cylinder_body(0.0, 0.0, 5.0, 0.0, 10.0);
    assert_eq!(b.faces.len(), 3);
    // Only the two cap rims; no seam lines along the side.
    let edges = b.feature_edges();
    assert!(!edges.is_empty());
    assert!(edges.iter().all(|[p, q]| p.z == q.z && (p.z == 0.0 || p.z == 10.0)));
}

fn plane_face(origin: DVec3, normal: DVec3, outward: bool) -> BodyFace {
    BodyFace { surface: Surface::Plane { origin, normal }, outward }
}

fn merged_count(faces: Vec<BodyFace>) -> usize {
    let mut b = Body { mesh: TriMesh::default(), tri_face: (0..faces.len() as u32).collect(), faces };
    b.merge_faces();
    assert!(b.tri_face.iter().all(|&f| (f as usize) < b.faces.len()));
    b.faces.len()
}

#[test]
fn coplanar_tops_of_a_union_are_one_face() {
    let u = boolean(&box_body([0.0; 3], [10.0; 3]), &box_body([5.0, 0.0, 0.0], [15.0, 10.0, 10.0]), BoolOp::Union).unwrap();
    assert_eq!(u.faces.len(), 6);
    for [p, q] in u.feature_edges() {
        // Every edge in the top plane lies on the outline of the top.
        if p.z == 10.0 && q.z == 10.0 {
            assert!(p.x == q.x && (p.x == 0.0 || p.x == 15.0) || p.y == q.y && (p.y == 0.0 || p.y == 10.0), "{p:?} {q:?}");
        }
    }
}

#[test]
fn parallel_planes_stay_apart() {
    let u = boolean(&box_body([0.0; 3], [10.0; 3]), &box_body([10.0, 0.0, 0.0], [20.0, 10.0, 5.0]), BoolOp::Union).unwrap();
    let top_at = |z: f64| u.faces.iter().any(|f| matches!(&f.surface, Surface::Plane { origin, normal } if f.outward && normal.z == 1.0 && origin.z == z));
    assert!(top_at(10.0) && top_at(5.0));
    assert_eq!(u.faces.len(), 8);
}

#[test]
fn only_the_same_oriented_plane_merges() {
    let (o, z) = (DVec3::ZERO, DVec3::Z);
    assert_eq!(merged_count(vec![plane_face(o, z, true), plane_face(DVec3::new(3.0, 4.0, 0.0), z, true)]), 1);
    assert_eq!(merged_count(vec![plane_face(o, z, true), plane_face(o, -z, false)]), 1);
    assert_eq!(merged_count(vec![plane_face(o, z, true), plane_face(o, z, false)]), 2);
    assert_eq!(merged_count(vec![plane_face(o, z, true), plane_face(DVec3::new(0.0, 0.0, 1e-3), z, true)]), 2);
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs().max(1.0), "{a} vs {b}");
}

#[test]
fn translated_box_keeps_volume_and_shifts_bbox() {
    let m = DAffine3::from_translation(DVec3::new(5.0, -3.0, 2.0));
    let t = box_body([0.0; 3], [40.0, 20.0, 10.0]).transformed(&m).unwrap();
    close(t.volume(), 8000.0, 1e-9);
    assert_eq!(t.bbox(), (DVec3::new(5.0, -3.0, 2.0), DVec3::new(45.0, 17.0, 12.0)));
}

#[test]
fn quarter_turn_swaps_extents() {
    let m = DAffine3::from_rotation_translation(DQuat::from_rotation_z(FRAC_PI_2), DVec3::ZERO);
    let (lo, hi) = box_body([0.0; 3], [40.0, 20.0, 10.0]).transformed(&m).unwrap().bbox();
    assert!(lo.distance(DVec3::new(-20.0, 0.0, 0.0)) < 1e-9 && hi.distance(DVec3::new(0.0, 40.0, 10.0)) < 1e-9, "{lo:?} {hi:?}");
}

#[test]
fn mirrored_prism_is_a_closed_outward_solid() {
    let m = DAffine3::from_scale(DVec3::new(-1.0, 1.0, 1.0));
    let (orig, mirrored) = (l_prism(5.0), l_prism(5.0).transformed(&m).unwrap());
    assert!(mirrored.mesh.is_closed());
    assert!(mirrored.volume() > 0.0);
    close(mirrored.volume(), orig.volume(), 1e-9);
    let e = mirrored.nearest_edge(DVec3::new(-10.0, 10.0, 2.5), 0.05).expect("concave edge at the mirrored position");
    assert!(e.distance(DVec3::new(-10.0, 10.0, 2.5)) < 1e-9);
    for (k, t) in mirrored.mesh.indices.as_chunks::<3>().0.iter().enumerate() {
        let [a, b, c] = [0, 1, 2].map(|j| mirrored.mesh.positions[t[j] as usize]);
        assert!(mirrored.mesh.normals[3 * k..3 * k + 3].iter().all(|n| (b - a).cross(c - a).dot(*n) > 0.0));
    }
}

#[test]
fn transformed_cylinder_corners_stay_on_their_surfaces() {
    let m = DAffine3::from_rotation_translation(DQuat::from_axis_angle(DVec3::new(1.0, 2.0, 3.0).normalize(), 0.7), DVec3::new(4.0, -5.0, 6.0));
    for m in [m, DAffine3::from_scale(DVec3::new(1.0, -1.0, 1.0)) * m] {
        let t = cylinder_body(3.0, 1.0, 5.0, 0.0, 10.0).transformed(&m).unwrap();
        assert!(t.volume() > 0.0);
        close(t.volume(), cylinder_body(3.0, 1.0, 5.0, 0.0, 10.0).volume(), 1e-9);
        for (k, &i) in t.mesh.indices.iter().enumerate() {
            let face = &t.faces[t.tri_face[k / 3] as usize];
            let p = t.mesh.positions[i as usize];
            assert!(face.surface.distance(p) < 1e-9);
            let n = t.mesh.normals[k];
            assert!((n.length() - 1.0).abs() < 1e-9);
            let expect = if face.outward { face.surface.normal_at(p) } else { -face.surface.normal_at(p) };
            assert!(n.distance(expect) < 1e-6, "{n:?} vs {expect:?}");
        }
    }
}

#[test]
fn scaling_is_rejected() {
    let r = box_body([0.0; 3], [1.0; 3]).transformed(&DAffine3::from_scale(DVec3::splat(2.0)));
    assert_eq!(r.err(), Some(ModelError::Invalid("transform must be a rigid motion or a mirror")));
}

#[test]
fn union_all_folds_and_handles_empty_and_disjoint() {
    assert_eq!(union_all(&[]).unwrap(), None);
    let three = [box_body([0.0; 3], [10.0; 3]), box_body([5.0; 3], [15.0; 3]), box_body([10.0, 0.0, 0.0], [20.0, 10.0, 10.0])];
    // Inclusion-exclusion: 1000 + 1000 + 1000 - 125 - 0 (touching face) - 125 + 0.
    close(union_all(&three).unwrap().unwrap().volume(), 2750.0, 1e-9);
    let two = union_all(&[box_body([0.0; 3], [10.0; 3]), box_body([20.0, 0.0, 0.0], [25.0, 5.0, 5.0])]).unwrap().unwrap();
    close(two.volume(), 1125.0, 1e-9);
}

/// A cube of 10 mm: 8 corners, 12 outward triangles; `flip` swaps corners 1 and 2 of every triangle.
fn cube_mesh(flip: bool) -> (Vec<DVec3>, Vec<[u32; 3]>) {
    let positions: Vec<DVec3> = (0..8).map(|i| DVec3::new((i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64) * 10.0).collect();
    let quads: [[u32; 4]; 6] = [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]];
    let mut tris = Vec::new();
    for q in quads {
        tris.push([q[0], q[1], q[2]]);
        tris.push([q[0], q[2], q[3]]);
    }
    if flip {
        for t in &mut tris {
            t.swap(1, 2);
        }
    }
    (positions, tris)
}

#[test]
fn from_mesh_makes_a_cube_with_six_faces() {
    let (p, t) = cube_mesh(false);
    let b = Body::from_mesh(p, &t).unwrap();
    assert!((b.volume() - 1000.0).abs() < 1e-9);
    assert_eq!(b.faces.len(), 6);
    assert_eq!(b.edges().len(), 12);
    assert!(b.mesh.normals.iter().all(|n| (n.length() - 1.0).abs() < 1e-12));
}

#[test]
fn from_mesh_turns_an_inside_out_mesh() {
    let (p, t) = cube_mesh(true);
    assert!((Body::from_mesh(p, &t).unwrap().volume() - 1000.0).abs() < 1e-9);
}

#[test]
fn from_mesh_refuses_open_and_bad_meshes() {
    let (p, mut t) = cube_mesh(false);
    t.pop();
    assert!(matches!(Body::from_mesh(p.clone(), &t), Err(ModelError::Invalid(msg)) if msg.contains("not closed")));
    let (_, mut t) = cube_mesh(false);
    t[0][0] = 99;
    assert!(matches!(Body::from_mesh(p, &t), Err(ModelError::Invalid("mesh index out of range"))));
}

#[test]
fn from_mesh_keeps_two_shells() {
    let (mut p, mut t) = cube_mesh(false);
    let (p2, t2) = cube_mesh(false);
    p.extend(p2.iter().map(|q| *q + DVec3::new(20.0, 0.0, 0.0)));
    t.extend(t2.iter().map(|q| q.map(|i| i + 8)));
    let b = Body::from_mesh(p, &t).unwrap();
    assert!((b.volume() - 2000.0).abs() < 1e-9);
    assert_eq!(b.faces.len(), 12);
}
