use super::*;
use crate::fixtures::v;
use crate::ifc_class;
use cad_geometry::{DVec3, RoofKind, Shape};

fn close6(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-6 * b.abs().max(1.0), "{a} vs {b}");
}

/// The rectangle (0,0)-(8000,4000), 45 degrees, 200 thick.
fn rect(kind: RoofKind, overhang: f64) -> Roof {
    Roof { outline: vec![v(0.0, 0.0), v(8000.0, 0.0), v(8000.0, 4000.0), v(0.0, 4000.0)], kind, pitch: 45.0, thickness: 200.0, overhang }
}

fn body(r: &Roof) -> Body {
    roof_body(r, &Plane3::XY, 3000.0, 0.02).unwrap()
}

fn stair() -> Stair {
    Stair { a: v(0.0, 0.0), b: v(3000.0, 0.0), width: 1000.0, height: 2880.0, risers: 16 }
}

#[test]
fn flat_roof_sits_on_the_level_top() {
    let b = body(&rect(RoofKind::Flat, 0.0));
    assert!(b.mesh.is_closed());
    close6(b.volume(), 6.4e9);
    let (lo, hi) = b.bbox();
    close6(lo.z, 3000.0);
    close6(hi.z, 3200.0);

    let b = body(&rect(RoofKind::Flat, 500.0));
    close6(b.volume(), 9.0e9);
    let (lo, hi) = b.bbox();
    close6(lo.x, -500.0);
    close6(hi.x, 8500.0);
}

#[test]
fn pitched_roofs_are_thick_at_right_angles() {
    for (kind, ridge) in [(RoofKind::Shed, 4000.0), (RoofKind::Gable, 2000.0), (RoofKind::Hip, 2000.0)] {
        let b = body(&rect(kind, 0.0));
        assert!(b.mesh.is_closed(), "{kind:?}");
        close6(b.volume(), 6.4e9 * 2f64.sqrt());
        let (lo, hi) = b.bbox();
        close6(lo.z, 3000.0);
        close6(hi.z, 3000.0 + ridge + 200.0 * 2f64.sqrt());
    }
}

#[test]
fn overhang_hangs_lower() {
    let b = body(&rect(RoofKind::Gable, 500.0));
    close6(b.volume(), 9.0e9 * 2f64.sqrt());
    let (lo, hi) = b.bbox();
    close6(lo.z, 2500.0);
    close6(hi.z, 5000.0 + 200.0 * 2f64.sqrt());
    close6(lo.x, -500.0);
    close6(hi.x, 8500.0);
}

#[test]
fn stair_is_stepped() {
    let b = stair_body(&stair(), &Plane3::XY, 0.02).unwrap();
    assert!(b.mesh.is_closed());
    close6(b.volume(), 4.32e9);
    let (lo, hi) = b.bbox();
    for (got, want) in [(lo, DVec3::new(0.0, -500.0, 0.0)), (hi, DVec3::new(3000.0, 500.0, 2700.0))] {
        close6(got.x, want.x);
        close6(got.y, want.y);
        close6(got.z, want.z);
    }

    let s = Stair { b: v(0.0, 3000.0), ..stair() };
    let b = stair_body(&s, &Plane3 { origin: DVec3::new(0.0, 0.0, 3000.0), ..Plane3::XY }, 0.02).unwrap();
    close6(b.volume(), 4.32e9);
    let (lo, hi) = b.bbox();
    for (got, want) in [(lo, DVec3::new(-500.0, 0.0, 3000.0)), (hi, DVec3::new(500.0, 3000.0, 5700.0))] {
        close6(got.x, want.x);
        close6(got.y, want.y);
        close6(got.z, want.z);
    }
}

#[test]
fn ifc_classes_of_roofs_and_stairs() {
    assert_eq!(ifc_class(&Shape::Roof(rect(RoofKind::Hip, 0.0))), Some("IfcRoof"));
    assert_eq!(ifc_class(&Shape::Stair(stair())), Some("IfcStair"));
}
