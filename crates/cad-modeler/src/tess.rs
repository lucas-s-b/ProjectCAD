use crate::ModelError;
use cad_topology::{Brep, CoEdge, Curve, Face, Surface};
use glam::DVec3;
use std::collections::HashMap;

/// `normals` has one unit entry per index (triangle corner), pointing out of the solid.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TriMesh {
    pub positions: Vec<DVec3>,
    pub indices: Vec<u32>,
    pub normals: Vec<DVec3>,
}

impl TriMesh {
    fn corners(&self) -> impl Iterator<Item = [DVec3; 3]> + '_ {
        self.indices.as_chunks::<3>().0.iter().map(|t| [0, 1, 2].map(|k| self.positions[t[k] as usize]))
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Every directed edge is used once and its reverse is used once.
    pub fn is_closed(&self) -> bool {
        let mut used: HashMap<(u32, u32), u32> = HashMap::new();
        for t in self.indices.as_chunks::<3>().0 {
            for k in 0..3 {
                *used.entry((t[k], t[(k + 1) % 3])).or_default() += 1;
            }
        }
        !used.is_empty() && used.iter().all(|(&(a, b), &n)| n == 1 && used.get(&(b, a)) == Some(&1))
    }

    /// Signed sum of tetrahedra to the origin, positive for outward-oriented meshes.
    pub fn volume(&self) -> f64 {
        self.corners().map(|[a, b, c]| a.dot(b.cross(c))).sum::<f64>() / 6.0
    }

    /// Centroid of the enclosed volume.
    pub fn centroid(&self) -> DVec3 {
        let moment: DVec3 = self.corners().map(|[a, b, c]| (a + b + c) * a.dot(b.cross(c))).sum();
        moment / (24.0 * self.volume())
    }

    pub fn area(&self) -> f64 {
        self.corners().map(|[a, b, c]| (b - a).cross(c - a).length() / 2.0).sum()
    }

    fn push(&mut self, tri: [u32; 3], normals: [DVec3; 3]) {
        self.indices.extend(tri);
        self.normals.extend(normals);
    }
}

const CURVED: ModelError = ModelError::Invalid("cannot tessellate this curved face yet");

/// Samples of the coedge's edge in loop direction, without the last point.
fn ring(samples: &[Vec<u32>], loop_: &[CoEdge]) -> Vec<u32> {
    let mut out = Vec::new();
    for c in loop_ {
        let s = &samples[c.edge as usize];
        if c.reversed {
            out.extend(s.iter().rev().take(s.len() - 1));
        } else {
            out.extend(&s[..s.len() - 1]);
        }
    }
    out
}

pub fn tessellate(brep: &Brep, tol: f64) -> Result<TriMesh, ModelError> {
    tessellate_with_faces(brep, tol).map(|(mesh, _)| mesh)
}

/// Like `tessellate`; the second value is the B-Rep face index of every triangle.
pub fn tessellate_with_faces(brep: &Brep, tol: f64) -> Result<(TriMesh, Vec<u32>), ModelError> {
    if !tol.is_finite() || tol < 1e-6 {
        return Err(ModelError::Invalid("tessellation tolerance too small"));
    }
    brep.validate()?;
    let mut mesh = TriMesh { positions: brep.vertices.clone(), ..Default::default() };
    let r_max = brep.edges.iter().fold(0.0_f64, |m, e| match e.curve {
        Curve::Arc { center, .. } => m.max(brep.vertices[e.v[0] as usize].distance(center)),
        Curve::Line => m,
    });
    let step = 2.0 * (1.0 - tol / r_max).clamp(-1.0, 1.0).acos();
    // Sample every edge once; interior points are shared by both faces.
    let mut samples = Vec::with_capacity(brep.edges.len());
    for e in &brep.edges {
        let mut s = vec![e.v[0]];
        if let Curve::Arc { sweep, .. } = e.curve {
            let (a, b) = (brep.vertices[e.v[0] as usize], brep.vertices[e.v[1] as usize]);
            let n = (sweep / step).ceil().clamp(2.0, 1e6) as usize;
            for k in 1..n {
                s.push(mesh.positions.len() as u32);
                mesh.positions.push(e.curve.point(a, b, k as f64 / n as f64));
            }
        }
        s.push(e.v[1]);
        samples.push(s);
    }
    let mut tri_face = Vec::new();
    for (i, f) in brep.faces.iter().enumerate() {
        match &f.surface {
            Surface::Plane { normal, .. } => plane_face(&mut mesh, f, *normal, &samples)?,
            surface => grid_face(&mut mesh, f, surface, &samples)?,
        }
        tri_face.resize(mesh.triangle_count(), i as u32);
    }
    Ok((mesh, tri_face))
}

