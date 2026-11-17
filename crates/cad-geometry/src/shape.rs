use crate::annot::{dim_layout, fills, labels, strokes, text_box};
use crate::tol::Tolerance;
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, TAU};

/// How a hatch fills its boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HatchPattern {
    #[default]
    Lines,
    /// The lines plus the same set turned by 90 degrees.
    Cross,
    /// A filled region, no lines.
    Solid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Line { a: DVec2, b: DVec2 },
    Circle { center: DVec2, radius: f64 },
    Rect { min: DVec2, max: DVec2 },
    Box { center: DVec3, size: DVec3 },
    /// Axis +Z, `center` is the centroid.
    Cylinder { center: DVec3, radius: f64, height: f64 },
    Sphere { center: DVec3, radius: f64 },
    /// Radians, runs counter-clockwise from angle `start`. 0 < sweep <= TAU.
    Arc { center: DVec2, radius: f64, start: f64, sweep: f64 },
    Polyline { points: Vec<DVec2>, closed: bool },
    /// `pos` is the left end of the baseline; `angle` in radians, counter-clockwise.
    Text { pos: DVec2, height: f64, angle: f64, text: String },
    /// Aligned linear dimension; the dimension line is `offset` to the left of a->b (negative = right).
    Dimension { a: DVec2, b: DVec2, offset: f64, text_height: f64 },
    /// `angle` is the direction of the `rx` axis, radians.
    Ellipse { center: DVec2, rx: f64, ry: f64, angle: f64 },
    /// Passes through every point.
    Spline { points: Vec<DVec2>, closed: bool },
    /// Closed polygon filled by `pattern`: parallel lines at `angle` (radians) every `spacing`, two crossing sets, or solid.
    Hatch {
        boundary: Vec<DVec2>,
        angle: f64,
        spacing: f64,
        #[serde(default)]
        pattern: HatchPattern,
    },
    /// Arrow at `points[0]`, text at the last point.
    Leader { points: Vec<DVec2>, text: String, text_height: f64 },
    /// Dimension of a circle, attached at `angle`; `diameter` selects Ø instead of R.
    RadialDim { center: DVec2, radius: f64, angle: f64, text_height: f64, diameter: bool },
    /// Angle `sweep` from direction `start`, arc drawn at `radius`.
    AngularDim { center: DVec2, start: f64, sweep: f64, radius: f64, text_height: f64 },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GeomError {
    #[error("non-finite coordinate")]
    NonFinite,
    #[error("degenerate geometry: {0}")]
    Degenerate(&'static str),
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
    #[error("no intersection found")]
    NoIntersection,
}

/// Angle in [0, TAU). `rem_euclid` alone returns TAU for a tiny negative input.
pub fn norm_angle(a: f64) -> f64 {
    let r = a.rem_euclid(TAU);
    if r >= TAU { 0.0 } else { r }
}

impl Shape {
    pub fn rect(p: DVec2, q: DVec2) -> Shape {
        Shape::Rect { min: p.min(q), max: p.max(q) }
    }

    /// Arc from `center` starting at `start_pt`, counter-clockwise to the angle of `end_pt`.
    pub fn arc_through(center: DVec2, start_pt: DVec2, end_pt: DVec2) -> Shape {
        let d = start_pt - center;
        let start = norm_angle(d.y.atan2(d.x));
        let e = end_pt - center;
        let mut sweep = (e.y.atan2(e.x) - start).rem_euclid(TAU);
        if sweep < 1e-9 {
            sweep = TAU;
        }
        Shape::Arc { center, radius: d.length(), start, sweep }
    }

    pub fn validate(&self, tol: &Tolerance) -> Result<(), GeomError> {
        let t = tol.linear;
        let finite = match self {
            Shape::Line { a, b } => a.is_finite() && b.is_finite(),
            Shape::Circle { center, radius } => center.is_finite() && radius.is_finite(),
            Shape::Rect { min, max } => min.is_finite() && max.is_finite(),
            Shape::Box { center, size } => center.is_finite() && size.is_finite(),
            Shape::Cylinder { center, radius, height } => {
                center.is_finite() && radius.is_finite() && height.is_finite()
            }
            Shape::Sphere { center, radius } => center.is_finite() && radius.is_finite(),
            Shape::Arc { center, radius, start, sweep } => {
                center.is_finite() && radius.is_finite() && start.is_finite() && sweep.is_finite()
            }
            Shape::Polyline { points, .. } => points.iter().all(|p| p.is_finite()),
            Shape::Text { pos, height, angle, .. } => pos.is_finite() && height.is_finite() && angle.is_finite(),
            Shape::Dimension { a, b, offset, text_height } => {
                a.is_finite() && b.is_finite() && offset.is_finite() && text_height.is_finite()
            }
            Shape::Ellipse { center, rx, ry, angle } => {
                center.is_finite() && rx.is_finite() && ry.is_finite() && angle.is_finite()
            }
            Shape::Spline { points, .. } => points.iter().all(|p| p.is_finite()),
            Shape::Hatch { boundary, angle, spacing, .. } => {
                boundary.iter().all(|p| p.is_finite()) && angle.is_finite() && spacing.is_finite()
            }
            Shape::Leader { points, text_height, .. } => points.iter().all(|p| p.is_finite()) && text_height.is_finite(),
            Shape::RadialDim { center, radius, angle, text_height, .. } => {
                center.is_finite() && radius.is_finite() && angle.is_finite() && text_height.is_finite()
            }
            Shape::AngularDim { center, start, sweep, radius, text_height } => {
                center.is_finite() && start.is_finite() && sweep.is_finite() && radius.is_finite() && text_height.is_finite()
            }
        };
        if !finite {
            return Err(GeomError::NonFinite);
        }
        match self {
            Shape::Line { a, b } if a.distance(*b) <= t => {
                Err(GeomError::Degenerate("line has zero length"))
            }
            Shape::Circle { radius, .. } | Shape::Sphere { radius, .. } if *radius <= t => {
                Err(GeomError::Degenerate("radius must be positive"))
            }
            Shape::Cylinder { radius, height, .. } if *radius <= t || *height <= t => {
                Err(GeomError::Degenerate("radius and height must be positive"))
            }
            Shape::Rect { min, max } if max.x - min.x <= t || max.y - min.y <= t => {
                Err(GeomError::Degenerate("rectangle has no area"))
            }
            Shape::Box { size, .. } if size.min_element() <= t => {
                Err(GeomError::Degenerate("box size must be positive"))
            }
            Shape::Arc { radius, .. } if *radius <= t => {
                Err(GeomError::Degenerate("radius must be positive"))
            }
            Shape::Arc { sweep, .. } if *sweep <= 1e-9 || *sweep > TAU + 1e-9 => {
                Err(GeomError::Degenerate("arc sweep out of range"))
            }
            Shape::Text { height, .. } if *height <= t => Err(GeomError::Degenerate("text height must be positive")),
            Shape::Text { text, .. } if text.trim().is_empty() => Err(GeomError::Degenerate("empty text")),
            Shape::Dimension { a, b, .. } if a.distance(*b) <= t => {
                Err(GeomError::Degenerate("dimension has zero length"))
            }
            Shape::Dimension { text_height, .. } if *text_height <= t => {
                Err(GeomError::Degenerate("text height must be positive"))
            }
            Shape::Polyline { points, closed } => {
                if points.len() < if *closed { 3 } else { 2 } {
                    return Err(GeomError::Degenerate("polyline has too few points"));
                }
                let n = points.len();
                let segs = if *closed { n } else { n - 1 };
                if (0..segs).any(|i| points[i].distance(points[(i + 1) % n]) <= t) {
                    return Err(GeomError::Degenerate("polyline has a zero-length segment"));
                }
                Ok(())
            }
            Shape::Ellipse { rx, ry, .. } if *rx <= t || *ry <= t => {
                Err(GeomError::Degenerate("ellipse radii must be positive"))
            }
            Shape::Spline { points, closed } => {
                if points.len() < if *closed { 3 } else { 2 } {
                    return Err(GeomError::Degenerate("spline has too few points"));
                }
                let n = points.len();
                let segs = if *closed { n } else { n - 1 };
                if (0..segs).any(|i| points[i].distance(points[(i + 1) % n]) <= t) {
                    return Err(GeomError::Degenerate("spline has coincident points"));
                }
                Ok(())
            }
            Shape::Hatch { boundary, spacing, .. } => {
                let n = boundary.len();
                if n < 3 {
                    return Err(GeomError::Degenerate("hatch has too few points"));
                }
                if (0..n).any(|i| boundary[i].distance(boundary[(i + 1) % n]) <= t) {
                    return Err(GeomError::Degenerate("hatch has coincident points"));
                }
                if *spacing <= t {
                    return Err(GeomError::Degenerate("hatch spacing must be positive"));
                }
                let lo = boundary.iter().fold(DVec2::INFINITY, |m, p| m.min(*p));
                let hi = boundary.iter().fold(DVec2::NEG_INFINITY, |m, p| m.max(*p));
                if lo.distance(hi) / spacing > 5000.0 {
                    return Err(GeomError::Degenerate("hatch spacing too small"));
                }
                Ok(())
            }
            Shape::Leader { points, text, text_height } => {
                if points.len() < 2 {
                    return Err(GeomError::Degenerate("leader has too few points"));
                }
                if points.windows(2).any(|w| w[0].distance(w[1]) <= t) {
                    return Err(GeomError::Degenerate("leader has coincident points"));
                }
                if *text_height <= t {
                    return Err(GeomError::Degenerate("text height must be positive"));
                }
                if text.trim().is_empty() {
                    return Err(GeomError::Degenerate("empty text"));
                }
                Ok(())
            }
            Shape::RadialDim { radius, text_height, .. } if *radius <= t || *text_height <= t => {
                Err(GeomError::Degenerate("radius and text height must be positive"))
            }
            Shape::AngularDim { radius, text_height, .. } if *radius <= t || *text_height <= t => {
                Err(GeomError::Degenerate("radius and text height must be positive"))
            }
            Shape::AngularDim { sweep, .. } if *sweep <= 1e-9 || *sweep > TAU + 1e-9 => {
                Err(GeomError::Degenerate("angle sweep out of range"))
            }
            _ => Ok(()),
        }
    }

    pub fn is_solid(&self) -> bool {
        matches!(self, Shape::Box { .. } | Shape::Cylinder { .. } | Shape::Sphere { .. })
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Shape::Line { .. } => "Line",
            Shape::Circle { .. } => "Circle",
            Shape::Rect { .. } => "Rect",
            Shape::Box { .. } => "Box",
            Shape::Cylinder { .. } => "Cylinder",
            Shape::Sphere { .. } => "Sphere",
            Shape::Arc { .. } => "Arc",
            Shape::Polyline { .. } => "Polyline",
            Shape::Text { .. } => "Text",
            Shape::Dimension { .. } => "Dimension",
            Shape::Ellipse { .. } => "Ellipse",
            Shape::Spline { .. } => "Spline",
            Shape::Hatch { .. } => "Hatch",
            Shape::Leader { .. } => "Leader",
            Shape::RadialDim { .. } => "Radial dimension",
            Shape::AngularDim { .. } => "Angular dimension",
        }
    }

    pub fn translate(&mut self, d: DVec3) {
        let d2 = d.truncate();
        match self {
            Shape::Line { a, b } => {
                *a += d2;
                *b += d2;
            }
            Shape::Circle { center, .. }
            | Shape::Arc { center, .. }
            | Shape::Ellipse { center, .. }
            | Shape::RadialDim { center, .. }
            | Shape::AngularDim { center, .. } => *center += d2,
            Shape::Hatch { boundary: points, .. } | Shape::Leader { points, .. } => points.iter_mut().for_each(|p| *p += d2),
            Shape::Polyline { points, .. } | Shape::Spline { points, .. } => points.iter_mut().for_each(|p| *p += d2),
            Shape::Text { pos, .. } => *pos += d2,
            Shape::Dimension { a, b, .. } => {
                *a += d2;
                *b += d2;
            }
            Shape::Rect { min, max } => {
                *min += d2;
                *max += d2;
            }
            Shape::Box { center, .. }
            | Shape::Cylinder { center, .. }
            | Shape::Sphere { center, .. } => *center += d,
        }
    }

    pub fn bbox(&self) -> (DVec3, DVec3) {
        match self {
            Shape::Line { a, b } => (a.min(*b).extend(0.0), a.max(*b).extend(0.0)),
            Shape::Circle { center, radius } => {
                let r = DVec2::splat(*radius);
                ((*center - r).extend(0.0), (*center + r).extend(0.0))
            }
            Shape::Rect { min, max } => (min.extend(0.0), max.extend(0.0)),
            Shape::Box { center, size } => (*center - *size / 2.0, *center + *size / 2.0),
            Shape::Cylinder { center, radius, height } => {
                let h = DVec3::new(*radius, *radius, height / 2.0);
                (*center - h, *center + h)
            }
            Shape::Sphere { center, radius } => {
                let r = DVec3::splat(*radius);
                (*center - r, *center + r)
            }
            Shape::Arc { center, radius, start, sweep } => {
                let at = |a: f64| *center + DVec2::new(a.cos(), a.sin()) * *radius;
                let mut lo = at(*start).min(at(start + sweep));
                let mut hi = at(*start).max(at(start + sweep));
                for k in 0..4 {
                    let a = FRAC_PI_2 * k as f64;
                    if (a - start).rem_euclid(TAU) <= *sweep {
                        lo = lo.min(at(a));
                        hi = hi.max(at(a));
                    }
                }
                (lo.extend(0.0), hi.extend(0.0))
            }
            Shape::Polyline { points, .. } => {
                let lo = points.iter().fold(DVec2::INFINITY, |m, p| m.min(*p));
                let hi = points.iter().fold(DVec2::NEG_INFINITY, |m, p| m.max(*p));
                (lo.extend(0.0), hi.extend(0.0))
            }
            Shape::Text { pos, height, angle, text } => {
                let c = text_box(*pos, *height, *angle, text);
                let lo = c.iter().fold(DVec2::INFINITY, |m, p| m.min(*p));
                let hi = c.iter().fold(DVec2::NEG_INFINITY, |m, p| m.max(*p));
                (lo.extend(0.0), hi.extend(0.0))
            }
            Shape::Dimension { a, b, offset, text_height } => {
                let l = dim_layout(*a, *b, *offset, *text_height);
                let g = DVec2::splat(*text_height);
                let pts = [*a, *b, l.line[0], l.line[1]];
                let lo = pts.iter().fold(l.text_center - g, |m, p| m.min(*p));
                let hi = pts.iter().fold(l.text_center + g, |m, p| m.max(*p));
                (lo.extend(0.0), hi.extend(0.0))
            }
            Shape::Ellipse { center, rx, ry, angle } => {
                let (s, c) = angle.sin_cos();
                let h = DVec2::new((rx * rx * c * c + ry * ry * s * s).sqrt(), (rx * rx * s * s + ry * ry * c * c).sqrt());
                ((*center - h).extend(0.0), (*center + h).extend(0.0))
            }
            Shape::Hatch { .. } | Shape::Leader { .. } | Shape::RadialDim { .. } | Shape::AngularDim { .. } => {
                let mut pts: Vec<DVec2> = strokes(self).into_iter().flatten().chain(fills(self).into_iter().flatten()).collect();
                let labels = labels(self);
                pts.extend(labels.iter().map(|l| l.anchor));
                let grow = DVec2::splat(labels.iter().fold(0.0, |m, l| f64::max(m, l.height)));
                let lo = pts.iter().fold(DVec2::INFINITY, |m, p| m.min(*p)) - grow;
                let hi = pts.iter().fold(DVec2::NEG_INFINITY, |m, p| m.max(*p)) + grow;
                (lo.extend(0.0), hi.extend(0.0))
            }
            Shape::Spline { points, closed } => {
                // shortcut: bbox from the 16-per-span flattening, can miss the true extreme by a hair
                let path = crate::curve::spline_path(points, *closed, 16);
                let lo = path.iter().fold(DVec2::INFINITY, |m, p| m.min(*p));
                let hi = path.iter().fold(DVec2::NEG_INFINITY, |m, p| m.max(*p));
                (lo.extend(0.0), hi.extend(0.0))
            }
        }
    }
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
