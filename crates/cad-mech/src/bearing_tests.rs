use super::*;

fn b6204() -> Bearing {
    Bearing { size: bearing("6204").unwrap() }
}

#[test]
fn bearing_table() {
    assert_eq!(BEARINGS.len(), 22);
    assert_eq!(bearing("6204"), Some(BearingSize { name: "6204", bore: 20.0, outer: 47.0, width: 14.0 }));
    assert_eq!(bearing(" 6000 ").unwrap().outer, 26.0);
    assert_eq!(bearing("6210").unwrap().width, 20.0);
    assert_eq!(bearing("6300"), None);
}

#[test]
fn derived_ball_set() {
    assert!((b6204().ball() - 8.1).abs() <= 1e-12);
    assert_eq!(b6204().pitch_diameter(), 33.5);
    assert_eq!(b6204().balls(), 8);
    let balls = |n: &str| Bearing { size: bearing(n).unwrap() }.balls();
    assert_eq!((balls("6000"), balls("6200"), balls("6210")), (7, 6, 11));
    assert_eq!(b6204().designation(), "Ball bearing 6204 20x47x14");
}

#[test]
fn bearing_solid() {
    let b = b6204().body(&Plane3::XY, 0.02).unwrap();
    assert!(b.mesh.is_closed());
    let v = b.mesh.volume();
    assert!((v - 13146.684979704914).abs() <= 1e-2 * 13146.684979704914, "{v}");
    let (lo, hi) = b.bbox();
    assert!(lo.z.abs() <= 1e-9 && (hi.z - 14.0).abs() <= 1e-9, "{lo} {hi}");
    assert!((23.48..=23.5 + 1e-9).contains(&hi.x), "{}", hi.x);
    assert_eq!(b.faces.iter().filter(|f| matches!(f.surface, Surface::Sphere { .. })).count(), 8);
    let up = b6204().body(&Plane3 { origin: DVec3::new(0.0, 0.0, 5.0), ..Plane3::XY }, 0.02).unwrap();
    let (lo, hi) = up.bbox();
    assert!((lo.z - 5.0).abs() <= 1e-9 && (hi.z - 19.0).abs() <= 1e-9, "{lo} {hi}");
}
