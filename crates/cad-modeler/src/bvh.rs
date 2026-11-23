//! Bounding volume hierarchy over the triangles of a mesh, for ray picking.
use crate::TriMesh;
use glam::DVec3;

/// Distance along the ray to the triangle, Moller-Trumbore; `None` when missed or behind the origin.
pub fn ray_triangle(origin: DVec3, dir: DVec3, [a, b, c]: [DVec3; 3]) -> Option<f64> {
    let (e1, e2) = (b - a, c - a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let s = origin - a;
    let u = s.dot(p) / det;
    let q = s.cross(e1);
    let v = dir.dot(q) / det;
    let t = e2.dot(q) / det;
    (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0).then_some(t)
}

/// The point of triangle [a, b, c] nearest to `p` (Ericson, Real-Time Collision Detection, 5.1.5: vertex, edge or
/// face region). A degenerate triangle gives its nearest corner.
pub fn closest_on_triangle(p: DVec3, [a, b, c]: [DVec3; 3]) -> DVec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let sum = va + vb + vc;
    if sum <= 0.0 {
        return [a, b, c].into_iter().min_by(|x, y| x.distance_squared(p).total_cmp(&y.distance_squared(p))).unwrap_or(a);
    }
    a + ab * (vb / sum) + ac * (vc / sum)
}

struct Node {
    lo: DVec3,
    hi: DVec3,
    /// Leaf: first index into `order`. Inner: index of the right child (the left child is the next node).
    first: u32,
    /// Triangles in a leaf; 0 for an inner node.
    count: u32,
}

pub struct Bvh {
    nodes: Vec<Node>,
    /// Triangle indices, grouped by leaf.
    order: Vec<u32>,
}

const LEAF: usize = 4;

fn split(nodes: &mut Vec<Node>, order: &mut [u32], offset: usize, tris: &[[DVec3; 3]], centers: &[DVec3]) {
    let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
    let (mut clo, mut chi) = (lo, hi);
    for &t in order.iter() {
        for p in tris[t as usize] {
            (lo, hi) = (lo.min(p), hi.max(p));
        }
        (clo, chi) = (clo.min(centers[t as usize]), chi.max(centers[t as usize]));
    }
    let ext = chi - clo;
    if order.len() <= LEAF || ext.max_element() <= 0.0 {
        nodes.push(Node { lo, hi, first: offset as u32, count: order.len() as u32 });
        return;
    }
    let axis = if ext.x >= ext.y && ext.x >= ext.z { 0 } else if ext.y >= ext.z { 1 } else { 2 };
    let mid = order.len() / 2;
    order.select_nth_unstable_by(mid, |&a, &b| centers[a as usize][axis].total_cmp(&centers[b as usize][axis]));
    let at = nodes.len();
    nodes.push(Node { lo, hi, first: 0, count: 0 });
    let (left, right) = order.split_at_mut(mid);
    split(nodes, left, offset, tris, centers);
    nodes[at].first = nodes.len() as u32;
    split(nodes, right, offset + mid, tris, centers);
}

fn corners(mesh: &TriMesh, tri: usize) -> [DVec3; 3] {
    [0, 1, 2].map(|k| mesh.positions[mesh.indices[3 * tri + k] as usize])
}

impl Bvh {
    pub fn build(mesh: &TriMesh) -> Bvh {
        let tris: Vec<[DVec3; 3]> = (0..mesh.indices.len() / 3).map(|t| corners(mesh, t)).collect();
        let centers: Vec<DVec3> = tris.iter().map(|[a, b, c]| (*a + *b + *c) / 3.0).collect();
        let mut order: Vec<u32> = (0..tris.len() as u32).collect();
        let mut nodes = Vec::new();
        if !order.is_empty() {
            split(&mut nodes, &mut order, 0, &tris, &centers);
        }
        Bvh { nodes, order }
    }

    /// Nearest hit: (distance along `dir`, triangle index).
    pub fn ray(&self, mesh: &TriMesh, origin: DVec3, dir: DVec3) -> Option<(f64, usize)> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = DVec3::ONE / dir;
        let mut best: Option<(f64, usize)> = None;
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i];
            let (t0, t1) = ((node.lo - origin) * inv, (node.hi - origin) * inv);
            let (a, b) = (t0.min(t1), t0.max(t1));
            // f64::max and f64::min skip a NaN (0 * inf on a slab boundary).
            let tmin = a.x.max(a.y).max(a.z);
            let tmax = b.x.min(b.y).min(b.z);
            if tmax < tmin.max(0.0) || best.is_some_and(|(t, _)| tmin > t) {
                continue;
            }
            if node.count == 0 {
                stack.extend([node.first as usize, i + 1]);
                continue;
            }
            for &tri in &self.order[node.first as usize..(node.first + node.count) as usize] {
                if let Some(t) = ray_triangle(origin, dir, corners(mesh, tri as usize))
                    && best.is_none_or(|(b, _)| t < b)
                {
                    best = Some((t, tri as usize));
                }
            }
        }
        best
    }

    /// Every hit of the ray: (distance along `dir`, triangle index), unordered. A ray through an edge or a vertex hits
    /// every triangle there (`ray_triangle` includes the triangle's border); the caller merges such hits.
    pub fn hits(&self, mesh: &TriMesh, origin: DVec3, dir: DVec3) -> Vec<(f64, usize)> {
        let mut out = Vec::new();
        if self.nodes.is_empty() {
            return out;
        }
        let inv = DVec3::ONE / dir;
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i];
            let (t0, t1) = ((node.lo - origin) * inv, (node.hi - origin) * inv);
            let (a, b) = (t0.min(t1), t0.max(t1));
            // f64::max and f64::min skip a NaN (0 * inf on a slab boundary), as in `ray`.
            if b.x.min(b.y).min(b.z) < a.x.max(a.y).max(a.z).max(0.0) {
                continue;
            }
            if node.count == 0 {
                stack.extend([node.first as usize, i + 1]);
                continue;
            }
            for &tri in &self.order[node.first as usize..(node.first + node.count) as usize] {
                if let Some(t) = ray_triangle(origin, dir, corners(mesh, tri as usize)) {
                    out.push((t, tri as usize));
                }
            }
        }
        out
    }

    /// The point of the mesh nearest to `p` and its triangle; None for an empty mesh. Branch and bound on the boxes.
    pub fn nearest(&self, mesh: &TriMesh, p: DVec3) -> Option<(DVec3, usize)> {
        // (squared distance, point, triangle)
        let mut best: Option<(f64, DVec3, usize)> = None;
        let mut stack = if self.nodes.is_empty() { Vec::new() } else { vec![0usize] };
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i];
            let gap = (node.lo - p).max(p - node.hi).max(DVec3::ZERO);
            if best.is_some_and(|(d, ..)| gap.length_squared() > d) {
                continue;
            }
            if node.count == 0 {
                stack.extend([node.first as usize, i + 1]);
                continue;
            }
            for &tri in &self.order[node.first as usize..(node.first + node.count) as usize] {
                let q = closest_on_triangle(p, corners(mesh, tri as usize));
                let d = q.distance_squared(p);
                if best.is_none_or(|(b, ..)| d < b) {
                    best = Some((d, q, tri as usize));
                }
            }
        }
        best.map(|(_, q, tri)| (q, tri))
    }
}

#[cfg(test)]
#[path = "bvh_tests.rs"]
mod tests;
