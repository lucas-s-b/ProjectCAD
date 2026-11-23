//! Tetrahedral meshes of solids for finite elements: a grid of cells cut into Kuhn tetrahedra, boundary nodes
//! snapped onto the surface, quadratic 10-node elements with straight sides.
use cad_modeler::{Bvh, TriMesh};
use glam::DVec3;
use std::collections::HashMap;

/// Most grid cells `tet_mesh` accepts (6 candidate tetrahedra each): keeps meshing interactive in a debug build.
pub const MAX_CELLS: usize = 50_000;

/// A boundary node move is refused when any tetrahedron of the node would end up with a radius ratio below this.
const QUALITY_FLOOR: f64 = 0.15;

/// The six Kuhn (Freudenthal) tetrahedra of a cell, as cell corners (bit 0 = +x, bit 1 = +y, bit 2 = +z): the
/// paths from corner 0 to corner 7 along cell edges, each ordered for positive volume. They share the diagonal 0-7
/// and cut every cell face along the diagonal from its lowest to its highest corner, so neighbouring cells match.
pub const KUHN: [[usize; 4]; 6] = [[0, 1, 3, 7], [0, 2, 6, 7], [0, 4, 5, 7], [0, 5, 1, 7], [0, 3, 2, 7], [0, 6, 4, 7]];

/// Faces of a tetrahedron (corner indices), face k opposite corner k, ordered outward for a positive tetrahedron.
pub const TET_FACES: [[usize; 3]; 4] = [[1, 2, 3], [0, 3, 2], [0, 1, 3], [0, 2, 1]];

/// Corner pairs of the mid-side nodes 4..10 of a 10-node tetrahedron, in the order of Abaqus C3D10 and
/// VTK_QUADRATIC_TETRA (0-based): 4 = (0, 1), 5 = (1, 2), 6 = (2, 0), 7 = (0, 3), 8 = (1, 3), 9 = (2, 3).
pub const TET10_EDGES: [[usize; 2]; 6] = [[0, 1], [1, 2], [2, 0], [0, 3], [1, 3], [2, 3]];

/// A quadratic tetrahedral mesh with straight sides. mm.
#[derive(Debug, Clone, PartialEq)]
pub struct TetMesh {
    /// Node positions: the `corners` corner nodes first (in grid order: x fastest, then y, then z), then one
    /// mid-side node per edge, in the order the elements first use them.
    pub nodes: Vec<DVec3>,
    pub corners: usize,
    /// Nodes of each element: corners 0..4 with (p1 − p0) × (p2 − p0) · (p3 − p0) > 0, then the mid-side nodes 4..10
    /// on the corner pairs of `TET10_EDGES`. Elements come cell by cell (x fastest, then y, then z), `KUHN` order within.
    pub elements: Vec<[u32; 10]>,
    /// Grid cell size along x, y and z, each at most the element size asked for.
    pub cell: DVec3,
    /// Boundary corner nodes left off the surface (fully or partly) because moving them would have spoilt an element.
    pub unsnapped: usize,
}

/// Size and quality of a `TetMesh`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshStats {
    pub nodes: usize,
    pub elements: usize,
    /// Sum of the element volumes, mm³.
    pub volume: f64,
    /// Smallest radius ratio (`radius_ratio`).
    pub min_ratio: f64,
    /// Smallest dihedral angle, degrees.
    pub min_dihedral: f64,
    /// Elements whose volume is not positive; 0 by construction.
    pub inverted: usize,
}

/// Signed volume of the tetrahedron, positive when (b − a) × (c − a) · (d − a) > 0.
pub fn tet_volume([a, b, c, d]: [DVec3; 4]) -> f64 {
    (b - a).cross(c - a).dot(d - a) / 6.0
}

