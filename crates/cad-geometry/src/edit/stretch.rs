//! Stretch: the defining points of a shape that lie inside a window move by a displacement; the rest stay. A shape
//! whose points are all inside moves whole. Circles, arcs, ellipses, text and radial or angular dimensions move whole
//! when their anchor (centre or insertion point) is inside, as in AutoCAD.
use crate::shape::Shape;
use glam::DVec2;

/// `shape` stretched: points for which `inside` holds move by `d`. None when nothing of it moves.
pub fn stretch(shape: &Shape, inside: impl Fn(DVec2) -> bool, d: DVec2) -> Option<Shape> {
    let mv = |p: DVec2| if inside(p) { p + d } else { p };
    let whole = || {
        let mut s = shape.clone();
        s.translate(d.extend(0.0));
        Some(s)
    };
    let points = |pts: &[DVec2]| -> Option<Vec<DVec2>> { pts.iter().any(|p| inside(*p)).then(|| pts.iter().map(|p| mv(*p)).collect()) };
    match shape {
        Shape::Line { a, b } => (inside(*a) || inside(*b)).then(|| Shape::Line { a: mv(*a), b: mv(*b) }),
        Shape::Polyline { points: p, closed } => points(p).map(|points| Shape::Polyline { points, closed: *closed }),
        Shape::Spline { points: p, closed } => points(p).map(|points| Shape::Spline { points, closed: *closed }),
        Shape::Leader { points: p, text, text_height } => points(p).map(|points| Shape::Leader { points, text: text.clone(), text_height: *text_height }),
        Shape::Hatch { boundary, angle, spacing, pattern } => {
            points(boundary).map(|boundary| Shape::Hatch { boundary, angle: *angle, spacing: *spacing, pattern: *pattern })
        }
        Shape::Dimension { a, b, offset, text_height } => {
            (inside(*a) || inside(*b)).then(|| Shape::Dimension { a: mv(*a), b: mv(*b), offset: *offset, text_height: *text_height })
        }
        Shape::Wall(w) => (inside(w.a) || inside(w.b)).then(|| {
            let mut w = *w;
            (w.a, w.b) = (mv(w.a), mv(w.b));
            Shape::Wall(w)
        }),
        Shape::Rect { min, max } => {
            let c = [*min, DVec2::new(max.x, min.y), *max, DVec2::new(min.x, max.y)];
            let moved = c.map(mv);
            if moved == c {
                return None;
            }
            // Still a rectangle when every corner moved or one whole side did.
            let axis_aligned = moved[0].y == moved[1].y && moved[2].y == moved[3].y && moved[0].x == moved[3].x && moved[1].x == moved[2].x;
            Some(if axis_aligned { Shape::rect(moved[0], moved[2]) } else { Shape::Polyline { points: moved.to_vec(), closed: true } })
        }
        Shape::Circle { center, .. }
        | Shape::Arc { center, .. }
        | Shape::Ellipse { center, .. }
        | Shape::RadialDim { center, .. }
        | Shape::AngularDim { center, .. } => inside(*center).then(whole).flatten(),
        Shape::Text { pos, .. } => inside(*pos).then(whole).flatten(),
        Shape::Room(r) => inside(r.seed).then(whole).flatten(),
        Shape::XLine { point, .. } => inside(*point).then(whole).flatten(),
        // shortcut: solids, openings, slabs and stairs are left alone; stretch them when users ask
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn left_half(p: DVec2) -> bool {
        p.x < 5.0
    }

    #[test]
    fn line_end_inside_moves() {
        let l = Shape::Line { a: DVec2::ZERO, b: DVec2::new(10.0, 0.0) };
        let s = stretch(&l, left_half, DVec2::new(-2.0, 1.0)).unwrap();
        assert_eq!(s, Shape::Line { a: DVec2::new(-2.0, 1.0), b: DVec2::new(10.0, 0.0) });
        assert_eq!(stretch(&l, |_| false, DVec2::X), None);
    }

    #[test]
    fn rect_side_stays_a_rect_and_a_corner_makes_a_polyline() {
        let r = Shape::rect(DVec2::ZERO, DVec2::new(10.0, 4.0));
        let s = stretch(&r, left_half, DVec2::new(-3.0, 0.0)).unwrap();
        assert_eq!(s, Shape::rect(DVec2::new(-3.0, 0.0), DVec2::new(10.0, 4.0)));
        let corner = |p: DVec2| p.x < 5.0 && p.y < 2.0;
        let Shape::Polyline { points, closed } = stretch(&r, corner, DVec2::new(-3.0, 0.0)).unwrap() else { panic!("not a polyline") };
        assert!(closed);
        assert_eq!(points[0], DVec2::new(-3.0, 0.0));
        assert_eq!(points[3], DVec2::new(0.0, 4.0));
    }

    #[test]
    fn polyline_points_inside_move_and_circles_follow_their_centre() {
        let p = Shape::Polyline { points: vec![DVec2::ZERO, DVec2::new(10.0, 0.0), DVec2::new(10.0, 10.0)], closed: false };
        let Shape::Polyline { points, .. } = stretch(&p, left_half, DVec2::Y).unwrap() else { panic!() };
        assert_eq!(points, vec![DVec2::Y, DVec2::new(10.0, 0.0), DVec2::new(10.0, 10.0)]);
        let c = Shape::Circle { center: DVec2::new(1.0, 1.0), radius: 20.0 };
        assert_eq!(stretch(&c, left_half, DVec2::X), Some(Shape::Circle { center: DVec2::new(2.0, 1.0), radius: 20.0 }));
        let far = Shape::Circle { center: DVec2::new(9.0, 1.0), radius: 2.0 };
        assert_eq!(stretch(&far, left_half, DVec2::X), None);
    }
}
