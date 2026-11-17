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
