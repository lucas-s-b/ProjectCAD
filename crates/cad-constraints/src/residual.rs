use crate::types::{Constraint, Id, PointKind, PointRef, SolveError};
use cad_geometry::shape::norm_angle;
use cad_geometry::{DVec2, Shape};
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::{PI, TAU};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    Line,
    Circle,
    Arc,
}

/// Entity id -> (offset into the variable vector, kind). Line `[a.x, a.y, b.x, b.y]`,
/// Circle `[c.x, c.y, r]`, Arc `[c.x, c.y, r, start, sweep]`.
#[derive(Debug)]
pub(crate) struct Layout {
    entries: BTreeMap<Id, (usize, Kind)>,
    n: usize,
}

impl Layout {
    pub(crate) fn len(&self) -> usize {
        self.n
    }

    fn at(&self, id: Id) -> (usize, Kind) {
        self.entries[&id]
    }
}

fn bad(index: usize, reason: &'static str) -> SolveError {
    SolveError::BadReference { index, reason }
}

fn kind_of(shapes: &BTreeMap<Id, Shape>, index: usize, id: Id) -> Result<Kind, SolveError> {
    match shapes.get(&id) {
        None => Err(bad(index, "entity not found")),
        Some(Shape::Line { .. }) => Ok(Kind::Line),
        Some(Shape::Circle { .. }) => Ok(Kind::Circle),
        Some(Shape::Arc { .. }) => Ok(Kind::Arc),
        Some(_) => Err(bad(index, "shape cannot be constrained")),
    }
}

fn validate(index: usize, c: &Constraint, shapes: &BTreeMap<Id, Shape>) -> Result<(), SolveError> {
    let k = |id: Id| kind_of(shapes, index, id);
    for id in c.entities() {
        k(id)?;
    }
    let line = |id: Id| -> Result<(), SolveError> {
        if k(id)? == Kind::Line { Ok(()) } else { Err(bad(index, "wrong shape kind")) }
    };
    let curve = |id: Id| -> Result<(), SolveError> {
        if k(id)? != Kind::Line { Ok(()) } else { Err(bad(index, "wrong shape kind")) }
    };
    let point = |p: PointRef| -> Result<(), SolveError> {
        let ok = matches!(
            (k(p.entity)?, p.kind),
            (Kind::Line, PointKind::Start | PointKind::End) | (Kind::Circle, PointKind::Center) | (Kind::Arc, _)
        );
        if ok { Ok(()) } else { Err(bad(index, "no such point")) }
    };
    let distinct = |a: Id, b: Id| -> Result<(), SolveError> {
        if a != b { Ok(()) } else { Err(bad(index, "same entity twice")) }
    };
    let finite = |v: f64| -> Result<(), SolveError> {
        if v.is_finite() { Ok(()) } else { Err(bad(index, "bad value")) }
    };
    let positive = |v: f64| -> Result<(), SolveError> {
        if v.is_finite() && v > 0.0 { Ok(()) } else { Err(bad(index, "bad value")) }
    };
    match c {
        Constraint::Coincident(p, q) => {
            point(*p)?;
            point(*q)?;
            if p == q { Err(bad(index, "same entity twice")) } else { Ok(()) }
        }
        Constraint::Distance(p, q, v) => {
            point(*p)?;
            point(*q)?;
            if p == q {
                return Err(bad(index, "same entity twice"));
            }
            positive(*v)
        }
        Constraint::Horizontal(l) | Constraint::Vertical(l) => line(*l),
        Constraint::Parallel(a, b) | Constraint::Perpendicular(a, b) | Constraint::Collinear(a, b) => {
            line(*a)?;
            line(*b)?;
            distinct(*a, *b)
        }
        Constraint::Angle(a, b, v) => {
            line(*a)?;
            line(*b)?;
            distinct(*a, *b)?;
            finite(*v)
        }
        Constraint::Tangent(a, b) => {
            if k(*a)? == Kind::Line && k(*b)? == Kind::Line {
                return Err(bad(index, "wrong shape kind"));
            }
            distinct(*a, *b)
        }
        Constraint::Concentric(a, b) => {
            curve(*a)?;
            curve(*b)?;
            distinct(*a, *b)
        }
        Constraint::Equal(a, b) => {
            if (k(*a)? == Kind::Line) != (k(*b)? == Kind::Line) {
                return Err(bad(index, "wrong shape kind"));
            }
            distinct(*a, *b)
        }
        Constraint::Symmetric(p, q, l) => {
            point(*p)?;
            point(*q)?;
            line(*l)
        }
        Constraint::Fixed { point: p, at } => {
            point(*p)?;
            finite(at.x)?;
            finite(at.y)
        }
        Constraint::Radius(e, v) | Constraint::Diameter(e, v) => {
            curve(*e)?;
            positive(*v)
        }
    }
}

