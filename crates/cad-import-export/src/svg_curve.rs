//! Curve helpers of the SVG importer: flattening and the endpoint form of an elliptical arc.
use cad_geometry::{DAffine2, DVec2};
use std::f64::consts::{PI, TAU};

/// Largest distance of a flattened curve from the true curve, mm.
pub(crate) const FLATNESS: f64 = 0.01;
const MAX_DEPTH: u32 = 16;

fn flat_enough(p: &[DVec2; 4]) -> bool {
    let ab = p[3] - p[0];
    let len = ab.length();
    p[1..3].iter().all(|c| {
        if len < 1e-12 {
            return c.distance(p[0]) <= FLATNESS;
        }
        // the control point must project inside the chord, or the curve doubles back
        let along = ab.dot(*c - p[0]) / (len * len);
        (0.0..=1.0).contains(&along) && ab.perp_dot(*c - p[0]).abs() / len <= FLATNESS
    })
}

/// Appends the points of a cubic Bezier after its first point.
pub(crate) fn flatten_cubic(p: [DVec2; 4], out: &mut Vec<DVec2>) {
    split(p, 0, out);
}

fn split(p: [DVec2; 4], depth: u32, out: &mut Vec<DVec2>) {
    if depth >= MAX_DEPTH || flat_enough(&p) {
        out.push(p[3]);
        return;
    }
    let mid = |a: DVec2, b: DVec2| (a + b) / 2.0;
    let (a, b, c) = (mid(p[0], p[1]), mid(p[1], p[2]), mid(p[2], p[3]));
    let (d, e) = (mid(a, b), mid(b, c));
    let f = mid(d, e);
    split([p[0], a, d, f], depth + 1, out);
    split([f, e, c, p[3]], depth + 1, out);
}

/// The centre form of an SVG arc.
pub(crate) struct ArcGeom {
    pub center: DVec2,
    pub rx: f64,
    pub ry: f64,
    /// X axis rotation, radians.
    pub phi: f64,
    pub theta: f64,
    /// Signed sweep: positive when the angle grows.
    pub delta: f64,
}

/// SVG implementation note F.6.5. None when the arc has no length or a zero radius (a straight line then).
pub(crate) fn arc_center(p1: DVec2, p2: DVec2, rx: f64, ry: f64, rotation_deg: f64, large: bool, sweep: bool) -> Option<ArcGeom> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if p1.distance(p2) < 1e-12 || rx == 0.0 || ry == 0.0 {
        return None;
    }
    let phi = rotation_deg.to_radians();
    let (sin, cos) = phi.sin_cos();
    let half = (p1 - p2) / 2.0;
    let (x1, y1) = (cos * half.x + sin * half.y, -sin * half.x + cos * half.y);
    let lambda = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lambda > 1.0 {
        let k = lambda.sqrt();
        rx *= k;
        ry *= k;
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let coef = if large == sweep { -1.0 } else { 1.0 } * (num / den).max(0.0).sqrt();
    let (cx, cy) = (coef * rx * y1 / ry, -coef * ry * x1 / rx);
    let mid = (p1 + p2) / 2.0;
    let center = DVec2::new(cos * cx - sin * cy + mid.x, sin * cx + cos * cy + mid.y);
    let angle = |u: DVec2, v: DVec2| u.perp_dot(v).atan2(u.dot(v));
    let u = DVec2::new((x1 - cx) / rx, (y1 - cy) / ry);
    let v = DVec2::new((-x1 - cx) / rx, (-y1 - cy) / ry);
    let mut delta = angle(u, v);
    if !sweep && delta > 0.0 {
        delta -= TAU;
    } else if sweep && delta < 0.0 {
        delta += TAU;
    }
    Some(ArcGeom { center, rx, ry, phi, theta: angle(DVec2::X, u), delta })
}

/// Points of an elliptical arc mapped by `m`, first and last included, within `FLATNESS` of the curve.
pub(crate) fn sample_arc(a: &ArcGeom, m: &DAffine2) -> Vec<DVec2> {
    let r = a.rx.max(a.ry) * m.matrix2.x_axis.length().max(m.matrix2.y_axis.length());
    let step = if r <= FLATNESS { PI / 2.0 } else { 2.0 * (1.0 - FLATNESS / r).acos() };
    let n = ((a.delta.abs() / step).ceil() as usize).clamp(2, 100_000);
    let turn = DVec2::from_angle(a.phi);
    (0..=n)
        .map(|i| {
            let t = a.theta + a.delta * i as f64 / n as f64;
            m.transform_point2(a.center + turn.rotate(DVec2::new(a.rx * t.cos(), a.ry * t.sin())))
        })
        .collect()
}
