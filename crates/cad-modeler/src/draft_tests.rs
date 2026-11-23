use super::*;
use crate::fixtures::*;

const T: f64 = 0.08748866352592401; // tan 5 degrees

fn deg(a: f64) -> f64 {
    a.to_radians()
}

/// Index of the planar face whose outward normal is `n` and whose plane passes `n · p = d`.
fn face(b: &Body, n: DVec3, d: f64) -> usize {
    b.faces
        .iter()
        .position(|f| matches!(f.surface, Surface::Plane { origin, normal }
            if (if f.outward { normal } else { -normal }).normalize().distance(n) <= 1e-9 && (origin.dot(n) - d).abs() <= 1e-9))
        .unwrap()
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs(), "{a} vs {b}");
}

/// Largest x of the vertices at height z.
fn max_x_at(b: &Body, z: f64) -> f64 {
    b.mesh.positions.iter().filter(|p| (p.z - z).abs() <= 1e-9).map(|p| p.x).fold(f64::MIN, f64::max)
}

fn bx() -> Body {
    box_body([0.0; 3], [40.0, 20.0, 10.0])
}

#[test]
fn draft_one_side() {
    let bx = bx();
    let x = face(&bx, DVec3::X, 40.0);
    let d = draft_face(&bx, x, DVec3::ZERO, DVec3::Z, deg(5.0)).unwrap();
    assert!(d.mesh.is_closed());
    close(d.volume(), 8000.0 - 1000.0 * T, 1e-9);
    assert!((max_x_at(&d, 10.0) - (40.0 - 10.0 * T)).abs() <= 1e-9);
    assert!((d.bbox().1.x - 40.0).abs() <= 1e-12);
    assert_eq!(d.faces.len(), bx.faces.len());
    let Surface::Plane { normal, .. } = d.faces[x].surface else { panic!("not a plane") };
    assert!(normal.distance(DVec3::new(0.9961946980917455, 0.0, 0.08715574274765817)) <= 1e-12, "{normal}");
    assert!(d.faces[x].outward);
}

#[test]
fn draft_all_four_sides() {
    let bx = bx();
    let ids = [face(&bx, DVec3::X, 40.0), face(&bx, -DVec3::X, 0.0), face(&bx, DVec3::Y, 20.0), face(&bx, -DVec3::Y, 0.0)];
    let mut d = bx.clone();
    for i in ids {
        d = draft_face(&d, i, DVec3::ZERO, DVec3::Z, deg(5.0)).unwrap();
    }
    assert!(d.mesh.is_closed());
    close(d.volume(), 7485.273707171859, 1e-9);
}

#[test]
fn neutral_plane_and_sign() {
    let bx = bx();
    let x = face(&bx, DVec3::X, 40.0);
    let d = draft_face(&bx, x, DVec3::new(0.0, 0.0, 5.0), DVec3::Z, deg(5.0)).unwrap();
    close(d.volume(), 8000.0, 1e-9);
    close(d.bbox().1.x, 40.43744331762962, 1e-9);
    let d = draft_face(&bx, x, DVec3::ZERO, DVec3::Z, deg(-5.0)).unwrap();
    close(d.volume(), 8087.488663525924, 1e-9);
}

#[test]
fn concave_neighbours() {
    let l = l_prism(10.0);
    let i = face(&l, DVec3::X, 10.0);
    let d = draft_face(&l, i, DVec3::ZERO, DVec3::Z, deg(5.0)).unwrap();
    assert!(d.mesh.is_closed());
    close(d.volume(), 2956.255668237038, 1e-9);
}

#[test]
fn draft_refusals() {
    let bx = bx();
    let x = face(&bx, DVec3::X, 40.0);
    let err = |b: &Body, i: usize, a: f64| draft_face(b, i, DVec3::ZERO, DVec3::Z, a).err();
    let invalid = |s: &'static str| Some(ModelError::Invalid(s));
    assert_eq!(err(&bx, face(&bx, DVec3::Z, 10.0), deg(5.0)), invalid("the face must be parallel to the pull direction"));
    let df = d_flat();
    assert_eq!(err(&df, face(&df, DVec3::X, 3.0), deg(5.0)), invalid("faces next to a drafted face must be planes at right angles to it"));
    assert_eq!(err(&bx, x, deg(89.0)), invalid("draft angle must be between -89 and 89 degrees"));
    assert_eq!(err(&bx, x, f64::NAN), invalid("draft angle must be between -89 and 89 degrees"));
    assert_eq!(err(&bx, x, deg(80.0)), invalid("draft angle too large for the face"));
    assert_eq!(err(&bx, 99, deg(5.0)), invalid("face not found; pick it again"));
}
