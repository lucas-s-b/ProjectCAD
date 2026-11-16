use super::*;

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

#[test]
fn absolute_and_spaced() {
    assert_eq!(parse_coord("10,20", None, 1.0), Ok(v2(10.0, 20.0)));
    assert_eq!(parse_coord(" 10 , -2.5 ", None, 1.0), Ok(v2(10.0, -2.5)));
    assert_eq!(parse_coord("1,2", None, 25.4), Ok(v2(25.4, 50.8)));
}

#[test]
fn relative_forms() {
    assert_eq!(parse_coord("@5,5", Some(v2(1.0, 1.0)), 1.0), Ok(v2(6.0, 6.0)));
    let p = parse_coord("@10<90", Some(v2(0.0, 0.0)), 1.0).unwrap();
    assert!(p.distance(v2(0.0, 10.0)) < 1e-9, "{p}");
    let q = parse_coord("@1<0", Some(v2(1.0, 1.0)), 25.4).unwrap();
    assert!(q.distance(v2(26.4, 1.0)) < 1e-9, "{q}");
}

#[test]
fn errors() {
    assert_eq!(parse_coord("", None, 1.0), Err("empty input".into()));
    assert_eq!(parse_coord("   ", None, 1.0), Err("empty input".into()));
    assert!(parse_coord("a,2", None, 1.0).unwrap_err().contains("not a number"));
    assert!(parse_coord("1,inf", None, 1.0).unwrap_err().contains("not finite"));
    assert!(parse_coord("@NaN<3", Some(v2(0.0, 0.0)), 1.0).unwrap_err().contains("not finite"));
    assert_eq!(parse_coord("@5,5", None, 1.0), Err("no previous point".into()));
    assert_eq!(parse_coord("@5<45", None, 1.0), Err("no previous point".into()));
    for bad in ["10", "10<90", "@10", "@1;2"] {
        assert_eq!(parse_coord(bad, Some(v2(0.0, 0.0)), 1.0), Err(USAGE.into()), "{bad}");
    }
}
