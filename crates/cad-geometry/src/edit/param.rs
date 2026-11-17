use crate::curve::{FLATTEN_TOL, ellipse_point, flatten_param, spline_point, spline_spans};
use crate::shape::{GeomError, Shape};
use crate::tol::Tolerance;
use glam::DVec2;
use std::f64::consts::TAU;

/// Vertices and closed flag of a Polyline or Rect (Rect = closed, corner min first).
fn poly(shape: &Shape) -> Option<(Vec<DVec2>, bool)> {
    match shape {
        Shape::Polyline { points, closed } => Some((points.clone(), *closed)),
        Shape::Rect { min, max } => {
            Some((vec![*min, DVec2::new(max.x, min.y), *max, DVec2::new(min.x, max.y)], true))
        }
        _ => None,
    }
}

fn seg_count(n: usize, closed: bool) -> usize {
    if closed { n } else { n.saturating_sub(1) }
}

/// (end parameter, closed). None for solids.
pub(crate) fn domain(shape: &Shape) -> Option<(f64, bool)> {
    match shape {
        Shape::Line { .. } => Some((1.0, false)),
        Shape::Arc { sweep, .. } => Some((*sweep, false)),
        Shape::Circle { .. } => Some((TAU, true)),
        Shape::Ellipse { .. } => Some((TAU, true)),
        Shape::Spline { points, closed } => Some((spline_spans(points, *closed) as f64, *closed)),
        Shape::Polyline { .. } | Shape::Rect { .. } => {
            let (pts, closed) = poly(shape)?;
            Some((seg_count(pts.len(), closed) as f64, closed))
        }
        _ => None,
    }
}

/// Error for edit operations that ellipses and splines do not support (extend, offset), or on a shape that has no curve parameter.
pub(crate) fn not_curve(shape: &Shape) -> GeomError {
    match shape {
        Shape::Ellipse { .. } | Shape::Spline { .. } => GeomError::Unsupported("not supported for this shape yet"),
        _ => GeomError::Unsupported("this shape is not a curve"),
    }
}

// shortcut: ellipse and spline parameters come from a 1e-4 mm flattening, replace with exact projection onto the conic and the spline
/// Parameter of the point of the flattening nearest to `p`, interpolated along its chord.
fn nearest_on_flattening(shape: &Shape, p: DVec2) -> f64 {
    let flat = flatten_param(shape, FLATTEN_TOL);
    let mut best = (f64::INFINITY, 0.0);
    for w in flat.windows(2) {
        let ((t0, a), (t1, b)) = (w[0], w[1]);
        let s = project(a, b, p);
        let d = (a + (b - a) * s).distance(p);
        if d < best.0 {
            best = (d, t0 + (t1 - t0) * s);
        }
    }
    best.1
}

fn project(a: DVec2, b: DVec2, p: DVec2) -> f64 {
    ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0)
}

/// Parameter of the point of the shape nearest to `p`.
pub(crate) fn param_of(shape: &Shape, p: DVec2) -> f64 {
    match shape {
        Shape::Line { a, b } => project(*a, *b, p),
        Shape::Circle { center, .. } => (p - *center).to_angle().rem_euclid(TAU),
        Shape::Ellipse { .. } | Shape::Spline { .. } => nearest_on_flattening(shape, p),
        Shape::Arc { center, start, sweep, .. } => {
            let off = ((p - *center).to_angle() - start).rem_euclid(TAU);
            if off <= *sweep {
                off
            } else if off - sweep < TAU - off {
                *sweep
            } else {
                0.0
            }
        }
        _ => {
            let Some((pts, closed)) = poly(shape) else { return 0.0 };
            let n = pts.len();
            let mut best = (f64::INFINITY, 0.0);
            for i in 0..seg_count(n, closed) {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                let t = project(a, b, p);
                let d = (a + (b - a) * t).distance(p);
                if d < best.0 {
                    best = (d, i as f64 + t);
                }
            }
            best.1
        }
    }
}

