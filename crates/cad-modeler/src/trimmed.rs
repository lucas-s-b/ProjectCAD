use crate::{Body, EdgeGeom, ModelError, Surface};
use glam::DVec3;
use std::collections::{HashMap, HashSet};

/// A solid rebuilt as an analytic B-Rep: faces on planes, cylinders, cones and spheres, bounded by loops of
/// line, circle and ellipse edges. Vertices lie on all their faces within `SNAP_TOL`.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimmedBody {
    pub vertices: Vec<DVec3>,
    pub edges: Vec<TrimEdge>,
    pub faces: Vec<TrimFace>,
}

/// An edge between two faces. A closed edge has the same start and end vertex.
/// The curve runs from `vertices[0]` to `vertices[1]` (for circles and ellipses: counter-clockwise about their axis).
#[derive(Debug, Clone, PartialEq)]
pub struct TrimEdge {
    pub vertices: [u32; 2],
    pub geom: EdgeGeom,
}

/// One connected face. Each loop is a list of (edge index, forward). Every loop runs counter-clockwise
/// seen from outside the solid, with the face on its left. `outer` is the outer loop of a planar face;
/// it is `None` on a curved face, where loops have no inside or outside.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimFace {
    pub surface: Surface,
    pub outward: bool,
    pub loops: Vec<Vec<(u32, bool)>>,
    pub outer: Option<usize>,
}

const SNAP_TOL: f64 = 1e-6;
const SNAP_ROUNDS: usize = 100;
const NOT_ANALYTIC: ModelError = ModelError::Invalid("face is not a plane, cylinder, cone or sphere");
const OTHER_EDGE: ModelError = ModelError::Invalid("edge is not a line, circle or ellipse");
const BAD_LOOP: ModelError = ModelError::Invalid("face boundary could not be rebuilt");
const OFF_SURFACE: ModelError = ModelError::Invalid("vertex does not lie on its faces");

/// The boundary half-edge that follows boundary half-edge `(a, b)` of triangle `t` along the face boundary.
fn next_boundary(body: &Body, tri_of: &HashMap<(u32, u32), usize>, mut t: usize, a: u32, b: u32) -> Result<(usize, u32, u32), ModelError> {
    let mut from = a;
    for _ in 0..10_000 {
        let tri = &body.mesh.indices[3 * t..3 * t + 3];
        let k = tri.iter().position(|&v| v == b).ok_or(BAD_LOOP)?;
        let c = tri[(k + 1) % 3]; // the half-edge b -> c follows from -> b inside t
        debug_assert_ne!(c, from);
        let twin = *tri_of.get(&(c, b)).ok_or(BAD_LOOP)?;
        if body.tri_face[twin] != body.tri_face[t] {
            return Ok((t, b, c));
        }
        (t, from) = (twin, c);
    }
    Err(BAD_LOOP)
}

