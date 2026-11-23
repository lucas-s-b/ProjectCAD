use super::*;

#[test]
fn colour_ramp() {
    assert_eq!(ramp(0.0), [0.0, 0.0, 1.0]);
    assert_eq!(ramp(0.5), [0.0, 1.0, 0.0]);
    assert_eq!(ramp(1.0), [1.0, 0.0, 0.0]);
    assert_eq!(ramp(-3.0), ramp(0.0));
    let r = ramp(0.125);
    for (got, want) in r.iter().zip([0.0, 0.5, 1.0]) {
        assert!((got - want).abs() <= 1e-6, "{r:?}");
    }
}

#[test]
fn face_key_texts() {
    assert_eq!(key_text(&FaceKey::Plane(-DVec3::X)), "plane -X");
    assert_eq!(key_text(&FaceKey::Plane(DVec3::new(0.6, 0.8, 0.0))), "plane (0.600, 0.800, 0.000)");
    assert_eq!(key_text(&FaceKey::Cylinder(-DVec3::Z)), "cylinder Z");
    assert_eq!(key_text(&FaceKey::Other), "surface");
}

#[test]
fn value_range() {
    assert_eq!(range(&[3.0, -1.0, 2.0]), (-1.0, 3.0));
}
