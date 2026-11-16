use crate::Body;
use cad_topology::Surface;
use glam::DVec3;
use std::collections::{BTreeMap, HashMap};
use std::f64::consts::TAU;

#[derive(Debug, Clone, PartialEq)]
pub enum EdgeGeom {
    Line { a: DVec3, b: DVec3 },
    /// Arc of a circle: from `start`, by `sweep` radians counter-clockwise about `axis` (TAU for a closed circle).
    Circle { center: DVec3, axis: DVec3, radius: f64, start: DVec3, sweep: f64 },
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Convexity {
    Convex,
    Concave,
    Smooth,
}

/// An edge of a solid: where two faces meet. A closed edge does not repeat its first point in `points`.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyEdge {
    pub faces: [u32; 2],
    pub points: Vec<DVec3>,
    pub closed: bool,
    pub geom: EdgeGeom,
    pub convexity: Convexity,
}

/// Collinearity of a line edge, and the parallelism of a cylinder axis and a plane normal.
const LINE_TOL: f64 = 1e-6;
const AXIS_TOL: f64 = 1e-9;
const CONVEX_TOL: f64 = 1e-7;

pub(crate) fn point_segment(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let d = b - a;
    let l2 = d.length_squared();
    let t = if l2 > 0.0 { ((p - a).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + d * t)
}

impl BodyEdge {
    pub(crate) fn segments(&self) -> impl Iterator<Item = (DVec3, DVec3)> + '_ {
        let n = self.points.len();
        let count = if self.closed { n } else { n.saturating_sub(1) };
        (0..count).map(move |i| (self.points[i], self.points[(i + 1) % n]))
    }

    pub fn distance(&self, p: DVec3) -> f64 {
        self.segments().map(|(a, b)| point_segment(p, a, b)).fold(f64::INFINITY, f64::min)
    }

    /// The point halfway along the polyline.
    pub fn midpoint(&self) -> DVec3 {
        let mut left = self.segments().map(|(a, b)| a.distance(b)).sum::<f64>() / 2.0;
        for (a, b) in self.segments() {
            let l = a.distance(b);
            if left <= l {
                return if l > 0.0 { a.lerp(b, left / l) } else { a };
            }
            left -= l;
        }
        self.points[0]
    }
}

/// Connected chains of undirected segments; a vertex used by other than two segments ends a chain.
/// Each chain is its vertex path and whether it is a loop (the path then does not repeat its first vertex).
fn chains(segs: &[(u32, u32)]) -> Vec<(Vec<u32>, bool)> {
    let mut adj: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, &(a, b)) in segs.iter().enumerate() {
        adj.entry(a).or_default().push(i);
        adj.entry(b).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    let mut walk = |from: u32, first: usize, used: &mut Vec<bool>| {
        let (mut path, mut cur, mut seg) = (vec![from], from, first);
        loop {
            used[seg] = true;
            let next = if segs[seg].0 == cur { segs[seg].1 } else { segs[seg].0 };
            path.push(next);
            let at = &adj[&next];
            if at.len() != 2 {
                break;
            }
            let following = if at[0] == seg { at[1] } else { at[0] };
            if used[following] {
                break;
            }
            (cur, seg) = (next, following);
        }
        let closed = path.len() > 2 && path[0] == path[path.len() - 1];
        if closed {
            path.pop();
        }
        out.push((path, closed));
    };
    for (&v, at) in &adj {
        if at.len() != 2 {
            for &s in at {
                if !used[s] {
                    walk(v, s, &mut used);
                }
            }
        }
    }
    for s in 0..segs.len() {
        if !used[s] {
            walk(segs[s].0, s, &mut used);
        }
    }
    out
}

fn circle_geom(plane: (DVec3, DVec3), cyl: (DVec3, DVec3, f64), pts: &[DVec3], closed: bool) -> EdgeGeom {
    let ((po, n), (co, axis, radius)) = (plane, cyl);
    if axis.cross(n).length() > AXIS_TOL {
        return EdgeGeom::Other;
    }
    let center = co + axis * ((po - co).dot(n) / axis.dot(n));
    let count = if closed { pts.len() } else { pts.len() - 1 };
    let turns: Vec<(f64, f64)> = (0..count)
        .map(|i| {
            let (a, b) = (pts[i] - center, pts[(i + 1) % pts.len()] - center);
            (a.cross(b).dot(n), a.dot(b))
        })
        .collect();
    let sign = turns.iter().map(|t| t.0).sum::<f64>();
    if sign.abs() < 1e-12 {
        return EdgeGeom::Other;
    }
    let sign = sign.signum();
    let sweep = if closed { TAU } else { turns.iter().map(|&(c, d)| (c * sign).atan2(d)).sum() };
    EdgeGeom::Circle { center, axis: n * sign, radius, start: pts[0], sweep }
}

