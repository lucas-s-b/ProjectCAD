use super::*;
use cad_geometry::DVec3;

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs().max(1.0), "{a} vs {b}");
}

fn near3(a: DVec3, b: DVec3, tol: f64) {
    assert!(a.distance(b) <= tol, "{a} vs {b}");
}

fn m8() -> Metric {
    metric("M8").unwrap()
}

#[test]
fn size_table() {
    assert_eq!(METRIC.len(), 10);
    assert_eq!(m8(), Metric { name: "M8", d: 8.0, pitch: 1.25, s: 13.0, k: 5.3, m: 6.8 });
    assert_eq!(metric(" m10 ").unwrap().s, 16.0);
    assert_eq!(metric("M7"), None);
    assert_eq!(m8().thread(), "M8x1.25");
    assert_eq!(metric("M20").unwrap().thread(), "M20x2.5");
    assert_eq!(crate::num(30.0), "30");
    assert_eq!(num(1.25), "1.25");
    assert_eq!(num(0.1 + 0.2), "0.3");
    assert_eq!(num(100.0), "100");
}

#[test]
fn bolt_solid() {
    let bolt = Bolt { size: m8(), length: 30.0 };
    let b = bolt.body(&Plane3::XY, 0.001).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), 2283.6634278928223, 1e-3);
    let (lo, hi) = b.bbox();
    near3(lo, DVec3::new(-6.5, -7.505553499465135, -30.0), 1e-6);
    near3(hi, DVec3::new(6.5, 7.505553499465135, 5.3), 1e-6);
    assert_eq!(bolt.designation(), "ISO 4017 M8x30");
    let zero = Bolt { size: m8(), length: 0.0 };
    assert_eq!(zero.body(&Plane3::XY, 0.001), Err(ModelError::Invalid("bolt length must be positive")));
}

#[test]
fn nut_solid() {
    let nut = Nut { size: m8() };
    let b = nut.body(&Plane3::XY, 0.001).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), 653.4311133185073, 1e-3);
    let (lo, hi) = b.bbox();
    assert!(lo.z.abs() <= 1e-9 && (hi.z - 6.8).abs() <= 1e-9);
    assert_eq!(nut.designation(), "ISO 4032 M8");
    let up = Plane3 { origin: DVec3::new(0.0, 0.0, 10.0), ..Plane3::XY };
    let (lo, hi) = nut.body(&up, 0.001).unwrap().bbox();
    assert!((lo.z - 10.0).abs() <= 1e-9 && (hi.z - 16.8).abs() <= 1e-9);
}
