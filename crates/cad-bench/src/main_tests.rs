use super::utc_date;

#[test]
fn utc_dates() {
    assert_eq!(utc_date(0), "1970-01-01");
    assert_eq!(utc_date(951_782_400), "2000-02-29");
    assert_eq!(utc_date(1_700_000_000), "2023-11-14");
    assert_eq!(utc_date(1_795_392_000), "2026-11-23");
}
