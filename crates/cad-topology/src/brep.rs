use crate::geom::{Curve, Surface};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub v: [u32; 2],
    pub curve: Curve,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CoEdge {
    pub edge: u32,
    pub reversed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Face {
    pub surface: Surface,
    /// True: the solid's outside is on the side of the natural normal.
    pub outward: bool,
    /// `loops[0]` is the outer boundary; seen from outside the solid it runs counter-clockwise, inner loops clockwise.
    pub loops: Vec<Vec<CoEdge>>,
}

/// One closed shell.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Brep {
    pub vertices: Vec<DVec3>,
    pub edges: Vec<Edge>,
    pub faces: Vec<Face>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TopoError {
    #[error("non-finite coordinate")]
    NonFinite,
    #[error("index out of range")]
    BadIndex,
    #[error("edge {0} is degenerate")]
    DegenerateEdge(u32),
    #[error("face {0} has an open loop")]
    OpenLoop(u32),
    #[error("edge {0} is used {1} time(s) instead of twice in opposite directions")]
    NotManifold(u32, usize),
    #[error("face {0}: vertex is {1:.3e} away from its surface")]
    OffSurface(u32, f64),
    #[error("edge {0}: arc data does not match its end points")]
    BadArc(u32),
    #[error("face {0} has no analytic surface")]
    NoSurface(u32),
    #[error("empty shell")]
    Empty,
}

impl Brep {
    /// Start and end vertex of a coedge in loop direction.
    pub fn coedge_ends(&self, c: CoEdge) -> (u32, u32) {
        let [a, b] = self.edges[c.edge as usize].v;
        if c.reversed { (b, a) } else { (a, b) }
    }

    pub fn faces_of_edge(&self, edge: u32) -> Vec<u32> {
        let uses = |f: &Face| f.loops.iter().flatten().any(|c| c.edge == edge);
        (0..self.faces.len() as u32).filter(|&i| uses(&self.faces[i as usize])).collect()
    }

    pub fn edges_of_vertex(&self, v: u32) -> Vec<u32> {
        (0..self.edges.len() as u32).filter(|&i| self.edges[i as usize].v.contains(&v)).collect()
    }

    pub fn adjacent_faces(&self, face: u32) -> Vec<u32> {
        let mut out: Vec<u32> = self.faces[face as usize]
            .loops
            .iter()
            .flatten()
            .flat_map(|c| self.faces_of_edge(c.edge))
            .filter(|&f| f != face)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Of the vertices and the arc extremes.
    pub fn bbox(&self) -> (DVec3, DVec3) {
        let mut pts = self.vertices.clone();
        for e in &self.edges {
            let Curve::Arc { center, normal, sweep } = &e.curve else { continue };
            let (a, b) = (self.vertices[e.v[0] as usize], self.vertices[e.v[1] as usize]);
            let (u, w) = (a - *center, normal.cross(a - *center));
            for axis in 0..3 {
                // Coordinate along `axis` is c + u cos(t) + w sin(t): maximal at atan2(w, u), minimal half a turn later.
                let top = w[axis].atan2(u[axis]);
                for t in [top, top + std::f64::consts::PI] {
                    let t = t.rem_euclid(TAU);
                    if t <= *sweep {
                        pts.push(e.curve.point(a, b, t / sweep));
                    }
                }
            }
        }
        let lo = pts.iter().fold(DVec3::INFINITY, |m, p| m.min(*p));
        let hi = pts.iter().fold(DVec3::NEG_INFINITY, |m, p| m.max(*p));
        if pts.is_empty() { (DVec3::ZERO, DVec3::ZERO) } else { (lo, hi) }
    }

    /// V - E + F - (number of inner loops).
    pub fn euler(&self) -> i64 {
        let inner: usize = self.faces.iter().map(|f| f.loops.len().saturating_sub(1)).sum();
        self.vertices.len() as i64 - self.edges.len() as i64 + self.faces.len() as i64 - inner as i64
    }
}
