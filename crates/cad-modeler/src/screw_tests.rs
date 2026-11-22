use super::*;
use glam::DVec3;

/// The M8x1.25 ISO basic profile less 0.04 mm at 48 samples per pitch (what `cad_mech::thread_radii` gives a bolt).
fn m8() -> Vec<f64> {
    (0..48)
        .map(|i: usize| {
            let v = i.min(48 - i) as f64 * 1.25 / 48.0;
            3.96 - 3f64.sqrt() * (v - 1.25 / 16.0).clamp(0.0, 5.0 * 1.25 / 16.0)
        })
        .collect()
}
const BAD: ModelError = ModelError::Invalid("screw needs at least 3 positive radii, a positive pitch and z0 < z1");

fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} vs {b}");
}

#[test]
fn constant_radii_make_a_prism() {
    let b = screw(&[5.0; 12], 2.0, &Plane3::XY, 0.3, 2.0).unwrap();
    assert!(b.mesh.is_closed());
    close(b.mesh.volume(), 127.5, 1e-9);
    let (lo, hi) = b.bbox();
    assert!(lo.distance(DVec3::new(-5.0, -5.0, 0.3)) <= 1e-9, "{lo}");
    assert!(hi.distance(DVec3::new(5.0, 5.0, 2.0)) <= 1e-9, "{hi}");
    assert_eq!(b.faces.len(), 3);
    assert_eq!(b.faces[1].surface, Surface::Faceted);
}

#[test]
fn lattice_rod_keeps_profile_samples() {
    let b = screw(&m8(), 1.25, &Plane3::XY, 0.0, 12.5).unwrap();
    assert_eq!(b.mesh.positions.len(), 23090);
    assert_eq!(b.mesh.triangle_count(), 46176);
    close(b.mesh.volume(), 504.2365972775556, 5e-7);
    for p in &b.mesh.positions[1..23089] {
        let r = p.truncate().length();
        assert!((3.2834176532934074 - 1e-9..=3.96 + 1e-9).contains(&r), "{r}");
    }
    let p = b.mesh.positions[1 + 10 * 48];
    close(p.truncate().length(), 3.6442615715369233, 1e-9);
    let p = b.mesh.positions[1 + 30 * 48 + 6];
    close(p.truncate().length(), 3.2834176532934074, 1e-9);
    close(p.z, 0.78125, 1e-12);
}

#[test]
fn off_lattice_ends() {
    let b = screw(&m8(), 1.25, &Plane3::XY, 0.3, 2.0).unwrap();
    assert!(b.mesh.is_closed());
    close(b.mesh.volume(), 68.57616215088191, 1e-9);
    let (lo, hi) = b.bbox();
    close(lo.z, 0.3, 1e-12);
    close(hi.z, 2.0, 1e-12);
}

#[test]
fn screw_refuses_bad_input() {
    assert_eq!(screw(&[1.0, 1.0], 1.0, &Plane3::XY, 0.0, 1.0), Err(BAD));
    assert_eq!(screw(&[1.0; 4], 1.0, &Plane3::XY, 1.0, 1.0), Err(BAD));
    assert_eq!(screw(&[1.0, 0.0, 1.0], 1.0, &Plane3::XY, 0.0, 1.0), Err(BAD));
    assert_eq!(screw(&[1.0; 4], 0.0, &Plane3::XY, 0.0, 1.0), Err(BAD));
}
