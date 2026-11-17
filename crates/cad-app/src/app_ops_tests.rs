use super::*;

#[test]
fn rect_offsets_skip_the_original() {
    let o = array_rect_offsets(3, 2, 10.0, 5.0);
    assert_eq!(o.len(), 5);
    assert!(!o.contains(&DVec2::ZERO));
    assert!(o.contains(&DVec2::new(20.0, 5.0)) && o.contains(&DVec2::new(0.0, 5.0)));
    assert!(array_rect_offsets(1, 1, 1.0, 1.0).is_empty());
}

#[test]
fn polar_steps() {
    assert_eq!(array_polar_angles(4, 360.0), [90.0, 180.0, 270.0]);
    assert_eq!(array_polar_angles(3, 90.0), [45.0, 90.0]);
    assert!(array_polar_angles(1, 90.0).is_empty());
}

#[test]
fn section_clip_plane() {
    let mut v = crate::app::SectionView { on: false, axis: 0, offset: "5".into(), flip: false };
    assert_eq!(v.clip(), [0.0, 0.0, 0.0, 1.0]);
    v.on = true;
    assert_eq!(v.clip(), [1.0, 0.0, 0.0, 5.0]);
    v.flip = true;
    assert_eq!(v.clip(), [-1.0, 0.0, 0.0, -5.0]);
    v.offset = "abc".into();
    assert_eq!(v.clip(), [0.0, 0.0, 0.0, 1.0]);
}
