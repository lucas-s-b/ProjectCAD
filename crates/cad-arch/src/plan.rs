//! The plan of the walls of one level: filled outlines, and the outline edges with the joins cleaned.
use crate::join::footprints;
use cad_geometry::{DVec2, Wall, point_segment_distance};

/// How far (mm) a point may be from an outline and still count as on it.
const ON: f64 = 1e-6;

/// What the 2D view draws for the walls of a level; every item carries the index of its wall in the input.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// One counter-clockwise outline per wall, as `footprints` gives them.
    pub footprints: Vec<Vec<DVec2>>,
    /// Triangles filling the outlines, opening gaps left out.
    pub fill: Vec<(usize, [DVec2; 3])>,
    /// Pieces of outline and gap edges that lie on or in no other piece: the joins show no lines, the gaps show their jambs.
    pub edges: Vec<(usize, [DVec2; 2])>,
}

/// The plan of `walls` with the gaps of their openings: `gaps` holds (wall index, (start, end) along that wall's
/// baseline from `a`), as `gap` gives them. Edge pieces are not merged across walls or across cut points.
// shortcut: O(edges x pieces) per call; add a grid index when a level has thousands of walls
pub fn plan(walls: &[Wall], gaps: &[(usize, (f64, f64))]) -> Plan {
    let footprints = footprints(walls);
    let pieces: Vec<(usize, Vec<DVec2>)> = footprints
        .iter()
        .enumerate()
        .flat_map(|(i, f)| {
            let mine: Vec<(f64, f64)> = gaps.iter().filter(|g| g.0 == i).map(|g| g.1).collect();
            split(f, &walls[i], &mine).into_iter().map(move |p| (i, p))
        })
        .collect();
    let mut fill = Vec::new();
    for (i, f) in &pieces {
        let flat: Vec<f64> = f.iter().flat_map(|p| [p.x, p.y]).collect();
        // earcut fails only on non-finite input, which `Shape::validate` rules out
        let idx = earcutr::earcut(&flat, &[], 2).unwrap_or_default();
        fill.extend(idx.chunks(3).map(|t| (*i, [f[t[0]], f[t[1]], f[t[2]]])));
    }
    let mut edges = Vec::new();
    for (pi, (i, f)) in pieces.iter().enumerate() {
        for k in 0..f.len() {
            let (a, b) = (f[k], f[(k + 1) % f.len()]);
            let others = || pieces.iter().enumerate().filter(move |(j, _)| *j != pi).map(|(_, (_, g))| g);
            let mut ts = vec![0.0, 1.0];
            for g in others() {
                ts.extend(cuts(a, b, g));
            }
            ts.sort_by(f64::total_cmp);
            for w in ts.windows(2) {
                if w[1] - w[0] > 1e-9 && !others().any(|g| covers(g, a.lerp(b, (w[0] + w[1]) / 2.0))) {
                    edges.push((*i, [a.lerp(b, w[0]), a.lerp(b, w[1])]));
                }
            }
        }
    }
    Plan { footprints, fill, edges }
}

/// `footprint` minus the slabs `gaps` across its wall (start and end along the baseline from `a`): the pieces beside
/// and between them, counter-clockwise. Wall outlines are convex, so each piece is one clip on each side.
fn split(footprint: &[DVec2], w: &Wall, gaps: &[(f64, f64)]) -> Vec<Vec<DVec2>> {
    let d = (w.b - w.a).normalize();
    let along = |p: DVec2| (p - w.a).dot(d);
    let mut gaps = gaps.to_vec();
    gaps.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out = Vec::new();
    let mut lo = f64::NEG_INFINITY;
    for (g0, g1) in gaps.into_iter().chain([(f64::INFINITY, f64::INFINITY)]) {
        if g0 > lo {
            let mut piece = footprint.to_vec();
            // infinite bounds clip nothing (and inf - inf would be NaN)
            if lo.is_finite() {
                piece = clip(&piece, |p| along(p) - lo);
            }
            if g0.is_finite() {
                piece = clip(&piece, |p| g0 - along(p));
            }
            if piece.len() >= 3 && shoelace(&piece) > 1e-9 {
                out.push(piece);
            }
        }
        lo = lo.max(g1);
    }
    out
}

/// Sutherland-Hodgman: the part of `poly` where the linear function `side` is not negative; consecutive points
/// closer than 1e-9 merged.
fn clip(poly: &[DVec2], side: impl Fn(DVec2) -> f64) -> Vec<DVec2> {
    let mut out: Vec<DVec2> = Vec::new();
    for k in 0..poly.len() {
        let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
        let (sp, sq) = (side(p), side(q));
        if sp >= 0.0 {
            out.push(p);
        }
        if (sp >= 0.0) != (sq >= 0.0) {
            out.push(p + (q - p) * (sp / (sp - sq)));
        }
    }
    out.dedup_by(|q, p| q.distance(*p) <= 1e-9);
    if out.len() > 1 && out[0].distance(out[out.len() - 1]) <= 1e-9 {
        out.pop();
    }
    out
}

/// Signed area, positive counter-clockwise.
fn shoelace(p: &[DVec2]) -> f64 {
    (0..p.len()).map(|k| p[k].perp_dot(p[(k + 1) % p.len()])).sum::<f64>() / 2.0
}

/// Parameters in [0, 1] along a -> b where the outline `g` meets it: its vertices on the segment and the crossings of its edges.
fn cuts(a: DVec2, b: DVec2, g: &[DVec2]) -> Vec<f64> {
    let d = b - a;
    let mut out = Vec::new();
    for k in 0..g.len() {
        let (p, q) = (g[k], g[(k + 1) % g.len()]);
        let t = (p - a).dot(d) / d.length_squared();
        if (0.0..=1.0).contains(&t) && (a + d * t).distance(p) <= ON {
            out.push(t);
        }
        let e = q - p;
        let den = d.perp_dot(e);
        if den.abs() > 1e-12 {
            let (t, u) = ((p - a).perp_dot(e) / den, (p - a).perp_dot(d) / den);
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                out.push(t);
            }
        }
    }
    out
}

/// `p` is inside the closed polygon `g` (even-odd rule) or within `ON` of its boundary.
fn covers(g: &[DVec2], p: DVec2) -> bool {
    let mut odd = false;
    for k in 0..g.len() {
        let (a, b) = (g[k], g[(k + 1) % g.len()]);
        if point_segment_distance(p, a, b) <= ON {
            return true;
        }
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            odd = !odd;
        }
    }
    odd
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
