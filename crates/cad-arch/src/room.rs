//! Rooms: the areas the walls of a level enclose, found from the cleaned plan outline. Doors do not open an area.
use crate::plan::{ON, covers, plan, shoelace};
use cad_geometry::{DVec2, Wall};

/// An area enclosed by walls, inside their faces.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    /// Counter-clockwise, without collinear points.
    pub outline: Vec<DVec2>,
    /// Outlines of the wall groups standing free inside the area, counter-clockwise.
    pub islands: Vec<Vec<DVec2>>,
    /// Outline area minus the island areas (net floor area), mm².
    pub area: f64,
    /// Length of the outline plus the island outlines.
    pub perimeter: f64,
}

/// For each seed the area `walls` enclose around it; `None` for a seed outside every enclosed area or inside a wall.
/// Openings play no part: a door does not join two rooms.
pub fn regions(walls: &[Wall], seeds: &[DVec2]) -> Vec<Option<Region>> {
    // Every boundary edge has wall on its left: loops around enclosed areas run clockwise, loops around wall groups
    // counter-clockwise.
    let (holes, groups): (Vec<Vec<DVec2>>, Vec<Vec<DVec2>>) =
        loops(&plan(walls, &[]).edges).into_iter().partition(|l| shoelace(l) < 0.0);
    // The enclosed area a point lies in: the smallest clockwise loop around it.
    let hole_of = |p: DVec2| {
        (0..holes.len())
            .filter(|&i| covers(&holes[i], p))
            .min_by(|&i, &j| shoelace(&holes[i]).abs().total_cmp(&shoelace(&holes[j]).abs()))
    };
    let parent: Vec<Option<usize>> = groups.iter().map(|g| hole_of(g[0])).collect();
    let length = |l: &[DVec2]| (0..l.len()).map(|k| l[k].distance(l[(k + 1) % l.len()])).sum::<f64>();
    seeds
        .iter()
        .map(|&s| {
            let i = hole_of(s)?;
            let islands: Vec<Vec<DVec2>> =
                groups.iter().zip(&parent).filter(|(_, p)| **p == Some(i)).map(|(g, _)| g.clone()).collect();
            if islands.iter().any(|g| covers(g, s)) {
                return None;
            }
            let mut outline = holes[i].clone();
            outline.reverse();
            let area = shoelace(&outline) - islands.iter().map(|g| shoelace(g)).sum::<f64>();
            let perimeter = length(&outline) + islands.iter().map(|g| length(g)).sum::<f64>();
            Some(Region { outline, islands, area, perimeter })
        })
        .collect()
}

/// Chains `edges` (as `plan` gives them) into closed loops: each edge continues with an unused edge that starts within
/// `ON` of its end, the one turning furthest right when there are several, so a loop keeps to the area on its right.
/// Chains that do not close are dropped; collinear points are removed.
// shortcut: O(edges^2) search for the next edge; index edge starts in a grid when levels get large
fn loops(edges: &[(usize, [DVec2; 2])]) -> Vec<Vec<DVec2>> {
    let turn = |d: DVec2, [p, q]: [DVec2; 2]| {
        let e = q - p;
        d.perp_dot(e).atan2(d.dot(e))
    };
    let mut used = vec![false; edges.len()];
    let mut out = Vec::new();
    for first in 0..edges.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let [mut a, mut b] = edges[first].1;
        let start = a;
        let mut pts = vec![start];
        loop {
            if b.distance(start) <= ON {
                let l = simplify(pts);
                if l.len() >= 3 {
                    out.push(l);
                }
                break;
            }
            let next = (0..edges.len())
                .filter(|&k| !used[k] && edges[k].1[0].distance(b) <= ON)
                .min_by(|&k, &l| turn(b - a, edges[k].1).total_cmp(&turn(b - a, edges[l].1)));
            let Some(k) = next else { break };
            used[k] = true;
            pts.push(edges[k].1[0]);
            [a, b] = edges[k].1;
        }
    }
    out
}

/// `pts` without points within `ON` of the previous one and without points on the straight line through their
/// neighbours.
fn simplify(mut pts: Vec<DVec2>) -> Vec<DVec2> {
    pts.dedup_by(|q, p| q.distance(*p) <= ON);
    while pts.len() >= 3 {
        let n = pts.len();
        let straight = (0..n).find(|&k| {
            let (p, q, r) = (pts[(k + n - 1) % n], pts[k], pts[(k + 1) % n]);
            let (u, w) = ((q - p).normalize(), (r - q).normalize());
            u.perp_dot(w).abs() <= 1e-9 && u.dot(w) > 0.0
        });
        let Some(k) = straight else { break };
        pts.remove(k);
    }
    pts
}

#[cfg(test)]
#[path = "room_tests.rs"]
mod tests;
