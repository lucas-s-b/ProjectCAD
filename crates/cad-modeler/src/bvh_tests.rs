use super::*;
use crate::fixtures::{circle, holed_rect, make, rect};
use crate::{Body, Plane3, extrude};

fn mesh_of(profile: &cad_geometry::profile::Profile, height: f64) -> TriMesh {
    Body::from_brep(&extrude(profile, &Plane3::XY, height).unwrap(), 0.01).unwrap().mesh
}

fn brute(mesh: &TriMesh, origin: DVec3, dir: DVec3) -> Option<f64> {
    (0..mesh.indices.len() / 3).filter_map(|t| ray_triangle(origin, dir, corners(mesh, t))).min_by(f64::total_cmp)
}

fn check(mesh: &TriMesh) {
    let bvh = Bvh::build(mesh);
    let mut hits = 0;
    for i in 0..21 {
        for j in 0..21 {
            let (u, v) = (-2.0 + 2.2 * f64::from(i), -2.0 + 1.2 * f64::from(j));
            let rays = [
                (DVec3::new(u, v, 100.0), DVec3::new(0.0, 0.0, -1.0)),
                (DVec3::new(-100.0, v, -1.0 + 0.6 * f64::from(i)), DVec3::X),
            ];
            for (origin, dir) in rays {
                let (fast, slow) = (bvh.ray(mesh, origin, dir), brute(mesh, origin, dir));
                assert_eq!(fast.is_some(), slow.is_some(), "{origin:?} {dir:?}");
                if let (Some((t, tri)), Some(s)) = (fast, slow) {
                    assert!((t - s).abs() < 1e-9, "{t} vs {s}");
                    assert!(ray_triangle(origin, dir, corners(mesh, tri)).is_some());
                    hits += 1;
                }
            }
        }
    }
    assert!(hits > 100, "only {hits} hits");
}

#[test]
fn bvh_matches_brute_force() {
    check(&mesh_of(&make(&[circle(20.0, 10.0, 8.0)]), 10.0));
    check(&mesh_of(&make(&[rect(0.0, 0.0, 40.0, 20.0)]), 10.0));
    check(&mesh_of(&holed_rect(), 10.0));
}

#[test]
fn empty_mesh_has_no_hit() {
    let mesh = TriMesh::default();
    assert!(Bvh::build(&mesh).ray(&mesh, DVec3::ZERO, DVec3::X).is_none());
}

#[test]
fn closest_on_triangle_regions() {
    let tri = [DVec3::ZERO, DVec3::new(2.0, 0.0, 0.0), DVec3::new(0.0, 2.0, 0.0)];
    let cases = [
        ((-1.0, -1.0, 3.0), (0.0, 0.0, 0.0)),
        ((3.0, -1.0, 0.0), (2.0, 0.0, 0.0)),
        ((-1.0, 3.0, 0.0), (0.0, 2.0, 0.0)),
        ((1.0, -1.0, 0.0), (1.0, 0.0, 0.0)),
        ((-1.0, 1.0, 0.0), (0.0, 1.0, 0.0)),
        ((2.0, 2.0, 0.0), (1.0, 1.0, 0.0)),
        ((0.5, 0.5, 7.0), (0.5, 0.5, 0.0)),
    ];
    for (p, want) in cases {
        let (p, want) = (DVec3::from(p), DVec3::from(want));
        assert!(closest_on_triangle(p, tri).distance(want) <= 1e-12, "{p:?}");
    }
    let flat = [DVec3::ZERO, DVec3::X, DVec3::new(2.0, 0.0, 0.0)];
    assert!(closest_on_triangle(DVec3::new(3.0, 1.0, 0.0), flat).distance(DVec3::new(2.0, 0.0, 0.0)) <= 1e-12);
}

#[test]
fn hits_and_nearest_on_a_box() {
    let mesh = mesh_of(&make(&[rect(0.0, 0.0, 4.0, 2.0)]), 1.0);
    let bvh = Bvh::build(&mesh);
    let mut h = bvh.hits(&mesh, DVec3::new(-10.0, 0.5, 0.3), DVec3::X);
    h.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(h.len(), 2);
    assert!((h[0].0 - 10.0).abs() < 1e-9 && (h[1].0 - 14.0).abs() < 1e-9, "{h:?}");
    assert!(bvh.hits(&mesh, DVec3::new(-10.0, 5.0, 0.3), DVec3::X).is_empty());
    for i in 0..21 {
        for j in 0..21 {
            let (u, v) = (-2.0 + 2.2 * f64::from(i), -2.0 + 1.2 * f64::from(j));
            let rays = [
                (DVec3::new(u, v, 100.0), DVec3::new(0.0, 0.0, -1.0)),
                (DVec3::new(-100.0, v, -1.0 + 0.6 * f64::from(i)), DVec3::X),
                (DVec3::new(u, v, -100.0), DVec3::Z),
                (DVec3::new(100.0, v, -1.0 + 0.6 * f64::from(i)), -DVec3::X),
            ];
            for (origin, dir) in rays {
                let slow = (0..mesh.indices.len() / 3).filter(|&t| ray_triangle(origin, dir, corners(&mesh, t)).is_some()).count();
                assert_eq!(bvh.hits(&mesh, origin, dir).len(), slow, "{origin:?} {dir:?}");
            }
        }
    }
    let near = |p: (f64, f64, f64), want: (f64, f64, f64)| {
        let (q, _) = bvh.nearest(&mesh, DVec3::from(p)).unwrap();
        assert!(q.distance(DVec3::from(want)) <= 1e-12, "{p:?} gave {q:?}");
    };
    near((2.0, 1.0, 5.0), (2.0, 1.0, 1.0));
    near((-1.0, -1.0, -1.0), (0.0, 0.0, 0.0));
    near((2.0, 1.0, 0.3), (2.0, 1.0, 0.0));
    for i in 0..9 {
        for j in 0..9 {
            for k in 0..9 {
                let p = DVec3::new(-1.0 + 0.75 * f64::from(i), -1.0 + 0.5 * f64::from(j), -1.0 + 0.375 * f64::from(k));
                let dist = |t: usize| closest_on_triangle(p, corners(&mesh, t)).distance(p);
                let best = (0..mesh.indices.len() / 3).map(dist).fold(f64::INFINITY, f64::min);
                let (q, tri) = bvh.nearest(&mesh, p).unwrap();
                assert!((q.distance(p) - best).abs() <= 1e-12, "{p:?}");
                assert!((dist(tri) - best).abs() <= 1e-12, "{p:?}");
            }
        }
    }
}