fn plane_face(mesh: &mut TriMesh, f: &Face, normal: DVec3, samples: &[Vec<u32>]) -> Result<(), ModelError> {
    let n = if f.outward { normal } else { -normal };
    // (u, w, n) is right-handed, so counter-clockwise in (u, w) is counter-clockwise seen from outside.
    let u = n.any_orthonormal_vector();
    let w = n.cross(u);
    let (mut flat, mut data, mut holes) = (Vec::new(), Vec::new(), Vec::new());
    for (i, l) in f.loops.iter().enumerate() {
        if i > 0 {
            holes.push(flat.len());
        }
        for idx in ring(samples, l) {
            let p = mesh.positions[idx as usize];
            flat.push(idx);
            data.extend([p.dot(u), p.dot(w)]);
        }
    }
    let tri = earcutr::earcut(&data, &holes, 2).map_err(|_| ModelError::Invalid("cannot triangulate a planar face"))?;
    // earcut drops collinear boundary points, which would leave T-junctions in the shared edges.
    if tri.len() / 3 + 2 != flat.len() + 2 * holes.len() {
        return Err(ModelError::Invalid("cannot triangulate a planar face with collinear boundary points"));
    }
    let at = |i: usize| (data[2 * i], data[2 * i + 1]);
    let signed: f64 = tri
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let (a, b, c) = (at(t[0]), at(t[1]), at(t[2]));
            (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
        })
        .sum();
    for t in tri.as_chunks::<3>().0 {
        let (b, c) = if signed < 0.0 { (t[2], t[1]) } else { (t[1], t[2]) };
        mesh.push([flat[t[0]], flat[b], flat[c]], [n; 3]);
    }
    Ok(())
}

/// One 4-coedge loop filled with a Coons patch of the boundary samples, interior points projected onto the surface.
fn grid_face(mesh: &mut TriMesh, f: &Face, surface: &Surface, samples: &[Vec<u32>]) -> Result<(), ModelError> {
    let [l] = f.loops.as_slice() else { return Err(CURVED) };
    if l.len() != 4 {
        return Err(CURVED);
    }
    let side = |c: &CoEdge| {
        let mut s = samples[c.edge as usize].clone();
        if c.reversed {
            s.reverse();
        }
        s
    };
    let [bottom, right, top, left] = [side(&l[0]), side(&l[1]), side(&l[2]), side(&l[3])];
    if bottom.len() != top.len() || right.len() != left.len() {
        return Err(CURVED);
    }
    let (nu, nv) = (bottom.len() - 1, right.len() - 1);
    let p = |mesh: &TriMesh, i: u32| mesh.positions[i as usize];
    let (p00, p10, p01, p11) = (p(mesh, bottom[0]), p(mesh, bottom[nu]), p(mesh, top[nu]), p(mesh, top[0]));
    let mut grid = vec![vec![0u32; nu + 1]; nv + 1];
    for (j, row) in grid.iter_mut().enumerate() {
        for (i, cell) in row.iter_mut().enumerate() {
            *cell = if j == 0 {
                bottom[i]
            } else if j == nv {
                top[nu - i]
            } else if i == 0 {
                left[nv - j]
            } else if i == nu {
                right[j]
            } else {
                let (u, v) = (i as f64 / nu as f64, j as f64 / nv as f64);
                let (b, t, lf, r) = (p(mesh, bottom[i]), p(mesh, top[nu - i]), p(mesh, left[nv - j]), p(mesh, right[j]));
                let corners = p00 * ((1.0 - u) * (1.0 - v)) + p10 * (u * (1.0 - v)) + p01 * ((1.0 - u) * v) + p11 * (u * v);
                let coons = b * (1.0 - v) + t * v + lf * (1.0 - u) + r * u - corners;
                mesh.positions.push(surface.project(coons));
                mesh.positions.len() as u32 - 1
            };
        }
    }
    let sign = if f.outward { 1.0 } else { -1.0 };
    for j in 0..nv {
        for i in 0..nu {
            let (a, b, c, d) = (grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]);
            for t in [[a, b, c], [a, c, d]] {
                mesh.push(t, t.map(|k| surface.normal_at(p(mesh, k)) * sign));
            }
        }
    }
    Ok(())
}
