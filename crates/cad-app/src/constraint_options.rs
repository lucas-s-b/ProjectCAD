use cad_document::{Constraint, Document, EntityId, PointKind, PointRef};
use cad_geometry::{DVec2, Shape};
use std::collections::BTreeSet;

type Pts = Vec<(PointRef, DVec2)>;

/// The constrainable points of a Line, Circle or Arc; None for every other shape.
fn points(id: EntityId, shape: &Shape) -> Option<Pts> {
    let p = |kind, at| (PointRef { entity: id, kind }, at);
    match shape {
        Shape::Line { a, b } => Some(vec![p(PointKind::Start, *a), p(PointKind::End, *b)]),
        Shape::Circle { center, .. } => Some(vec![p(PointKind::Center, *center)]),
        Shape::Arc { center, radius, start, sweep } => Some(vec![
            p(PointKind::Center, *center),
            p(PointKind::Start, *center + *radius * DVec2::from_angle(*start)),
            p(PointKind::End, *center + *radius * DVec2::from_angle(start + sweep)),
        ]),
        _ => None,
    }
}

fn closest(a: &Pts, b: &Pts) -> Option<Constraint> {
    let pairs = a.iter().flat_map(|p| b.iter().map(move |q| (p, q)));
    let (p, q) = pairs.min_by(|x, y| x.0.1.distance(x.1.1).total_cmp(&y.0.1.distance(y.1.1)))?;
    Some(Constraint::Coincident(p.0, q.0))
}

fn direction(s: &Shape) -> DVec2 {
    match s {
        Shape::Line { a, b } => *b - *a,
        _ => DVec2::X,
    }
}

fn is_line(s: &Shape) -> bool {
    matches!(s, Shape::Line { .. })
}

