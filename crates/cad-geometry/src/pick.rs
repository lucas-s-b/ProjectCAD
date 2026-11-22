use crate::mesh::{outline, tessellate};
use crate::annot::{inside_polygon, text_box};
use crate::shape::Shape;
use glam::{DVec2, DVec3};

pub fn point_segment_distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 == 0.0 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

pub fn distance_2d(shape: &Shape, p: DVec2) -> f64 {
    if let Shape::Room(r) = shape {
        return p.distance(r.seed);
    }
    if let Shape::Hatch { boundary, .. } = shape
        && inside_polygon(boundary, p)
    {
        return 0.0;
    }
    if let Shape::Text { pos, height, angle, text } = shape {
        let c = text_box(*pos, *height, *angle, text);
        // inside a convex box every edge has the point on its left
        if (0..4).all(|i| (c[(i + 1) % 4] - c[i]).perp_dot(p - c[i]) >= 0.0) {
            return 0.0;
        }
    }
    let corners = match shape {
        Shape::Wall(w) => Some(w.corners()),
        Shape::Opening(o) => Some(o.corners()),
        _ => None,
    };
    if let Some(c) = corners
        && (0..4).all(|i| (c[(i + 1) % 4] - c[i]).perp_dot(p - c[i]) >= 0.0)
    {
        return 0.0;
    }
    outline(shape)
        .iter()
        .map(|[a, b]| point_segment_distance(p, a.truncate(), b.truncate()))
        .fold(f64::INFINITY, f64::min)
}

// shortcut: tessellates on every call and scans all triangles, add a mesh cache + BVH when the selection benchmark needs it
pub fn ray_hit(shape: &Shape, origin: DVec3, dir: DVec3) -> Option<f64> {
    let mesh = tessellate(shape)?;
    let vtx = |i: usize| DVec3::from_array(mesh.positions[i].map(f64::from));
    let mut best: Option<f64> = None;
    for tri in 0..mesh.positions.len() / 3 {
        let (v0, v1, v2) = (vtx(tri * 3), vtx(tri * 3 + 1), vtx(tri * 3 + 2));
        let (e1, e2) = (v1 - v0, v2 - v0);
        let h = dir.cross(e2);
        let det = e1.dot(h);
        if det.abs() < 1e-12 {
            continue;
        }
        let s = origin - v0;
        let u = s.dot(h) / det;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = s.cross(e1);
        let v = dir.dot(q) / det;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let t = e2.dot(q) / det;
        if t > 0.0 && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v2(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }

    #[test]
    fn segment_distance() {
        let (a, b) = (v2(0.0, 0.0), v2(10.0, 0.0));
        assert_eq!(point_segment_distance(v2(-3.0, 4.0), a, b), 5.0);
        assert_eq!(point_segment_distance(v2(5.0, 2.0), a, b), 2.0);
        assert_eq!(point_segment_distance(v2(3.0, 4.0), a, a), 5.0);
    }

    #[test]
    fn circle_distance_inside_and_outside() {
        let c = Shape::Circle { center: v2(0.0, 0.0), radius: 10.0 };
        assert!((distance_2d(&c, v2(0.0, 0.0)) - 10.0).abs() < 0.1);
        assert!((distance_2d(&c, v2(13.0, 0.0)) - 3.0).abs() < 0.05);
        assert!((distance_2d(&c, v2(6.0, 0.0)) - 4.0).abs() < 0.05);
    }

    #[test]
    fn ray_against_box() {
        let b = Shape::Box { center: DVec3::ZERO, size: DVec3::splat(2.0) };
        let t = ray_hit(&b, DVec3::new(0.0, 0.0, 10.0), DVec3::NEG_Z).unwrap();
        assert!((t - 9.0).abs() < 1e-6);
        assert!(ray_hit(&b, DVec3::new(3.0, 0.0, 10.0), DVec3::NEG_Z).is_none());
        assert!(ray_hit(&b, DVec3::new(0.0, 0.0, 10.0), DVec3::Z).is_none());
        let c = Shape::Circle { center: v2(0.0, 0.0), radius: 1.0 };
        assert!(ray_hit(&c, DVec3::new(0.0, 0.0, 10.0), DVec3::NEG_Z).is_none());
    }
}

#[cfg(test)]
mod text_tests {
    use super::*;

    #[test]
    fn inside_text_box_is_zero() {
        let t = Shape::Text { pos: DVec2::ZERO, height: 10.0, angle: 0.0, text: "abc".into() };
        assert_eq!(distance_2d(&t, DVec2::new(9.0, 5.0)), 0.0);
        assert!((distance_2d(&t, DVec2::new(20.0, 5.0)) - 2.0).abs() < 1e-9);
    }
}
