use crate::shape::{GeomError, Shape};
use glam::DVec2;
use std::f64::consts::PI;

type Joined = (Shape, Shape, Option<Shape>);

struct Corner {
    x: DVec2,
    da: DVec2,
    db: DVec2,
    len_a: f64,
    len_b: f64,
}

/// Direction from `x` toward the picked side of a line, and how far the line reaches that way.
fn side_of(x: DVec2, (p, q): (DVec2, DVec2), pick: DVec2) -> (DVec2, f64) {
    let d = (q - p).normalize();
    let proj = p + d * (pick - p).dot(d);
    let toward = if proj.distance(x) > 1e-9 {
        (proj - x).normalize()
    } else if p.distance(x) > q.distance(x) {
        (p - x).normalize()
    } else {
        (q - x).normalize()
    };
    (toward, (p - x).dot(toward).max((q - x).dot(toward)))
}

fn corner(a: &Shape, pick_a: DVec2, b: &Shape, pick_b: DVec2) -> Result<Corner, GeomError> {
    let (Shape::Line { a: a1, b: a2 }, Shape::Line { a: b1, b: b2 }) = (a, b) else {
        return Err(GeomError::Unsupported("fillet and chamfer need two lines"));
    };
    let (d1, d2) = (*a2 - *a1, *b2 - *b1);
    let cross = d1.perp_dot(d2);
    if cross.abs() <= 1e-12 * d1.length() * d2.length() {
        return Err(GeomError::NoIntersection);
    }
    let x = *a1 + d1 * ((*b1 - *a1).perp_dot(d2) / cross);
    let (da, len_a) = side_of(x, (*a1, *a2), pick_a);
    let (db, len_b) = side_of(x, (*b1, *b2), pick_b);
    Ok(Corner { x, da, db, len_a, len_b })
}

/// Both lines cut back by `t` from the corner; the caller adds the connecting shape.
fn cut_back(c: &Corner, t: f64) -> Result<(Shape, Shape), GeomError> {
    if c.len_a <= t + 1e-9 || c.len_b <= t + 1e-9 {
        return Err(GeomError::Degenerate("radius or distance too large"));
    }
    let line = |d: DVec2, len: f64| Shape::Line { a: c.x + d * t, b: c.x + d * len };
    Ok((line(c.da, c.len_a), line(c.db, c.len_b)))
}

pub fn fillet(a: &Shape, pick_a: DVec2, b: &Shape, pick_b: DVec2, radius: f64) -> Result<Joined, GeomError> {
    if !radius.is_finite() || radius < 0.0 {
        return Err(GeomError::Degenerate("fillet radius"));
    }
    let c = corner(a, pick_a, b, pick_b)?;
    if radius == 0.0 {
        let (la, lb) = cut_back(&c, 0.0)?;
        return Ok((la, lb, None));
    }
    let theta = c.da.dot(c.db).clamp(-1.0, 1.0).acos();
    let t = radius / (theta / 2.0).tan();
    let (la, lb) = cut_back(&c, t)?;
    let centre = c.x + (c.da + c.db).normalize() * (radius / (theta / 2.0).sin());
    let (ta, tb) = (c.x + c.da * t, c.x + c.db * t);
    let arc = Shape::arc_through(centre, ta, tb);
    let short = matches!(arc, Shape::Arc { sweep, .. } if sweep <= PI);
    Ok((la, lb, Some(if short { arc } else { Shape::arc_through(centre, tb, ta) })))
}