impl TrimmedBody {
    /// Rebuilds the faces of `body` with their boundary loops. Fails for bodies with a face that is not
    /// a plane, cylinder, cone or sphere, an edge that is not a line, circle or ellipse, or a boundary that does not split cleanly into edges.
    pub fn from_body(body: &Body) -> Result<TrimmedBody, ModelError> {
        if !body.faces.iter().all(|f| matches!(f.surface, Surface::Plane { .. } | Surface::Cylinder { .. } | Surface::Cone { .. } | Surface::Sphere { .. })) {
            return Err(NOT_ANALYTIC);
        }
        let mut paths = body.edge_paths();
        // Body::edges leaves edges on cones and spheres Other (blends cannot use them); here they are exact circles.
        for (edge, _) in &mut paths {
            let curved = edge.faces.iter().any(|&f| matches!(body.faces[f as usize].surface, Surface::Cone { .. } | Surface::Sphere { .. }));
            if curved && edge.geom == EdgeGeom::Other {
                edge.geom = crate::edges::circle_geom(&edge.points, edge.closed);
            }
        }
        if paths.iter().any(|(e, _)| e.geom == EdgeGeom::Other) {
            return Err(OTHER_EDGE);
        }
        // Topological vertices: the ends of the edge paths.
        let mut vmap: HashMap<u32, u32> = HashMap::new();
        let mut mesh_of: Vec<u32> = Vec::new();
        let mut vertex = |m: u32| {
            *vmap.entry(m).or_insert_with(|| {
                mesh_of.push(m);
                mesh_of.len() as u32 - 1
            })
        };
        let mut edges = Vec::new();
        for (e, path) in &paths {
            let first = vertex(path[0]);
            let last = if e.closed { first } else { vertex(path[path.len() - 1]) };
            edges.push(TrimEdge { vertices: [first, last], geom: e.geom.clone() });
        }
        // Every mesh segment of an edge, both ways.
        let mut seg: HashMap<(u32, u32), (u32, bool)> = HashMap::new();
        let mut seg_count = Vec::new();
        for (i, (e, path)) in paths.iter().enumerate() {
            let n = if e.closed { path.len() } else { path.len() - 1 };
            for k in 0..n {
                let (p, q) = (path[k], path[(k + 1) % path.len()]);
                seg.insert((p, q), (i as u32, true));
                seg.insert((q, p), (i as u32, false));
            }
            seg_count.push(n);
        }
        let tris = body.mesh.indices.as_chunks::<3>().0;
        let mut tri_of: HashMap<(u32, u32), usize> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            for k in 0..3 {
                tri_of.insert((t[k], t[(k + 1) % 3]), i);
            }
        }
        // Connected components of triangles of one face.
        let mut comp = vec![u32::MAX; tris.len()];
        let mut ncomp = 0;
        for s in 0..tris.len() {
            if comp[s] != u32::MAX {
                continue;
            }
            comp[s] = ncomp;
            let mut stack = vec![s];
            while let Some(t) = stack.pop() {
                for k in 0..3 {
                    let Some(&u) = tri_of.get(&(tris[t][(k + 1) % 3], tris[t][k])) else { continue };
                    if comp[u] == u32::MAX && body.tri_face[u] == body.tri_face[t] {
                        comp[u] = ncomp;
                        stack.push(u);
                    }
                }
            }
            ncomp += 1;
        }
        // Faces in order of first appearance.
        let mut face_of: HashMap<(u32, u32), usize> = HashMap::new();
        let mut faces: Vec<TrimFace> = Vec::new();
        for (t, &c) in comp.iter().enumerate() {
            face_of.entry((body.tri_face[t], c)).or_insert_with(|| {
                let f = &body.faces[body.tri_face[t] as usize];
                faces.push(TrimFace { surface: f.surface.clone(), outward: f.outward, loops: Vec::new(), outer: None });
                faces.len() - 1
            });
        }
        // Boundary loops.
        let mut visited: HashSet<(u32, u32)> = HashSet::new();
        let mut loop_points: Vec<Vec<Vec<DVec3>>> = vec![Vec::new(); faces.len()];
        for t0 in 0..tris.len() {
            for k in 0..3 {
                let (a0, b0) = (tris[t0][k], tris[t0][(k + 1) % 3]);
                let twin = *tri_of.get(&(b0, a0)).ok_or(BAD_LOOP)?;
                if body.tri_face[twin] == body.tri_face[t0] || visited.contains(&(a0, b0)) {
                    continue;
                }
                let mut halves = Vec::new();
                let (mut t, mut a, mut b) = (t0, a0, b0);
                loop {
                    if halves.len() > body.mesh.indices.len() {
                        return Err(BAD_LOOP);
                    }
                    visited.insert((a, b));
                    halves.push((a, b));
                    (t, a, b) = next_boundary(body, &tri_of, t, a, b)?;
                    if (a, b) == (a0, b0) {
                        break;
                    }
                }
                let f = face_of[&(body.tri_face[t0], comp[t0])];
                let start = halves.iter().position(|h| vmap.contains_key(&h.0)).ok_or(BAD_LOOP)?;
                halves.rotate_left(start);
                let mut uses = Vec::new();
                let mut i = 0;
                while i < halves.len() {
                    let mut j = i + 1;
                    while j < halves.len() && !vmap.contains_key(&halves[j].0) {
                        j += 1;
                    }
                    let (e, fwd) = *seg.get(&halves[i]).ok_or(BAD_LOOP)?;
                    if j - i != seg_count[e as usize] || !halves[i..j].iter().all(|h| seg.get(h) == Some(&(e, fwd))) {
                        return Err(BAD_LOOP);
                    }
                    uses.push((e, fwd));
                    i = j;
                }
                loop_points[f].push(halves.iter().map(|h| body.mesh.positions[h.0 as usize]).collect());
                faces[f].loops.push(uses);
            }
        }
        // The outer loop of a planar face is the one with positive area seen against its outward normal.
        for (face, loops) in faces.iter_mut().zip(&loop_points) {
            let Surface::Plane { normal, .. } = face.surface else { continue };
            let n = if face.outward { normal } else { -normal };
            let area = |pts: &Vec<DVec3>| 0.5 * (0..pts.len()).map(|i| pts[i].cross(pts[(i + 1) % pts.len()]).dot(n)).sum::<f64>();
            let mut positive = (0..loops.len()).filter(|&i| area(&loops[i]) > 0.0);
            match (positive.next(), positive.next()) {
                (Some(i), None) => face.outer = Some(i),
                _ => return Err(BAD_LOOP),
            }
        }
        // A face without a boundary (a whole sphere) cannot be written as a bounded STEP face: it stays faceted.
        if faces.iter().any(|f| f.loops.is_empty()) {
            return Err(BAD_LOOP);
        }
        // Snap every vertex onto all the surfaces of the edges that meet there.
        let mut surfaces: Vec<Vec<u32>> = vec![Vec::new(); mesh_of.len()];
        for ((e, _), te) in paths.iter().zip(&edges) {
            for v in te.vertices {
                for f in e.faces {
                    if !surfaces[v as usize].contains(&f) {
                        surfaces[v as usize].push(f);
                    }
                }
            }
        }
        let mut vertices = Vec::new();
        for (v, &m) in mesh_of.iter().enumerate() {
            let surf: Vec<&Surface> = surfaces[v].iter().map(|&f| &body.faces[f as usize].surface).collect();
            let mut p = body.mesh.positions[m as usize];
            for _ in 0..SNAP_ROUNDS {
                for s in &surf {
                    p = s.project(p);
                }
            }
            if surf.iter().any(|s| s.distance(p) > SNAP_TOL) {
                return Err(OFF_SURFACE);
            }
            vertices.push(p);
        }
        Ok(TrimmedBody { vertices, edges, faces })
    }
}

#[cfg(test)]
#[path = "trimmed_tests.rs"]
mod tests;
