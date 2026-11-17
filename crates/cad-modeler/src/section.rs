//! Cut faces of a body on an axis-aligned plane, for the section view.
use crate::Body;
use cad_geometry::annot::inside_polygon;
use glam::{DVec2, DVec3};
use std::collections::{HashMap, HashSet};

type Key = (u32, u32);

impl Body {
    /// Triangles that fill the cut of the body by the plane `p[axis] = offset` (axis 0, 1, 2 = X, Y, Z).
    /// Empty when the plane misses the body. Winding is arbitrary.
    pub fn section_cap(&self, axis: usize, offset: f64) -> Vec<[DVec3; 3]> {
        let pos = &self.mesh.positions;
        let d = |i: u32| pos[i as usize][axis] - offset;
        let above = |i: u32| d(i) > 0.0;
        let mut point: HashMap<Key, DVec3> = HashMap::new();
        let mut next: HashMap<Key, Vec<Key>> = HashMap::new();
        for t in self.mesh.indices.as_chunks::<3>().0 {
            let mut keys = Vec::with_capacity(2);
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                if above(a) != above(b) {
                    let (pa, pb) = (pos[a as usize], pos[b as usize]);
                    let key = (a.min(b), a.max(b));
                    point.insert(key, pa + (pb - pa) * (d(a) / (d(a) - d(b))));
                    keys.push(key);
                }
            }
            if let [k0, k1] = keys[..] {
                next.entry(k0).or_default().push(k1);
                next.entry(k1).or_default().push(k0);
            }
        }
        let mut keys: Vec<Key> = next.keys().copied().collect();
        keys.sort_unstable();
        let mut seen: HashSet<Key> = HashSet::new();
        let mut loops: Vec<Vec<DVec3>> = Vec::new();
        for &start in &keys {
            if seen.contains(&start) {
                continue;
            }
            let (mut chain, mut prev, mut cur, mut ok) = (Vec::new(), None, start, true);
            loop {
                seen.insert(cur);
                chain.push(cur);
                let nb = &next[&cur];
                if nb.len() != 2 {
                    ok = false;
                    break;
                }
                let n = if Some(nb[0]) != prev { nb[0] } else { nb[1] };
                if n == start {
                    break;
                }
                if seen.contains(&n) {
                    ok = false;
                    break;
                }
                (prev, cur) = (Some(cur), n);
            }
            if ok && chain.len() >= 3 {
                loops.push(chain.iter().map(|k| point[k]).collect());
            }
        }
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let flat: Vec<Vec<DVec2>> = loops.iter().map(|l| l.iter().map(|p| DVec2::new(p[u], p[v])).collect()).collect();
        let depth: Vec<usize> =
            (0..flat.len()).map(|i| (0..flat.len()).filter(|&j| j != i && inside_polygon(&flat[j], flat[i][0])).count()).collect();
        let mut out = Vec::new();
        for o in (0..flat.len()).filter(|&i| depth[i].is_multiple_of(2)) {
            let mut members = vec![o];
            members.extend((0..flat.len()).filter(|&h| depth[h] == depth[o] + 1 && inside_polygon(&flat[o], flat[h][0])));
            let rings: Vec<Vec<DVec2>> = members.iter().map(|&m| flat[m].clone()).collect();
            let pts: Vec<DVec3> = members.iter().flat_map(|&m| loops[m].iter().copied()).collect();
            if let Ok(tris) = crate::cap::triangulate(&rings) {
                out.extend(tris.into_iter().map(|[a, b, c]| [pts[a], pts[b], pts[c]]));
            }
        }
        out
    }
}

#[cfg(test)]
#[path = "section_tests.rs"]
mod tests;
