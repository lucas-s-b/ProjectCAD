use super::param::{domain, not_curve, param_of, sub};
use crate::shape::{GeomError, Shape};
use crate::snap::intersections;
use glam::DVec2;
use std::f64::consts::TAU;

/// Removes the part of `target` under `pick`, between the nearest intersections with `cutters`. Returns what remains (0 to 2 shapes).
pub fn trim(target: &Shape, pick: DVec2, cutters: &[&Shape]) -> Result<Vec<Shape>, GeomError> {
    let Some((end, closed)) = domain(target) else {
        return Err(not_curve(target));
    };
    let mut cuts: Vec<f64> =
        cutters.iter().flat_map(|c| intersections(target, c)).map(|x| param_of(target, x)).collect();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|b, a| *b - *a < 1e-9);
    let tp = param_of(target, pick);
    let lo = cuts.iter().rev().find(|c| **c <= tp).copied();
    let hi = cuts.iter().find(|c| **c > tp).copied();
    if closed {
        if cuts.len() < 2 {
            return Err(GeomError::NoIntersection);
        }
        let lo = lo.unwrap_or(cuts[cuts.len() - 1] - end);
        let hi = hi.unwrap_or(cuts[0] + end);
        return Ok(sub(target, hi, lo + end).into_iter().collect());
    }
    if lo.is_none() && hi.is_none() {
        return Err(GeomError::NoIntersection);
    }
    let mut out = Vec::new();
    out.extend(lo.and_then(|lo| sub(target, 0.0, lo)));
    out.extend(hi.and_then(|hi| sub(target, hi, end)));
    Ok(out)
}

/// Lengthens the end of `target` nearest to `pick` until it meets the first boundary.
pub fn extend(target: &Shape, pick: DVec2, boundaries: &[&Shape]) -> Result<Shape, GeomError> {
    if matches!(target, Shape::Ellipse { .. } | Shape::Spline { .. }) {
        return Err(not_curve(target));
    }
    let Some((end, false)) = domain(target) else {
        return Err(GeomError::Unsupported("extend needs an open curve"));
    };
    let at_start = param_of(target, pick) < end / 2.0;
    match target {
        Shape::Arc { center, radius, start, sweep } => {
            let probe = Shape::Circle { center: *center, radius: *radius };
            let offs = boundaries.iter().flat_map(|b| intersections(&probe, b)).filter_map(|h| {
                let angle = (h - *center).to_angle();
                let off = (angle - start).rem_euclid(TAU);
                (off > sweep + 1e-9).then_some((off, angle))
            });
            let (off, angle) = if at_start {
                offs.max_by(|a, b| a.0.total_cmp(&b.0))
            } else {
                offs.min_by(|a, b| a.0.total_cmp(&b.0))
            }
            .ok_or(GeomError::NoIntersection)?;
            let (center, radius) = (*center, *radius);
            Ok(if at_start {
                Shape::Arc { center, radius, start: crate::shape::norm_angle(angle), sweep: sweep + (TAU - off) }
            } else {
                Shape::Arc { center, radius, start: *start, sweep: off }
            })
        }
        Shape::Line { a, b } => Ok(if at_start {
            Shape::Line { a: ray_hit(target, *a, *a - *b, boundaries)?, b: *b }
        } else {
            Shape::Line { a: *a, b: ray_hit(target, *b, *b - *a, boundaries)? }
        }),
        Shape::Polyline { points, closed } => {
            let mut points = points.clone();
            let n = points.len();
            let (e, prev) = if at_start { (0, 1) } else { (n - 1, n - 2) };
            points[e] = ray_hit(target, points[e], points[e] - points[prev], boundaries)?;
            Ok(Shape::Polyline { points, closed: *closed })
        }
        _ => Err(GeomError::Unsupported("extend needs an open curve")),
    }
}

