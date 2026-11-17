//! Helpers shared by the tests.
use cad_geometry::{DVec2, Wall, WallAlign};
pub fn v(x: f64, y: f64) -> DVec2 { DVec2::new(x, y) }
/// Centred wall, 2500 high.
pub fn wall(a: (f64, f64), b: (f64, f64), t: f64) -> Wall {
    Wall { a: v(a.0, a.1), b: v(b.0, b.1), thickness: t, height: 2500.0, align: WallAlign::Center }
}
/// Signed area, positive counter-clockwise.
pub fn area(p: &[DVec2]) -> f64 {
    (0..p.len()).map(|k| p[k].perp_dot(p[(k + 1) % p.len()])).sum::<f64>() / 2.0
}
/// `got` is `want`, possibly starting at another vertex (same direction), within 1e-9.
pub fn assert_ring(got: &[DVec2], want: &[DVec2]) {
    let n = want.len();
    let ok = got.len() == n && (0..n).any(|s| (0..n).all(|k| got[(s + k) % n].distance(want[k]) <= 1e-9));
    assert!(ok, "{got:?} vs {want:?}");
}
