//! Ear clipping of a simple polygon.
use glam::DVec2;

/// Twice the area below which a corner counts as straight.
const FLAT: f64 = 1e-12;

fn inside(t: &[DVec2; 3], p: DVec2) -> bool {
    let side = |a: DVec2, b: DVec2| (b - a).perp_dot(p - a);
    side(t[0], t[1]) >= 0.0 && side(t[1], t[2]) >= 0.0 && side(t[2], t[0]) >= 0.0
}

/// Triangles that cover a simple polygon without holes, either winding.
/// shortcut: O(n^2) for typical outlines (O(n^3) worst case), no holes; a self-intersecting outline is only partly filled, add a sweep-line triangulator when needed
pub fn triangulate(poly: &[DVec2]) -> Vec<[DVec2; 3]> {
    let area2: f64 = (0..poly.len()).map(|i| poly[i].perp_dot(poly[(i + 1) % poly.len()])).sum();
    let mut ring: Vec<DVec2> = if area2 < 0.0 { poly.iter().rev().copied().collect() } else { poly.to_vec() };
    let mut out = Vec::new();
    while ring.len() > 3 {
        let n = ring.len();
        let corner = |i: usize| [ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]];
        let turn = |t: &[DVec2; 3]| (t[1] - t[0]).perp_dot(t[2] - t[1]);
        let convex = |t: &[DVec2; 3]| turn(t) > FLAT;
        let flat = |t: &[DVec2; 3]| turn(t).abs() <= FLAT;
        let ear = (0..n).find(|&i| {
            let t = corner(i);
            convex(&t) && !ring.iter().any(|p| !t.contains(p) && inside(&t, *p))
        });
        match ear {
            Some(i) => {
                out.push(corner(i));
                ring.remove(i);
            }
            // A straight corner covers nothing: drop it. Otherwise the outline crosses itself.
            None => match (0..n).find(|&i| flat(&corner(i))) {
                Some(i) => {
                    ring.remove(i);
                }
                None => break,
            },
        }
    }
    if let [a, b, c] = ring[..]
        && (b - a).perp_dot(c - b) > FLAT
    {
        out.push([a, b, c]);
    }
    out
}
