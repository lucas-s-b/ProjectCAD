use crate::shape::Shape;
use glam::DVec2;
use std::f64::consts::{FRAC_PI_2, TAU};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SnapKind {
    Endpoint,
    Intersection,
    Midpoint,
    Center,
    Perpendicular,
    Tangent,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snap {
    pub point: DVec2,
    pub kind: SnapKind,
}

pub fn snap_points(shape: &Shape) -> Vec<Snap> {
    let s = |point, kind| Snap { point, kind };
    let mut out = Vec::new();
    match shape {
        Shape::Line { a, b } => {
            out.extend([s(*a, SnapKind::Endpoint), s(*b, SnapKind::Endpoint), s((*a + *b) / 2.0, SnapKind::Midpoint)]);
        }
        Shape::Wall(w) => {
            out.extend([s(w.a, SnapKind::Endpoint), s(w.b, SnapKind::Endpoint), s((w.a + w.b) / 2.0, SnapKind::Midpoint)]);
        }
        Shape::Rect { min, max } => {
            let c = [*min, DVec2::new(max.x, min.y), *max, DVec2::new(min.x, max.y)];
            for i in 0..4 {
                out.push(s(c[i], SnapKind::Endpoint));
                out.push(s((c[i] + c[(i + 1) % 4]) / 2.0, SnapKind::Midpoint));
            }
            out.push(s((*min + *max) / 2.0, SnapKind::Center));
        }
        Shape::Circle { center, radius } => {
            out.push(s(*center, SnapKind::Center));
            for k in 0..4 {
                let a = FRAC_PI_2 * k as f64;
                out.push(s(*center + DVec2::new(a.cos(), a.sin()) * *radius, SnapKind::Endpoint));
            }
        }
        Shape::Arc { center, radius, start, sweep } => {
            let at = |a: f64| *center + DVec2::new(a.cos(), a.sin()) * *radius;
            out.extend([
                s(at(*start), SnapKind::Endpoint),
                s(at(start + sweep), SnapKind::Endpoint),
                s(at(start + sweep / 2.0), SnapKind::Midpoint),
                s(*center, SnapKind::Center),
            ]);
        }
        Shape::Polyline { points, closed } => {
            let n = points.len();
            out.extend(points.iter().map(|p| s(*p, SnapKind::Endpoint)));
            let segs = if *closed { n } else { n.saturating_sub(1) };
            out.extend((0..segs).map(|i| s((points[i] + points[(i + 1) % n]) / 2.0, SnapKind::Midpoint)));
        }
        Shape::Ellipse { center, rx, ry, angle } => {
            out.push(s(*center, SnapKind::Center));
            for k in 0..4 {
                let p = crate::curve::ellipse_point(*center, *rx, *ry, *angle, FRAC_PI_2 * k as f64);
                out.push(s(p, SnapKind::Endpoint));
            }
        }
        Shape::Spline { points, .. } => out.extend(points.iter().map(|p| s(*p, SnapKind::Endpoint))),
        Shape::Text { pos, .. } => out.push(s(*pos, SnapKind::Endpoint)),
        Shape::Dimension { .. } => {}
        Shape::Hatch { boundary: points, .. } | Shape::Leader { points, .. } => {
            out.extend(points.iter().map(|p| s(*p, SnapKind::Endpoint)));
        }
        Shape::RadialDim { center, .. } | Shape::AngularDim { center, .. } => out.push(s(*center, SnapKind::Center)),
        Shape::Box { center, .. } | Shape::Cylinder { center, .. } | Shape::Sphere { center, .. } => {
            out.push(s(center.truncate(), SnapKind::Center));
        }
    }
    out
}

enum Prim {
    Seg(DVec2, DVec2),
    Arc { c: DVec2, r: f64, start: f64, sweep: f64 },
}

fn prims(shape: &Shape) -> Vec<Prim> {
    match shape {
        Shape::Line { a, b } => vec![Prim::Seg(*a, *b)],
        Shape::Wall(w) => vec![Prim::Seg(w.a, w.b)],
        Shape::Rect { min, max } => {
            let c = [*min, DVec2::new(max.x, min.y), *max, DVec2::new(min.x, max.y)];
            (0..4).map(|i| Prim::Seg(c[i], c[(i + 1) % 4])).collect()
        }
        Shape::Polyline { points, closed } => {
            let n = points.len();
            let segs = if *closed { n } else { n.saturating_sub(1) };
            (0..segs).map(|i| Prim::Seg(points[i], points[(i + 1) % n])).collect()
        }
        Shape::Circle { center, radius } => vec![Prim::Arc { c: *center, r: *radius, start: 0.0, sweep: TAU }],
        Shape::Arc { center, radius, start, sweep } => {
            vec![Prim::Arc { c: *center, r: *radius, start: *start, sweep: *sweep }]
        }
        // shortcut: ellipse and spline intersect through a 1e-4 mm flattening, replace with exact conic and NURBS intersection
        Shape::Ellipse { .. } | Shape::Spline { .. } => {
            crate::curve::flatten(shape, crate::curve::FLATTEN_TOL).windows(2).map(|w| Prim::Seg(w[0], w[1])).collect()
        }
        Shape::Box { .. }
        | Shape::Cylinder { .. }
        | Shape::Sphere { .. }
        | Shape::Text { .. }
        | Shape::Dimension { .. }
        | Shape::Hatch { .. }
        | Shape::Leader { .. }
        | Shape::RadialDim { .. }
        | Shape::AngularDim { .. } => Vec::new(),
    }
}

fn in_sweep(angle: f64, start: f64, sweep: f64) -> bool {
    (angle - start).rem_euclid(TAU) <= sweep + 1e-9
}

fn angle_of(c: DVec2, p: DVec2) -> f64 {
    (p.y - c.y).atan2(p.x - c.x)
}

fn seg_seg(a: DVec2, b: DVec2, c: DVec2, d: DVec2, out: &mut Vec<DVec2>) {
    let (d1, d2) = (b - a, d - c);
    let cross = d1.perp_dot(d2);
    if cross.abs() <= 1e-12 * d1.length() * d2.length() {
        return;
    }
    let t = (c - a).perp_dot(d2) / cross;
    let u = (c - a).perp_dot(d1) / cross;
    let ok = |v: f64| (-1e-9..=1.0 + 1e-9).contains(&v);
    if ok(t) && ok(u) {
        out.push(a + d1 * t);
    }
}

fn seg_arc(a: DVec2, b: DVec2, c: DVec2, r: f64, start: f64, sweep: f64, out: &mut Vec<DVec2>) {
    let d = b - a;
    let f = a - c;
    let (qa, qb, qc) = (d.length_squared(), 2.0 * f.dot(d), f.length_squared() - r * r);
    let disc = qb * qb - 4.0 * qa * qc;
    if qa == 0.0 || disc < 0.0 {
        return;
    }
    let sq = disc.sqrt();
    let roots = if sq == 0.0 { vec![-qb / (2.0 * qa)] } else { vec![(-qb - sq) / (2.0 * qa), (-qb + sq) / (2.0 * qa)] };
    for t in roots {
        let p = a + d * t;
        if (-1e-9..=1.0 + 1e-9).contains(&t) && in_sweep(angle_of(c, p), start, sweep) {
            out.push(p);
        }
    }
}

fn arc_arc((c1, r1, s1, w1): (DVec2, f64, f64, f64), (c2, r2, s2, w2): (DVec2, f64, f64, f64), out: &mut Vec<DVec2>) {
    let dist = c1.distance(c2);
    if dist <= 1e-12 || dist > r1 + r2 || dist < (r1 - r2).abs() {
        return;
    }
    let x = (dist * dist + r1 * r1 - r2 * r2) / (2.0 * dist);
    let h = (r1 * r1 - x * x).max(0.0).sqrt();
    let dir = (c2 - c1) / dist;
    let base = c1 + dir * x;
    let pts = if h == 0.0 { vec![base] } else { vec![base + dir.perp() * h, base - dir.perp() * h] };
    for p in pts {
        if in_sweep(angle_of(c1, p), s1, w1) && in_sweep(angle_of(c2, p), s2, w2) {
            out.push(p);
        }
    }
}

pub fn intersections(a: &Shape, b: &Shape) -> Vec<DVec2> {
    let mut out = Vec::new();
    let pb = prims(b);
    for p in prims(a) {
        for q in &pb {
            match (&p, q) {
                (Prim::Seg(a1, a2), Prim::Seg(b1, b2)) => seg_seg(*a1, *a2, *b1, *b2, &mut out),
                (Prim::Seg(a1, a2), Prim::Arc { c, r, start, sweep })
                | (Prim::Arc { c, r, start, sweep }, Prim::Seg(a1, a2)) => {
                    seg_arc(*a1, *a2, *c, *r, *start, *sweep, &mut out)
                }
                (
                    Prim::Arc { c: c1, r: r1, start: s1, sweep: w1 },
                    Prim::Arc { c: c2, r: r2, start: s2, sweep: w2 },
                ) => arc_arc((*c1, *r1, *s1, *w1), (*c2, *r2, *s2, *w2), &mut out),
            }
        }
    }
    // a hit on the shared vertex of two flattened segments is found twice
    let mut unique: Vec<DVec2> = Vec::with_capacity(out.len());
    for p in out {
        if !unique.iter().any(|q| q.distance(p) <= 1e-9) {
            unique.push(p);
        }
    }
    unique
}

/// Perpendicular feet and tangent points of the shape as seen from `f`.
fn from_points(shape: &Shape, f: DVec2) -> Vec<Snap> {
    let mut out = Vec::new();
    let mut push = |point, kind| out.push(Snap { point, kind });
    for prim in prims(shape) {
        match prim {
            Prim::Seg(a, b) => {
                let t = (f - a).dot(b - a) / (b - a).length_squared();
                if (-1e-9..=1.0 + 1e-9).contains(&t) {
                    push(a + (b - a) * t, SnapKind::Perpendicular);
                }
            }
            Prim::Arc { c, r, start, sweep } => {
                let d = f.distance(c);
                if d <= 1e-9 {
                    continue;
                }
                let theta = angle_of(c, f);
                let at = |a: f64| c + DVec2::new(a.cos(), a.sin()) * r;
                for a in [theta, theta + std::f64::consts::PI] {
                    if in_sweep(a, start, sweep) {
                        push(at(a), SnapKind::Perpendicular);
                    }
                }
                if d > r + 1e-9 {
                    let alpha = (r / d).acos();
                    for a in [theta + alpha, theta - alpha] {
                        if in_sweep(a, start, sweep) {
                            push(at(a), SnapKind::Tangent);
                        }
                    }
                }
            }
        }
    }
    out
}

// shortcut: scans all shapes with a bbox reject, add an rstar index when the large-drawing benchmark needs it
pub fn object_snap<'a>(
    cursor: DVec2,
    radius: f64,
    from: Option<DVec2>,
    shapes: impl IntoIterator<Item = &'a Shape>,
) -> Option<Snap> {
    let near: Vec<&Shape> = shapes
        .into_iter()
        .filter(|s| {
            let (lo, hi) = s.bbox();
            let r = DVec2::splat(radius);
            cursor.cmpge(lo.truncate() - r).all() && cursor.cmple(hi.truncate() + r).all()
        })
        .collect();
    let mut cands: Vec<Snap> = near.iter().flat_map(|s| snap_points(s)).collect();
    if let Some(f) = from {
        cands.extend(near.iter().flat_map(|s| from_points(s, f)));
    }
    for (i, a) in near.iter().enumerate() {
        for b in &near[i + 1..] {
            cands.extend(intersections(a, b).into_iter().map(|point| Snap { point, kind: SnapKind::Intersection }));
        }
    }
    cands
        .into_iter()
        .map(|s| (s.point.distance(cursor), s))
        .filter(|(d, _)| *d <= radius)
        .reduce(|best, c| {
            if c.0 < best.0 - 1e-9 || ((c.0 - best.0).abs() < 1e-9 && c.1.kind < best.1.kind) { c } else { best }
        })
        .map(|(_, s)| s)
}

pub fn grid_snap(cursor: DVec2, step: f64) -> Snap {
    Snap { point: (cursor / step).round() * step, kind: SnapKind::Grid }
}

#[cfg(test)]
#[path = "snap_tests.rs"]
mod tests;
