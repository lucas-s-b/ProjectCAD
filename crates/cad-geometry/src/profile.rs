use crate::shape::{GeomError, Shape};
use crate::tol::Tolerance;
use glam::DVec2;
use std::f64::consts::PI;

/// End points closer than this are the same profile vertex.
const JOIN: f64 = 1e-6;

#[derive(Debug, Clone, PartialEq)]
pub enum Seg {
    Line { a: DVec2, b: DVec2 },
    /// `sweep` is signed: > 0 counter-clockwise, < 0 clockwise; |sweep| <= PI.
    Arc { center: DVec2, radius: f64, start: f64, sweep: f64 },
}

impl Seg {
    pub fn start(&self) -> DVec2 {
        self.point(0.0)
    }

    pub fn end(&self) -> DVec2 {
        self.point(1.0)
    }

    pub fn reversed(&self) -> Seg {
        match self {
            Seg::Line { a, b } => Seg::Line { a: *b, b: *a },
            Seg::Arc { center, radius, start, sweep } => {
                Seg::Arc { center: *center, radius: *radius, start: start + sweep, sweep: -sweep }
            }
        }
    }

    /// `t` in 0..=1.
    pub fn point(&self, t: f64) -> DVec2 {
        match self {
            Seg::Line { a, b } => {
                if t == 0.0 {
                    *a
                } else if t == 1.0 {
                    *b
                } else {
                    a.lerp(*b, t)
                }
            }
            Seg::Arc { center, radius, start, sweep } => *center + DVec2::from_angle(start + sweep * t) * *radius,
        }
    }
}

/// Closed chain: each end = next start, last end = first start.
pub type Loop = Vec<Seg>;

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// Counter-clockwise.
    pub outer: Loop,
    /// Clockwise.
    pub holes: Vec<Loop>,
}

/// Signed area and first moment (integral of p over the region) of a loop, arcs included.
fn loop_moment(l: &Loop) -> (f64, DVec2) {
    let (mut area, mut mom) = (0.0, DVec2::ZERO);
    for s in l {
        let (p, q) = (s.start(), s.end());
        let c = p.perp_dot(q);
        area += c / 2.0;
        mom += (p + q) * c / 6.0;
        if let Seg::Arc { center, radius, start, sweep } = s {
            // Circular segment between chord and arc, added when the arc bulges outward (sweep > 0).
            let (k, w) = (sweep.signum(), sweep.abs());
            let seg = radius * radius / 2.0 * (w - w.sin());
            let q = 2.0 / 3.0 * radius.powi(3) * (w / 2.0).sin().powi(3);
            area += k * seg;
            mom += k * (seg * *center + q * DVec2::from_angle(start + sweep / 2.0));
        }
    }
    (area, mom)
}

/// Signed area, positive for counter-clockwise.
pub fn loop_area(l: &Loop) -> f64 {
    loop_moment(l).0
}

/// Even-odd test with a ray towards +x, on the true curves.
pub fn point_in_loop(l: &Loop, p: DVec2) -> bool {
    let mut inside = false;
    for s in l {
        match s {
            Seg::Line { a, b } => {
                if (a.y > p.y) != (b.y > p.y) && a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) > p.x {
                    inside = !inside;
                }
            }
            Seg::Arc { center, radius, start, sweep } => {
                let (mut lo, mut hi) = (*start, start + sweep);
                if lo > hi {
                    std::mem::swap(&mut lo, &mut hi);
                }
                // Split at the top and bottom of the circle so each piece is monotonic in y.
                let mut cuts = vec![lo];
                let mut k = ((lo - PI / 2.0) / PI).floor() + 1.0;
                while PI / 2.0 + k * PI < hi {
                    let c = PI / 2.0 + k * PI;
                    if c > lo {
                        cuts.push(c);
                    }
                    k += 1.0;
                }
                cuts.push(hi);
                let dy = p.y - center.y;
                for w in cuts.windows(2) {
                    let y = |t: f64| center.y + radius * t.sin();
                    if (y(w[0]) > p.y) != (y(w[1]) > p.y) {
                        let side = ((w[0] + w[1]) / 2.0).cos().signum();
                        if center.x + side * (radius * radius - dy * dy).max(0.0).sqrt() > p.x {
                            inside = !inside;
                        }
                    }
                }
            }
        }
    }
    inside
}

fn reverse_loop(l: &Loop) -> Loop {
    l.iter().rev().map(Seg::reversed).collect()
}

fn shape_segs(shape: &Shape, out: &mut Vec<Seg>) -> Result<(), GeomError> {
    shape.validate(&Tolerance::default())?;
    match shape {
        Shape::Line { a, b } => out.push(Seg::Line { a: *a, b: *b }),
        Shape::Rect { min, max } => {
            let c = [*min, DVec2::new(max.x, min.y), *max, DVec2::new(min.x, max.y)];
            out.extend((0..4).map(|i| Seg::Line { a: c[i], b: c[(i + 1) % 4] }));
        }
        Shape::Circle { center, radius } => {
            out.extend([0.0, PI].map(|start| Seg::Arc { center: *center, radius: *radius, start, sweep: PI }));
        }
        Shape::Arc { center, radius, start, sweep } => {
            let n = (sweep / PI - 1e-9).ceil().max(1.0);
            let part = sweep / n;
            out.extend((0..n as usize).map(|k| Seg::Arc {
                center: *center,
                radius: *radius,
                start: start + part * k as f64,
                sweep: part,
            }));
        }
        Shape::Polyline { points, closed } => {
            let n = points.len();
            let count = if *closed { n } else { n - 1 };
            out.extend((0..count).map(|i| Seg::Line { a: points[i], b: points[(i + 1) % n] }));
        }
        _ => return Err(GeomError::Unsupported("this shape cannot be part of a profile yet")),
    }
    Ok(())
}

