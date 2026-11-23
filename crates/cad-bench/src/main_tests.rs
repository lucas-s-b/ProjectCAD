use super::{assembly, utc_date};

#[test]
fn utc_dates() {
    assert_eq!(utc_date(0), "1970-01-01");
    assert_eq!(utc_date(951_782_400), "2000-02-29");
    assert_eq!(utc_date(1_700_000_000), "2023-11-14");
    assert_eq!(utc_date(1_795_392_000), "2026-11-23");
}

#[test]
fn assembly_places_its_components() {
    let d = assembly(12);
    let r = cad_document::Rebuilder::default().rebuild(&d);
    assert_eq!(r.components.len(), 12);
    assert!(r.components.iter().all(|(_, b)| b.is_ok()));
    assert_eq!(d.component(1).unwrap().name, "Gear #1");
    assert_eq!(d.component(12).unwrap().name, "Gear #12");
    let delta = r.component(12).unwrap().bbox().0 - r.component(1).unwrap().bbox().0;
    assert!((delta - cad_geometry::DVec3::new(100.0, 100.0, 0.0)).length() < 1e-9, "{delta:?}");
}