pub(crate) fn point_at(shape: &Shape, u: f64) -> DVec2 {
    match shape {
        Shape::Line { a, b } => *a + (*b - *a) * u,
        Shape::Circle { center, radius } => *center + DVec2::from_angle(u) * *radius,
        Shape::Ellipse { center, rx, ry, angle } => ellipse_point(*center, *rx, *ry, *angle, u),
        Shape::Spline { points, closed } => {
            let spans = spline_spans(points, *closed);
            let u = if *closed { u.rem_euclid(spans as f64) } else { u };
            let i = (u.floor().max(0.0) as usize).min(spans - 1);
            spline_point(points, *closed, i, u - i as f64)
        }
        Shape::Arc { center, radius, start, .. } => *center + DVec2::from_angle(start + u) * *radius,
        _ => {
            let Some((pts, closed)) = poly(shape) else { return DVec2::ZERO };
            let n = pts.len();
            let segs = seg_count(n, closed);
            let u = if closed { u.rem_euclid(segs as f64) } else { u };
            let i = (u.floor().max(0.0) as usize).min(segs - 1);
            pts[i] + (pts[(i + 1) % n] - pts[i]) * (u - i as f64)
        }
    }
}

/// The piece from `u0` to `u1`; closed shapes may wrap (`u1` up to one period past the end).
pub(crate) fn sub(shape: &Shape, u0: f64, u1: f64) -> Option<Shape> {
    let piece = match shape {
        Shape::Line { .. } => Shape::Line { a: point_at(shape, u0), b: point_at(shape, u1) },
        Shape::Arc { center, radius, start, .. } => {
            Shape::Arc { center: *center, radius: *radius, start: start + u0, sweep: u1 - u0 }
        }
        Shape::Circle { center, radius } => {
            Shape::Arc { center: *center, radius: *radius, start: u0, sweep: u1 - u0 }
        }
        Shape::Ellipse { .. } => {
            // an ellipse piece is not an ellipse: it becomes the polyline of the flattening between the two angles
            let flat = flatten_param(shape, FLATTEN_TOL);
            let mut out = vec![point_at(shape, u0)];
            for lap in [0.0, TAU] {
                out.extend(flat[..flat.len() - 1].iter().filter(|(t, _)| t + lap > u0 + 1e-9 && t + lap < u1 - 1e-9).map(|(_, p)| *p));
            }
            out.push(point_at(shape, u1));
            Shape::Polyline { points: out, closed: false }
        }
        Shape::Spline { points, .. } => {
            // shortcut: the piece is a new spline through the cut points and the original fit points between them, its shape differs slightly near the cuts, replace with exact knot insertion
            let mut out = vec![point_at(shape, u0)];
            let n = points.len() as i64;
            let first = (u0 + 1e-9).floor() as i64 + 1;
            for i in first..=(u1 - 1e-9).ceil() as i64 {
                if (i as f64) > u0 + 1e-9 && (i as f64) < u1 - 1e-9 {
                    out.push(points[(i % n) as usize]);
                }
            }
            out.push(point_at(shape, u1));
            Shape::Spline { points: out, closed: false }
        }
        _ => {
            let (pts, _) = poly(shape)?;
            let mut out = vec![point_at(shape, u0)];
            let first = (u0 + 1e-9).floor() as i64 + 1;
            for i in first..=(u1 - 1e-9).ceil() as i64 {
                if (i as f64) > u0 + 1e-9 && (i as f64) < u1 - 1e-9 {
                    out.push(pts[i as usize % pts.len()]);
                }
            }
            out.push(point_at(shape, u1));
            Shape::Polyline { points: out, closed: false }
        }
    };
    piece.validate(&Tolerance::default()).ok().map(|_| piece)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI};

    fn v2(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }
    fn line() -> Shape {
        Shape::Line { a: v2(0.0, 0.0), b: v2(4.0, 0.0) }
    }
    fn arc() -> Shape {
        Shape::Arc { center: v2(0.0, 0.0), radius: 2.0, start: 0.0, sweep: FRAC_PI_2 }
    }
    fn pl(closed: bool) -> Shape {
        Shape::Polyline { points: vec![v2(0.0, 0.0), v2(2.0, 0.0), v2(2.0, 2.0), v2(0.0, 2.0)], closed }
    }
    fn rect() -> Shape {
        Shape::rect(v2(0.0, 0.0), v2(2.0, 1.0))
    }
    fn near(a: DVec2, b: DVec2) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn round_trip() {
        let c = Shape::Circle { center: v2(1.0, 1.0), radius: 3.0 };
        let cases = [
            (line(), v2(1.0, 0.0)),
            (arc(), v2(2.0f64.sqrt(), 2.0f64.sqrt())),
            (c, v2(1.0, 4.0)),
            (pl(false), v2(2.0, 1.5)),
            (pl(true), v2(1.0, 2.0)),
            (rect(), v2(2.0, 0.5)),
        ];
        for (s, p) in cases {
            let u = param_of(&s, p);
            assert!(near(point_at(&s, u), p), "{s:?}");
        }
        assert_eq!(domain(&pl(true)), Some((4.0, true)));
        assert_eq!(domain(&pl(false)), Some((3.0, false)));
        assert_eq!(domain(&rect()), Some((4.0, true)));
        assert_eq!(domain(&Shape::Sphere { center: glam::DVec3::ZERO, radius: 1.0 }), None);
    }

    #[test]
    fn arc_param_clamps_to_nearer_end() {
        let a = arc();
        assert!((param_of(&a, v2(1.0, -0.2)) - 0.0).abs() < 1e-12);
        assert!((param_of(&a, v2(-0.2, 1.0)) - FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn sub_line_arc_circle() {
        let Some(Shape::Line { a, b }) = sub(&line(), 0.25, 0.75) else { panic!() };
        assert!(near(a, v2(1.0, 0.0)) && near(b, v2(3.0, 0.0)));
        let Some(Shape::Arc { start, sweep, .. }) = sub(&arc(), 0.5, 1.0) else { panic!() };
        assert!((start - 0.5).abs() < 1e-12 && (sweep - 0.5).abs() < 1e-12);
        let c = Shape::Circle { center: v2(0.0, 0.0), radius: 1.0 };
        let Some(Shape::Arc { start, sweep, .. }) = sub(&c, 6.0, TAU + 1.0) else { panic!() };
        assert!((start - 6.0).abs() < 1e-12 && (sweep - (TAU + 1.0 - 6.0)).abs() < 1e-12);
        assert!(sub(&c, 0.0, PI).is_some());
    }

    #[test]
    fn sub_polyline_and_rect() {
        let Some(Shape::Polyline { points, closed }) = sub(&pl(false), 0.5, 2.5) else { panic!() };
        assert!(!closed && points.len() == 4);
        assert!(near(points[0], v2(1.0, 0.0)) && near(points[1], v2(2.0, 0.0)));
        assert!(near(points[2], v2(2.0, 2.0)) && near(points[3], v2(1.0, 2.0)));
        let Some(Shape::Polyline { points, .. }) = sub(&rect(), 3.5, 4.5) else { panic!() };
        assert_eq!(points.len(), 3);
        assert!(near(points[0], v2(0.0, 0.5)) && near(points[1], v2(0.0, 0.0)) && near(points[2], v2(1.0, 0.0)));
    }

    #[test]
    fn sub_zero_length_is_none() {
        assert!(sub(&line(), 0.5, 0.5).is_none());
        assert!(sub(&pl(false), 1.0, 1.0).is_none());
        assert!(sub(&arc(), 0.3, 0.3).is_none());
    }
}
