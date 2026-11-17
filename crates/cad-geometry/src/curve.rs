use crate::pick::point_segment_distance;
use crate::shape::Shape;
use glam::DVec2;
use std::f64::consts::TAU;

/// Point at angle `t` (radians around the ellipse); `angle` is the direction of the `rx` axis.
pub fn ellipse_point(center: DVec2, rx: f64, ry: f64, angle: f64, t: f64) -> DVec2 {
    center + DVec2::from_angle(angle).rotate(DVec2::new(rx * t.cos(), ry * t.sin()))
}

pub fn spline_spans(points: &[DVec2], closed: bool) -> usize {
    if closed { points.len() } else { points.len().saturating_sub(1) }
}

/// Point on span `span` (between points[span] and points[span + 1], wrapping when closed), u in 0..=1.
/// Centripetal Catmull-Rom (Barry-Goldman): knots advance by sqrt(distance). Open ends use the mirrored
/// phantom points 2*P0 - P1 and 2*Pn - Pn-1.
pub fn spline_point(points: &[DVec2], closed: bool, span: usize, u: f64) -> DVec2 {
    let n = points.len();
    let at = |k: isize| -> DVec2 {
        if closed {
            points[k.rem_euclid(n as isize) as usize]
        } else if k < 0 {
            2.0 * points[0] - points[1]
        } else if k as usize >= n {
            2.0 * points[n - 1] - points[n - 2]
        } else {
            points[k as usize]
        }
    };
    let s = span as isize;
    let p = [at(s - 1), at(s), at(s + 1), at(s + 2)];
    let mut t = [0.0; 4];
    for k in 1..4 {
        t[k] = t[k - 1] + p[k].distance(p[k - 1]).sqrt();
    }
    let x = t[1] + u * (t[2] - t[1]);
    let lerp = |a: DVec2, b: DVec2, ta: f64, tb: f64| a * ((tb - x) / (tb - ta)) + b * ((x - ta) / (tb - ta));
    let a1 = lerp(p[0], p[1], t[0], t[1]);
    let a2 = lerp(p[1], p[2], t[1], t[2]);
    let a3 = lerp(p[2], p[3], t[2], t[3]);
    let b1 = lerp(a1, a2, t[0], t[2]);
    let b2 = lerp(a2, a3, t[1], t[3]);
    lerp(b1, b2, t[1], t[2])
}

/// Flattened spline: `per_span` segments per span.
pub fn spline_path(points: &[DVec2], closed: bool, per_span: usize) -> Vec<DVec2> {
    let mut out = Vec::new();
    for span in 0..spline_spans(points, closed) {
        out.extend((0..per_span).map(|i| spline_point(points, closed, span, i as f64 / per_span as f64)));
    }
    out.push(if closed { points[0] } else { points[points.len() - 1] });
    out
}

/// Chord tolerance in mm of the flattening that trim, split and intersections use for ellipses and splines.
pub const FLATTEN_TOL: f64 = 1e-4;
const MAX_DEPTH: u32 = 12;

/// One span of a spline being flattened.
struct Span<'a> {
    points: &'a [DVec2],
    closed: bool,
    index: usize,
    tol: f64,
}

impl Span<'_> {
    /// Appends the flattening of the part between `(u0, p0)` and `(u1, p1)`, splitting until the midpoint is within the tolerance of its chord.
    fn refine(&self, (u0, p0): (f64, DVec2), (u1, p1): (f64, DVec2), depth: u32, out: &mut Vec<(f64, DVec2)>) {
        let um = (u0 + u1) / 2.0;
        let pm = spline_point(self.points, self.closed, self.index, um);
        // the first two levels are forced so that a midpoint on the chord by chance does not hide a bend
        if depth >= MAX_DEPTH || (depth >= 2 && point_segment_distance(pm, p0, p1) < self.tol) {
            out.push((self.index as f64 + u1, p1));
            return;
        }
        self.refine((u0, p0), (um, pm), depth + 1, out);
        self.refine((um, pm), (u1, p1), depth + 1, out);
    }
}

/// Flattening of an Ellipse or Spline with the curve parameter of every point (ellipse angle, or span + u).
/// Closed shapes repeat the first point at the end. Empty for every other shape.
pub(crate) fn flatten_param(shape: &Shape, tol: f64) -> Vec<(f64, DVec2)> {
    match shape {
        Shape::Ellipse { center, rx, ry, angle } => {
            let ratio = (tol / rx.max(*ry)).min(1.0);
            let n = ((TAU / (2.0 * (1.0 - ratio).acos())).ceil() as usize).max(8);
            (0..=n)
                .map(|i| {
                    let t = TAU * i as f64 / n as f64;
                    (t, ellipse_point(*center, *rx, *ry, *angle, t))
                })
                .collect()
        }
        Shape::Spline { points, closed } => {
            let mut out = vec![(0.0, spline_point(points, *closed, 0, 0.0))];
            for span in 0..spline_spans(points, *closed) {
                let start = out[out.len() - 1].1;
                let end = if *closed { points[(span + 1) % points.len()] } else { points[span + 1] };
                Span { points, closed: *closed, index: span, tol }.refine((0.0, start), (1.0, end), 0, &mut out);
            }
            out
        }
        _ => Vec::new(),
    }
}

/// Points of an Ellipse or Spline whose chords stay within `tol` of the curve; closed shapes repeat the first point at the end.
pub fn flatten(shape: &Shape, tol: f64) -> Vec<DVec2> {
    flatten_param(shape, tol).into_iter().map(|(_, p)| p).collect()
}

#[cfg(test)]
#[path = "curve_tests.rs"]
mod tests;