/// Only entities referenced by a constraint get variables, in ascending id order.
pub(crate) fn layout(shapes: &BTreeMap<Id, Shape>, constraints: &[Constraint]) -> Result<(Layout, Vec<f64>), SolveError> {
    let mut used = BTreeSet::new();
    for (i, c) in constraints.iter().enumerate() {
        validate(i, c, shapes)?;
        used.extend(c.entities());
    }
    let mut entries = BTreeMap::new();
    let mut x = Vec::new();
    for id in used {
        let offset = x.len();
        match &shapes[&id] {
            Shape::Line { a, b } => {
                entries.insert(id, (offset, Kind::Line));
                x.extend([a.x, a.y, b.x, b.y]);
            }
            Shape::Circle { center, radius } => {
                entries.insert(id, (offset, Kind::Circle));
                x.extend([center.x, center.y, *radius]);
            }
            Shape::Arc { center, radius, start, sweep } => {
                entries.insert(id, (offset, Kind::Arc));
                x.extend([center.x, center.y, *radius, *start, *sweep]);
            }
            _ => unreachable!("validated above"),
        }
    }
    let n = x.len();
    Ok((Layout { entries, n }, x))
}

struct Ctx<'a> {
    l: &'a Layout,
    x: &'a [f64],
}

impl Ctx<'_> {
    fn v(&self, o: usize) -> DVec2 {
        DVec2::new(self.x[o], self.x[o + 1])
    }

    fn point(&self, p: PointRef) -> DVec2 {
        let (o, kind) = self.l.at(p.entity);
        match (kind, p.kind) {
            (Kind::Line, PointKind::Start) => self.v(o),
            (Kind::Line, PointKind::End) => self.v(o + 2),
            (_, PointKind::Center) => self.v(o),
            (Kind::Arc, PointKind::Start) => self.v(o) + self.x[o + 2] * DVec2::from_angle(self.x[o + 3]),
            (Kind::Arc, PointKind::End) => self.v(o) + self.x[o + 2] * DVec2::from_angle(self.x[o + 3] + self.x[o + 4]),
            _ => unreachable!("validated in layout"),
        }
    }

    fn line(&self, id: Id) -> (DVec2, DVec2) {
        let (o, _) = self.l.at(id);
        (self.v(o), self.v(o + 2))
    }

    fn dir(&self, id: Id) -> DVec2 {
        let (a, b) = self.line(id);
        b - a
    }

    fn curve(&self, id: Id) -> (DVec2, f64) {
        let (o, _) = self.l.at(id);
        (self.v(o), self.x[o + 2])
    }

    fn is_line(&self, id: Id) -> bool {
        self.l.at(id).1 == Kind::Line
    }

    fn parallel(&self, l1: Id, l2: Id) -> f64 {
        let (d1, d2) = (self.dir(l1), self.dir(l2));
        cross(d1, d2) / (d1.length() * d2.length())
    }
}

fn cross(u: DVec2, v: DVec2) -> f64 {
    u.x * v.y - u.y * v.x
}

/// Into (-PI, PI].
fn wrap(a: f64) -> f64 {
    let w = a.rem_euclid(TAU);
    if w > PI { w - TAU } else { w }
}

