use cad_geometry::DVec2;
use serde::{Deserialize, Serialize};

/// The document's `EntityId`.
pub type Id = u64;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PointKind {
    Start,
    End,
    Center,
}

/// Line: Start = a, End = b. Circle: Center. Arc: Center, Start, End (computed from centre, radius, angles).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointRef {
    pub entity: Id,
    pub kind: PointKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Constraint {
    Coincident(PointRef, PointRef),
    /// Line.
    Horizontal(Id),
    /// Line.
    Vertical(Id),
    /// Lines.
    Parallel(Id, Id),
    /// Lines.
    Perpendicular(Id, Id),
    /// Line with circle/arc, or two circles/arcs.
    Tangent(Id, Id),
    /// Circles/arcs.
    Concentric(Id, Id),
    /// Two lines (length) or two circles/arcs (radius).
    Equal(Id, Id),
    /// Lines.
    Collinear(Id, Id),
    /// Two points mirrored about a line.
    Symmetric(PointRef, PointRef, Id),
    Fixed { point: PointRef, at: DVec2 },
    Distance(PointRef, PointRef, f64),
    /// Radians from the first line's direction to the second's, in (-PI, PI].
    Angle(Id, Id, f64),
    Radius(Id, f64),
    Diameter(Id, f64),
}

impl Constraint {
    /// Every entity it references, without duplicates, in order of first appearance.
    pub fn entities(&self) -> Vec<Id> {
        let all = match self {
            Constraint::Coincident(p, q) | Constraint::Distance(p, q, _) => vec![p.entity, q.entity],
            Constraint::Horizontal(a) | Constraint::Vertical(a) | Constraint::Radius(a, _) | Constraint::Diameter(a, _) => {
                vec![*a]
            }
            Constraint::Parallel(a, b)
            | Constraint::Perpendicular(a, b)
            | Constraint::Tangent(a, b)
            | Constraint::Concentric(a, b)
            | Constraint::Equal(a, b)
            | Constraint::Collinear(a, b)
            | Constraint::Angle(a, b, _) => vec![*a, *b],
            Constraint::Symmetric(p, q, l) => vec![p.entity, q.entity, *l],
            Constraint::Fixed { point, .. } => vec![point.entity],
        };
        let mut out: Vec<Id> = Vec::with_capacity(all.len());
        for id in all {
            if !out.contains(&id) {
                out.push(id);
            }
        }
        out
    }

    pub fn name(&self) -> &'static str {
        match self {
            Constraint::Coincident(..) => "Coincident",
            Constraint::Horizontal(_) => "Horizontal",
            Constraint::Vertical(_) => "Vertical",
            Constraint::Parallel(..) => "Parallel",
            Constraint::Perpendicular(..) => "Perpendicular",
            Constraint::Tangent(..) => "Tangent",
            Constraint::Concentric(..) => "Concentric",
            Constraint::Equal(..) => "Equal",
            Constraint::Collinear(..) => "Collinear",
            Constraint::Symmetric(..) => "Symmetric",
            Constraint::Fixed { .. } => "Fixed",
            Constraint::Distance(..) => "Distance",
            Constraint::Angle(..) => "Angle",
            Constraint::Radius(..) => "Radius",
            Constraint::Diameter(..) => "Diameter",
        }
    }

    /// Distance, Angle, Radius, Diameter.
    pub fn value(&self) -> Option<f64> {
        match self {
            Constraint::Distance(_, _, v) | Constraint::Angle(_, _, v) | Constraint::Radius(_, v) | Constraint::Diameter(_, v) => {
                Some(*v)
            }
            _ => None,
        }
    }

    /// The same constraint with a new value; None for constraints without one.
    pub fn with_value(&self, v: f64) -> Option<Constraint> {
        match self {
            Constraint::Distance(p, q, _) => Some(Constraint::Distance(*p, *q, v)),
            Constraint::Angle(a, b, _) => Some(Constraint::Angle(*a, *b, v)),
            Constraint::Radius(a, _) => Some(Constraint::Radius(*a, v)),
            Constraint::Diameter(a, _) => Some(Constraint::Diameter(*a, v)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SolveError {
    #[error("constraint {index}: {reason}")]
    BadReference { index: usize, reason: &'static str },
    #[error("constraints cannot be satisfied (residual {residual:.3e})")]
    NotConverged { residual: f64 },
}
