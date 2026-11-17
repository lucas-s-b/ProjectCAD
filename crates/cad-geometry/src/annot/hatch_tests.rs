use super::*;
use crate::shape::HatchPattern;
use std::f64::consts::FRAC_PI_2;

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn square() -> Vec<DVec2> {
    vec![v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0), v2(0.0, 10.0)]
}
fn hatch(boundary: Vec<DVec2>, pattern: HatchPattern) -> Shape {
    Shape::Hatch { boundary, angle: 0.0, spacing: 2.0, pattern }
}
fn area(t: &[DVec2; 3]) -> f64 {
    ((t[1] - t[0]).perp_dot(t[2] - t[0]) / 2.0).abs()
}
fn polygon_area(p: &[DVec2]) -> f64 {
    ((0..p.len()).map(|i| p[i].perp_dot(p[(i + 1) % p.len()])).sum::<f64>() / 2.0).abs()
}

#[test]
fn cross_has_the_lines_of_both_directions() {
    let along = hatch_lines(&square(), 0.0, 2.0).len();
    let across = hatch_lines(&square(), FRAC_PI_2, 2.0).len();
    // Turned by 90 degrees the half-open rule meets rounding noise at x = 0, so both vertical edges get a line: 6, not 5.
    assert_eq!((along, across), (5, 6));
    let count = |pattern| strokes(&hatch(square(), pattern)).len();
    // one boundary ring and the lines
    assert_eq!(count(HatchPattern::Lines), 1 + 5);
    assert_eq!(count(HatchPattern::Cross), 1 + 5 + 6);
}

#[test]
fn solid_has_a_ring_and_fills_but_no_lines() {
    let solid = hatch(square(), HatchPattern::Solid);
    assert_eq!(strokes(&solid).len(), 1);
    assert_eq!(fills(&solid).len(), 2);
    assert!(fills(&hatch(square(), HatchPattern::Lines)).is_empty());
    let (lo, hi) = solid.bbox();
    assert!(lo.truncate().distance(v2(0.0, 0.0)) < 1e-9 && hi.truncate().distance(v2(10.0, 10.0)) < 1e-9);
}

#[test]
fn triangulation_covers_convex_and_concave_polygons() {
    let l = vec![v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 4.0), v2(4.0, 4.0), v2(4.0, 10.0), v2(0.0, 10.0)];
    let arrow = vec![v2(0.0, 0.0), v2(8.0, 3.0), v2(4.0, 3.5), v2(5.0, 9.0), v2(-2.0, 6.0)];
    let hexagon: Vec<DVec2> = (0..6).map(|k| DVec2::from_angle(k as f64 * FRAC_PI_2 / 1.5) * 5.0).collect();
    for poly in [square(), l, arrow, hexagon] {
        for ring in [poly.clone(), poly.iter().rev().copied().collect::<Vec<_>>()] {
            let tris = triangulate(&ring);
            assert_eq!(tris.len(), ring.len() - 2);
            let total: f64 = tris.iter().map(area).sum();
            assert!((total - polygon_area(&ring)).abs() < 1e-9, "{total} vs {}", polygon_area(&ring));
            for t in &tris {
                assert!(inside_polygon(&ring, (t[0] + t[1] + t[2]) / 3.0), "{t:?}");
            }
        }
    }
}

#[test]
fn straight_corners_and_degenerate_input() {
    let with_midpoint = vec![v2(0.0, 0.0), v2(5.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0), v2(0.0, 10.0)];
    let total: f64 = triangulate(&with_midpoint).iter().map(area).sum();
    assert!((total - 100.0).abs() < 1e-9);
    assert!(triangulate(&[v2(0.0, 0.0), v2(1.0, 0.0)]).is_empty());
    assert!(triangulate(&[v2(0.0, 0.0), v2(1.0, 1.0), v2(2.0, 2.0)]).is_empty());
}
