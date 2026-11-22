use super::*;
use crate::shape::{GeomError, Shape};
use crate::snap::snap_points;
use crate::xform::mirror_across;
use crate::{Tolerance, distance_2d};

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

/// The rectangle (0,0)-(8000,4000), counter-clockwise, 45 degrees, 200 thick.
fn rect(kind: RoofKind, overhang: f64) -> Roof {
    Roof { outline: vec![v(0.0, 0.0), v(8000.0, 0.0), v(8000.0, 4000.0), v(0.0, 4000.0)], kind, pitch: 45.0, thickness: 200.0, overhang }
}

fn areas(r: &Roof) -> Vec<f64> {
    r.pieces().iter().map(|p| shoelace(&p.outline)).collect()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

fn close_all(a: &[f64], b: &[f64]) {
    assert_eq!(a.len(), b.len(), "{a:?} vs {b:?}");
    a.iter().zip(b).for_each(|(x, y)| close(*x, *y));
}

fn near(a: DVec2, b: DVec2, tol: f64) {
    assert!(a.distance(b) <= tol, "{a} vs {b}");
}

#[test]
fn pieces_by_kind() {
    let flat = rect(RoofKind::Flat, 0.0).pieces();
    assert_eq!(flat.len(), 1);
    assert_eq!(flat[0].eave, None);
    close(shoelace(&flat[0].outline), 3.2e7);

    let shed = rect(RoofKind::Shed, 0.0).pieces();
    assert_eq!(shed.len(), 1);
    close(shoelace(&shed[0].outline), 3.2e7);
    assert_eq!(shed[0].eave, Some((v(0.0, 0.0), v(1.0, 0.0))));

    let gable = rect(RoofKind::Gable, 0.0);
    close_all(&areas(&gable), &[1.6e7, 1.6e7]);
    let p = gable.pieces();
    assert_eq!(p[0].eave.unwrap().1, v(1.0, 0.0));
    assert_eq!(p[1].eave.unwrap().1, v(-1.0, 0.0));

    close_all(&areas(&rect(RoofKind::Hip, 0.0)), &[1.2e7, 4.0e6, 1.2e7, 4.0e6]);

    let square = Roof { outline: vec![v(0.0, 0.0), v(4000.0, 0.0), v(4000.0, 4000.0), v(0.0, 4000.0)], ..rect(RoofKind::Gable, 0.0) };
    close_all(&areas(&square), &[8.0e6, 8.0e6]);
    assert_eq!(square.pieces()[0].eave.unwrap().1, v(1.0, 0.0));
}

#[test]
fn overhang_moves_the_eaves() {
    let e = rect(RoofKind::Flat, 500.0).eaves();
    let want = [v(-500.0, -500.0), v(8500.0, -500.0), v(8500.0, 4500.0), v(-500.0, 4500.0)];
    assert_eq!(e.len(), 4);
    e.iter().zip(want).for_each(|(a, b)| near(*a, b, 1e-9));
    close_all(&areas(&rect(RoofKind::Gable, 500.0)), &[2.25e7, 2.25e7]);
    let (lo, hi) = Shape::Roof(rect(RoofKind::Gable, 500.0)).bbox();
    near(lo.truncate(), v(-500.0, -500.0), 1e-9);
    near(hi.truncate(), v(8500.0, 4500.0), 1e-9);
    assert_eq!((lo.z, hi.z), (0.0, 0.0));
}

#[test]
fn clockwise_and_mirrored_sheds_keep_their_low_edge() {
    let cw = Roof { outline: vec![v(0.0, 0.0), v(0.0, 4000.0), v(8000.0, 4000.0), v(8000.0, 0.0)], ..rect(RoofKind::Shed, 0.0) };
    let p = cw.pieces();
    assert_eq!(p.len(), 1);
    close(shoelace(&p[0].outline), 3.2e7);
    assert_eq!(p[0].eave, Some((v(0.0, 4000.0), v(0.0, -1.0))));

    let Shape::Roof(m) = Shape::Roof(rect(RoofKind::Shed, 0.0)).transformed(&mirror_across(DVec2::ZERO, DVec2::Y)).unwrap() else {
        panic!("not a roof")
    };
    let p = m.pieces();
    assert_eq!(p.len(), 1);
    let (a, d) = p[0].eave.unwrap();
    near(a, v(-8000.0, 0.0), 1e-12);
    near(d, v(1.0, 0.0), 1e-12);
    close(shoelace(&p[0].outline), 3.2e7);
}

#[test]
fn roof_validation() {
    let tol = Tolerance::default();
    let s = Shape::Roof(rect(RoofKind::Gable, 0.0));
    assert_eq!(s.validate(&tol), Ok(()));
    assert_eq!(s.kind(), "Roof");
    assert!(s.needs_level());

    let l = vec![v(0.0, 0.0), v(8000.0, 0.0), v(8000.0, 4000.0), v(4000.0, 4000.0), v(4000.0, 8000.0), v(0.0, 8000.0)];
    let with = |kind| Shape::Roof(Roof { outline: l.clone(), ..rect(kind, 0.0) });
    assert_eq!(with(RoofKind::Hip).validate(&tol), Err(GeomError::Degenerate("gable and hip roofs need a convex outline")));
    assert_eq!(with(RoofKind::Flat).validate(&tol), Ok(()));
    assert_eq!(with(RoofKind::Shed).validate(&tol), Ok(()));

    let pitch = |kind, pitch| Shape::Roof(Roof { pitch, ..rect(kind, 0.0) });
    assert_eq!(
        pitch(RoofKind::Gable, 0.0).validate(&tol),
        Err(GeomError::Degenerate("roof pitch must be above 0 and at most 75 degrees"))
    );
    assert_eq!(pitch(RoofKind::Flat, 0.0).validate(&tol), Ok(()));
    assert_eq!(pitch(RoofKind::Gable, f64::NAN).validate(&tol), Err(GeomError::NonFinite));

    let msg = GeomError::Degenerate("roof thickness must be positive and its overhang not negative");
    assert_eq!(Shape::Roof(Roof { thickness: 0.0, ..rect(RoofKind::Flat, 0.0) }).validate(&tol), Err(msg.clone()));
    assert_eq!(Shape::Roof(rect(RoofKind::Flat, -1.0)).validate(&tol), Err(msg));
}

#[test]
fn roof_picks_and_snaps_on_its_planes() {
    let s = Shape::Roof(rect(RoofKind::Gable, 0.0));
    assert!((distance_2d(&s, v(1000.0, 1500.0)) - 500.0).abs() <= 1e-9);
    assert_eq!(snap_points(&s).len(), 4);
}
