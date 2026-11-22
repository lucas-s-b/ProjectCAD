use super::*;
use crate::{Bolt, Nut, metric, polygon};
use cad_geometry::profile::Profile;
use cad_geometry::{DAffine3, DVec2};
use cad_modeler::{Plane3, extrude};
use std::f64::consts::PI;

const THICK: f64 = 0.04;

/// The box lo..hi, an extruded rectangle tessellated within 0.02.
fn block(lo: [f64; 3], hi: [f64; 3]) -> Body {
    let (lo, d) = (DVec3::from(lo), DVec3::from(hi) - DVec3::from(lo));
    let outer = polygon(&[DVec2::ZERO, DVec2::new(d.x, 0.0), DVec2::new(d.x, d.y), DVec2::new(0.0, d.y)]);
    let brep = extrude(&Profile { outer, holes: Vec::new() }, &Plane3::XY, d.z).unwrap();
    Body::from_brep(&brep, 0.02).unwrap().transformed(&DAffine3::from_translation(lo)).unwrap()
}

fn volume(o: Result<Option<Overlap>, ModelError>) -> f64 {
    match o {
        Ok(Some(Overlap::Interference { volume, .. })) => volume,
        other => panic!("expected interference, got {other:?}"),
    }
}

fn a() -> Body {
    block([0.0; 3], [10.0; 3])
}

fn within(x: f64, y: f64, t: f64) -> bool {
    (x - y).abs() <= t
}

#[test]
fn overlapping_blocks_interfere() {
    let a = a();
    let o = overlap(&a, &block([5.0; 3], [15.0; 3]), THICK);
    let Ok(Some(Overlap::Interference { volume: v, region })) = o else { panic!("expected interference, got {o:?}") };
    assert!(within(v, 125.0, 1e-6), "{v}");
    let (lo, hi) = region.bbox();
    assert!(lo.distance(DVec3::splat(5.0)) <= 1e-9 && hi.distance(DVec3::splat(10.0)) <= 1e-9);
    let v = volume(overlap(&a, &block([9.9, 0.0, 0.0], [20.0, 10.0, 10.0]), THICK));
    assert!(within(v, 10.0, 1e-6), "{v}");
}

#[test]
fn touching_and_thin_overlaps_are_contact() {
    let a = a();
    assert!(matches!(overlap(&a, &block([10.0, 0.0, 0.0], [20.0, 10.0, 10.0]), THICK), Ok(Some(Overlap::Contact))));
    assert!(matches!(overlap(&a, &block([9.99, 0.0, 0.0], [20.0, 10.0, 10.0]), THICK), Ok(Some(Overlap::Contact))));
}

fn nut() -> Body {
    Nut { size: metric("M8").unwrap(), modelled: false }.body(&Plane3::XY, 0.02).unwrap()
}

#[test]
fn apart_solids() {
    let a = a();
    assert!(matches!(overlap(&a, &block([12.0, 0.0, 0.0], [20.0, 10.0, 10.0]), THICK), Ok(None)));
    let (nut, pin) = (nut(), block([-1.0, -1.0, 2.0], [1.0, 1.0, 4.0]));
    assert!(matches!(overlap(&nut, &pin, THICK), Ok(None)));
    let d = clearance(&nut, &pin, 5.0).unwrap().distance;
    assert!((4.0 - 0.02 - 2f64.sqrt() - 1e-9..=4.0 - 2f64.sqrt() + 1e-9).contains(&d), "{d}");
}

#[test]
fn bolt_in_nut_is_contact() {
    let nut = nut();
    let nut_at = |turn_deg: f64| {
        let m = DAffine3::from_translation(DVec3::new(0.0, 0.0, -10.0)) * DAffine3::from_rotation_z(turn_deg.to_radians());
        nut.transformed(&m).unwrap()
    };
    let bolt = Bolt { size: metric("M8").unwrap(), length: 30.0, modelled: false }.body(&Plane3::XY, 0.02).unwrap();
    assert!(matches!(overlap(&bolt, &nut_at(0.0), THICK), Ok(Some(Overlap::Contact))));
    assert!(matches!(overlap(&bolt, &nut_at(7.0), THICK), Ok(Some(Overlap::Contact))));
    let big = Bolt { size: metric("M10").unwrap(), length: 30.0, modelled: false }.body(&Plane3::XY, 0.02).unwrap();
    let v = volume(overlap(&big, &nut_at(0.0), THICK));
    assert!(within(v, 192.27, 3.0), "{v}");
}