/// 3 r_in / R_circ: 1 for the regular tetrahedron, √3 (√2 − 1) = 0.7174 for a Kuhn tetrahedron of a cube, 0 when
/// flat or inverted. r_in = 3 V / (area of the four faces); R_circ = |a² (b × c) + b² (c × a) + c² (a × b)| / (12 V)
/// with a, b, c the edges from corner 0.
pub fn radius_ratio(p: [DVec3; 4]) -> f64 {
    let v = tet_volume(p);
    if v <= 0.0 {
        return 0.0;
    }
    let (a, b, c) = (p[1] - p[0], p[2] - p[0], p[3] - p[0]);
    let area: f64 = TET_FACES.iter().map(|f| (p[f[1]] - p[f[0]]).cross(p[f[2]] - p[f[0]]).length() / 2.0).sum();
    let r_in = 3.0 * v / area;
    let r_circ = (b.cross(c) * a.length_squared() + c.cross(a) * b.length_squared() + a.cross(b) * c.length_squared()).length() / (12.0 * v);
    3.0 * r_in / r_circ
}

/// Smallest dihedral angle of the tetrahedron, degrees: 180° minus the angle between the outward normals of each
/// pair of faces (70.53° for the regular tetrahedron, 45° for a Kuhn tetrahedron of a cube).
pub fn min_dihedral_angle(p: [DVec3; 4]) -> f64 {
    // Unit normal of the face opposite corner k, turned away from corner k.
    let normal = |k: usize| {
        let [a, b, c] = TET_FACES[k].map(|q| p[q]);
        let n = (b - a).cross(c - a).normalize_or_zero();
        if n.dot(p[k] - a) > 0.0 { -n } else { n }
    };
    let n = [0, 1, 2, 3].map(normal);
    let mut least = 180.0f64;
    for i in 0..4 {
        for j in i + 1..4 {
            least = least.min(180.0 - n[i].dot(n[j]).clamp(-1.0, 1.0).acos().to_degrees());
        }
    }
    least
}

/// Faces used by exactly one of `tets` (corner node ids), ordered outward, in element order.
fn outer_faces(tets: &[[u32; 4]]) -> Vec<[u32; 3]> {
    let faces = || tets.iter().flat_map(|t| TET_FACES.map(|f| f.map(|q| t[q])));
    let key = |f: [u32; 3]| {
        let mut k = f;
        k.sort_unstable();
        k
    };
    let mut count: HashMap<[u32; 3], u32> = HashMap::new();
    for f in faces() {
        *count.entry(key(f)).or_default() += 1;
    }
    faces().filter(|f| count[&key(*f)] == 1).collect()
}

/// x of the surface crossings of the ray from `origin` along +x, ascending. Hits within `eps` of each other are one
/// place where the ray meets an edge or a vertex: one crossing when all those triangles face the same way along x,
/// none when they disagree (the ray grazes the surface there).
fn crossings(surface: &TriMesh, bvh: &Bvh, origin: DVec3, eps: f64) -> Vec<f64> {
    let mut hits: Vec<(f64, bool)> = bvh
        .hits(surface, origin, DVec3::X)
        .into_iter()
        .map(|(t, tri)| {
            let [a, b, c] = [0, 1, 2].map(|q| surface.positions[surface.indices[3 * tri + q] as usize]);
            (t, (b - a).cross(c - a).x > 0.0)
        })
        .collect();
    hits.sort_by(|p, q| p.0.total_cmp(&q.0));
    let (mut out, mut i) = (Vec::new(), 0);
    while i < hits.len() {
        let mut j = i;
        while j + 1 < hits.len() && hits[j + 1].0 - hits[j].0 <= eps {
            j += 1;
        }
        if hits[i..=j].iter().all(|h| h.1 == hits[i].1) {
            out.push(origin.x + hits[i].0);
        }
        i = j + 1;
    }
    out
}

impl TetMesh {
    /// Corner positions of element `k`.
    pub fn corners_of(&self, k: usize) -> [DVec3; 4] {
        let el = &self.elements[k];
        [0, 1, 2, 3].map(|q| self.nodes[el[q] as usize])
    }

    /// Counts, volume and quality.
    pub fn stats(&self) -> MeshStats {
        let (mut volume, mut min_ratio, mut min_dihedral, mut inverted) = (0.0, f64::INFINITY, f64::INFINITY, 0);
        for k in 0..self.elements.len() {
            let p = self.corners_of(k);
            let v = tet_volume(p);
            volume += v;
            if v <= 0.0 {
                inverted += 1;
            }
            min_ratio = min_ratio.min(radius_ratio(p));
            min_dihedral = min_dihedral.min(min_dihedral_angle(p));
        }
        MeshStats { nodes: self.nodes.len(), elements: self.elements.len(), volume, min_ratio, min_dihedral, inverted }
    }

