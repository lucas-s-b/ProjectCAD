use crate::extrude::{Flat, cap_loops, flatten, seg_curve};
use crate::{ModelError, Plane3};
use cad_geometry::profile::{Profile, Seg};
use cad_topology::{Brep, CoEdge, Curve, Edge, Face, Surface, TOL};
use glam::{DQuat, DVec2, DVec3};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// Smallest and largest signed distance (left of `dir` positive) of the segment from the axis line.
fn side_range(seg: &Seg, axis_point: DVec2, dir: DVec2) -> (f64, f64) {
    let s = |p: DVec2| dir.perp_dot(p - axis_point);
    let (a, b) = (s(seg.start()), s(seg.end()));
    let (mut lo, mut hi) = (a.min(b), a.max(b));
    if let Seg::Arc { center, radius, start, sweep } = seg {
        // s(t) = s(centre) + r sin(t - phi): extreme at phi +- PI / 2.
        let phi = dir.y.atan2(dir.x);
        for t in [phi + FRAC_PI_2, phi - FRAC_PI_2] {
            let off = if *sweep >= 0.0 { (t - start).rem_euclid(TAU) } else { (start - t).rem_euclid(TAU) };
            if off <= sweep.abs() {
                let v = s(*center + DVec2::from_angle(t) * *radius);
                (lo, hi) = (lo.min(v), hi.max(v));
            }
        }
    }
    (lo, hi)
}

/// Surface swept by a segment and whether its natural normal points out of the solid.
fn swept_surface(seg: &Seg, plane: &Plane3, axis_point: DVec2, dir: DVec2) -> (Surface, bool) {
    let (n, a3) = (plane.normal(), plane.point(axis_point));
    let d3 = plane.x * dir.x + plane.y * dir.y;
    let to3 = |v: DVec2| plane.x * v.x + plane.y * v.y;
    let dist = |p: DVec2| dir.perp_dot(p - axis_point).abs();
    // `out` is the in-plane direction pointing out of the profile region (to the right of the travel).
    let (surface, mid, out) = match seg {
        Seg::Line { a, b } => {
            let t = (*b - *a).normalize();
            let (c, d) = (dir.perp_dot(t), dir.dot(t));
            let surface = if c.abs() < 1e-9 {
                Surface::Cylinder { origin: a3, axis: d3, radius: dist(*a) }
            } else if d.abs() < 1e-9 {
                Surface::Plane { origin: plane.point(*a), normal: d3 }
            } else {
                let apex = *a - t * (dir.perp_dot(*a - axis_point) / c);
                let axis = if dir.dot((*a + *b) / 2.0 - apex) >= 0.0 { d3 } else { -d3 };
                Surface::Cone { apex: plane.point(apex), axis, half_angle: c.abs().atan2(d.abs()) }
            };
            (surface, seg.point(0.5), to3(t).cross(n))
        }
        Seg::Arc { center, radius, sweep, .. } => {
            let mid = seg.point(0.5);
            let surface = if dist(*center) < 1e-6 {
                Surface::Sphere { center: plane.point(*center), radius: *radius }
            } else {
                let foot = a3 + d3 * d3.dot(plane.point(*center) - a3);
                Surface::Torus { center: foot, axis: d3, major: dist(*center), minor: *radius }
            };
            (surface, mid, to3((mid - *center) / *radius) * sweep.signum())
        }
    };
    let outward = surface.normal_at(plane.point(mid)).dot(out) > 0.0;
    (surface, outward)
}

