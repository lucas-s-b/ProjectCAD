use crate::annot::{dim_layout, fills, labels, strokes, text_box};
use crate::tol::Tolerance;
use crate::opening::{Opening, OpeningKind};
use crate::roof::{Roof, RoofKind};
use crate::room::{Room, Slab};
use crate::stair::Stair;
use crate::wall::Wall;
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
    /// A straight wall; only in level sketches (see cad-document).
    Wall(Wall),
    /// A door or window in a wall; only in level sketches, hosted by a wall (see cad-document).
    Opening(Opening),
    /// A room: seed point, name and number; only in level sketches. Its area comes from the walls (see cad-arch).
    Room(Room),
    /// A floor slab below its level; only in level sketches.
    Slab(Slab),
    /// A roof on top of its level; only in level sketches. Its planes and solid come from `Roof::pieces` and cad-arch.
    Roof(Roof),
    /// A straight stair; only in level sketches.
    Stair(Stair),
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
            Shape::Wall(w) => w.a.is_finite() && w.b.is_finite() && w.thickness.is_finite() && w.height.is_finite(),
            Shape::Opening(o) => {
                o.at.is_finite()
                    && o.width.is_finite()
                    && o.height.is_finite()
                    && o.sill.is_finite()
                    && o.wall.a.is_finite()
                    && o.wall.b.is_finite()
                    && o.wall.thickness.is_finite()
                    && o.wall.height.is_finite()
            }
            Shape::Room(r) => r.seed.is_finite(),
            Shape::Slab(s) => s.thickness.is_finite() && s.outline.iter().all(|p| p.is_finite()),
            Shape::Roof(r) => r.pitch.is_finite() && r.thickness.is_finite() && r.overhang.is_finite() && r.outline.iter().all(|p| p.is_finite()),
            Shape::Stair(s) => s.a.is_finite() && s.b.is_finite() && s.width.is_finite() && s.height.is_finite(),
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
            Shape::Wall(w) if w.a.distance(w.b) <= t => Err(GeomError::Degenerate("wall has zero length")),
            Shape::Wall(w) if w.thickness <= t || w.height <= t => {
                Err(GeomError::Degenerate("wall thickness and height must be positive"))
            }
            Shape::Opening(o) if o.width <= t || o.height <= t || o.sill < 0.0 => {
                Err(GeomError::Degenerate("opening width and height must be positive and its sill not negative"))
            }
            Shape::Opening(o) => Shape::Wall(o.wall).validate(tol),
            Shape::Slab(s) if s.thickness <= t => Err(GeomError::Degenerate("slab thickness must be positive")),
            Shape::Slab(s) if s.outline.len() < 3 || s.area().abs() <= t * t => {
                Err(GeomError::Degenerate("slab outline needs three points and an area"))
            }
            Shape::Roof(r) if r.thickness <= t || r.overhang < 0.0 => {
                Err(GeomError::Degenerate("roof thickness must be positive and its overhang not negative"))
            }
            Shape::Roof(r) if r.outline.len() < 3 || r.area().abs() <= t * t => {
                Err(GeomError::Degenerate("roof outline needs three points and an area"))
            }
            Shape::Roof(r) if r.kind != RoofKind::Flat && !(r.pitch > 0.0 && r.pitch <= 75.0) => {
                Err(GeomError::Degenerate("roof pitch must be above 0 and at most 75 degrees"))
            }
            Shape::Roof(r) if matches!(r.kind, RoofKind::Gable | RoofKind::Hip) && !r.is_convex() => {
                Err(GeomError::Degenerate("gable and hip roofs need a convex outline"))
            }
            Shape::Stair(s) if s.a.distance(s.b) <= t || s.width <= t || s.height <= t => {
                Err(GeomError::Degenerate("stair needs a run, a width and a height"))
            }
            Shape::Stair(s) if s.risers < 2 => Err(GeomError::Degenerate("stair needs at least two risers")),
            _ => Ok(()),
        }
    }

    /// Shapes that live only in level sketches (an opening is held there by its host wall instead).
    pub fn needs_level(&self) -> bool {
        matches!(self, Shape::Wall(_) | Shape::Room(_) | Shape::Slab(_) | Shape::Roof(_) | Shape::Stair(_))
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
            Shape::Wall(_) => "Wall",
            Shape::Opening(o) => match o.kind {
                OpeningKind::Door => "Door",
                OpeningKind::Window => "Window",
            },
            Shape::Room(_) => "Room",
            Shape::Slab(_) => "Floor",
            Shape::Roof(_) => "Roof",
            Shape::Stair(_) => "Stair",
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
            Shape::Wall(w) => {
                w.a += d2;
                w.b += d2;
            }
            Shape::Opening(o) => {
                o.wall.a += d2;
                o.wall.b += d2;
            }
            Shape::Room(r) => r.seed += d2,
            Shape::Slab(s) => s.outline.iter_mut().for_each(|p| *p += d2),
            Shape::Roof(r) => r.outline.iter_mut().for_each(|p| *p += d2),
            Shape::Stair(s) => {
                s.a += d2;
                s.b += d2;
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
            Shape::Wall(w) => {
                let c = w.corners();
                let lo = c.iter().fold(DVec2::INFINITY, |m, p| m.min(*p));
                let hi = c.iter().fold(DVec2::NEG_INFINITY, |m, p| m.max(*p));
                (lo.extend(0.0), hi.extend(0.0))
            }
            Shape::Opening(o) => {
                let c = o.corners();
                let lo = c.iter().fold(DVec2::INFINITY, |m, p| m.min(*p));
                let hi = c.iter().fold(DVec2::NEG_INFINITY, |m, p| m.max(*p));
                (lo.extend(0.0), hi.extend(0.0))
            }
            Shape::Room(r) => (r.seed.extend(0.0), r.seed.extend(0.0)),
            Shape::Slab(s) => {
                let lo = s.outline.iter().fold(DVec2::INFINITY, |m, p| m.min(*p));
                let hi = s.outline.iter().fold(DVec2::NEG_INFINITY, |m, p| m.max(*p));
                (lo.extend(0.0), hi.extend(0.0))
            }
            Shape::Roof(r) => {
                let e = r.eaves();
                let lo = e.iter().fold(DVec2::INFINITY, |m, p| m.min(*p));
                let hi = e.iter().fold(DVec2::NEG_INFINITY, |m, p| m.max(*p));
                (lo.extend(0.0), hi.extend(0.0))
            }
            Shape::Stair(s) => {
                let c = s.corners();
                let lo = c.iter().fold(DVec2::INFINITY, |m, p| m.min(*p));
                let hi = c.iter().fold(DVec2::NEG_INFINITY, |m, p| m.max(*p));
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
