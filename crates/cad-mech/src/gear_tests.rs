use super::*;
use cad_geometry::DVec3;

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs().max(1.0), "{a} vs {b}");
}

fn gear(module: f64, teeth: u32, bore: f64) -> Gear {
    Gear { module, teeth, pressure: 20f64.to_radians(), width: 10.0, bore }
}

fn invalid(g: Gear, msg: &'static str) {
    assert_eq!(g.profile(), Err(ModelError::Invalid(msg)));
}

#[test]
fn gear_diameters() {
    let g = gear(2.0, 20, 8.0);
    assert_eq!(g.pitch_diameter(), 40.0);
    assert_eq!(g.tip_diameter(), 44.0);
    assert_eq!(g.root_diameter(), 35.0);
    close(g.base_diameter(), 37.58770483143634, 1e-12);
    assert_eq!(g.designation(), "Spur gear m2 z20 b10");
}

#[test]
fn gear_profile_area() {
    let p = gear(2.0, 20, 8.0).profile().unwrap();
    assert_eq!(p.outer.len(), 480);
    assert_eq!(p.holes.len(), 1);
    close(p.area(), 1175.514129241726, 1e-9);
    close(p.area(), 1175.840217152105, 1e-3);
    close(gear(2.0, 24, 8.0).profile().unwrap().area(), 1724.718256226141, 1e-9);
    let p = gear(1.0, 50, 0.0).profile().unwrap();
    assert_eq!(p.outer.len(), 1100);
    assert!(p.holes.is_empty());
    close(p.area(), 1950.734205805349, 1e-9);
}

#[test]
fn gear_body() {
    let g = gear(2.0, 20, 8.0);
    let b = g.body(&Plane3::XY, 0.001).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), 11755.14129241726, 1e-3);
    let (lo, hi) = b.bbox();
    assert!((hi.x - 22.0).abs() <= 1e-3, "{}", hi.x);
    assert!(lo.z.abs() <= 1e-9 && (hi.z - 10.0).abs() <= 1e-9);
    let up = Plane3 { origin: DVec3::new(0.0, 0.0, 5.0), ..Plane3::XY };
    let (lo, hi) = g.body(&up, 0.001).unwrap().bbox();
    assert!((lo.z - 5.0).abs() <= 1e-9 && (hi.z - 15.0).abs() <= 1e-9);
}

#[test]
fn gear_validation() {
    invalid(gear(2.0, 3, 0.0), "gear needs 4 to 1000 teeth");
    invalid(gear(0.0, 20, 0.0), "gear module and face width must be positive");
    invalid(gear(2.0, 20, 35.0), "gear bore must be smaller than the root diameter");
    invalid(Gear { pressure: 5f64.to_radians(), ..gear(2.0, 20, 0.0) }, "gear pressure angle must be from 10 to 35 degrees");
    invalid(Gear { pressure: 35f64.to_radians(), ..gear(2.0, 6, 0.0) }, "gear teeth are pointed");
    invalid(Gear { pressure: 35f64.to_radians(), ..gear(1.0, 100, 0.0) }, "gear teeth overlap");
    assert!(gear(2.0, 4, 0.0).profile().is_ok());
}
