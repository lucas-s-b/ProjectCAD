use crate::annot::{dim_layout, fills, strokes, text_box};
use crate::curve::{ellipse_point, spline_path};
use crate::shape::Shape;
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;

/// Triangle soup, 3 vertices per triangle.
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
}

const CYL_SEGMENTS: usize = 32;
const SPHERE_SEGMENTS: usize = 32;
const SPHERE_RINGS: usize = 16;
const OUTLINE_SEGMENTS: usize = 64;
const ELLIPSE_SEGMENTS: usize = 96;
const SPLINE_SEGMENTS: usize = 16;

impl Mesh {
    fn push(&mut self, p: DVec3, n: DVec3) {
        self.positions.push(p.as_vec3().to_array());
        self.normals.push(n.as_vec3().to_array());
    }

    fn tri_flat(&mut self, a: DVec3, b: DVec3, c: DVec3, n: DVec3) {
        for p in [a, b, c] {
            self.push(p, n);
        }
    }
}

pub fn tessellate(shape: &Shape) -> Option<Mesh> {
    let mut m = Mesh { positions: Vec::new(), normals: Vec::new() };
    match shape {
        Shape::Box { center, size } => {
            let h = *size / 2.0;
            for axis in 0..3 {
                for sign in [-1.0, 1.0] {
                    let mut n = DVec3::ZERO;
                    n[axis] = sign;
                    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                    let corner = |su: f64, sv: f64| {
                        let mut p = *center;
                        p[axis] += sign * h[axis];
                        p[u] += su * h[u];
                        p[v] += sv * h[v];
                        p
                    };
                    let (a, b, c, d) = (corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0));
                    m.tri_flat(a, b, c, n);
                    m.tri_flat(a, c, d, n);
                }
            }
        }
        Shape::Cylinder { center, radius, height } => {
            let hz = DVec3::new(0.0, 0.0, height / 2.0);
            let ring = |i: usize| {
                let t = TAU * (i % CYL_SEGMENTS) as f64 / CYL_SEGMENTS as f64;
                DVec3::new(t.cos(), t.sin(), 0.0)
            };
            for i in 0..CYL_SEGMENTS {
                let (r0, r1) = (ring(i), ring(i + 1));
                let (b0, b1) = (*center - hz + r0 * *radius, *center - hz + r1 * *radius);
                let (t0, t1) = (b0 + hz * 2.0, b1 + hz * 2.0);
                for (p, n) in [(b0, r0), (b1, r1), (t1, r1), (b0, r0), (t1, r1), (t0, r0)] {
                    m.push(p, n);
                }
                m.tri_flat(*center + hz, t0, t1, DVec3::Z);
                m.tri_flat(*center - hz, b1, b0, DVec3::NEG_Z);
            }
        }
        Shape::Sphere { center, radius } => {
            let pt = |i: usize, j: usize| {
                let phi = std::f64::consts::PI * i as f64 / SPHERE_RINGS as f64;
                let th = TAU * (j % SPHERE_SEGMENTS) as f64 / SPHERE_SEGMENTS as f64;
                DVec3::new(phi.sin() * th.cos(), phi.sin() * th.sin(), phi.cos())
            };
            for i in 0..SPHERE_RINGS {
                for j in 0..SPHERE_SEGMENTS {
                    let (a, b, c, d) = (pt(i, j), pt(i, j + 1), pt(i + 1, j + 1), pt(i + 1, j));
                    let mut quad = Vec::with_capacity(6);
                    if i > 0 {
                        quad.extend([a, d, b]);
                    }
                    if i + 1 < SPHERE_RINGS {
                        quad.extend([b, d, c]);
                    }
                    for n in quad {
                        m.push(*center + n * *radius, n);
                    }
                }
            }
        }
        _ => return None,
    }
    Some(m)
}

fn circle_pts(c: DVec3, r: f64) -> Vec<DVec3> {
    (0..OUTLINE_SEGMENTS)
        .map(|i| {
            let t = TAU * i as f64 / OUTLINE_SEGMENTS as f64;
            c + DVec3::new(r * t.cos(), r * t.sin(), 0.0)
        })
        .collect()
}

/// A closed path: the first point is repeated at the end.
fn ring(pts: &[DVec3]) -> Vec<DVec3> {
    pts.iter().chain(pts.first()).copied().collect()
}

