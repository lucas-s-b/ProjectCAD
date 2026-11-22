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
