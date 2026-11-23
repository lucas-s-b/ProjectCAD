use super::{assembly, assembly_at, pin, pin_turn, utc_date};

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

#[test]
fn assembly_at_spaces_by_pitch() {
    let d = assembly_at(12, 80.0);
    let r = cad_document::Rebuilder::default().rebuild(&d);
    let delta = r.component(12).unwrap().bbox().0 - r.component(1).unwrap().bbox().0;
    assert!((delta - cad_geometry::DVec3::new(80.0, 80.0, 0.0)).length() < 1e-9, "{delta:?}");
}

#[test]
fn pin_turns_freely() {
    let d = pin();
    let r = cad_document::Rebuilder::default().rebuild(&d);
    assert!(r.components.iter().all(|(_, b)| b.is_ok()));
    let m = d.drive(&r, &pin_turn(36)).unwrap();
    assert_eq!(m.values.len(), 37);
    assert!(matches!(d.first_collision(&r, &m), Ok(None)));
}

#[test]
fn beam_has_one_study() {
    let (d, r) = cad_document::validation::beam(5.0, "0", ["1000", "0", "0"]).unwrap();
    assert_eq!(d.studies().len(), 1);
    assert_eq!(r.bodies.len(), 1);
    assert_eq!(d.run_study(&r, 0).unwrap().result.dofs, 3000);
}