/// Nearest boundary hit on the ray from `e` along `dir`.
fn ray_hit(target: &Shape, e: DVec2, dir: DVec2, boundaries: &[&Shape]) -> Result<DVec2, GeomError> {
    let (mut lo, mut hi) = target.bbox();
    for b in boundaries {
        let (l, h) = b.bbox();
        lo = lo.min(l);
        hi = hi.max(h);
    }
    let len = 2.0 * (hi - lo).truncate().length() + 1.0;
    let probe = Shape::Line { a: e, b: e + dir.normalize() * len };
    boundaries
        .iter()
        .flat_map(|b| intersections(&probe, b))
        .filter(|p| p.distance(e) > 1e-9)
        .min_by(|p, q| p.distance(e).total_cmp(&q.distance(e)))
        .ok_or(GeomError::NoIntersection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_4, PI};

    fn v2(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }
    fn line(a: (f64, f64), b: (f64, f64)) -> Shape {
        Shape::Line { a: v2(a.0, a.1), b: v2(b.0, b.1) }
    }
    fn near(a: DVec2, b: DVec2) -> bool {
        a.distance(b) < 1e-9
    }
    fn ends(s: &Shape) -> (DVec2, DVec2) {
        match s {
            Shape::Line { a, b } => (*a, *b),
            Shape::Polyline { points, .. } => (points[0], *points.last().unwrap()),
            _ => panic!("{s:?}"),
        }
    }

    #[test]
    fn trim_line_middle_and_end() {
        let t = line((0.0, 0.0), (10.0, 0.0));
        let (c1, c2) = (line((3.0, -1.0), (3.0, 1.0)), line((7.0, -1.0), (7.0, 1.0)));
        let r = trim(&t, v2(5.0, 0.0), &[&c1, &c2]).unwrap();
        assert_eq!(r.len(), 2);
        assert!(near(ends(&r[0]).1, v2(3.0, 0.0)) && near(ends(&r[0]).0, v2(0.0, 0.0)));
        assert!(near(ends(&r[1]).0, v2(7.0, 0.0)) && near(ends(&r[1]).1, v2(10.0, 0.0)));
        let r = trim(&t, v2(1.0, 0.0), &[&c1]).unwrap();
        assert_eq!(r.len(), 1);
        assert!(near(ends(&r[0]).0, v2(3.0, 0.0)) && near(ends(&r[0]).1, v2(10.0, 0.0)));
        let far = line((20.0, -1.0), (20.0, 1.0));
        assert_eq!(trim(&t, v2(1.0, 0.0), &[&far]), Err(GeomError::NoIntersection));
    }

    #[test]
    fn trim_circle() {
        let c = Shape::Circle { center: v2(0.0, 0.0), radius: 5.0 };
        let l = line((-10.0, 0.0), (10.0, 0.0));
        let pick = v2(0.0, 5.0);
        let r = trim(&c, pick, &[&l]).unwrap();
        assert_eq!(r.len(), 1);
        let Shape::Arc { start, sweep, .. } = r[0] else { panic!() };
        assert!((start - PI).abs() < 1e-9 && (sweep - PI).abs() < 1e-9);
        assert!((PI / 2.0 - start).rem_euclid(TAU) > sweep);
        let tangent = line((-10.0, 5.0), (10.0, 5.0));
        assert_eq!(trim(&c, pick, &[&tangent]), Err(GeomError::NoIntersection));
    }

    #[test]
    fn trim_arc() {
        let a = Shape::Arc { center: v2(0.0, 0.0), radius: 5.0, start: 0.0, sweep: PI };
        let l = line((0.0, 0.0), (0.0, 10.0));
        let r = trim(&a, v2(5.0 * 0.5f64.cos(), 5.0 * 0.5f64.sin()), &[&l]).unwrap();
        assert_eq!(r.len(), 1);
        let Shape::Arc { start, sweep, .. } = r[0] else { panic!() };
        assert!((start - PI / 2.0).abs() < 1e-9 && (sweep - PI / 2.0).abs() < 1e-9);
    }

    #[test]
    fn trim_polyline_and_rect() {
        let p = Shape::Polyline { points: vec![v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0)], closed: false };
        let (c1, c2) = (line((5.0, -1.0), (5.0, 1.0)), line((9.0, 5.0), (11.0, 5.0)));
        let r = trim(&p, v2(10.0, 0.0), &[&c1, &c2]).unwrap();
        assert_eq!(r.len(), 2);
        assert!(near(ends(&r[0]).1, v2(5.0, 0.0)) && near(ends(&r[1]).0, v2(10.0, 5.0)));
        let rect = Shape::rect(v2(0.0, 0.0), v2(10.0, 10.0));
        let (c1, c2) = (line((3.0, -1.0), (3.0, 1.0)), line((7.0, -1.0), (7.0, 1.0)));
        let r = trim(&rect, v2(5.0, 0.0), &[&c1, &c2]).unwrap();
        assert_eq!(r.len(), 1);
        let Shape::Polyline { points, closed } = &r[0] else { panic!() };
        assert!(!closed && points.len() == 6);
        assert!(near(points[0], v2(7.0, 0.0)) && near(points[5], v2(3.0, 0.0)));
    }

    #[test]
    fn extend_line() {
        let t = line((0.0, 0.0), (5.0, 0.0));
        let b1 = line((10.0, -5.0), (10.0, 5.0));
        let r = extend(&t, v2(4.0, 0.0), &[&b1]).unwrap();
        assert_eq!(r, line((0.0, 0.0), (10.0, 0.0)));
        let b2 = line((7.0, -5.0), (7.0, 5.0));
        assert_eq!(extend(&t, v2(4.0, 0.0), &[&b1, &b2]).unwrap(), line((0.0, 0.0), (7.0, 0.0)));
        let behind = line((-3.0, -5.0), (-3.0, 5.0));
        assert_eq!(extend(&t, v2(4.0, 0.0), &[&behind]), Err(GeomError::NoIntersection));
        assert_eq!(extend(&t, v2(1.0, 0.0), &[&behind]).unwrap(), line((-3.0, 0.0), (5.0, 0.0)));
    }

    #[test]
    fn extend_arc_and_unsupported() {
        let a = Shape::Arc { center: v2(0.0, 0.0), radius: 5.0, start: 0.0, sweep: FRAC_PI_4 };
        let up = line((0.0, 0.0), (0.0, 10.0));
        let end_pick = v2(5.0 * FRAC_PI_4.cos(), 5.0 * FRAC_PI_4.sin());
        let Shape::Arc { start, sweep, .. } = extend(&a, end_pick, &[&up]).unwrap() else { panic!() };
        assert!(start.abs() < 1e-9 && (sweep - PI / 2.0).abs() < 1e-9);
        let down = line((-10.0, -3.0), (10.0, -3.0));
        let a0 = (-3.0f64).atan2(4.0);
        let Shape::Arc { start, sweep, .. } = extend(&a, v2(5.0, 0.0), &[&down]).unwrap() else { panic!() };
        assert!((start - (TAU + a0)).abs() < 1e-9 && (sweep - (FRAC_PI_4 - a0)).abs() < 1e-9);
        let c = Shape::Circle { center: v2(0.0, 0.0), radius: 5.0 };
        assert!(matches!(extend(&c, v2(5.0, 0.0), &[&up]), Err(GeomError::Unsupported(_))));
    }

    #[test]
    fn trim_of_text_is_unsupported() {
        let t = Shape::Text { pos: DVec2::ZERO, height: 1.0, angle: 0.0, text: "a".into() };
        assert!(matches!(trim(&t, DVec2::ZERO, &[]), Err(GeomError::Unsupported(_))));
    }
}