/// Replaces `out` with the equations of all constraints, in constraint order.
pub(crate) fn residuals(layout: &Layout, modes: &[bool], constraints: &[Constraint], x: &[f64], out: &mut Vec<f64>) {
    out.clear();
    let cx = Ctx { l: layout, x };
    for (i, c) in constraints.iter().enumerate() {
        match c {
            Constraint::Coincident(p, q) => {
                let d = cx.point(*p) - cx.point(*q);
                out.extend([d.x, d.y]);
            }
            Constraint::Horizontal(l) => {
                let (a, b) = cx.line(*l);
                out.push(a.y - b.y);
            }
            Constraint::Vertical(l) => {
                let (a, b) = cx.line(*l);
                out.push(a.x - b.x);
            }
            Constraint::Parallel(l1, l2) => out.push(cx.parallel(*l1, *l2)),
            Constraint::Perpendicular(l1, l2) => {
                let (d1, d2) = (cx.dir(*l1), cx.dir(*l2));
                out.push(d1.dot(d2) / (d1.length() * d2.length()));
            }
            Constraint::Tangent(a, b) if cx.is_line(*a) || cx.is_line(*b) => {
                let (line, curve) = if cx.is_line(*a) { (*a, *b) } else { (*b, *a) };
                let d = cx.dir(line);
                let (c, r) = cx.curve(curve);
                out.push(cross(c - cx.line(line).0, d).abs() / d.length() - r);
            }
            Constraint::Tangent(a, b) => {
                let ((c1, r1), (c2, r2)) = (cx.curve(*a), cx.curve(*b));
                let rhs = if modes[i] { (r1 - r2).abs() } else { r1 + r2 };
                out.push(c1.distance(c2) - rhs);
            }
            Constraint::Concentric(a, b) => {
                let d = cx.curve(*a).0 - cx.curve(*b).0;
                out.extend([d.x, d.y]);
            }
            Constraint::Equal(a, b) if cx.is_line(*a) => out.push(cx.dir(*a).length() - cx.dir(*b).length()),
            Constraint::Equal(a, b) => out.push(cx.curve(*a).1 - cx.curve(*b).1),
            Constraint::Collinear(l1, l2) => {
                out.push(cx.parallel(*l1, *l2));
                let d1 = cx.dir(*l1);
                out.push(cross(cx.line(*l2).0 - cx.line(*l1).0, d1) / d1.length());
            }
            Constraint::Symmetric(p, q, l) => {
                let (a, d) = (cx.line(*l).0, cx.dir(*l));
                let (p, q) = (cx.point(*p), cx.point(*q));
                out.push(cross((p + q) / 2.0 - a, d) / d.length());
                out.push((q - p).dot(d) / d.length());
            }
            Constraint::Fixed { point, at } => {
                let d = cx.point(*point) - *at;
                out.extend([d.x, d.y]);
            }
            Constraint::Distance(p, q, v) => out.push(cx.point(*p).distance(cx.point(*q)) - v),
            Constraint::Angle(l1, l2, v) => {
                let (d1, d2) = (cx.dir(*l1), cx.dir(*l2));
                out.push(wrap(cross(d1, d2).atan2(d1.dot(d2)) - v));
            }
            Constraint::Radius(e, v) => out.push(cx.curve(*e).1 - v),
            Constraint::Diameter(e, v) => out.push(2.0 * cx.curve(*e).1 - v),
        }
    }
}

/// One flag per constraint, true = internal tangency of two circles/arcs. Decided once from the starting values.
pub(crate) fn tangent_modes(layout: &Layout, constraints: &[Constraint], x: &[f64]) -> Vec<bool> {
    let cx = Ctx { l: layout, x };
    constraints
        .iter()
        .map(|c| match c {
            Constraint::Tangent(a, b) if !cx.is_line(*a) && !cx.is_line(*b) => {
                let ((c1, r1), (c2, r2)) = (cx.curve(*a), cx.curve(*b));
                c1.distance(c2) < r1.max(r2)
            }
            _ => false,
        })
        .collect()
}

/// New shapes for the laid-out entities only.
pub(crate) fn write_back(layout: &Layout, x: &[f64], shapes: &BTreeMap<Id, Shape>) -> BTreeMap<Id, Shape> {
    let cx = Ctx { l: layout, x };
    let mut out = BTreeMap::new();
    for id in shapes.keys().filter(|id| layout.entries.contains_key(id)) {
        let (o, kind) = layout.at(*id);
        let shape = match kind {
            Kind::Line => Shape::Line { a: cx.v(o), b: cx.v(o + 2) },
            Kind::Circle => Shape::Circle { center: cx.v(o), radius: x[o + 2] },
            Kind::Arc => Shape::Arc { center: cx.v(o), radius: x[o + 2], start: norm_angle(x[o + 3]), sweep: x[o + 4] },
        };
        out.insert(*id, shape);
    }
    out
}
