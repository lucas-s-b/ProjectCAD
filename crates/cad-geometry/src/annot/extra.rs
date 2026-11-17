//! Layout of the hatch, leader, radial and angular dimension shapes.
use super::triangulate::triangulate;
use super::{Align, Label, LabelText};
use crate::shape::{HatchPattern, Shape};
use glam::DVec2;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
#[cfg(test)]
use std::f64::consts::FRAC_PI_4;

const ARC_SEGMENTS: f64 = 64.0;

pub(super) struct Parts {
    pub strokes: Vec<Vec<DVec2>>,
    pub fills: Vec<[DVec2; 3]>,
    pub labels: Vec<Label>,
}

/// Parallel lines at `angle` (radians) every `spacing`, aligned to the origin, clipped to the polygon (even-odd).
pub fn hatch_lines(boundary: &[DVec2], angle: f64, spacing: f64) -> Vec<[DVec2; 2]> {
    let n = boundary.len();
    if n < 3 || spacing.is_nan() || spacing <= 0.0 {
        return Vec::new();
    }
    let (fwd, back) = (DVec2::from_angle(angle), DVec2::from_angle(-angle));
    let pts: Vec<DVec2> = boundary.iter().map(|p| back.rotate(*p)).collect();
    let lo = pts.iter().fold(f64::INFINITY, |m, p| m.min(p.y));
    let hi = pts.iter().fold(f64::NEG_INFINITY, |m, p| m.max(p.y));
    let mut out = Vec::new();
    for k in (lo / spacing).ceil() as i64..=(hi / spacing).floor() as i64 {
        let y = k as f64 * spacing;
        let mut xs: Vec<f64> = (0..n)
            .filter_map(|i| {
                let (p, q) = (pts[i], pts[(i + 1) % n]);
                ((p.y <= y) != (q.y <= y)).then(|| p.x + (y - p.y) / (q.y - p.y) * (q.x - p.x))
            })
            .collect();
        xs.sort_by(f64::total_cmp);
        for w in xs.as_chunks::<2>().0.iter().filter(|w| w[1] - w[0] > 1e-12) {
            out.push([fwd.rotate(DVec2::new(w[0], y)), fwd.rotate(DVec2::new(w[1], y))]);
        }
    }
    out
}

/// Even-odd point-in-polygon test.
pub fn inside_polygon(poly: &[DVec2], p: DVec2) -> bool {
    let n = poly.len();
    (0..n).fold(false, |inside, i| {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y <= p.y) != (b.y <= p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) { !inside } else { inside }
    })
}

/// Arrow head with its tip at `tip`, body toward `along` (unit vector); length `h`, half-width `h / 6`.
fn arrow(tip: DVec2, along: DVec2, h: f64) -> [DVec2; 3] {
    let base = tip + along * h;
    let w = along.perp() * h / 6.0;
    [tip, base + w, base - w]
}

fn label(text: LabelText, anchor: DVec2, align: Align, angle: f64, height: f64) -> Label {
    Label { text, anchor, align, angle, height }
}

pub(super) fn parts(shape: &Shape) -> Option<Parts> {
    let mut p = Parts { strokes: Vec::new(), fills: Vec::new(), labels: Vec::new() };
    match shape {
        Shape::Hatch { boundary, angle, spacing, pattern } => {
            p.strokes.push(boundary.iter().chain(boundary.first()).copied().collect());
            let turns = match pattern {
                HatchPattern::Lines => vec![*angle],
                HatchPattern::Cross => vec![*angle, angle + FRAC_PI_2],
                HatchPattern::Solid => {
                    p.fills = triangulate(boundary);
                    Vec::new()
                }
            };
            for a in turns {
                p.strokes.extend(hatch_lines(boundary, a, *spacing).iter().map(|l| l.to_vec()));
            }
        }
        Shape::Leader { points, text, text_height } => {
            if points.len() < 2 {
                return None;
            }
            let h = *text_height;
            let (n, last) = (points.len(), points[points.len() - 1]);
            p.strokes.push(points.clone());
            p.fills.push(arrow(points[0], (points[1] - points[0]).normalize_or_zero(), h));
            let right = last.x - points[n - 2].x >= 0.0;
            let (dx, align) = if right { (0.5 * h, Align::Left) } else { (-0.5 * h, Align::Right) };
            p.labels.push(label(LabelText::Literal(text.clone()), last + DVec2::new(dx, -0.35 * h), align, 0.0, h));
        }
        Shape::RadialDim { center, radius, angle, text_height, diameter } => {
            let (h, dir) = (*text_height, DVec2::from_angle(*angle));
            let tip = *center + dir * *radius;
            p.strokes.push(vec![if *diameter { *center - dir * *radius } else { *center }, tip]);
            p.fills.push(arrow(tip, -dir, h));
            let text = if *diameter { LabelText::Diameter(2.0 * radius) } else { LabelText::Radius(*radius) };
            let align = if dir.x >= 0.0 { Align::Left } else { Align::Right };
            p.labels.push(label(text, tip + dir * h + DVec2::new(0.0, -0.35 * h), align, 0.0, h));
        }
        Shape::AngularDim { center, start, sweep, radius, text_height } => {
            let h = *text_height;
            let at = |a: f64| *center + DVec2::from_angle(a) * *radius;
            let n = ((ARC_SEGMENTS * sweep / TAU).ceil() as usize).max(2);
            p.strokes.push((0..=n).map(|i| at(start + sweep * i as f64 / n as f64)).collect());
            let end = start + sweep;
            // outward = along the arc direction beyond each end; the body lies the other way
            p.fills.push(arrow(at(*start), DVec2::from_angle(start + FRAC_PI_2), h));
            p.fills.push(arrow(at(end), DVec2::from_angle(end - FRAC_PI_2), h));
            let mid = start + sweep / 2.0;
            let anchor = *center + DVec2::from_angle(mid) * (radius + 0.9 * h);
            p.labels.push(label(LabelText::Angle(*sweep), anchor, Align::Center, half_turn(mid - FRAC_PI_2), h));
        }
        _ => return None,
    }
    Some(p)
}

/// Angle equivalent mod PI, in (-PI/2, PI/2], so text never reads upside-down.
pub(super) fn half_turn(a: f64) -> f64 {
    let mut a = (a + PI).rem_euclid(TAU) - PI;
    if a > FRAC_PI_2 + 1e-9 {
        a -= PI;
    } else if a <= -FRAC_PI_2 + 1e-9 {
        a += PI;
    }
    a
}

#[cfg(test)]
#[path = "extra_tests.rs"]
mod tests;
