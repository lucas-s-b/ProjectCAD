use super::*;
use crate::shape::{GeomError, Shape};
use crate::xform::mirror_across;
use crate::{Tolerance, distance_2d};
use glam::DVec3;

/// 16 risers of 180 over 2880, 15 treads of 200 along +x, 1000 wide.
fn stair() -> Stair {
    Stair { a: DVec2::ZERO, b: DVec2::new(3000.0, 0.0), width: 1000.0, height: 2880.0, risers: 16 }
}

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn near(a: DVec2, b: DVec2) {
    assert!(a.distance(b) <= 1e-9, "{a} vs {b}");
}

#[test]
fn stair_geometry() {
    let s = stair();
    assert!((s.tread() - 200.0).abs() <= 1e-9);
    assert!((s.riser() - 180.0).abs() <= 1e-9);
    let want = [v(0.0, -500.0), v(3000.0, -500.0), v(3000.0, 500.0), v(0.0, 500.0)];
    s.corners().iter().zip(want).for_each(|(a, b)| near(*a, b));
    let sym = s.symbol();
    assert_eq!(sym.len(), 21);
    let pair = |i: usize, a: DVec2, b: DVec2| {
        near(sym[i][0], a);
        near(sym[i][1], b);
    };
    pair(2, v(0.0, -500.0), v(0.0, 500.0));
    pair(17, v(3000.0, -500.0), v(3000.0, 500.0));
    pair(19, v(3000.0, 0.0), v(2750.0, 125.0));
    pair(20, v(3000.0, 0.0), v(2750.0, -125.0));
}

#[test]
fn risers_from_height() {
    assert_eq!(Stair::risers_for(2880.0, 180.0), 16);
    assert_eq!(Stair::risers_for(3000.0, 180.0), 17);
    assert_eq!(Stair::risers_for(100.0, 180.0), 2);
    assert_eq!(Stair::risers_for(25.0, 2.5), 10);
}

#[test]
fn stair_shape() {
    let tol = Tolerance::default();
    let s = Shape::Stair(stair());
    assert_eq!(s.validate(&tol), Ok(()));
    assert_eq!(s.kind(), "Stair");
    assert!(s.needs_level());
    assert!(distance_2d(&s, v(1500.0, 0.0)).abs() <= 1e-9);
    assert!((distance_2d(&s, v(1500.0, 520.0)) - 20.0).abs() <= 1e-9);
    let (lo, hi) = s.bbox();
    assert_eq!((lo, hi), (DVec3::new(0.0, -500.0, 0.0), DVec3::new(3000.0, 500.0, 0.0)));

    let mut moved = s.clone();
    moved.translate(DVec3::new(100.0, 0.0, 0.0));
    let Shape::Stair(m) = moved else { panic!("not a stair") };
    assert_eq!((m.a, m.b), (v(100.0, 0.0), v(3100.0, 0.0)));

    assert_eq!(s.transformed(&mirror_across(DVec2::ZERO, DVec2::X)).unwrap(), s);
    assert_eq!(
        Shape::Stair(Stair { risers: 1, ..stair() }).validate(&tol),
        Err(GeomError::Degenerate("stair needs at least two risers"))
    );
    assert_eq!(
        Shape::Stair(Stair { width: 0.0, ..stair() }).validate(&tol),
        Err(GeomError::Degenerate("stair needs a run, a width and a height"))
    );
}
