use super::*;
use std::f64::consts::FRAC_PI_2;

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

#[test]
fn ellipse_point_rotated() {
    let c = v2(1.0, 2.0);
    let p0 = ellipse_point(c, 4.0, 2.0, FRAC_PI_2, 0.0);
    assert!(p0.distance(v2(1.0, 6.0)) < 1e-12);
    let p1 = ellipse_point(c, 4.0, 2.0, FRAC_PI_2, FRAC_PI_2);
    assert!(p1.distance(v2(-1.0, 2.0)) < 1e-12);
}

fn pts() -> Vec<DVec2> {
    vec![v2(0.0, 0.0), v2(10.0, 5.0), v2(20.0, 0.0), v2(30.0, 8.0)]
}

#[test]
fn passes_through_points_and_is_continuous() {
    for closed in [false, true] {
        let p = pts();
        let spans = spline_spans(&p, closed);
        assert_eq!(spans, if closed { 4 } else { 3 });
        for s in 0..spans {
            assert!(spline_point(&p, closed, s, 0.0).distance(p[s]) < 1e-9);
            let end = spline_point(&p, closed, s, 1.0);
            assert!(end.distance(p[(s + 1) % p.len()]) < 1e-9, "closed {closed} span {s}");
            if s + 1 < spans {
                assert!(end.distance(spline_point(&p, closed, s + 1, 0.0)) < 1e-9);
            }
        }
    }
    let p = pts();
    assert!(spline_point(&p, true, 3, 1.0).distance(p[0]) < 1e-9);
}

#[test]
fn collinear_points_stay_on_the_line() {
    let p = vec![v2(0.0, 0.0), v2(1.0, 0.0), v2(5.0, 0.0)];
    for s in 0..2 {
        for i in 0..=8 {
            assert!(spline_point(&p, false, s, i as f64 / 8.0).y.abs() < 1e-12);
        }
    }
}

#[test]
fn path_lengths() {
    let p = pts();
    assert_eq!(spline_path(&p, false, 16).len(), 3 * 16 + 1);
    assert_eq!(spline_path(&p, true, 16).len(), 4 * 16 + 1);
}
