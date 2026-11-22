use crate::shape::{GeomError, Shape};
use crate::opening::Opening;
use crate::wall::{Wall, WallAlign};
use glam::{DAffine2, DMat2, DVec2};

pub fn rotation_about(c: DVec2, angle: f64) -> DAffine2 {
    DAffine2::from_translation(c) * DAffine2::from_angle(angle) * DAffine2::from_translation(-c)
}

pub fn scale_about(c: DVec2, factor: f64) -> DAffine2 {
    DAffine2::from_translation(c) * DAffine2::from_scale(DVec2::splat(factor)) * DAffine2::from_translation(-c)
}

/// Reflection across the line through `p` with direction `dir`.
pub fn mirror_across(p: DVec2, dir: DVec2) -> DAffine2 {
    let d = dir.normalize();
    let (a, b) = (d.x * d.x - d.y * d.y, 2.0 * d.x * d.y);
    let m = DAffine2::from_mat2(DMat2::from_cols(DVec2::new(a, b), DVec2::new(b, -a)));
    DAffine2::from_translation(p) * m * DAffine2::from_translation(-p)
}

impl Shape {
    /// Applies a similarity transform (rotation, uniform scale, mirror, translation) to a 2D shape.
    pub fn transformed(&self, m: &DAffine2) -> Result<Shape, GeomError> {
        let (x, y) = (m.matrix2.x_axis, m.matrix2.y_axis);
        let s = x.length();
        if !(s > 0.0 && x.dot(y).abs() <= 1e-9 * s * s && (s - y.length()).abs() <= 1e-9 * s) {
            return Err(GeomError::Unsupported("non-uniform transform"));
        }
        let mirrored = m.matrix2.determinant() < 0.0;
        Ok(match self {
            Shape::Line { a, b } => Shape::Line { a: m.transform_point2(*a), b: m.transform_point2(*b) },
            Shape::Polyline { points, closed } => Shape::Polyline {
                points: points.iter().map(|p| m.transform_point2(*p)).collect(),
                closed: *closed,
            },
            Shape::Circle { center, radius } => {
                Shape::Circle { center: m.transform_point2(*center), radius: radius * s }
            }
            Shape::Arc { center, radius, start, sweep } => {
                // a mirror reverses direction, so the image of the old end becomes the new start
                let a = if mirrored { start + sweep } else { *start };
                let d = m.transform_vector2(DVec2::new(a.cos(), a.sin()));
                Shape::Arc {
                    center: m.transform_point2(*center),
                    radius: radius * s,
                    start: crate::shape::norm_angle(d.y.atan2(d.x)),
                    sweep: *sweep,
                }
            }
            Shape::Rect { min, max } => {
                if x.y.abs() <= 1e-9 * s || x.x.abs() <= 1e-9 * s {
                    Shape::rect(m.transform_point2(*min), m.transform_point2(*max))
                } else {
                    let c = [*min, DVec2::new(max.x, min.y), *max, DVec2::new(min.x, max.y)];
                    Shape::Polyline { points: c.map(|p| m.transform_point2(p)).to_vec(), closed: true }
                }
            }
            Shape::Text { pos, height, angle, text } => {
                // a mirrored text stays readable: only its position and direction change
                let d = m.transform_vector2(DVec2::from_angle(*angle));
                Shape::Text { pos: m.transform_point2(*pos), height: height * s, angle: d.to_angle(), text: text.clone() }
            }
            Shape::Dimension { a, b, offset, text_height } => Shape::Dimension {
                a: m.transform_point2(*a),
                b: m.transform_point2(*b),
                offset: if mirrored { -offset * s } else { offset * s },
                text_height: text_height * s,
            },
            Shape::Ellipse { center, rx, ry, angle } => {
                let d = m.transform_vector2(DVec2::from_angle(*angle));
                Shape::Ellipse { center: m.transform_point2(*center), rx: rx * s, ry: ry * s, angle: d.to_angle() }
            }
            Shape::Spline { points, closed } => {
                Shape::Spline { points: points.iter().map(|p| m.transform_point2(*p)).collect(), closed: *closed }
            }
            Shape::Hatch { boundary, angle, spacing, pattern } => Shape::Hatch {
                boundary: boundary.iter().map(|p| m.transform_point2(*p)).collect(),
                angle: m.transform_vector2(DVec2::from_angle(*angle)).to_angle(),
                spacing: spacing * s,
                pattern: *pattern,
            },
            Shape::Leader { points, text, text_height } => Shape::Leader {
                points: points.iter().map(|p| m.transform_point2(*p)).collect(),
                text: text.clone(),
                text_height: text_height * s,
            },
            Shape::RadialDim { center, radius, angle, text_height, diameter } => Shape::RadialDim {
                center: m.transform_point2(*center),
                radius: radius * s,
                angle: m.transform_vector2(DVec2::from_angle(*angle)).to_angle(),
                text_height: text_height * s,
                diameter: *diameter,
            },
            Shape::AngularDim { center, start, sweep, radius, text_height } => {
                // a mirror reverses direction, so the image of the old end becomes the new start
                let a = if mirrored { start + sweep } else { *start };
                Shape::AngularDim {
                    center: m.transform_point2(*center),
                    start: crate::shape::norm_angle(m.transform_vector2(DVec2::from_angle(a)).to_angle()),
                    sweep: *sweep,
                    radius: radius * s,
                    text_height: text_height * s,
                }
            }
            Shape::Wall(w) => Shape::Wall(wall_transformed(w, m, mirrored)),
            // The wall copy moves like the host (the document resets it afterwards); a mirror flips the swing side, the
            // hinge stays on the jamb toward the same wall end.
            Shape::Opening(o) => {
                Shape::Opening(Opening { wall: wall_transformed(&o.wall, m, mirrored), swing_left: o.swing_left != mirrored, ..*o })
            }
            Shape::Box { .. } | Shape::Cylinder { .. } | Shape::Sphere { .. } => {
                return Err(GeomError::Unsupported("2D transform of a solid"));
            }
        })
    }
}