    /// The boundary triangles (element faces no other element shares), as corner node triples ordered outward, in
    /// element order: E integrates loads over them and draws its colour maps on them.
    pub fn boundary(&self) -> Vec<[u32; 3]> {
        let tets: Vec<[u32; 4]> = self.elements.iter().map(|el| [el[0], el[1], el[2], el[3]]).collect();
        outer_faces(&tets)
    }

    /// The nodes (corner and mid-side) within `tol` of the triangles of `face` (one body face), ascending: where a
    /// load or support on that face acts.
    pub fn nodes_near(&self, face: &TriMesh, tol: f64) -> Vec<u32> {
        let bvh = Bvh::build(face);
        (0..self.nodes.len() as u32)
            .filter(|&v| {
                let p = self.nodes[v as usize];
                bvh.nearest(face, p).is_some_and(|(q, _)| q.distance(p) <= tol)
            })
            .collect()
    }
}

/// The tetrahedral mesh of the solid bounded by `surface` (closed, outward) with element size `h` (mm, the cell edge
/// limit). Errors: "element size must be positive", "the body has no inside (open or inside-out surface)",
/// "element size 0.1 mm gives 2000000 grid cells (at most 50000); use a larger element size", "no element lies
/// inside the body; use a smaller element size".
pub fn tet_mesh(surface: &TriMesh, h: f64) -> Result<TetMesh, String> {
    if !(h > 0.0 && h.is_finite()) {
        return Err("element size must be positive".into());
    }
    if surface.volume().is_nan() || surface.volume() <= 0.0 {
        return Err("the body has no inside (open or inside-out surface)".into());
    }
    let lo = surface.positions.iter().fold(DVec3::splat(f64::INFINITY), |m, p| m.min(*p));
    let hi = surface.positions.iter().fold(DVec3::splat(f64::NEG_INFINITY), |m, p| m.max(*p));
    let ext = hi - lo;
    // 1e-9 keeps 20 / 2.5 = 8.000000000000002 from adding a ninth layer.
    let count = |len: f64| (len / h - 1e-9).ceil().max(1.0);
    let f = DVec3::new(count(ext.x), count(ext.y), count(ext.z));
    let cells = f.x * f.y * f.z;
    if cells > MAX_CELLS as f64 {
        return Err(format!("element size {h} mm gives {cells} grid cells (at most {MAX_CELLS}); use a larger element size"));
    }
    let n = [f.x as usize, f.y as usize, f.z as usize];
    let cell = ext / f;
    let grid = |i: usize, j: usize, k: usize| i + (n[0] + 1) * (j + (n[1] + 1) * k);
    let bit = |c: usize| [c & 1, (c >> 1) & 1, (c >> 2) & 1];
    let bvh = Bvh::build(surface);
    let eps = 1e-9 * ext.length();

    // Inside test: the centroids of Kuhn type t lie at the same (y, z) fractions in every cell, so for each t and
    // each cell row (j, k) one ray along +x classifies the whole row by the parity of the crossings before each
    // centroid. `inside[((k * n[1] + j) * n[0] + i) * 6 + t]`.
    let mut inside = vec![false; n[0] * n[1] * n[2] * 6];
    for (t, tet) in KUHN.iter().enumerate() {
        // Centroid in cell fractions: the mean of the four corners' bits.
        let frac = tet.iter().fold(DVec3::ZERO, |s, &c| s + DVec3::from(bit(c).map(|b| b as f64))) / 4.0;
        for k in 0..n[2] {
            for j in 0..n[1] {
                // Rows moved by (√2, √3) · 1e-3 of a cell: centroids sit at quarters of a cell, where the diagonals of
                // faces lying on grid lines pass; the moved ray misses their edges and vertices.
                let y = lo.y + (j as f64 + frac.y + std::f64::consts::SQRT_2 * 1e-3) * cell.y;
                let z = lo.z + (k as f64 + frac.z + 3f64.sqrt() * 1e-3) * cell.z;
                let xs = crossings(surface, &bvh, DVec3::new(lo.x - cell.x, y, z), eps);
                for i in 0..n[0] {
                    let x = lo.x + (i as f64 + frac.x) * cell.x;
                    inside[((k * n[1] + j) * n[0] + i) * 6 + t] = xs.iter().filter(|&&s| s < x).count() % 2 == 1;
                }
            }
        }
    }

    // Elements and corner nodes: kept tetrahedra as grid ids, cell by cell.
    let mut kept: Vec<[usize; 4]> = Vec::new();
    for k in 0..n[2] {
        for j in 0..n[1] {
            for i in 0..n[0] {
                for (t, tet) in KUHN.iter().enumerate() {
                    if inside[((k * n[1] + j) * n[0] + i) * 6 + t] {
                        kept.push(tet.map(|c| {
                            let b = bit(c);
                            grid(i + b[0], j + b[1], k + b[2])
                        }));
                    }
                }
            }
        }
    }
    if kept.is_empty() {
        return Err("no element lies inside the body; use a smaller element size".into());
    }
    let mut number = vec![u32::MAX; (n[0] + 1) * (n[1] + 1) * (n[2] + 1)];
    for id in kept.iter().flatten() {
        number[*id] = 0;
    }
    let mut nodes: Vec<DVec3> = Vec::new();
    for (id, slot) in number.iter_mut().enumerate() {
        if *slot == 0 {
            *slot = nodes.len() as u32;
            let (i, j, k) = (id % (n[0] + 1), (id / (n[0] + 1)) % (n[1] + 1), id / ((n[0] + 1) * (n[1] + 1)));
            nodes.push(lo + DVec3::new(i as f64, j as f64, k as f64) * cell);
        }
    }
    let tets: Vec<[u32; 4]> = kept.iter().map(|t| t.map(|id| number[id])).collect();

    // Snapping (the inversion repair is the guard: a move that would spoil a tetrahedron is shortened or dropped, so
    // no element is ever inverted). Boundary corner nodes, ascending, each with its nearest surface point; the
    // elements of every node.
    let mut boundary: Vec<u32> = outer_faces(&tets).into_iter().flatten().collect();
    boundary.sort_unstable();
    boundary.dedup();
    let mut of_node: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (k, t) in tets.iter().enumerate() {
        for &v in t {
            of_node[v as usize].push(k);
        }
    }
    let goals: Vec<DVec3> = boundary.iter().map(|&v| bvh.nearest(surface, nodes[v as usize]).map_or(nodes[v as usize], |(q, _)| q)).collect();
    let fits = |nodes: &[DVec3], v: usize| {
        of_node[v].iter().all(|&k| {
            let p = tets[k].map(|q| nodes[q as usize]);
            tet_volume(p) > 0.0 && radius_ratio(p) >= QUALITY_FLOOR
        })
    };
    let mut done = vec![false; boundary.len()];
    // Two passes: a node refused in the first may fit once its neighbours have moved.
    for _ in 0..2 {
        for (b, &v) in boundary.iter().enumerate() {
            if done[b] {
                continue;
            }
            let (v, from) = (v as usize, nodes[v as usize]);
            for s in [1.0, 0.5, 0.25] {
                nodes[v] = from + (goals[b] - from) * s;
                if fits(&nodes, v) {
                    done[b] = s == 1.0;
                    break;
                }
                nodes[v] = from;
            }
        }
    }
    let unsnapped = done.iter().filter(|d| !**d).count();

    // Mid-side nodes after snapping, straight (the midpoint of the snapped corners).
    let corners = nodes.len();
    let mut mid: HashMap<(u32, u32), u32> = HashMap::new();
    let mut elements = Vec::with_capacity(tets.len());
    for t in &tets {
        let mut el = [0u32; 10];
        el[..4].copy_from_slice(t);
        for (q, [a, b]) in TET10_EDGES.iter().enumerate() {
            let key = (t[*a].min(t[*b]), t[*a].max(t[*b]));
            el[4 + q] = match mid.get(&key) {
                Some(&id) => id,
                None => {
                    let id = nodes.len() as u32;
                    nodes.push((nodes[key.0 as usize] + nodes[key.1 as usize]) / 2.0);
                    mid.insert(key, id);
                    id
                }
            };
        }
        elements.push(el);
    }
    Ok(TetMesh { nodes, corners, elements, cell, unsnapped })
}

#[cfg(test)]
#[path = "tet_mesh_tests.rs"]
mod tests;