#[test]
fn clearances() {
    let a = a();
    let c = clearance(&a, &block([13.0, 0.0, 0.0], [23.0, 10.0, 10.0]), 5.0).unwrap();
    let (p, q) = c.points.unwrap();
    assert!(within(c.distance, 3.0, 1e-6) && within(p.x, 10.0, 1e-6) && within(q.x, 13.0, 1e-6), "{c:?}");
    assert!(clearance(&a, &block([13.0, 0.0, 0.0], [23.0, 10.0, 10.0]), 2.0).is_none());
    let c = clearance(&a, &block([13.0, 14.0, 0.0], [23.0, 24.0, 10.0]), 10.0).unwrap();
    let (p, q) = c.points.unwrap();
    assert!(within(c.distance, 5.0, 1e-6), "{c:?}");
    assert!(p.truncate().distance(DVec2::new(10.0, 10.0)) <= 1e-6 && q.truncate().distance(DVec2::new(13.0, 14.0)) <= 1e-6);
    assert!(within(p.z, q.z, 1e-6));
    assert_eq!(clearance(&a, &block([5.0; 3], [15.0; 3]), 1.0), Some(Clearance { distance: 0.0, points: None }));
}

#[test]
fn check_sorts_pairs() {
    let a = a();
    let b2 = block([5.0, 0.0, 0.0], [15.0, 10.0, 10.0]);
    let b3 = block([15.0, 0.0, 0.0], [25.0, 10.0, 10.0]);
    let b7 = block([0.0, 0.0, 13.0], [10.0, 10.0, 23.0]);
    let solids = [(1, &a), (2, &b2), (3, &b3), (7, &b7)];
    let c = check(&solids, THICK, 4.0);
    assert_eq!(c.meetings.len(), 2);
    assert_eq!((c.meetings[0].a, c.meetings[0].b), (1, 2));
    let Ok(Overlap::Interference { volume, .. }) = &c.meetings[0].kind else { panic!("{:?}", c.meetings[0]) };
    assert!(within(*volume, 500.0, 1e-6), "{volume}");
    assert_eq!((c.meetings[1].a, c.meetings[1].b), (2, 3));
    assert!(matches!(c.meetings[1].kind, Ok(Overlap::Contact)));
    let mut pairs: Vec<(u64, u64)> = c.clearances.iter().map(|(a, b, _)| (*a, *b)).collect();
    pairs.sort();
    assert_eq!(pairs, [(1, 7), (2, 7)]);
    assert!(c.clearances.iter().all(|(_, _, k)| within(k.distance, 3.0, 1e-6)));
    assert!(check(&solids, THICK, 2.0).clearances.is_empty());
}

#[test]
fn modelled_threads_fit_at_whole_pitches() {
    let bolt = Bolt { size: metric("M8").unwrap(), length: 30.0, modelled: true }.body(&Plane3::XY, 0.02).unwrap();
    let nut = Nut { size: metric("M8").unwrap(), modelled: true }.body(&Plane3::XY, 0.02).unwrap();
    let at = |z: f64, turn: f64| nut.transformed(&(DAffine3::from_translation(DVec3::new(0.0, 0.0, z)) * DAffine3::from_rotation_z(turn))).unwrap();
    let n = at(-10.0, 0.0);
    assert!(matches!(overlap(&bolt, &n, THICK), Ok(None)));
    let d = clearance(&bolt, &n, 1.0).unwrap().distance;
    assert!((0.0185..=0.0205).contains(&d), "{d}");
    let n = at(-10.625, 0.0);
    let v = volume(overlap(&bolt, &n, THICK));
    assert!((25.0..=40.0).contains(&v), "{v}");
    let n = at(-10.625, PI);
    assert!(matches!(overlap(&bolt, &n, THICK), Ok(None)));
}
