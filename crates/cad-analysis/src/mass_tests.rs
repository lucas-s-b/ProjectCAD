use super::*;
use glam::DQuat;

/// Box between `lo` and `hi`: 8 vertices, 12 outward triangles.
pub(crate) fn box_mesh(lo: DVec3, hi: DVec3) -> TriMesh {
    let p = |x: f64, y: f64, z: f64| DVec3::new(if x > 0.0 { hi.x } else { lo.x }, if y > 0.0 { hi.y } else { lo.y }, if z > 0.0 { hi.z } else { lo.z });
    let positions = vec![p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.), p(0., 0., 1.), p(1., 0., 1.), p(1., 1., 1.), p(0., 1., 1.)];
    let indices = vec![0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3, 0, 4, 3, 4, 7];
    TriMesh { positions, indices, normals: Vec::new() }
}

/// Both meshes in one (disjoint solids).
fn join(a: &TriMesh, b: &TriMesh) -> TriMesh {
    let n = a.positions.len() as u32;
    TriMesh {
        positions: a.positions.iter().chain(&b.positions).copied().collect(),
        indices: a.indices.iter().copied().chain(b.indices.iter().map(|i| i + n)).collect(),
        normals: Vec::new(),
    }
}

/// Regular `n`-gon prism, circumradius `r`, z 0..h, axis Z: a faceted cylinder.
pub(crate) fn prism(n: usize, r: f64, h: f64) -> TriMesh {
    let mut positions = vec![DVec3::ZERO, DVec3::new(0.0, 0.0, h)];
    for k in 0..n {
        let a = std::f64::consts::TAU * k as f64 / n as f64;
        positions.push(DVec3::new(r * a.cos(), r * a.sin(), 0.0));
        positions.push(DVec3::new(r * a.cos(), r * a.sin(), h));
    }
    let mut indices = Vec::new();
    for k in 0..n as u32 {
        let (p0, p1, q0, q1) = (2 + 2 * k, 3 + 2 * k, 2 + 2 * ((k + 1) % n as u32), 3 + 2 * ((k + 1) % n as u32));
        indices.extend([0, q0, p0, 1, p1, q1, p0, q0, q1, p0, q1, p1]);
    }
    TriMesh { positions, indices, normals: Vec::new() }
}

/// Every entry of `a` within `t` of `b`.
fn mat_close(a: DMat3, b: DMat3, t: f64) {
    for (x, y) in a.to_cols_array().iter().zip(b.to_cols_array()) {
        assert!((x - y).abs() <= t, "{a} vs {b}");
    }
}

fn rel(a: f64, b: f64, r: f64) -> bool {
    (a - b).abs() <= r * b.abs()
}

fn steel_box() -> MassProps {
    MassProps::of(&box_mesh(DVec3::ZERO, DVec3::new(40.0, 20.0, 10.0)), 7.85).unwrap()
}

#[test]
fn box_mass_properties() {
    let m = steel_box();
    assert!(rel(m.volume, 8000.0, 1e-9));
    assert!(rel(m.mass, 62.8, 1e-9));
    assert!(m.com.distance(DVec3::new(20.0, 10.0, 5.0)) <= 1e-9);
    mat_close(m.inertia, DMat3::from_diagonal(DVec3::new(2616.6666666666667, 8896.666666666667, 10466.666666666667)), 1e-6);
}

#[test]
fn parallel_axis_to_a_corner() {
    let m = steel_box();
    let expect = DMat3::from_cols(
        DVec3::new(10466.666666666667, -12560.0, -6280.0),
        DVec3::new(-12560.0, 35586.666666666667, -3140.0),
        DVec3::new(-6280.0, -3140.0, 41866.666666666667),
    );
    mat_close(m.inertia_at(DVec3::ZERO), expect, 1e-6);
}

#[test]
fn far_from_the_origin() {
    let o = 1e4;
    let m = MassProps::of(&box_mesh(DVec3::splat(o), DVec3::new(o + 40.0, o + 20.0, o + 10.0)), 7.85).unwrap();
    assert!(m.com.distance(DVec3::new(10020.0, 10010.0, 10005.0)) <= 1e-6);
    mat_close(m.inertia, steel_box().inertia, 1e-5);
}

#[test]
fn rotated_box() {
    let mut mesh = box_mesh(DVec3::new(-20.0, -10.0, -5.0), DVec3::new(20.0, 10.0, 5.0));
    let q = DQuat::from_rotation_z(30f64.to_radians());
    for p in &mut mesh.positions {
        *p = q * *p;
    }
    let m = MassProps::of(&mesh, 7.85).unwrap();
    let expect = DMat3::from_cols(
        DVec3::new(4186.666666666667, -2719.3197678831373, 0.0),
        DVec3::new(-2719.3197678831373, 7326.666666666667, 0.0),
        DVec3::new(0.0, 0.0, 10466.666666666667),
    );
    mat_close(m.inertia, expect, 1e-6);
}

