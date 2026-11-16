use crate::{BoolOp, ModelError, TriMesh, boolean, tessellate_with_faces};
use cad_topology::{Brep, Surface};
use glam::{DAffine3, DVec3};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct BodyFace {
    pub surface: Surface,
    /// The surface's natural normal points out of the solid.
    pub outward: bool,
}

const MERGE_AXIS: f64 = 1e-9;
const MERGE_DIST: f64 = 1e-7;

/// Both faces lie on the same surface with the same side outside.
fn same_surface(a: &BodyFace, b: &BodyFace) -> bool {
    use Surface::*;
    let near = |x: f64, y: f64| (x - y).abs() <= MERGE_DIST;
    let close = |p: DVec3, q: DVec3| p.distance(q) <= MERGE_DIST;
    let parallel = |u: DVec3, v: DVec3| u.cross(v).length() <= MERGE_AXIS;
    if a.outward != b.outward && !matches!((&a.surface, &b.surface), (Plane { .. }, Plane { .. })) {
        return false;
    }
    match (&a.surface, &b.surface) {
        (Plane { origin: o1, normal: n1 }, Plane { origin: o2, normal: n2 }) => {
            let (s1, s2) = (n1 * if a.outward { 1.0 } else { -1.0 }, n2 * if b.outward { 1.0 } else { -1.0 });
            s1.distance(s2) <= MERGE_AXIS && near(s1.dot(*o1), s2.dot(*o2))
        }
        (Cylinder { origin: o1, axis: x1, radius: r1 }, Cylinder { origin: o2, axis: x2, radius: r2 }) => {
            parallel(*x1, *x2) && (*o2 - *o1).cross(*x1).length() <= MERGE_DIST && near(*r1, *r2)
        }
        (Cone { apex: p1, axis: x1, half_angle: h1 }, Cone { apex: p2, axis: x2, half_angle: h2 }) => {
            close(*p1, *p2) && x1.distance(*x2) <= MERGE_AXIS && near(*h1, *h2)
        }
        (Sphere { center: c1, radius: r1 }, Sphere { center: c2, radius: r2 }) => close(*c1, *c2) && near(*r1, *r2),
        (Torus { center: c1, axis: x1, major: m1, minor: n1 }, Torus { center: c2, axis: x2, major: m2, minor: n2 }) => {
            close(*c1, *c2) && parallel(*x1, *x2) && near(*m1, *m2) && near(*n1, *n2)
        }
        // Two free-form faces are never the same surface.
        _ => false,
    }
}

/// A solid as a closed triangle mesh in which every triangle knows the analytic surface it lies on.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    pub mesh: TriMesh,
    /// Index into `faces` of every triangle.
    pub tri_face: Vec<u32>,
    pub faces: Vec<BodyFace>,
}

impl Body {
    pub fn from_brep(brep: &Brep, tol: f64) -> Result<Body, ModelError> {
        let (mesh, tri_face) = tessellate_with_faces(brep, tol)?;
        let faces = brep.faces.iter().map(|f| BodyFace { surface: f.surface.clone(), outward: f.outward }).collect();
        let mut body = Body { mesh, tri_face, faces };
        body.merge_faces();
        Ok(body)
    }

    /// Rigid motion or mirror: `m` must be an isometry (rotation, translation, reflection).
    pub fn transformed(&self, m: &DAffine3) -> Result<Body, ModelError> {
        let c = [m.matrix3.x_axis, m.matrix3.y_axis, m.matrix3.z_axis];
        let unit = c.iter().all(|v| (v.length() - 1.0).abs() <= 1e-9);
        if !unit || (0..3).any(|i| c[i].dot(c[(i + 1) % 3]).abs() > 1e-9) {
            return Err(ModelError::Invalid("transform must be a rigid motion or a mirror"));
        }
        let mut mesh = TriMesh {
            positions: self.mesh.positions.iter().map(|p| m.transform_point3(*p)).collect(),
            indices: self.mesh.indices.clone(),
            normals: Vec::new(),
        };
        if m.matrix3.determinant() < 0.0 {
            for k in 0..mesh.triangle_count() {
                mesh.indices.swap(3 * k + 1, 3 * k + 2);
            }
        }
        let faces = self.faces.iter().map(|f| BodyFace { surface: f.surface.transformed(m), outward: f.outward }).collect();
        let mut body = Body { mesh, tri_face: self.tri_face.clone(), faces };
        body.recompute_normals();
        Ok(body)
    }