pub fn chamfer(a: &Shape, pick_a: DVec2, b: &Shape, pick_b: DVec2, distance: f64) -> Result<Joined, GeomError> {
    if !distance.is_finite() || distance <= 0.0 {
        return Err(GeomError::Degenerate("chamfer distance"));
    }
    let c = corner(a, pick_a, b, pick_b)?;
    let (la, lb) = cut_back(&c, distance)?;
    let third = Shape::Line { a: c.x + c.da * distance, b: c.x + c.db * distance };
    Ok((la, lb, Some(third)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

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
        let Shape::Line { a, b } = s else { panic!("{s:?}") };
        (*a, *b)
    }

    #[test]
    fn fillet_right_angle() {
        let (a, b) = (line((0.0, 0.0), (10.0, 0.0)), line((0.0, 0.0), (0.0, 10.0)));
        let (la, lb, arc) = fillet(&a, v2(5.0, 0.0), &b, v2(0.0, 5.0), 2.0).unwrap();
        assert!(near(ends(&la).0, v2(2.0, 0.0)) && near(ends(&la).1, v2(10.0, 0.0)));
        assert!(near(ends(&lb).0, v2(0.0, 2.0)) && near(ends(&lb).1, v2(0.0, 10.0)));
        let Some(Shape::Arc { center, radius, start, sweep }) = arc else { panic!() };
        assert!(near(center, v2(2.0, 2.0)) && (radius - 2.0).abs() < 1e-9);
        assert!((start - PI).abs() < 1e-9 && (sweep - FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn fillet_crossing_picks_quadrant() {
        let (a, b) = (line((-10.0, 0.0), (10.0, 0.0)), line((0.0, -10.0), (0.0, 10.0)));
        let (la, lb, arc) = fillet(&a, v2(5.0, 0.0), &b, v2(0.0, 5.0), 2.0).unwrap();
        assert!(near(ends(&la).0, v2(2.0, 0.0)) && near(ends(&lb).0, v2(0.0, 2.0)));
        let Some(Shape::Arc { center, sweep, .. }) = arc else { panic!() };
        assert!(near(center, v2(2.0, 2.0)) && (sweep - FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn fillet_extends_short_lines() {
        let (a, b) = (line((3.0, 0.0), (10.0, 0.0)), line((0.0, 3.0), (0.0, 10.0)));
        let (la, lb, _) = fillet(&a, v2(5.0, 0.0), &b, v2(0.0, 5.0), 2.0).unwrap();
        assert!(near(ends(&la).0, v2(2.0, 0.0)) && near(ends(&lb).0, v2(0.0, 2.0)));
    }

    #[test]
    fn fillet_zero_radius_and_errors() {
        let (a, b) = (line((1.0, 0.0), (10.0, 0.0)), line((0.0, 1.0), (0.0, 10.0)));
        let (la, lb, arc) = fillet(&a, v2(5.0, 0.0), &b, v2(0.0, 5.0), 0.0).unwrap();
        assert!(arc.is_none() && near(ends(&la).0, v2(0.0, 0.0)) && near(ends(&lb).0, v2(0.0, 0.0)));
        let (a, b) = (line((0.0, 0.0), (3.0, 0.0)), line((0.0, 0.0), (0.0, 3.0)));
        let r = fillet(&a, v2(1.0, 0.0), &b, v2(0.0, 1.0), 5.0);
        assert!(matches!(r, Err(GeomError::Degenerate(_))));
        let p = line((0.0, 1.0), (10.0, 1.0));
        assert_eq!(fillet(&a, v2(1.0, 0.0), &p, v2(1.0, 1.0), 1.0), Err(GeomError::NoIntersection));
        let c = Shape::Circle { center: v2(0.0, 0.0), radius: 1.0 };
        assert!(matches!(fillet(&a, v2(1.0, 0.0), &c, v2(1.0, 0.0), 1.0), Err(GeomError::Unsupported(_))));
    }

    #[test]
    fn fillet_sixty_degrees() {
        let a = line((0.0, 0.0), (10.0, 0.0));
        let b = line((0.0, 0.0), (5.0, 5.0 * 3.0f64.sqrt()));
        let (_, _, arc) = fillet(&a, v2(5.0, 0.0), &b, v2(2.5, 2.5 * 3.0f64.sqrt()), 1.0).unwrap();
        let t = 1.0 / (PI / 6.0).tan();
        let Some(Shape::Arc { center, radius, start, sweep }) = arc else { panic!() };
        let at = |ang: f64| center + DVec2::from_angle(ang) * radius;
        let (p, q) = (at(start), at(start + sweep));
        let (ta, tb) = (v2(t, 0.0), DVec2::from_angle(PI / 3.0) * t);
        assert!((near(p, ta) && near(q, tb)) || (near(p, tb) && near(q, ta)));
        assert!(sweep <= PI);
    }

    #[test]
    fn chamfer_cases() {
        let (a, b) = (line((0.0, 0.0), (10.0, 0.0)), line((0.0, 0.0), (0.0, 10.0)));
        let (la, lb, third) = chamfer(&a, v2(5.0, 0.0), &b, v2(0.0, 5.0), 3.0).unwrap();
        assert!(near(ends(&la).0, v2(3.0, 0.0)) && near(ends(&lb).0, v2(0.0, 3.0)));
        assert_eq!(ends(&third.unwrap()), (v2(3.0, 0.0), v2(0.0, 3.0)));
        assert!(matches!(chamfer(&a, v2(5.0, 0.0), &b, v2(0.0, 5.0), 0.0), Err(GeomError::Degenerate(_))));
    }
}