#[test]
fn faceted_cylinder() {
    let m = MassProps::of(&prism(64, 10.0, 20.0), 1.0).unwrap();
    assert!(rel(m.volume, 6273.096981091878, 1e-9));
    assert!(m.com.distance(DVec3::new(0.0, 0.0, 10.0)) <= 1e-9);
    assert!(rel(m.inertia.x_axis.x, 365.67893492556146, 1e-9));
    assert!(rel(m.inertia.y_axis.y, 365.67893492556146, 1e-9));
    assert!(rel(m.inertia.z_axis.z, 313.1514044449977, 1e-9));
    let low = (6283.185307179586 - m.volume) / 6283.185307179586;
    assert!((0.0015..0.0017).contains(&low), "{low}");
}

#[test]
fn combine_two_cubes() {
    let a = box_mesh(DVec3::ZERO, DVec3::splat(10.0));
    let b = box_mesh(DVec3::new(20.0, 0.0, 0.0), DVec3::new(30.0, 10.0, 10.0));
    let (pa, pb) = (MassProps::of(&a, 1.0).unwrap(), MassProps::of(&b, 1.0).unwrap());
    let c = MassProps::combine(&[pa, pb]).unwrap();
    let expect = DMat3::from_diagonal(DVec3::new(33.333333333333336, 233.33333333333334, 233.33333333333334));
    assert!((c.mass - 2.0).abs() <= 1e-12);
    assert!((c.volume - 2000.0).abs() <= 1e-9);
    assert!(c.com.distance(DVec3::new(15.0, 5.0, 5.0)) <= 1e-9);
    mat_close(c.inertia, expect, 1e-9);
    let j = MassProps::of(&join(&a, &b), 1.0).unwrap();
    assert!((j.mass - c.mass).abs() <= 1e-9);
    assert!(j.com.distance(c.com) <= 1e-9);
    mat_close(j.inertia, c.inertia, 1e-9);
    assert_eq!(MassProps::combine(&[]), None);
}

#[test]
fn bad_meshes() {
    assert_eq!(MassProps::of(&TriMesh::default(), 1.0), None);
    let mut m = box_mesh(DVec3::ZERO, DVec3::splat(1.0));
    for t in m.indices.as_chunks_mut::<3>().0 {
        t.swap(1, 2);
    }
    assert_eq!(MassProps::of(&m, 1.0), None);
}

fn close3(a: [f64; 3], b: [f64; 3], t: f64) {
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() <= t, "{a:?} vs {b:?}");
    }
}

fn orthonormal(axes: &[DVec3; 3]) {
    for i in 0..3 {
        for j in 0..3 {
            let want = if i == j { 1.0 } else { 0.0 };
            assert!((axes[i].dot(axes[j]) - want).abs() <= 1e-12);
        }
    }
}

#[test]
fn principal_of_an_axis_aligned_box() {
    let (m, axes) = steel_box().principal();
    close3(m, [2616.6666666666667, 8896.666666666667, 10466.666666666667], 1e-6);
    for (a, e) in axes.iter().zip([DVec3::X, DVec3::Y, DVec3::Z]) {
        assert!(a.distance(e) <= 1e-9, "{a} vs {e}");
    }
}

#[test]
fn principal_of_a_rotated_box() {
    let mut mesh = box_mesh(DVec3::new(-20.0, -10.0, -5.0), DVec3::new(20.0, 10.0, 5.0));
    let q = DQuat::from_rotation_z(30f64.to_radians());
    for p in &mut mesh.positions {
        *p = q * *p;
    }
    let (m, axes) = MassProps::of(&mesh, 7.85).unwrap().principal();
    close3(m, [2616.6666666666667, 8896.666666666667, 10466.666666666667], 1e-6);
    assert!(axes[0].distance(DVec3::new(0.8660254037844387, 0.5, 0.0)) <= 1e-9, "{}", axes[0]);
    assert!(axes[1].distance(DVec3::new(-0.5, 0.8660254037844387, 0.0)) <= 1e-9, "{}", axes[1]);
    assert!(axes[2].distance(DVec3::Z) <= 1e-9, "{}", axes[2]);
}

#[test]
fn principal_of_a_general_tensor() {
    let inertia = DMat3::from_cols(DVec3::new(2.0, 1.0, 0.5), DVec3::new(1.0, 3.0, 0.2), DVec3::new(0.5, 0.2, 1.0));
    let p = MassProps { volume: 1.0, mass: 1.0, com: DVec3::ZERO, inertia };
    let (m, axes) = p.principal();
    close3(m, [0.7674170126131582, 1.543652512300615, 3.6889304750862264], 1e-12);
    for k in 0..3 {
        assert!((inertia * axes[k]).distance(axes[k] * m[k]) <= 1e-12);
    }
    orthonormal(&axes);
    assert!(axes[0].cross(axes[1]).distance(axes[2]) <= 1e-12);
}

#[test]
fn principal_of_a_cube() {
    let (m, axes) = MassProps::of(&box_mesh(DVec3::ZERO, DVec3::splat(10.0)), 1.0).unwrap().principal();
    close3(m, [16.666666666666668; 3], 1e-9);
    orthonormal(&axes);
}
