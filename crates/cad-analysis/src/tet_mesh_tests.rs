use super::*;
use crate::mass::tests::{box_mesh, join, prism};
use glam::DMat3;

fn box20() -> TriMesh {
    box_mesh(DVec3::ZERO, DVec3::new(20.0, 10.0, 10.0))
}

/// √3 (√2 − 1): the radius ratio of a Kuhn tetrahedron of a cube.
const KUHN_RATIO: f64 = 0.7174389352143;

fn err(m: &TetMesh, body: f64) -> f64 {
    (m.stats().volume - body) / body
}

fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} vs {b}");
}

#[test]
fn element_measures() {
    close(KUHN_RATIO, 3f64.sqrt() * (2f64.sqrt() - 1.0), 1e-12);
    let regular = [DVec3::new(1.0, 1.0, 1.0), DVec3::new(1.0, -1.0, -1.0), DVec3::new(-1.0, -1.0, 1.0), DVec3::new(-1.0, 1.0, -1.0)];
    close(tet_volume(regular), 8.0 / 3.0, 1e-12);
    close(radius_ratio(regular), 1.0, 1e-12);
    close(min_dihedral_angle(regular), 70.52877936550931, 1e-9);
    let kuhn = [DVec3::ZERO, DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::ONE];
    close(tet_volume(kuhn), 1.0 / 6.0, 1e-12);
    close(radius_ratio(kuhn), KUHN_RATIO, 1e-12);
    close(min_dihedral_angle(kuhn), 45.0, 1e-9);
    for row in KUHN {
        let p = row.map(|c| DVec3::new((c & 1) as f64, ((c >> 1) & 1) as f64, ((c >> 2) & 1) as f64));
        close(tet_volume(p), 1.0 / 6.0, 1e-15);
    }
    let flat = [DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::new(1.0, 1.0, 0.0)];
    assert_eq!((tet_volume(flat), radius_ratio(flat)), (0.0, 0.0));
    let swapped = [kuhn[0], kuhn[2], kuhn[1], kuhn[3]];
    close(tet_volume(swapped), -1.0 / 6.0, 1e-12);
    assert_eq!(radius_ratio(swapped), 0.0);
}

#[test]
fn box_meshes_exactly() {
    let m = tet_mesh(&box20(), 5.0).unwrap();
    assert_eq!(m.cell, DVec3::splat(5.0));
    assert_eq!((m.corners, m.nodes.len(), m.elements.len(), m.unsnapped), (45, 225, 96, 0));
    let s = m.stats();
    close(s.volume, 2000.0, 1e-9);
    close(s.min_ratio, KUHN_RATIO, 1e-12);
    close(s.min_dihedral, 45.0, 1e-9);
    assert_eq!(s.inverted, 0);
    assert_eq!(m.elements[0], [0, 1, 6, 21, 45, 46, 47, 48, 49, 50]);
    let want = [(2.5, 0.0, 0.0), (5.0, 2.5, 0.0), (2.5, 2.5, 0.0), (2.5, 2.5, 2.5), (5.0, 2.5, 2.5), (5.0, 5.0, 2.5)];
    for (q, w) in want.iter().enumerate() {
        assert_eq!(m.nodes[m.elements[0][4 + q] as usize], DVec3::from(*w));
    }
    for (q, w) in [(0.0, 0.0, 0.0), (5.0, 0.0, 0.0), (5.0, 5.0, 0.0), (5.0, 5.0, 5.0)].iter().enumerate() {
        assert_eq!(m.nodes[m.elements[0][q] as usize], DVec3::from(*w));
    }
    for k in 0..m.elements.len() {
        assert!(tet_volume(m.corners_of(k)) > 0.0);
        for (q, [a, b]) in TET10_EDGES.iter().enumerate() {
            let c = m.corners_of(k);
            assert!(m.nodes[m.elements[k][4 + q] as usize].distance((c[*a] + c[*b]) / 2.0) <= 1e-12);
        }
    }
    let b = m.boundary();
    assert_eq!(b.len(), 80);
    let mut area = 0.0;
    for t in &b {
        let [p, q, r] = t.map(|v| m.nodes[v as usize]);
        let n = (q - p).cross(r - p);
        area += n.length() / 2.0;
        assert!(n.dot((p + q + r) / 3.0 - DVec3::new(10.0, 5.0, 5.0)) > 0.0);
    }
    close(area, 1000.0, 1e-9);
    let coarse = tet_mesh(&box20(), 10.0).unwrap();
    assert_eq!((coarse.elements.len(), coarse.nodes.len()), (12, 45));
    let b22 = tet_mesh(&box_mesh(DVec3::ZERO, DVec3::new(22.0, 10.0, 10.0)), 5.0).unwrap();
    assert!(b22.cell.distance(DVec3::new(4.4, 5.0, 5.0)) <= 1e-12);
    assert_eq!((b22.corners, b22.nodes.len(), b22.elements.len()), (54, 275, 120));
    let s = b22.stats();
    close(s.volume, 2200.0, 1e-9);
    close(s.min_ratio, 0.711292554081177, 1e-9);
    close(s.min_dihedral, 41.34777721969365, 1e-9);
}

