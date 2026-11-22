use super::*;

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs().max(1.0), "{a} vs {b}");
}

fn near3(a: DVec3, b: DVec3, tol: f64) {
    assert!(a.distance(b) <= tol, "{a} vs {b}");
}

fn spring(turns: f64) -> Spring {
    Spring { wire: 2.0, mean: 20.0, pitch: 5.0, turns }
}

#[test]
fn spring_helix() {
    let h = spring(4.0).helix(&Plane3::XY);
    assert_eq!(h.len(), 193);
    near3(h[0], DVec3::new(10.0, 0.0, 0.0), 1e-9);
    near3(h[192], DVec3::new(10.0, 0.0, 20.0), 1e-9);
    let h = spring(2.5).helix(&Plane3::XY);
    assert_eq!(h.len(), 121);
    near3(h[120], DVec3::new(-10.0, 0.0, 12.5), 1e-9);
    assert_eq!(spring(4.0).free_length(), 22.0);
    assert_eq!(spring(4.0).designation(), "Compression spring d2 D20 L22");
    assert_eq!(spring(6.0).designation(), "Compression spring d2 D20 L32");
}

#[test]
fn spring_volume() {
    let b = spring(4.0).body(&Plane3::XY, 0.02).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), 782.4920733690758, 1e-6);
    close(b.volume(), 792.0644067107494, 2e-2);
    let (_, hi) = b.bbox();
    assert!(hi.x > 10.9 && hi.x < 11.01, "{}", hi.x);
}

#[test]
fn spring_validation() {
    let p = Plane3::XY;
    let low = Spring { pitch: 2.0, ..spring(4.0) };
    assert_eq!(low.body(&p, 0.02), Err(ModelError::Invalid("spring pitch must be larger than the wire diameter")));
    let narrow = Spring { mean: 3.0, ..spring(4.0) };
    assert_eq!(narrow.body(&p, 0.02), Err(ModelError::Invalid("spring mean diameter must be at least twice the wire diameter")));
    let none = spring(0.0);
    assert_eq!(none.body(&p, 0.02), Err(ModelError::Invalid("spring wire diameter and turns must be positive, at most 200 turns")));
}