/// Connected polylines of a shape. Closed shapes repeat their first point at the end.
pub fn outline_paths(shape: &Shape) -> Vec<Vec<DVec3>> {
    let mut out: Vec<Vec<DVec3>> = Vec::new();
    match shape {
        Shape::Line { a, b } => out.push(vec![a.extend(0.0), b.extend(0.0)]),
        Shape::XLine { point, dir, ray } => {
            let (a, b) = Shape::xline_ends(*point, *dir, *ray);
            out.push(vec![a.extend(0.0), b.extend(0.0)]);
        }
        Shape::Wall(w) => out.push(ring(&w.corners().map(|p| p.extend(0.0)))),
        Shape::Opening(o) => out.push(ring(&o.corners().map(|p| p.extend(0.0)))),
        Shape::Slab(s) => out.push(ring(&s.outline.iter().map(|p| p.extend(0.0)).collect::<Vec<_>>())),
        Shape::Room(_) => {}
        Shape::Roof(r) => out.extend(r.pieces().iter().map(|p| ring(&p.outline.iter().map(|q| q.extend(0.0)).collect::<Vec<_>>()))),
        Shape::Stair(s) => out.extend(s.symbol().into_iter().map(|[p, q]| vec![p.extend(0.0), q.extend(0.0)])),
        Shape::Rect { min, max } => {
            let c = [*min, DVec2::new(max.x, min.y), *max, DVec2::new(min.x, max.y)];
            out.push(ring(&c.map(|p| p.extend(0.0))));
        }
        Shape::Circle { center, radius } => out.push(ring(&circle_pts(center.extend(0.0), *radius))),
        Shape::Arc { center, radius, start, sweep } => {
            let n = ((OUTLINE_SEGMENTS as f64 * sweep / TAU).ceil() as usize).max(2);
            out.push(
                (0..=n)
                    .map(|i| {
                        let a = start + sweep * i as f64 / n as f64;
                        (*center + DVec2::new(a.cos(), a.sin()) * *radius).extend(0.0)
                    })
                    .collect(),
            );
        }
        Shape::Polyline { points, closed } => {
            let mut p: Vec<DVec3> = points.iter().map(|p| p.extend(0.0)).collect();
            if *closed {
                p.extend(p.first().copied());
            }
            out.push(p);
        }
        Shape::Text { pos, height, angle, text } => {
            out.push(ring(&text_box(*pos, *height, *angle, text).map(|p| p.extend(0.0))));
        }
        Shape::Dimension { a, b, offset, text_height } => {
            let l = dim_layout(*a, *b, *offset, *text_height);
            out.extend(l.ext.iter().chain([&l.line]).map(|[p, q]| vec![p.extend(0.0), q.extend(0.0)]));
            out.extend(l.arrows.iter().map(|t| ring(&t.map(|p| p.extend(0.0)))));
        }
        Shape::Hatch { .. } | Shape::Leader { .. } | Shape::RadialDim { .. } | Shape::AngularDim { .. } => {
            out.extend(strokes(shape).iter().map(|p| p.iter().map(|v| v.extend(0.0)).collect()));
            out.extend(fills(shape).iter().map(|t| ring(&t.map(|p| p.extend(0.0)))));
        }
        Shape::Ellipse { center, rx, ry, angle } => {
            let n = ELLIPSE_SEGMENTS;
            out.push(
                (0..=n)
                    .map(|i| ellipse_point(*center, *rx, *ry, *angle, TAU * i as f64 / n as f64).extend(0.0))
                    .collect(),
            );
        }
        Shape::Spline { points, closed } => {
            out.push(spline_path(points, *closed, SPLINE_SEGMENTS).iter().map(|p| p.extend(0.0)).collect());
        }
        Shape::Box { .. } => {
            let (lo, hi) = shape.bbox();
            let c = |x: bool, y: bool, z: bool| {
                DVec3::new(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z })
            };
            for (u, v) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
                let corner = |k: usize, z: bool| c(matches!(k, 1 | 2), matches!(k, 2 | 3), z);
                out.push(vec![corner(u, false), corner(v, false)]);
                out.push(vec![corner(u, true), corner(v, true)]);
                out.push(vec![corner(u, false), corner(u, true)]);
            }
        }
        Shape::Cylinder { center, radius, height } => {
            let hz = DVec3::new(0.0, 0.0, height / 2.0);
            out.push(ring(&circle_pts(*center - hz, *radius)));
            out.push(ring(&circle_pts(*center + hz, *radius)));
            for k in 0..4 {
                let t = TAU * k as f64 / 4.0;
                let o = DVec3::new(radius * t.cos(), radius * t.sin(), 0.0);
                out.push(vec![*center - hz + o, *center + hz + o]);
            }
        }
        Shape::Sphere { center, radius } => {
            for plane in 0..3 {
                let pts: Vec<DVec3> = circle_pts(DVec3::ZERO, *radius)
                    .into_iter()
                    .map(|p| match plane {
                        0 => p,
                        1 => DVec3::new(p.x, 0.0, p.y),
                        _ => DVec3::new(0.0, p.x, p.y),
                    } + *center)
                    .collect();
                out.push(ring(&pts));
            }
        }
    }
    out
}

/// Every consecutive pair of every path.
pub fn outline(shape: &Shape) -> Vec<[DVec3; 2]> {
    outline_paths(shape).iter().flat_map(|p| p.windows(2).map(|w| [w[0], w[1]])).collect()
}

#[cfg(test)]
#[path = "mesh_tests.rs"]
mod tests;