/// Revolves the profile about an axis lying in the profile plane. `angle` in radians, 0 < angle <= TAU.
pub fn revolve(profile: &Profile, plane: &Plane3, axis_point: DVec2, axis_dir: DVec2, angle: f64) -> Result<Brep, ModelError> {
    if !(angle > 1e-9 && angle <= TAU + 1e-9) {
        return Err(ModelError::Invalid("revolve angle out of range"));
    }
    if !(axis_dir.length() >= TOL && axis_point.is_finite()) {
        return Err(ModelError::Invalid("revolve axis is degenerate"));
    }
    let dir = axis_dir.normalize();
    let Flat { segs, nxt, ranges } = flatten(profile);
    let (lo, hi) = segs.iter().map(|s| side_range(s, axis_point, dir)).fold((f64::INFINITY, f64::NEG_INFINITY), |m, r| (m.0.min(r.0), m.1.max(r.1)));
    if !(lo > 1e-6 || hi < -1e-6) {
        return Err(ModelError::Invalid("profile must lie on one side of the axis without touching it"));
    }
    // Rotation is right-handed about the axis, so a profile on the left sweeps toward +normal; on the right toward -normal.
    let sigma = if dir.perp_dot(profile.centroid() - axis_point) > 0.0 { 1.0 } else { -1.0 };

    let full = angle >= TAU - 1e-9;
    let angle = if full { TAU } else { angle };
    let steps = (angle / PI - 1e-9).ceil().max(1.0) as usize;
    let stations = if full { steps } else { steps + 1 };
    let step = angle / steps as f64;
    let m = segs.len();
    let (n, a3) = (plane.normal(), plane.point(axis_point));
    let d3 = plane.x * dir.x + plane.y * dir.y;
    let rot = |k: usize| DQuat::from_axis_angle(d3, step * k as f64);
    let at = |k: usize, p: DVec3| a3 + rot(k) * (p - a3);
    // Vertex (station, profile vertex), edge (station, segment) and rotation edge (step, profile vertex) numbering.
    let vid = |st: usize, i: usize| ((st % stations) * m + i) as u32;
    let seg_edge = |st: usize, i: usize| ((st % stations) * m + i) as u32;
    let rot_edge = |j: usize, i: usize| ((stations + j) * m + i) as u32;

    let base: Vec<DVec3> = segs.iter().map(|s| plane.point(s.start())).collect();
    let mut vertices = Vec::with_capacity(stations * m);
    let mut edges = Vec::with_capacity((stations + steps) * m);
    for st in 0..stations {
        vertices.extend(base.iter().map(|p| at(st, *p)));
    }
    for st in 0..stations {
        for (i, s) in segs.iter().enumerate() {
            let center = if let Seg::Arc { center, .. } = s { at(st, plane.point(*center)) } else { DVec3::ZERO };
            edges.push(Edge { v: [vid(st, i), vid(st, nxt[i])], curve: seg_curve(s, center, rot(st) * n) });
        }
    }
    for j in 0..steps {
        for (i, p) in base.iter().enumerate() {
            let curve = Curve::Arc { center: a3 + d3 * d3.dot(*p - a3), normal: d3, sweep: step };
            edges.push(Edge { v: [vid(j, i), vid(j + 1, i)], curve });
        }
    }

    let mut faces = Vec::new();
    if !full {
        for (st, outward) in [(0, sigma < 0.0), (steps, sigma > 0.0)] {
            let surface = Surface::Plane { origin: at(st, plane.origin), normal: rot(st) * n };
            faces.push(Face { surface, outward, loops: cap_loops(&ranges, st * m, outward) });
        }
    }
    let swept: Vec<_> = segs.iter().map(|s| swept_surface(s, plane, axis_point, dir)).collect();
    let c = |edge, reversed| CoEdge { edge, reversed };
    for j in 0..steps {
        for (i, (surface, outward)) in swept.iter().enumerate() {
            let (bottom, top, vp, vq) = (seg_edge(j, i), seg_edge(j + 1, i), rot_edge(j, i), rot_edge(j, nxt[i]));
            // Same ring as in `extrude` with the sweep direction as "up".
            let ring = if sigma > 0.0 {
                vec![c(bottom, false), c(vq, false), c(top, true), c(vp, true)]
            } else {
                vec![c(vp, false), c(top, false), c(vq, true), c(bottom, true)]
            };
            faces.push(Face { surface: surface.clone(), outward: *outward, loops: vec![ring] });
        }
    }
    let brep = Brep { vertices, edges, faces };
    brep.validate()?;
    Ok(brep)
}
