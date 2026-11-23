use super::*;

fn f(deg: f64) -> f64 {
    four_bar_rocker(20.0, 60.0, 40.0, 50.0, deg.to_radians(), 1.0).unwrap().to_degrees()
}

#[test]
fn rocker_angles_of_a_crank_rocker() {
    let near = |a: f64, b: f64| assert!((a - b).abs() <= 1e-9, "{a} vs {b}");
    near(f(0.0), 62.72038726402191);
    near(f(90.0), 80.25691282921025);
    near(f(180.0), 121.18862233347662);
    near(f(210.0), 127.35761099706751);
    near(four_bar_rocker(20.0, 60.0, 40.0, 50.0, 0.0, -1.0).unwrap().to_degrees(), -62.72038726402192);
    for k in 0..12 {
        let t2 = (30.0 * k as f64).to_radians();
        let t4 = f(30.0 * k as f64).to_radians();
        assert!((2.5 * t4.cos() - 1.25 * t2.cos() + 0.5625 - (t2 - t4).cos()).abs() <= 1e-12);
    }
    assert_eq!(four_bar_rocker(20.0, 10.0, 10.0, 50.0, 0.0, 1.0), None);
}