    /// Corner normals pointing out of the solid: analytic faces use their surface; a `Faceted` face
    /// averages the area-weighted triangle normals around each vertex within that face (smooth inside, sharp between faces).
    pub(crate) fn recompute_normals(&mut self) {
        let tris = self.mesh.indices.as_chunks::<3>().0;
        let corner = |t: &[u32; 3]| t.map(|i| self.mesh.positions[i as usize]);
        let mut around: HashMap<(u32, u32), DVec3> = HashMap::new();
        for (t, &f) in tris.iter().zip(&self.tri_face) {
            if self.faces[f as usize].surface == Surface::Faceted {
                let [a, b, c] = corner(t);
                // The cross product's length is twice the triangle's area.
                let n = (b - a).cross(c - a);
                for &i in t {
                    *around.entry((f, i)).or_default() += n;
                }
            }
        }
        let mut normals = Vec::with_capacity(self.mesh.indices.len());
        for (t, &f) in tris.iter().zip(&self.tri_face) {
            let face = &self.faces[f as usize];
            for &i in t {
                normals.push(if face.surface == Surface::Faceted {
                    let [a, b, c] = corner(t);
                    around[&(f, i)].try_normalize().unwrap_or_else(|| (b - a).cross(c - a).normalize_or_zero())
                } else {
                    let n = face.surface.normal_at(self.mesh.positions[i as usize]);
                    if face.outward { n } else { -n }
                });
            }
        }
        self.mesh.normals = normals;
    }

    /// Faces lying on the same oriented surface become one face; unused faces are dropped.
    pub fn merge_faces(&mut self) {
        let mut keep: Vec<usize> = Vec::new();
        let mut target = vec![0u32; self.faces.len()];
        for (i, f) in self.faces.iter().enumerate() {
            let at = keep.iter().position(|&k| same_surface(&self.faces[k], f));
            target[i] = at.unwrap_or_else(|| {
                keep.push(i);
                keep.len() - 1
            }) as u32;
        }
        for t in &mut self.tri_face {
            *t = target[*t as usize];
        }
        self.faces = keep.into_iter().map(|k| self.faces[k].clone()).collect();
    }

    pub fn volume(&self) -> f64 {
        self.mesh.volume()
    }

    pub fn area(&self) -> f64 {
        self.mesh.area()
    }

    pub fn centroid(&self) -> DVec3 {
        self.mesh.centroid()
    }

    pub fn bbox(&self) -> (DVec3, DVec3) {
        let Some(&first) = self.mesh.positions.first() else { return (DVec3::ZERO, DVec3::ZERO) };
        self.mesh.positions.iter().fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)))
    }

    /// Mesh edges whose two triangles lie on different faces (the visible edges of the solid).
    pub fn feature_edges(&self) -> Vec<[DVec3; 2]> {
        let tris = self.mesh.indices.as_chunks::<3>().0;
        let mut face_of: HashMap<(u32, u32), u32> = HashMap::new();
        for (t, &f) in tris.iter().zip(&self.tri_face) {
            for k in 0..3 {
                face_of.insert((t[k], t[(k + 1) % 3]), f);
            }
        }
        let mut out = Vec::new();
        for (t, &f) in tris.iter().zip(&self.tri_face) {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                // Each undirected edge once: from the triangle that runs it upward.
                if a < b && face_of.get(&(b, a)).is_some_and(|&g| g != f) {
                    out.push([self.mesh.positions[a as usize], self.mesh.positions[b as usize]]);
                }
            }
        }
        out
    }
}

/// Union of all bodies, `None` for an empty slice.
pub fn union_all(bodies: &[Body]) -> Result<Option<Body>, ModelError> {
    let Some((first, rest)) = bodies.split_first() else { return Ok(None) };
    rest.iter().try_fold(first.clone(), |acc, b| boolean(&acc, b, BoolOp::Union)).map(Some)
}

#[cfg(test)]
#[path = "body_tests.rs"]
mod tests;