#[test]
fn rows_with_several_crossings() {
    let two = join(&box20(), &box_mesh(DVec3::new(30.0, 0.0, 0.0), DVec3::new(40.0, 10.0, 10.0)));
    let m = tet_mesh(&two, 5.0).unwrap();
    assert_eq!((m.elements.len(), m.nodes.len(), m.unsnapped), (144, 350, 0));
    close(m.stats().volume, 3000.0, 1e-9);
}

#[test]
fn cylinder_volume_and_quality() {
    let s = prism(64, 10.0, 20.0);
    let body = s.volume();
    close(body, 6273.096981091879, 1e-9);
    let m = tet_mesh(&s, 2.5).unwrap();
    assert_eq!(m.cell, DVec3::splat(2.5));
    assert_eq!((m.corners, m.nodes.len(), m.elements.len()), (617, 4049, 2432));
    let st = m.stats();
    assert_eq!(st.inverted, 0);
    assert!(st.min_ratio >= 0.15, "{}", st.min_ratio);
    assert!(err(&m, body).abs() <= 0.015, "{}", err(&m, body));
    let fine = tet_mesh(&s, 1.25).unwrap();
    assert_eq!((fine.corners, fine.nodes.len(), fine.elements.len()), (4021, 28949, 19424));
    let fs = fine.stats();
    assert_eq!(fs.inverted, 0);
    assert!(fs.min_ratio >= 0.15);
    assert!(err(&fine, body).abs() < err(&m, body).abs());
}

#[test]
fn turned_box_stays_valid() {
    let mut s = box20();
    let turn = DMat3::from_rotation_z(30f64.to_radians());
    for p in &mut s.positions {
        *p = turn * *p;
    }
    let m = tet_mesh(&s, 2.5).unwrap();
    assert_eq!((m.corners, m.nodes.len(), m.elements.len()), (264, 1545, 816));
    let st = m.stats();
    assert_eq!(st.inverted, 0);
    assert!(st.min_ratio >= 0.15, "{}", st.min_ratio);
    assert!(err(&m, 2000.0).abs() <= 0.06, "{}", err(&m, 2000.0));
}

#[test]
fn nodes_near_a_face() {
    let m = tet_mesh(&box20(), 5.0).unwrap();
    let v = |x: f64, y: f64, z: f64| DVec3::new(x, y, z);
    let x0 = TriMesh { positions: vec![v(0.0, 0.0, 0.0), v(0.0, 10.0, 0.0), v(0.0, 10.0, 10.0), v(0.0, 0.0, 10.0)], indices: vec![0, 1, 2, 0, 2, 3], normals: Vec::new() };
    let on = m.nodes_near(&x0, 1.25);
    assert_eq!(on.len(), 25);
    assert!(on.windows(2).all(|w| w[0] < w[1]));
    assert!(on.iter().all(|&i| m.nodes[i as usize].x == 0.0));
    let z0 = TriMesh { positions: vec![v(0.0, 0.0, 0.0), v(20.0, 0.0, 0.0), v(20.0, 10.0, 0.0), v(0.0, 10.0, 0.0)], indices: vec![0, 1, 2, 0, 2, 3], normals: Vec::new() };
    let on = m.nodes_near(&z0, 1.25);
    assert_eq!(on.len(), 45);
    assert!(on.iter().all(|&i| m.nodes[i as usize].z == 0.0));
}

#[test]
fn refusals() {
    for h in [0.0, -1.0, f64::NAN] {
        assert_eq!(tet_mesh(&box20(), h), Err("element size must be positive".to_string()));
    }
    let mut inside_out = box20();
    for t in inside_out.indices.chunks_mut(3) {
        t.swap(1, 2);
    }
    assert_eq!(tet_mesh(&inside_out, 5.0), Err("the body has no inside (open or inside-out surface)".to_string()));
    assert_eq!(tet_mesh(&box20(), 0.1), Err("element size 0.1 mm gives 2000000 grid cells (at most 50000); use a larger element size".to_string()));
}
