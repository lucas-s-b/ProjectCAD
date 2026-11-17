use super::*;
use crate::fixtures::*;
use cad_geometry::WallAlign;

fn all_positive(f: &[Vec<DVec2>]) {
    assert!(f.iter().all(|r| area(r) > 0.0), "{f:?}");
}

#[test]
fn free_wall_is_its_rectangle() {
    let f = footprints(&[wall((0.0, 0.0), (4000.0, 0.0), 200.0)]);
    assert_eq!(f.len(), 1);
    assert_ring(&f[0], &[v(0.0, -100.0), v(4000.0, -100.0), v(4000.0, 100.0), v(0.0, 100.0)]);
    all_positive(&f);
    let left = Wall { align: WallAlign::Left, ..wall((0.0, 0.0), (4000.0, 0.0), 200.0) };
    let f = footprints(&[left]);
    assert_ring(&f[0], &[v(0.0, 0.0), v(4000.0, 0.0), v(4000.0, 200.0), v(0.0, 200.0)]);
    all_positive(&f);
}

#[test]
fn l_join_mitres_both_walls() {
    let want0 = [v(0.0, -100.0), v(4100.0, -100.0), v(3900.0, 100.0), v(0.0, 100.0)];
    let want1 = [v(4100.0, -100.0), v(4100.0, 3000.0), v(3900.0, 3000.0), v(3900.0, 100.0)];
    for second in [((4000.0, 0.0), (4000.0, 3000.0)), ((4000.0, 3000.0), (4000.0, 0.0))] {
        let f = footprints(&[wall((0.0, 0.0), (4000.0, 0.0), 200.0), wall(second.0, second.1, 200.0)]);
        assert_ring(&f[0], &want0);
        assert_ring(&f[1], &want1);
        assert!((area(&f[0]) - 800000.0).abs() < 1e-6 && (area(&f[1]) - 600000.0).abs() < 1e-6);
        all_positive(&f);
    }
}

#[test]
fn t_join_stops_at_the_face() {
    let f = footprints(&[wall((0.0, 0.0), (4000.0, 0.0), 200.0), wall((2000.0, 0.0), (2000.0, 3000.0), 100.0)]);
    assert_ring(&f[0], &[v(0.0, -100.0), v(4000.0, -100.0), v(4000.0, 100.0), v(0.0, 100.0)]);
    assert_ring(&f[1], &[v(2050.0, 100.0), v(2050.0, 3000.0), v(1950.0, 3000.0), v(1950.0, 100.0)]);
    assert!((area(&f[1]) - 290000.0).abs() < 1e-6);
    all_positive(&f);
}

#[test]
fn collinear_walls_meet_square() {
    let f = footprints(&[wall((0.0, 0.0), (2000.0, 0.0), 200.0), wall((2000.0, 0.0), (4000.0, 0.0), 200.0)]);
    assert_ring(&f[0], &[v(0.0, -100.0), v(2000.0, -100.0), v(2000.0, 100.0), v(0.0, 100.0)]);
    assert_ring(&f[1], &[v(2000.0, -100.0), v(4000.0, -100.0), v(4000.0, 100.0), v(2000.0, 100.0)]);
    all_positive(&f);
}

#[test]
fn three_walls_share_the_hub() {
    let f = footprints(&[
        wall((0.0, 0.0), (2000.0, 0.0), 200.0),
        wall((0.0, 0.0), (0.0, 2000.0), 200.0),
        wall((0.0, 0.0), (-2000.0, 0.0), 200.0),
    ]);
    assert_ring(&f[0], &[v(0.0, -100.0), v(2000.0, -100.0), v(2000.0, 100.0), v(100.0, 100.0), v(0.0, 0.0)]);
    assert_ring(&f[1], &[v(100.0, 100.0), v(100.0, 2000.0), v(-100.0, 2000.0), v(-100.0, 100.0), v(0.0, 0.0)]);
    assert_ring(&f[2], &[v(-100.0, 100.0), v(-2000.0, 100.0), v(-2000.0, -100.0), v(0.0, -100.0), v(0.0, 0.0)]);
    assert!((area(&f[0]) - 395000.0).abs() < 1e-6 && (area(&f[1]) - 390000.0).abs() < 1e-6 && (area(&f[2]) - 395000.0).abs() < 1e-6);
    assert!((f.iter().map(|r| area(r)).sum::<f64>() - 1180000.0).abs() < 1e-6);
    all_positive(&f);
}

#[test]
fn crossing_walls_stay_whole() {
    let f = footprints(&[wall((0.0, 0.0), (4000.0, 0.0), 200.0), wall((2000.0, -2000.0), (2000.0, 2000.0), 200.0)]);
    assert_ring(&f[0], &[v(0.0, -100.0), v(4000.0, -100.0), v(4000.0, 100.0), v(0.0, 100.0)]);
    assert_ring(&f[1], &[v(2100.0, -2000.0), v(2100.0, 2000.0), v(1900.0, 2000.0), v(1900.0, -2000.0)]);
    all_positive(&f);
}

#[test]
fn sharp_angle_gets_square_ends() {
    let f = footprints(&[wall((0.0, 0.0), (1000.0, 0.0), 100.0), wall((0.0, 0.0), (1000.0, 10.0), 100.0)]);
    assert_ring(&f[0], &[v(0.0, -50.0), v(1000.0, -50.0), v(1000.0, 50.0), v(0.0, 50.0)]);
    all_positive(&f);
}
