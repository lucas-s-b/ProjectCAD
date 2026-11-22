use super::*;
use crate::shape::{GeomError, Shape};
use crate::snap::snap_points;
use crate::xform::mirror_across;
use crate::{Tolerance, distance_2d};
use glam::DVec3;

fn room() -> Room {
    Room { seed: DVec2::new(1000.0, 500.0), name: "Kitchen".into(), number: "1".into() }
}

/// The square (0,0)-(1000,1000), counter-clockwise, 200 thick.
fn slab() -> Slab {
    Slab { outline: vec![DVec2::ZERO, DVec2::new(1000.0, 0.0), DVec2::new(1000.0, 1000.0), DVec2::new(0.0, 1000.0)], thickness: 200.0 }
}

#[test]
fn room_is_its_seed() {
    let tol = Tolerance::default();
    let mut s = Shape::Room(room());
    assert_eq!(s.validate(&tol), Ok(()));
    assert_eq!(s.kind(), "Room");
    assert!(s.needs_level());
    assert_eq!(s.bbox(), (DVec3::new(1000.0, 500.0, 0.0), DVec3::new(1000.0, 500.0, 0.0)));
    assert!((distance_2d(&s, DVec2::new(1003.0, 504.0)) - 5.0).abs() < 1e-9);
    let Shape::Room(m) = s.transformed(&mirror_across(DVec2::ZERO, DVec2::Y)).unwrap() else { panic!() };
    assert!(m.seed.distance(DVec2::new(-1000.0, 500.0)) < 1e-12);
    assert_eq!(m.number, "1");
    s.translate(DVec3::new(100.0, 200.0, 0.0));
    let Shape::Room(r) = &s else { panic!() };
    assert_eq!(r.seed, DVec2::new(1100.0, 700.0));
    assert_eq!(r.name, "Kitchen");
    let bad = Shape::Room(Room { seed: DVec2::new(f64::NAN, 0.0), ..room() });
    assert_eq!(bad.validate(&tol), Err(GeomError::NonFinite));
}

#[test]
fn slab_checks_its_outline() {
    let tol = Tolerance::default();
    assert_eq!(slab().area(), 1.0e6);
    let s = Shape::Slab(slab());
    assert_eq!(s.validate(&tol), Ok(()));
    assert_eq!(s.kind(), "Floor");
    assert!(s.needs_level());
    let thin = Shape::Slab(Slab { thickness: 0.0, ..slab() });
    assert_eq!(thin.validate(&tol), Err(GeomError::Degenerate("slab thickness must be positive")));
    let msg = "slab outline needs three points and an area";
    let two = Shape::Slab(Slab { outline: vec![DVec2::ZERO, DVec2::X], ..slab() });
    assert_eq!(two.validate(&tol), Err(GeomError::Degenerate(msg)));
    let flat = Shape::Slab(Slab { outline: vec![DVec2::ZERO, DVec2::new(1000.0, 0.0), DVec2::new(2000.0, 0.0)], ..slab() });
    assert_eq!(flat.validate(&tol), Err(GeomError::Degenerate(msg)));
    let nan = Shape::Slab(Slab { thickness: f64::NAN, ..slab() });
    assert_eq!(nan.validate(&tol), Err(GeomError::NonFinite));
}

#[test]
fn slab_picks_bounds_and_snaps_on_its_outline() {
    let s = Shape::Slab(slab());
    assert!((distance_2d(&s, DVec2::new(500.0, 500.0)) - 500.0).abs() < 1e-9);
    assert!((distance_2d(&s, DVec2::new(500.0, -20.0)) - 20.0).abs() < 1e-9);
    assert_eq!(s.bbox(), (DVec3::ZERO, DVec3::new(1000.0, 1000.0, 0.0)));
    assert_eq!(snap_points(&s).len(), 4);
    assert!(!Shape::Line { a: DVec2::ZERO, b: DVec2::X }.needs_level());
}