/// A wall under `m`. Thickness and height are kept under scaling. A mirror turns the baseline around, so the wall
/// would land on the other side unless the side swaps too.
fn wall_transformed(w: &Wall, m: &DAffine2, mirrored: bool) -> Wall {
    let align = match w.align {
        WallAlign::Left if mirrored => WallAlign::Right,
        WallAlign::Right if mirrored => WallAlign::Left,
        a => a,
    };
    Wall { a: m.transform_point2(w.a), b: m.transform_point2(w.b), align, ..*w }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI};

    fn v2(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }
    fn close(a: DVec2, b: DVec2) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn rotate_line() {
        let l = Shape::Line { a: v2(1.0, 0.0), b: v2(2.0, 0.0) };
        let Shape::Line { a, b } = l.transformed(&rotation_about(DVec2::ZERO, FRAC_PI_2)).unwrap() else { panic!() };
        assert!(close(a, v2(0.0, 1.0)) && close(b, v2(0.0, 2.0)));
    }

    #[test]
    fn scale_circle() {
        let c = Shape::Circle { center: v2(3.0, 4.0), radius: 1.5 };
        let Shape::Circle { center, radius } = c.transformed(&scale_about(v2(3.0, 4.0), 2.0)).unwrap() else { panic!() };
        assert!(close(center, v2(3.0, 4.0)) && (radius - 3.0).abs() < 1e-12);
    }

    #[test]
    fn mirror_arc() {
        let arc = Shape::Arc { center: v2(1.0, 1.0), radius: 2.0, start: 0.3, sweep: 1.2 };
        let m = mirror_across(DVec2::ZERO, DVec2::X);
        let Shape::Arc { center, radius, start, sweep } = arc.transformed(&m).unwrap() else { panic!() };
        let Shape::Arc { center: c0, radius: r0, start: s0, sweep: w0 } = arc else { panic!() };
        let at = |c: DVec2, r: f64, a: f64| c + v2(a.cos(), a.sin()) * r;
        let mut ends = [at(center, radius, start), at(center, radius, start + sweep)];
        let mut want = [m.transform_point2(at(c0, r0, s0)), m.transform_point2(at(c0, r0, s0 + w0))];
        let key = |p: &DVec2| (p.x, p.y);
        ends.sort_by(|a, b| key(a).partial_cmp(&key(b)).unwrap());
        want.sort_by(|a, b| key(a).partial_cmp(&key(b)).unwrap());
        assert!(close(ends[0], want[0]) && close(ends[1], want[1]));
        assert!(close(at(center, radius, start + sweep / 2.0), m.transform_point2(at(c0, r0, s0 + w0 / 2.0))));
    }

    #[test]
    fn rotate_rect() {
        let r = Shape::rect(v2(0.0, 0.0), v2(2.0, 1.0));
        let q = r.transformed(&rotation_about(DVec2::ZERO, FRAC_PI_2)).unwrap();
        assert!(matches!(q, Shape::Rect { .. }));
        let p = r.transformed(&rotation_about(DVec2::ZERO, PI / 6.0)).unwrap();
        assert!(matches!(p, Shape::Polyline { ref points, closed: true } if points.len() == 4));
    }

    #[test]
    fn rejects_non_similarity_and_solids() {
        let l = Shape::Line { a: v2(0.0, 0.0), b: v2(1.0, 0.0) };
        let m = DAffine2::from_scale(v2(1.0, 2.0));
        assert!(matches!(l.transformed(&m), Err(GeomError::Unsupported(_))));
        let b = Shape::Box { center: glam::DVec3::ZERO, size: glam::DVec3::ONE };
        assert!(matches!(b.transformed(&DAffine2::IDENTITY), Err(GeomError::Unsupported(_))));
    }
}

#[cfg(test)]
mod annot_tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn text_rotates_and_scales() {
        let t = Shape::Text { pos: DVec2::new(1.0, 0.0), height: 2.0, angle: 0.0, text: "a".into() };
        let m = DAffine2::from_scale(DVec2::splat(2.0)) * rotation_about(DVec2::ZERO, FRAC_PI_2);
        let Shape::Text { pos, height, angle, .. } = t.transformed(&m).unwrap() else { panic!() };
        assert!(pos.distance(DVec2::new(0.0, 2.0)) < 1e-9 && (height - 4.0).abs() < 1e-9 && (angle - FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn mirrored_dimension_negates_offset() {
        let d = Shape::Dimension { a: DVec2::ZERO, b: DVec2::new(10.0, 0.0), offset: 3.0, text_height: 1.0 };
        let Shape::Dimension { offset, .. } = d.transformed(&mirror_across(DVec2::ZERO, DVec2::X)).unwrap() else { panic!() };
        assert_eq!(offset, -3.0);
    }
}
