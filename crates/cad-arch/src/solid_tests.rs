use super::*;
use crate::fixtures::v;
use cad_geometry::DVec3;

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

#[test]
fn wall_solid_is_area_times_height() {
    let f = [v(0.0, -100.0), v(4100.0, -100.0), v(3900.0, 100.0), v(0.0, 100.0)];
    let plane = Plane3 { origin: DVec3::new(0.0, 0.0, 3000.0), ..Plane3::XY };
    let b = wall_body(&f, &plane, 2500.0, 0.02).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), 2.0e9);
    let (lo, hi) = b.bbox();
    close(lo.z, 3000.0);
    close(hi.z, 5500.0);
}

#[test]
fn hub_outline_makes_a_solid() {
    let f = [v(0.0, -100.0), v(2000.0, -100.0), v(2000.0, 100.0), v(100.0, 100.0), v(0.0, 0.0)];
    let b = wall_body(&f, &Plane3::XY, 1000.0, 0.02).unwrap();
    close(b.volume(), 3.95e8);
}

#[test]
fn degenerate_outline_is_refused() {
    let r = wall_body(&[v(0.0, 0.0), v(1.0, 0.0), v(2.0, 0.0)], &Plane3::XY, 1000.0, 0.02);
    assert!(matches!(r, Err(ModelError::Invalid("wall outline is degenerate"))));
}