/// Buttons of the "Constrain" section for the selection: label and the constraints one click adds.
/// Entities are taken in ascending id order; a "curve" is a Circle or an Arc.
pub fn proposals(doc: &Document, selection: &BTreeSet<EntityId>) -> Vec<(&'static str, Vec<Constraint>)> {
    let items: Vec<(EntityId, &Shape)> = selection.iter().filter_map(|id| doc.get(*id).map(|e| (*id, &e.shape))).collect();
    if items.len() != selection.len() {
        return Vec::new();
    }
    let fixed = |pt: PointRef, at: DVec2| Constraint::Fixed { point: pt, at };
    match items.as_slice() {
        [(id, shape @ Shape::Line { a, b })] => {
            let pts = points(*id, shape).unwrap_or_default();
            vec![
                ("Horizontal", vec![Constraint::Horizontal(*id)]),
                ("Vertical", vec![Constraint::Vertical(*id)]),
                ("Length", vec![Constraint::Distance(pts[0].0, pts[1].0, a.distance(*b))]),
                ("Fix", vec![fixed(pts[0].0, *a), fixed(pts[1].0, *b)]),
            ]
        }
        [(id, Shape::Circle { center, radius } | Shape::Arc { center, radius, .. })] => {
            let c = PointRef { entity: *id, kind: PointKind::Center };
            vec![
                ("Radius", vec![Constraint::Radius(*id, *radius)]),
                ("Diameter", vec![Constraint::Diameter(*id, 2.0 * radius)]),
                ("Fix", vec![fixed(c, *center)]),
            ]
        }
        [(i, s), (j, t)] => {
            let (Some(p), Some(q)) = (points(*i, s), points(*j, t)) else { return Vec::new() };
            let mut out = match (is_line(s), is_line(t)) {
                (true, true) => {
                    let (d, e) = (direction(s), direction(t));
                    vec![
                        ("Parallel", vec![Constraint::Parallel(*i, *j)]),
                        ("Perpendicular", vec![Constraint::Perpendicular(*i, *j)]),
                        ("Equal", vec![Constraint::Equal(*i, *j)]),
                        ("Collinear", vec![Constraint::Collinear(*i, *j)]),
                        ("Angle", vec![Constraint::Angle(*i, *j, (d.x * e.y - d.y * e.x).atan2(d.dot(e)))]),
                    ]
                }
                (false, false) => vec![
                    ("Concentric", vec![Constraint::Concentric(*i, *j)]),
                    ("Equal", vec![Constraint::Equal(*i, *j)]),
                    ("Tangent", vec![Constraint::Tangent(*i, *j)]),
                ],
                _ => vec![("Tangent", vec![Constraint::Tangent(*i, *j)])],
            };
            out.extend(closest(&p, &q).map(|c| ("Coincident", vec![c])));
            out
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cad_document::{Command, Entity};
    use std::f64::consts::FRAC_PI_2;

    fn doc(shapes: Vec<Shape>) -> Document {
        let mut d = Document::new("t");
        for (i, shape) in shapes.into_iter().enumerate() {
            let id = i as u64 + 1;
            d.apply(Command::Add(Entity { id, name: format!("e{id}"), shape, layer: 0, sketch: 0 })).unwrap();
        }
        d
    }
    fn line(a: (f64, f64), b: (f64, f64)) -> Shape {
        Shape::Line { a: DVec2::new(a.0, a.1), b: DVec2::new(b.0, b.1) }
    }
    fn circle(x: f64, r: f64) -> Shape {
        Shape::Circle { center: DVec2::new(x, 0.0), radius: r }
    }
    fn labels(d: &Document, ids: &[u64]) -> Vec<&'static str> {
        proposals(d, &ids.iter().copied().collect()).into_iter().map(|(l, _)| l).collect()
    }

    #[test]
    fn buttons_per_selection_kind() {
        let d = doc(vec![line((0.0, 0.0), (3.0, 4.0)), line((0.0, 1.0), (1.0, 1.0)), circle(0.0, 2.0), circle(5.0, 1.0)]);
        assert_eq!(labels(&d, &[1]), ["Horizontal", "Vertical", "Length", "Fix"]);
        assert_eq!(labels(&d, &[3]), ["Radius", "Diameter", "Fix"]);
        assert_eq!(labels(&d, &[1, 2]), ["Parallel", "Perpendicular", "Equal", "Collinear", "Angle", "Coincident"]);
        assert_eq!(labels(&d, &[1, 3]), ["Tangent", "Coincident"]);
        assert_eq!(labels(&d, &[3, 4]), ["Concentric", "Equal", "Tangent", "Coincident"]);
        assert!(labels(&d, &[]).is_empty() && labels(&d, &[1, 2, 3]).is_empty());
    }

    #[test]
    fn no_buttons_for_other_shapes() {
        let rect = Shape::rect(DVec2::ZERO, DVec2::new(1.0, 1.0));
        let d = doc(vec![rect, line((0.0, 0.0), (1.0, 0.0))]);
        assert!(labels(&d, &[1]).is_empty() && labels(&d, &[1, 2]).is_empty());
    }

    #[test]
    fn values_come_from_current_geometry() {
        let d = doc(vec![line((0.0, 0.0), (3.0, 4.0)), line((9.0, 0.0), (9.0, 5.0)), circle(0.0, 2.0)]);
        let all = |ids: &[u64]| proposals(&d, &ids.iter().copied().collect());
        assert!(matches!(all(&[1])[2].1[..], [Constraint::Distance(_, _, v)] if (v - 5.0).abs() < 1e-12));
        assert_eq!(all(&[3])[1].1, [Constraint::Diameter(3, 4.0)]);
        let Constraint::Angle(1, 2, v) = all(&[1, 2])[4].1[0] else { panic!() };
        assert!((v - (FRAC_PI_2 - 4.0_f64.atan2(3.0))).abs() < 1e-12);
        assert_eq!(all(&[1])[3].1.len(), 2);
    }

    #[test]
    fn coincident_takes_the_closest_pair() {
        let d = doc(vec![line((0.0, 0.0), (10.0, 0.0)), line((10.5, 0.0), (20.0, 0.0)), line((-5.0, 0.0), (-0.5, 0.0))]);
        let first = |ids: &[u64]| proposals(&d, &ids.iter().copied().collect()).pop().unwrap().1;
        let end = |entity| PointRef { entity, kind: PointKind::End };
        let start = |entity| PointRef { entity, kind: PointKind::Start };
        assert_eq!(first(&[1, 2]), [Constraint::Coincident(end(1), start(2))]);
        assert_eq!(first(&[1, 3]), [Constraint::Coincident(start(1), end(3))]);
    }
}