/// Arc ends cannot move, so a line end adopts the neighbour's point (arc to arc keeps its < 1e-6 gap).
fn snap_ends(l: &mut [Seg], closed: bool) {
    let last = if closed { l.len() } else { l.len().saturating_sub(1) };
    for k in 0..last {
        let j = (k + 1) % l.len();
        let p = l[k].end();
        if let Seg::Line { a, .. } = &mut l[j] {
            *a = p;
        } else {
            let q = l[j].start();
            if let Seg::Line { b, .. } = &mut l[k] {
                *b = q;
            }
        }
    }
}

/// Joins shapes (Line, Arc, open Polyline) end to end into one path. Returns the segments in order and whether the path is closed.
pub fn chain(shapes: &[&Shape]) -> Result<(Vec<Seg>, bool), GeomError> {
    let mut segs = Vec::new();
    for s in shapes {
        shape_segs(s, &mut segs)?;
    }
    let n = segs.len();
    if n == 0 {
        return Err(GeomError::Degenerate("path has no segments"));
    }
    let pt = |e: usize| if e.is_multiple_of(2) { segs[e / 2].start() } else { segs[e / 2].end() };
    let mut mate = vec![None; 2 * n];
    for (e, slot) in mate.iter_mut().enumerate() {
        let near: Vec<usize> = (0..2 * n).filter(|&f| f != e && pt(e).distance(pt(f)) <= JOIN).collect();
        if near.len() > 1 {
            return Err(GeomError::Degenerate("path branches: more than two ends meet at a point"));
        }
        *slot = near.first().copied();
    }
    let free = (0..2 * n).find(|&e| mate[e].is_none());
    let mut enter = free.unwrap_or(0);
    let (mut used, mut out) = (vec![false; n], Vec::new());
    while !used[enter / 2] {
        used[enter / 2] = true;
        let s = &segs[enter / 2];
        out.push(if enter.is_multiple_of(2) { s.clone() } else { s.reversed() });
        match mate[enter ^ 1] {
            Some(m) => enter = m,
            None => break,
        }
    }
    if out.len() != n {
        return Err(GeomError::Degenerate("path must be one connected chain"));
    }
    snap_ends(&mut out, free.is_none());
    Ok((out, free.is_none()))
}

/// Splits the segments into closed loops, reversing segments as needed and snapping ends together.
fn close_loops(segs: &[Seg]) -> Result<Vec<Loop>, GeomError> {
    let bad = || GeomError::Degenerate("profile is not a set of closed loops");
    let n = segs.len();
    if n == 0 {
        return Err(bad());
    }
    // End number e: segment e / 2, its start when e is even, its end when odd.
    let pt = |e: usize| if e.is_multiple_of(2) { segs[e / 2].start() } else { segs[e / 2].end() };
    let mut mate = vec![0; 2 * n];
    for (e, slot) in mate.iter_mut().enumerate() {
        let near: Vec<usize> = (0..2 * n).filter(|&f| f != e && pt(e).distance(pt(f)) <= JOIN).collect();
        if near.len() != 1 {
            return Err(bad());
        }
        *slot = near[0];
    }
    let mut used = vec![false; n];
    let mut loops = Vec::new();
    for first in 0..n {
        let mut l = Loop::new();
        let mut enter = 2 * first;
        while !used[enter / 2] {
            used[enter / 2] = true;
            let s = &segs[enter / 2];
            l.push(if enter.is_multiple_of(2) { s.clone() } else { s.reversed() });
            enter = mate[enter ^ 1];
        }
        if l.is_empty() {
            continue;
        }
        snap_ends(&mut l, true);
        loops.push(l);
    }
    Ok(loops)
}

impl Profile {
    pub fn from_shapes(shapes: &[&Shape]) -> Result<Profile, GeomError> {
        let mut segs = Vec::new();
        for s in shapes {
            shape_segs(s, &mut segs)?;
        }
        let mut loops = close_loops(&segs)?;
        let bad = || GeomError::Degenerate("profile must be one region, with optional holes");
        for l in &mut loops {
            let a = loop_area(l);
            if a.abs() < 1e-9 {
                return Err(bad());
            }
            if a < 0.0 {
                *l = reverse_loop(l);
            }
        }
        let probe: Vec<DVec2> = loops.iter().map(|l| l[0].point(0.5)).collect();
        let n = loops.len();
        let contains = |i: usize, j: usize| point_in_loop(&loops[i], probe[j]);
        let outer: Vec<usize> = (0..n).filter(|&i| (0..n).all(|j| j == i || contains(i, j))).collect();
        if outer.len() != 1 {
            return Err(bad());
        }
        let o = outer[0];
        // shortcut: crossing loops are not detected, add the check with the boolean engine
        if (0..n).any(|i| (0..n).any(|j| i != j && i != o && j != o && contains(i, j))) {
            return Err(bad());
        }
        let holes = (0..n).filter(|&i| i != o).map(|i| reverse_loop(&loops[i])).collect();
        Ok(Profile { outer: loops.swap_remove(o), holes })
    }

    /// Outer area minus hole areas.
    pub fn area(&self) -> f64 {
        self.moment().0
    }

    pub fn centroid(&self) -> DVec2 {
        let (a, m) = self.moment();
        m / a
    }

    fn moment(&self) -> (f64, DVec2) {
        std::iter::once(&self.outer)
            .chain(&self.holes)
            .map(loop_moment)
            .fold((0.0, DVec2::ZERO), |(a, m), (b, n)| (a + b, m + n))
    }
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
