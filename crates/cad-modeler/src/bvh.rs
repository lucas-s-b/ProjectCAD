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
}

#[cfg(test)]
#[path = "bvh_tests.rs"]
mod tests;
