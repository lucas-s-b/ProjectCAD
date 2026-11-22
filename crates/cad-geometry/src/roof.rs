//! Roofs: the data of `Shape::Roof` and its plan (eaves and roof planes). Roof solids are in `cad-arch`.
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// How a roof slopes. Every sloped plane rises from an eave line of the outline at the roof's pitch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoofKind {
    /// No slope.
    Flat,
    /// One plane, rising from the outline's first edge.
    Shed,
    /// Two planes, from the longest edge and the edges parallel to it, meeting at a ridge; the other edges are gable
    /// ends. Needs a convex outline.
    Gable,
    /// One plane from every edge, meeting at hips and a ridge. Needs a convex outline.
    Hip,
}

/// A roof on a level: a closed outline in the level's sketch (the eave line, either orientation). Its underside meets
/// the eave line on top of the level (elevation plus storey height). Millimetres; `pitch` in degrees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Roof {
    pub outline: Vec<DVec2>,
    pub kind: RoofKind,
    /// Slope of every sloped plane, degrees; a flat roof ignores it.
    pub pitch: f64,
    /// At right angles to the roof planes.
    pub thickness: f64,
    /// How far the roof reaches past the outline, horizontally.
    pub overhang: f64,
}

/// One plane of a roof in plan.
#[derive(Debug, Clone, PartialEq)]
pub struct RoofPiece {
    /// Counter-clockwise.
    pub outline: Vec<DVec2>,
    /// The eave line the plane rises from: a point on it and its unit direction, with the piece on its left. None for
    /// a flat roof.
    pub eave: Option<(DVec2, DVec2)>,
}

impl Roof {
    /// Signed area of the outline, positive counter-clockwise.
    pub fn area(&self) -> f64 {
        shoelace(&self.outline)
    }

    /// No corner turns against the outline's direction (collinear points are allowed).
    pub fn is_convex(&self) -> bool {
        let (p, n, s) = (&self.outline, self.outline.len(), self.area().signum());
        (0..n).all(|k| {
            let (u, w) = (p[(k + 1) % n] - p[k], p[(k + 2) % n] - p[(k + 1) % n]);
            u.perp_dot(w) * s >= -1e-9 * u.length() * w.length()
        })
    }

    /// The outline counter-clockwise; a clockwise one is reversed so that its first edge stays the first edge (the
    /// low edge of a shed roof survives a mirror).
    fn ccw(&self) -> Vec<DVec2> {
        let mut p = self.outline.clone();
        if p.len() >= 3 && self.area() < 0.0 {
            p.reverse();
            p.rotate_right(2);
        }
        p
    }

    /// The outer edge of the roof: the counter-clockwise outline moved out by `overhang`, corners mitred.
    // shortcut: a mitre offset, which can cross itself on concave outlines with a large overhang; offset with a polygon
    // library when concave flat roofs with big overhangs matter
    pub fn eaves(&self) -> Vec<DVec2> {
        let p = self.ccw();
        let n = p.len();
        let normal = |k: usize| (p[(k + 1) % n] - p[k]).normalize_or_zero().perp();
        (0..n)
            .map(|k| {
                let (n1, n2) = (normal((k + n - 1) % n), normal(k));
                let c = 1.0 + n1.dot(n2);
                // A spike back along itself has no mitre: move along one normal.
                if c <= 1e-9 { p[k] - n2 * self.overhang } else { p[k] - (n1 + n2) * (self.overhang / c) }
            })
            .collect()
    }

    /// The roof planes in plan: each the part of the eaves outline where its eave line is the nearest sloped one,
    /// which on a convex outline is the straight skeleton. Collinear edges share one plane; a flat roof is one piece
    /// without an eave. Empty for fewer than three points.
    pub fn pieces(&self) -> Vec<RoofPiece> {
        let p = self.ccw();
        let n = p.len();
        if n < 3 {
            return Vec::new();
        }
        let eaves = self.eaves();
        let edge = |k: usize| (p[k], (p[(k + 1) % n] - p[k]).normalize_or_zero());
        let len = |k: usize| p[k].distance(p[(k + 1) % n]);
        let sloped: Vec<usize> = match self.kind {
            RoofKind::Flat => return vec![RoofPiece { outline: eaves, eave: None }],
            RoofKind::Shed => vec![0],
            RoofKind::Hip => (0..n).collect(),
            RoofKind::Gable => {
                // The first of the longest edges, so a square takes edge 0.
                let i = (1..n).fold(0, |i, k| if len(k) > len(i) + 1e-9 { k } else { i });
                (0..n).filter(|&k| edge(k).1.perp_dot(edge(i).1).abs() <= 1e-9).collect()
            }
        };
        let sloped: Vec<usize> = sloped.into_iter().filter(|&k| len(k) > 1e-9).collect();
        // Horizontal distance of `q` inside eave line k (the roof's rise over it, divided by the slope).
        let dist = |k: usize, q: DVec2| {
            let (a, d) = edge(k);
            d.perp_dot(q - a)
        };
        let same = |k: usize, j: usize| edge(k).1.dot(edge(j).1) > 1.0 - 1e-9 && dist(j, p[k]).abs() <= 1e-6;
        let mut out = Vec::new();
        for (m, &k) in sloped.iter().enumerate() {
            if sloped[..m].iter().any(|&j| same(k, j)) {
                continue;
            }
            let mut piece = eaves.clone();
            for &j in sloped.iter().filter(|&&j| !same(k, j)) {
                piece = clip(&piece, |q| dist(k, q) - dist(j, q));
            }
            if shoelace(&piece) > 1e-6 {
                out.push(RoofPiece { outline: piece, eave: Some(edge(k)) });
            }
        }
        out
    }
}

/// Signed area, positive counter-clockwise.
fn shoelace(p: &[DVec2]) -> f64 {
    (0..p.len()).map(|k| p[k].perp_dot(p[(k + 1) % p.len()])).sum::<f64>() / 2.0
}

/// The part of convex polygon `poly` where the affine function `f` is at most zero (Sutherland-Hodgman).
fn clip(poly: &[DVec2], f: impl Fn(DVec2) -> f64) -> Vec<DVec2> {
    let mut out = Vec::new();
    for k in 0..poly.len() {
        let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
        let (fp, fq) = (f(p), f(q));
        if fp <= 0.0 {
            out.push(p);
        }
        if fp * fq < 0.0 {
            out.push(p + (q - p) * (fp / (fp - fq)));
        }
    }
    out
}

#[cfg(test)]
#[path = "roof_tests.rs"]
mod tests;
