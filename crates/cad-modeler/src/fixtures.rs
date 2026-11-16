use crate::Plane3;
use cad_geometry::profile::Profile;
use cad_geometry::shape::Shape;
use glam::{DVec2, DVec3};
use std::f64::consts::{FRAC_PI_2, PI};

pub fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

pub fn make(shapes: &[Shape]) -> Profile {
    let refs: Vec<&Shape> = shapes.iter().collect();
    Profile::from_shapes(&refs).unwrap()
}

pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Shape {
    Shape::rect(v2(x0, y0), v2(x1, y1))
}

pub fn circle(cx: f64, cy: f64, r: f64) -> Shape {
    Shape::Circle { center: v2(cx, cy), radius: r }
}

pub fn rounded_rect(w: f64, h: f64, r: f64) -> Profile {
    let line = |a: (f64, f64), b: (f64, f64)| Shape::Line { a: v2(a.0, a.1), b: v2(b.0, b.1) };
    let arc = |cx: f64, cy: f64, start: f64| Shape::Arc { center: v2(cx, cy), radius: r, start, sweep: FRAC_PI_2 };
    make(&[
        line((r, 0.0), (w - r, 0.0)),
        arc(w - r, r, -FRAC_PI_2),
        line((w, r), (w, h - r)),
        arc(w - r, h - r, 0.0),
        line((w - r, h), (r, h)),
        arc(r, h - r, FRAC_PI_2),
        line((0.0, h - r), (0.0, r)),
        arc(r, r, PI),
    ])
}

pub fn holed_rect() -> Profile {
    make(&[rect(0.0, 0.0, 40.0, 20.0), circle(20.0, 10.0, 5.0)])
}

pub fn tilted() -> Plane3 {
    Plane3 {
        origin: DVec3::new(5.0, 5.0, 5.0),
        x: DVec3::new(1.0, 2.0, 2.0) / 3.0,
        y: DVec3::new(2.0, 1.0, -2.0) / 3.0,
    }
}

pub fn box_body(min: [f64; 3], max: [f64; 3]) -> crate::Body {
    let profile = make(&[rect(min[0], min[1], max[0], max[1])]);
    let plane = Plane3 { origin: DVec3::new(0.0, 0.0, min[2]), ..Plane3::XY };
    crate::Body::from_brep(&crate::extrude(&profile, &plane, max[2] - min[2]).unwrap(), 0.01).unwrap()
}

pub fn cylinder_body(cx: f64, cy: f64, r: f64, z0: f64, z1: f64) -> crate::Body {
    let plane = Plane3 { origin: DVec3::new(0.0, 0.0, z0), ..Plane3::XY };
    crate::Body::from_brep(&crate::extrude(&make(&[circle(cx, cy, r)]), &plane, z1 - z0).unwrap(), 0.01).unwrap()
}

/// Tessellation tolerance of the round-edge fixtures: the facet error of a rim must stay far below 1% of a 1 mm blend.
pub const FINE: f64 = 0.001;

fn fine_cylinder(cx: f64, cy: f64, r: f64, z0: f64, z1: f64) -> crate::Body {
    let plane = Plane3 { origin: DVec3::new(0.0, 0.0, z0), ..Plane3::XY };
    crate::Body::from_brep(&crate::extrude(&make(&[circle(cx, cy, r)]), &plane, z1 - z0).unwrap(), FINE).unwrap()
}

/// 40 x 20 x 10 plate with a through hole of radius `r` at (20, 10).
pub fn plate_with_hole(r: f64) -> crate::Body {
    let profile = make(&[rect(0.0, 0.0, 40.0, 20.0), circle(20.0, 10.0, r)]);
    crate::Body::from_brep(&crate::extrude(&profile, &Plane3::XY, 10.0).unwrap(), FINE).unwrap()
}

/// 40 x 20 x 10 plate with a round boss of radius `r` and height 10 on top, at (20, 10).
pub fn plate_with_boss(r: f64) -> crate::Body {
    let plate = box_body([0.0; 3], [40.0, 20.0, 10.0]);
    crate::boolean(&plate, &fine_cylinder(20.0, 10.0, r, 5.0, 20.0), crate::BoolOp::Union).unwrap()
}

/// 40 x 20 x 10 plate with a hole of radius `r` centred on its edge x = 40, so the top rim is a half circle.
pub fn plate_half_hole(r: f64) -> crate::Body {
    let plate = box_body([0.0; 3], [40.0, 20.0, 10.0]);
    crate::boolean(&plate, &fine_cylinder(40.0, 10.0, r, -5.0, 15.0), crate::BoolOp::Difference).unwrap()
}

/// L-shaped prism of height `h`: the concave corner edge is at (10, 10).
pub fn l_prism(h: f64) -> crate::Body {
    let pts = [(0.0, 0.0), (20.0, 0.0), (20.0, 10.0), (10.0, 10.0), (10.0, 20.0), (0.0, 20.0)];
    let lines: Vec<Shape> = (0..pts.len())
        .map(|i| Shape::Line { a: v2(pts[i].0, pts[i].1), b: v2(pts[(i + 1) % pts.len()].0, pts[(i + 1) % pts.len()].1) })
        .collect();
    crate::Body::from_brep(&crate::extrude(&make(&lines), &Plane3::XY, h).unwrap(), 0.01).unwrap()
}

/// Equilateral triangle prism, side 20, height `h`; every edge angle is 60 degrees.
pub fn triangle_prism(h: f64) -> crate::Body {
    let pts = [v2(0.0, 0.0), v2(20.0, 0.0), v2(10.0, 10.0 * 3.0_f64.sqrt())];
    let lines: Vec<Shape> = (0..3).map(|i| Shape::Line { a: pts[i], b: pts[(i + 1) % 3] }).collect();
    crate::Body::from_brep(&crate::extrude(&make(&lines), &Plane3::XY, h).unwrap(), 0.01).unwrap()
}