fn edge_geom(surfaces: [&Surface; 2], pts: &[DVec3], closed: bool) -> EdgeGeom {
    match surfaces {
        [Surface::Plane { .. }, Surface::Plane { .. }] if !closed => {
            let (a, b) = (pts[0], pts[pts.len() - 1]);
            let d = (b - a).normalize_or_zero();
            if d != DVec3::ZERO && pts.iter().all(|p| (*p - a).cross(d).length() <= LINE_TOL) {
                EdgeGeom::Line { a, b }
            } else {
                EdgeGeom::Other
            }
        }
        [Surface::Plane { origin, normal }, Surface::Cylinder { origin: o, axis, radius }]
        | [Surface::Cylinder { origin: o, axis, radius }, Surface::Plane { origin, normal }] => {
            circle_geom((*origin, *normal), (*o, *axis, *radius), pts, closed)
        }
        // Also any pair with a Faceted face.
        _ => EdgeGeom::Other,
    }
}

impl Body {
    pub fn edges(&self) -> Vec<BodyEdge> {
        let tris = self.mesh.indices.as_chunks::<3>().0;
        let mut tri_of: HashMap<(u32, u32), usize> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            for k in 0..3 {
                tri_of.insert((t[k], t[(k + 1) % 3]), i);
            }
        }
        let mut groups: BTreeMap<[u32; 2], Vec<(u32, u32)>> = BTreeMap::new();
        for (i, t) in tris.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                let Some(&j) = tri_of.get(&(b, a)).filter(|_| a < b) else { continue };
                let (f, g) = (self.tri_face[i], self.tri_face[j]);
                if f != g {
                    groups.entry([f.min(g), f.max(g)]).or_default().push((a, b));
                }
            }
        }
        let mut out = Vec::new();
        for (faces, segs) in groups {
            for (path, closed) in chains(&segs) {
                let points: Vec<DVec3> = path.iter().map(|&v| self.mesh.positions[v as usize]).collect();
                let surfaces = faces.map(|f| &self.faces[f as usize].surface);
                let geom = edge_geom(surfaces, &points, closed);
                let convexity = self.convexity(faces, [path[0], path[1]], &tri_of);
                out.push(BodyEdge { faces, points, closed, geom, convexity });
            }
        }
        out
    }

    /// From one segment `ab` and its two triangles: does the solid bend away from the other face's tangent plane?
    fn convexity(&self, faces: [u32; 2], [a, b]: [u32; 2], tri_of: &HashMap<(u32, u32), usize>) -> Convexity {
        let is_plane = |f: u32| matches!(self.faces[f as usize].surface, Surface::Plane { .. });
        // The triangle comes from the plane when there is one: a facet of a curved face never looks tangent.
        let (src, other) = if is_plane(faces[1]) && !is_plane(faces[0]) { (faces[1], faces[0]) } else { (faces[0], faces[1]) };
        let tri = [tri_of[&(a, b)], tri_of[&(b, a)]].into_iter().find(|&t| self.tri_face[t] == src);
        let Some(t) = tri else { return Convexity::Smooth };
        let third = self.mesh.indices[3 * t..3 * t + 3].iter().find(|&&v| v != a && v != b).copied().unwrap_or(a);
        let (pa, pb) = (self.mesh.positions[a as usize], self.mesh.positions[b as usize]);
        let m = (pa + pb) / 2.0;
        let w = self.mesh.positions[third as usize] - m;
        let face = &self.faces[other as usize];
        let n = if face.surface == Surface::Faceted {
            // A free-form face has no analytic normal: take the winding normal of its own triangle at the edge.
            let t = [tri_of[&(a, b)], tri_of[&(b, a)]].into_iter().find(|&t| self.tri_face[t] == other);
            let [p, q, r] = t.map_or([DVec3::ZERO; 3], |t| [0, 1, 2].map(|k| self.mesh.positions[self.mesh.indices[3 * t + k] as usize]));
            (q - p).cross(r - p).normalize_or_zero()
        } else {
            face.surface.normal_at(m) * if face.outward { 1.0 } else { -1.0 }
        };
        let s = n.dot(w);
        let eps = CONVEX_TOL * w.length();
        if s < -eps {
            Convexity::Convex
        } else if s > eps {
            Convexity::Concave
        } else {
            Convexity::Smooth
        }
    }

    /// The edge closest to `p`, if it is within `max_dist`.
    pub fn nearest_edge(&self, p: DVec3, max_dist: f64) -> Option<BodyEdge> {
        let (d, e) = self.edges().into_iter().map(|e| (e.distance(p), e)).min_by(|x, y| x.0.total_cmp(&y.0))?;
        (d <= max_dist).then_some(e)
    }
}

#[cfg(test)]
#[path = "edges_tests.rs"]
mod tests;
