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
    /// Triangles filling the outlines.
    pub fill: Vec<(usize, [DVec2; 3])>,
    /// Pieces of outline edges that lie on or in no other outline: the joins show no lines.
    pub edges: Vec<(usize, [DVec2; 2])>,
}

/// The plan of `walls`. Edge pieces are not merged across walls or across cut points.
// shortcut: O(edges x walls) per call; add a grid index when a level has thousands of walls
pub fn plan(walls: &[Wall]) -> Plan {
    let footprints = footprints(walls);
    let mut fill = Vec::new();
    for (i, f) in footprints.iter().enumerate() {
        let flat: Vec<f64> = f.iter().flat_map(|p| [p.x, p.y]).collect();
        // earcut fails only on non-finite input, which `Shape::validate` rules out
        let idx = earcutr::earcut(&flat, &[], 2).unwrap_or_default();
        fill.extend(idx.chunks(3).map(|t| (i, [f[t[0]], f[t[1]], f[t[2]]])));
    }
    let mut edges = Vec::new();
    for (i, f) in footprints.iter().enumerate() {
        for k in 0..f.len() {
            let (a, b) = (f[k], f[(k + 1) % f.len()]);
            let others = || footprints.iter().enumerate().filter(move |(j, _)| *j != i).map(|(_, g)| g);
            let mut ts = vec![0.0, 1.0];
            for g in others() {
                ts.extend(cuts(a, b, g));
            }
            ts.sort_by(f64::total_cmp);
            for w in ts.windows(2) {
                if w[1] - w[0] > 1e-9 && !others().any(|g| covers(g, a.lerp(b, (w[0] + w[1]) / 2.0))) {
                    edges.push((i, [a.lerp(b, w[0]), a.lerp(b, w[1])]));
                }
            }
        }
    }
    Plan { footprints, fill, edges }
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
