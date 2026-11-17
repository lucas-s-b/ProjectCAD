//! Joins of straight walls on one level. Wall ends that meet are cut against their neighbours around the meeting
//! point; an end on another wall's baseline (a T) stops at that wall's faces; walls that cross (an X) stay whole.
use cad_geometry::{DVec2, Wall, point_segment_distance};

/// Wall ends closer than this (mm) meet; an end this close to the inside of another wall's baseline makes a T.
pub const JOIN_TOL: f64 = 1e-3;
/// A corner farther from the meeting point than this many times the two thicknesses gets square ends instead.
// shortcut: very sharp joins (under about 6 degrees) leave a notch; add a bevel when someone draws them
const MITRE_LIMIT: f64 = 5.0;

/// One wall end at a meeting point: `dir` points away from the point along the wall, `left` and `right` are the
/// offsets of the faces on its left and right along `dir.perp()` (left > right). `wall` is (index, end is `b`), or
/// `None` for the two halves of a wall that passes through the point (a T).
struct End {
    wall: Option<(usize, bool)>,
    dir: DVec2,
    left: f64,
    right: f64,
}

/// The corners of one wall end on its left and right face (seen from the end, looking along the wall), and the
/// meeting point when it belongs to the outline (three or more wall ends meet there).
#[derive(Clone, Copy)]
struct Corners {
    left: DVec2,
    right: DVec2,
    hub: Option<DVec2>,
}

fn end(w: &Wall, index: usize, at_b: bool) -> End {
    let (l, r) = w.faces();
    let d = (w.b - w.a).normalize();
    if at_b {
        End { wall: Some((index, true)), dir: -d, left: -r, right: -l }
    } else {
        End { wall: Some((index, false)), dir: d, left: l, right: r }
    }
}

/// Square corners of an end that meets nothing.
fn free(w: &Wall, at_b: bool) -> Corners {
    let e = end(w, 0, at_b);
    let p = if at_b { w.b } else { w.a };
    Corners { left: p + e.dir.perp() * e.left, right: p + e.dir.perp() * e.right, hub: None }
}

/// One counter-clockwise outline per wall, in the order of `walls`: its four corners moved by the joins, plus the
/// meeting point where three or more wall ends meet. Consecutive points closer than 1e-9 are merged.
pub fn footprints(walls: &[Wall]) -> Vec<Vec<DVec2>> {
    let mut corners: Vec<[Corners; 2]> = walls.iter().map(|w| [free(w, false), free(w, true)]).collect();
    let mut done = vec![[false; 2]; walls.len()];
    for i in 0..walls.len() {
        for at_b in [false, true] {
            if done[i][usize::from(at_b)] {
                continue;
            }
            let p = if at_b { walls[i].b } else { walls[i].a };
            let mut ends = Vec::new();
            for (j, w) in walls.iter().enumerate() {
                for jb in [false, true] {
                    if (if jb { w.b } else { w.a }).distance(p) <= JOIN_TOL {
                        ends.push(end(w, j, jb));
                        done[j][usize::from(jb)] = true;
                    }
                }
                if w.a.distance(p) > JOIN_TOL && w.b.distance(p) > JOIN_TOL && point_segment_distance(p, w.a, w.b) <= JOIN_TOL {
                    let (l, r) = w.faces();
                    let d = (w.b - w.a).normalize();
                    ends.push(End { wall: None, dir: d, left: l, right: r });
                    ends.push(End { wall: None, dir: -d, left: -r, right: -l });
                }
            }
            if ends.len() >= 2 {
                join(p, &mut ends, &mut corners);
            }
        }
    }
    corners.iter().map(|[s, e]| outline(s, e)).collect()
}

/// Sorts the ends counter-clockwise and gives each pair of neighbours the crossing of their facing faces: the left
/// face of an end meets the right face of the next end counter-clockwise.
fn join(p: DVec2, ends: &mut [End], corners: &mut [[Corners; 2]]) {
    ends.sort_by(|x, y| x.dir.to_angle().total_cmp(&y.dir.to_angle()));
    let n = ends.len();
    let hub = (n >= 3 && ends.iter().all(|e| e.wall.is_some())).then_some(p);
    let (mut left, mut right) = (vec![p; n], vec![p; n]);
    for i in 0..n {
        let j = (i + 1) % n;
        let (e, f) = (&ends[i], &ends[j]);
        let o1 = p + e.dir.perp() * e.left;
        let o2 = p + f.dir.perp() * f.right;
        let cross = e.dir.perp_dot(f.dir);
        let corner = (cross.abs() > 1e-9).then(|| o1 + e.dir * ((o2 - o1).perp_dot(f.dir) / cross));
        let limit = MITRE_LIMIT * (e.left - e.right + f.left - f.right);
        match corner.filter(|c| c.distance(p) <= limit) {
            Some(c) => (left[i], right[j]) = (c, c),
            // Parallel (straight on) or too sharp: square ends.
            None => (left[i], right[j]) = (o1, o2),
        }
    }
    for (k, e) in ends.iter().enumerate() {
        if let Some((w, at_b)) = e.wall {
            corners[w][usize::from(at_b)] = Corners { left: left[k], right: right[k], hub };
        }
    }
}

/// At `a` the end's left is the wall's left face; at `b` the end's left is the wall's right face.
fn outline(s: &Corners, e: &Corners) -> Vec<DVec2> {
    let mut pts = vec![s.right, e.left];
    pts.extend(e.hub);
    pts.extend([e.right, s.left]);
    pts.extend(s.hub);
    pts.dedup_by(|q, p| q.distance(*p) <= 1e-9);
    if pts.len() > 1 && pts[0].distance(pts[pts.len() - 1]) <= 1e-9 {
        pts.pop();
    }
    pts
}

#[cfg(test)]
#[path = "join_tests.rs"]
mod tests;
