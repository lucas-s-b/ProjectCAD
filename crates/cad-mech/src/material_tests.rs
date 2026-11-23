use super::*;

#[test]
fn densities() {
    assert_eq!(MATERIALS.len(), 11);
    assert_eq!(density("Steel"), Some(7.85));
    assert_eq!(density(" aluminium "), Some(2.7));
    assert_eq!(density("POM"), Some(1.41));
    assert_eq!(density("Unobtainium"), None);
    assert_eq!(density(""), None);
}

#[test]
fn elastic_constants() {
    let steel = material(" steel ").unwrap();
    assert_eq!((steel.young, steel.poisson, steel.name), (210_000.0, 0.30, "Steel"));
    assert_eq!(material("aluminium").unwrap().young, 69_000.0);
    assert!(material("Unobtainium").is_none());
    for m in MATERIALS {
        assert!(m.young > 0.0 && (0.0..0.5).contains(&m.poisson) && m.density > 0.0, "{}", m.name);
    }
}
