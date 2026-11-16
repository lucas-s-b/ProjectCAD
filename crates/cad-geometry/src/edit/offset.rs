use crate::shape::{GeomError, Shape};
use crate::tol::Tolerance;
use glam::DVec2;

/// A parallel copy at `distance`, on the side of the shape where `side` lies.
pub fn offset(shape: &Shape, distance: f64, side: DVec2) -> Result<Shape, GeomError> {
    if !distance.is_finite() || distance <= 0.0 {
        return Err(GeomError::Degenerate("offset distance"));
    }
    let result = match shape {
        Shape::Circle { center, radius } => {
            Shape::Circle { center: *center, radius: radial(*center, *radius, distance, side) }
        }
        Shape::Arc { center, radius, start, sweep } => Shape::Arc {
            center: *center,
            radius: radial(*center, *radius, distance, side),
            start: *start,
            sweep: *sweep,
        },
        Shape::Rect { min, max } => {
            let inside = side.cmpgt(*min).all() && side.cmplt(*max).all();
            let d = DVec2::splat(if inside { -distance } else { distance });
            Shape::Rect { min: *min - d, max: *max + d }
        }
        Shape::Line { a, b } => {
            let pts = offset_points(&[*a, *b], false, distance, side);
            Shape::Line { a: pts[0], b: pts[1] }
        }
        Shape::Polyline { points, closed } => {
            Shape::Polyline { points: offset_points(points, *closed, distance, side), closed: *closed }
        }
        _ => return Err(super::param::not_curve(shape)),
    };
    result.validate(&Tolerance::default())?;
    Ok(result)
}

fn radial(center: DVec2, radius: f64, distance: f64, side: DVec2) -> f64 {
    if side.distance(center) > radius { radius + distance } else { radius - distance }
}

// shortcut: mitre joins without self-intersection cleanup, replace when B-Rep wire offset exists (Phase 3)
fn offset_points(pts: &[DVec2], closed: bool, distance: f64, side: DVec2) -> Vec<DVec2> {
    let n = pts.len();
    let segs = if closed { n } else { n - 1 };
    let seg = |i: usize| (pts[i], pts[(i + 1) % n]);
    let near = (0..segs)
        .min_by(|&i, &j| seg_dist(seg(i), side).total_cmp(&seg_dist(seg(j), side)))
        .unwrap_or(0);
    let (a, b) = seg(near);
    let s = if (b - a).perp_dot(side - a) > 0.0 { 1.0 } else { -1.0 };
    let dir = |i: usize| (seg(i).1 - seg(i).0).normalize();
    let shift = |i: usize| s * distance * dir(i).perp();
    let join = |prev: usize, next: usize| {
        let (d0, d1) = (dir(prev), dir(next));
        let cross = d0.perp_dot(d1);
        let p = pts[(prev + 1) % n];
        if cross.abs() < 1e-12 {
            return p + shift(next);
        }
        let (q0, q1) = (seg(prev).0 + shift(prev), p + shift(next));
        q0 + d0 * ((q1 - q0).perp_dot(d1) / cross)
    };
    let mut out = Vec::with_capacity(n);
    if closed {
        out.extend((0..n).map(|j| join((j + n - 1) % n, j)));
    } else {
        out.push(pts[0] + shift(0));
        out.extend((1..n - 1).map(|j| join(j - 1, j)));
        out.push(pts[n - 1] + shift(n - 2));
    }
    out
}

fn seg_dist((a, b): (DVec2, DVec2), p: DVec2) -> f64 {
    let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
    (a + (b - a) * t).distance(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v2(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }
    fn near(a: DVec2, b: DVec2) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn line_both_sides() {
        let l = Shape::Line { a: v2(0.0, 0.0), b: v2(10.0, 0.0) };
        assert_eq!(offset(&l, 5.0, v2(3.0, 4.0)), Ok(Shape::Line { a: v2(0.0, 5.0), b: v2(10.0, 5.0) }));
        assert_eq!(offset(&l, 5.0, v2(3.0, -4.0)), Ok(Shape::Line { a: v2(0.0, -5.0), b: v2(10.0, -5.0) }));
    }

    #[test]
    fn circle_and_arc() {
        let c = Shape::Circle { center: v2(0.0, 0.0), radius: 5.0 };
        assert_eq!(offset(&c, 2.0, v2(10.0, 0.0)), Ok(Shape::Circle { center: v2(0.0, 0.0), radius: 7.0 }));
        assert_eq!(offset(&c, 2.0, v2(1.0, 0.0)), Ok(Shape::Circle { center: v2(0.0, 0.0), radius: 3.0 }));
        assert!(offset(&c, 6.0, v2(1.0, 0.0)).is_err());
        let a = Shape::Arc { center: v2(0.0, 0.0), radius: 5.0, start: 0.5, sweep: 1.0 };
        assert_eq!(
            offset(&a, 1.0, v2(9.0, 0.0)),
            Ok(Shape::Arc { center: v2(0.0, 0.0), radius: 6.0, start: 0.5, sweep: 1.0 })
        );
    }

    #[test]
    fn rect_grow_shrink() {
        let r = Shape::Rect { min: v2(0.0, 0.0), max: v2(10.0, 6.0) };
        assert_eq!(offset(&r, 2.0, v2(-5.0, -5.0)), Ok(Shape::Rect { min: v2(-2.0, -2.0), max: v2(12.0, 8.0) }));
        assert_eq!(offset(&r, 2.0, v2(5.0, 3.0)), Ok(Shape::Rect { min: v2(2.0, 2.0), max: v2(8.0, 4.0) }));
        assert!(offset(&r, 3.0, v2(5.0, 3.0)).is_err());
    }

    #[test]
    fn polylines() {
        let l = Shape::Polyline { points: vec![v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0)], closed: false };
        let Ok(Shape::Polyline { points, .. }) = offset(&l, 2.0, v2(5.0, 3.0)) else { panic!() };
        assert!(near(points[0], v2(0.0, 2.0)) && near(points[1], v2(8.0, 2.0)) && near(points[2], v2(8.0, 10.0)));
        let sq = Shape::Polyline {
            points: vec![v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0), v2(0.0, 10.0)],
            closed: true,
        };
        let Ok(Shape::Polyline { points, closed }) = offset(&sq, 2.0, v2(-5.0, 5.0)) else { panic!() };
        assert!(closed && near(points[0], v2(-2.0, -2.0)) && near(points[2], v2(12.0, 12.0)));
    }

    #[test]
    fn bad_distance() {
        let l = Shape::Line { a: v2(0.0, 0.0), b: v2(10.0, 0.0) };
        for d in [0.0, -1.0, f64::NAN] {
            assert_eq!(offset(&l, d, v2(0.0, 1.0)), Err(GeomError::Degenerate("offset distance")));
        }
    }
}
