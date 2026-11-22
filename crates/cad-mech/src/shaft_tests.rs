use super::*;

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs().max(1.0), "{a} vs {b}");
}

fn shaft() -> Shaft {
    Shaft { steps: vec![(20.0, 30.0), (16.0, 50.0)], chamfer: 1.0 }
}

fn pts(v: &[(f64, f64)]) -> Vec<DVec2> {
    v.iter().map(|&(x, y)| DVec2::new(x, y)).collect()
}

#[test]
fn shaft_outline_and_size() {
    let want = pts(&[(0.0, 0.0), (9.0, 0.0), (10.0, 1.0), (10.0, 30.0), (8.0, 30.0), (8.0, 79.0), (7.0, 80.0), (0.0, 80.0)]);
    assert_eq!(shaft().outline(), want);
    assert_eq!(shaft().length(), 80.0);
    assert_eq!(shaft().max_diameter(), 20.0);
    assert_eq!(shaft().designation(), "Shaft \u{d8}20x80");
    assert_eq!(shaft().segments(0.001), 223);
    assert_eq!(shaft().segments(0.02), 50);
    let plain = Shaft { steps: vec![(10.0, 20.0), (10.0, 20.0)], chamfer: 0.0 };
    assert_eq!(plain.outline(), pts(&[(0.0, 0.0), (5.0, 0.0), (5.0, 40.0), (0.0, 40.0)]));
}

#[test]
fn shaft_volume() {
    let b = shaft().body(&Plane3::XY, 0.001).unwrap();
    assert!(b.mesh.is_closed());
    close(b.volume(), 19420.850332836813, 1e-9);
    close(b.volume(), 19423.420179594494, 1e-3);
    let (lo, hi) = b.bbox();
    assert!(lo.z.abs() <= 1e-9 && (hi.z - 80.0).abs() <= 1e-9 && (hi.x - 10.0).abs() <= 1e-9);
    let plain = Shaft { chamfer: 0.0, ..shaft() };
    close(plain.body(&Plane3::XY, 0.001).unwrap().volume(), 19475.297400839157, 1e-9);
    let one = Shaft { steps: vec![(10.0, 40.0)], chamfer: 1.0 };
    close(one.body(&Plane3::XY, 0.001).unwrap().volume(), 3111.450888858992, 1e-9);
}

#[test]
fn shaft_validation() {
    let t = 0.02;
    let none = Shaft { steps: Vec::new(), chamfer: 0.0 };
    assert_eq!(none.body(&Plane3::XY, t), Err(ModelError::Invalid("shaft needs at least one step")));
    let zero = Shaft { steps: vec![(0.0, 10.0)], chamfer: 0.0 };
    assert_eq!(zero.body(&Plane3::XY, t), Err(ModelError::Invalid("shaft diameters and lengths must be positive")));
    for (steps, chamfer) in [(vec![(10.0, 40.0)], 5.0), (vec![(10.0, 10.0)], 5.0), (vec![(10.0, 40.0)], -1.0)] {
        assert_eq!(Shaft { steps, chamfer }.body(&Plane3::XY, t), Err(ModelError::Invalid(BAD_CHAMFER)));
    }
    assert!(Shaft { steps: vec![(10.0, 40.0)], chamfer: 4.9 }.body(&Plane3::XY, t).is_ok());
}
