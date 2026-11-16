use crate::brep::{Brep, CoEdge, TopoError};
use crate::geom::{Curve, Surface};
use crate::TOL;
use glam::{DQuat, DVec3};
use std::f64::consts::PI;

const ARC_TOL: f64 = 1e-6;
const SURFACE_TOL: f64 = 1e-6;

fn arc_ok(center: DVec3, normal: DVec3, sweep: f64, a: DVec3, b: DVec3) -> bool {
    let (ra, rb) = (a - center, b - center);
    ra.length() > TOL
        && (ra.length() - rb.length()).abs() <= ARC_TOL
        && (normal.length() - 1.0).abs() <= ARC_TOL
        && normal.dot(ra).abs() <= ARC_TOL
        && normal.dot(rb).abs() <= ARC_TOL
        && sweep > 0.0
        && sweep <= PI + 1e-9
        && (center + DQuat::from_axis_angle(normal, sweep) * ra).distance(b) <= ARC_TOL
}

impl Brep {
    pub fn validate(&self) -> Result<(), TopoError> {
        if self.faces.is_empty() {
            return Err(TopoError::Empty);
        }
        let curve_finite = |c: &Curve| match c {
            Curve::Line => true,
            Curve::Arc { center, normal, sweep } => center.is_finite() && normal.is_finite() && sweep.is_finite(),
        };
        if !self.vertices.iter().all(|v| v.is_finite())
            || !self.edges.iter().all(|e| curve_finite(&e.curve))
            || !self.faces.iter().all(|f| f.surface.is_finite())
        {
            return Err(TopoError::NonFinite);
        }
        let (nv, ne) = (self.vertices.len(), self.edges.len());
        let coedges = || self.faces.iter().flat_map(|f| f.loops.iter().flatten());
        if self.edges.iter().any(|e| e.v.iter().any(|&i| i as usize >= nv)) || coedges().any(|c| c.edge as usize >= ne) {
            return Err(TopoError::BadIndex);
        }
        for (i, e) in self.edges.iter().enumerate() {
            let (a, b) = (self.vertices[e.v[0] as usize], self.vertices[e.v[1] as usize]);
            if e.v[0] == e.v[1] || a.distance(b) <= TOL {
                return Err(TopoError::DegenerateEdge(i as u32));
            }
            if let Curve::Arc { center, normal, sweep } = &e.curve
                && !arc_ok(*center, *normal, *sweep, a, b)
            {
                return Err(TopoError::BadArc(i as u32));
            }
        }
        // Manifold before loop closure: a flipped coedge shows up as a doubled direction.
        let mut uses = vec![[0usize; 2]; ne];
        for c in coedges() {
            uses[c.edge as usize][c.reversed as usize] += 1;
        }
        if let Some(i) = uses.iter().position(|u| *u != [1, 1]) {
            return Err(TopoError::NotManifold(i as u32, uses[i][0] + uses[i][1]));
        }
        for (fi, f) in self.faces.iter().enumerate() {
            let fi = fi as u32;
            if f.surface == Surface::Faceted {
                return Err(TopoError::NoSurface(fi));
            }
            let closed = |l: &Vec<CoEdge>| {
                !l.is_empty() && (0..l.len()).all(|k| self.coedge_ends(l[k]).1 == self.coedge_ends(l[(k + 1) % l.len()]).0)
            };
            if f.loops.is_empty() || !f.loops.iter().all(closed) {
                return Err(TopoError::OpenLoop(fi));
            }
            for c in f.loops.iter().flatten() {
                let e = &self.edges[c.edge as usize];
                let (a, b) = (self.vertices[e.v[0] as usize], self.vertices[e.v[1] as usize]);
                for p in [a, b, e.curve.point(a, b, 0.5)] {
                    let d = f.surface.distance(p);
                    // NaN fails too.
                    if d.is_nan() || d > SURFACE_TOL {
                        return Err(TopoError::OffSurface(fi, d));
                    }
                }
            }
        }
        Ok(())
    }
}
