use crate::{Command, Constraint, DocError, Document, Entity, EntityId, History, PointKind, PointRef};
use cad_geometry::annot::{LabelText, labels};
use cad_geometry::{DVec2, Shape};

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn add_in(id: EntityId, shape: Shape, sketch: u32) -> Command {
    Command::Add(Entity { id, name: format!("e{id}"), shape, layer: 0, sketch, construction: false, group: None, meta: Default::default() })
}
fn pt(entity: EntityId, kind: PointKind) -> PointRef {
    PointRef { entity, kind }
}
fn dim(a: DVec2, b: DVec2) -> Shape {
    Shape::Dimension { a, b, offset: 10.0, text_height: 3.0 }
}
fn line(a: DVec2, b: DVec2) -> Shape {
    Shape::Line { a, b }
}
fn ends(d: &Document, id: EntityId) -> (DVec2, DVec2) {
    match &d.get(id).unwrap().shape {
        Shape::Dimension { a, b, .. } | Shape::Line { a, b } => (*a, *b),
        other => panic!("{other:?}"),
    }
}
fn link() -> (PointRef, PointRef) {
    (pt(1, PointKind::Start), pt(1, PointKind::End))
}

/// Line 1 (0,0)-(40,0) and dimension 2 linked to its end points.
fn linked() -> (Document, History) {
    let mut d = Document::new("t");
    let mut h = History::default();
    h.execute(&mut d, vec![add_in(1, line(v(0.0, 0.0), v(40.0, 0.0)), 0), add_in(2, dim(v(0.0, 0.0), v(40.0, 0.0)), 0)]).unwrap();
    h.execute(&mut d, vec![Command::SetDimLink { id: 2, link: Some(link()) }]).unwrap();
    (d, h)
}

#[test]
fn point_of_covers_every_kind() {
    let mut d = Document::new("t");
    let arc = Shape::Arc { center: v(1.0, 1.0), radius: 2.0, start: 0.0, sweep: std::f64::consts::FRAC_PI_2 };
    d.apply(add_in(1, line(v(0.0, 0.0), v(4.0, 3.0)), 0)).unwrap();
    d.apply(add_in(2, arc, 0)).unwrap();
    d.apply(add_in(3, Shape::Circle { center: v(5.0, 6.0), radius: 1.0 }, 0)).unwrap();
    let at = |e, k| d.point_of(&pt(e, k)).unwrap();
    assert_eq!((at(1, PointKind::Start), at(1, PointKind::End)), (v(0.0, 0.0), v(4.0, 3.0)));
    assert!(at(2, PointKind::Start).distance(v(3.0, 1.0)) < 1e-12);
    assert!(at(2, PointKind::End).distance(v(1.0, 3.0)) < 1e-12);
    assert_eq!((at(2, PointKind::Center), at(3, PointKind::Center)), (v(1.0, 1.0), v(5.0, 6.0)));
    assert!(d.point_of(&pt(3, PointKind::Start)).is_none() && d.point_of(&pt(1, PointKind::Center)).is_none());
    assert!(d.point_of(&pt(9, PointKind::Start)).is_none());
}

#[test]
fn linked_dimension_follows_set_shape_in_one_undo_step() {
    let (mut d, mut h) = linked();
    h.execute(&mut d, vec![Command::SetShape { id: 1, shape: line(v(0.0, 0.0), v(60.0, 5.0)) }]).unwrap();
    assert_eq!(ends(&d, 2), (v(0.0, 0.0), v(60.0, 5.0)));
    assert!(matches!(d.get(2).unwrap().shape, Shape::Dimension { offset, text_height, .. } if offset == 10.0 && text_height == 3.0));
    let text = labels(&d.get(2).unwrap().shape);
    assert!(matches!(text[0].text, LabelText::Length(l) if (l - v(60.0, 5.0).length()).abs() < 1e-12));
    assert!(h.undo(&mut d));
    assert_eq!((ends(&d, 1), ends(&d, 2)), ((v(0.0, 0.0), v(40.0, 0.0)), (v(0.0, 0.0), v(40.0, 0.0))));
    assert!(h.redo(&mut d));
    assert_eq!(ends(&d, 2), (v(0.0, 0.0), v(60.0, 5.0)));
}

#[test]
fn linked_dimension_follows_a_constraint_solve() {
    let (mut d, mut h) = linked();
    let id = d.alloc_constraint_id();
    let dist = |len| Constraint::Distance(pt(1, PointKind::Start), pt(1, PointKind::End), len);
    h.execute(&mut d, vec![Command::AddConstraint(id, dist(40.0))]).unwrap();
    h.execute(&mut d, vec![Command::SetConstraint(id, dist(55.0))]).unwrap();
    let (a, b) = ends(&d, 1);
    assert!((a.distance(b) - 55.0).abs() < 1e-6);
    assert_eq!(ends(&d, 2), (a, b));
    assert!(matches!(labels(&d.get(2).unwrap().shape)[0].text, LabelText::Length(l) if (l - 55.0).abs() < 1e-6));
    assert!(h.undo(&mut d));
    assert_eq!(ends(&d, 2), (v(0.0, 0.0), v(40.0, 0.0)));
}

#[test]
fn unlinked_dimension_stays_and_unlink_inverts() {
    let (mut d, mut h) = linked();
    h.execute(&mut d, vec![Command::SetDimLink { id: 2, link: None }]).unwrap();
    assert!(d.dim_link(2).is_none());
    h.execute(&mut d, vec![Command::SetShape { id: 1, shape: line(v(0.0, 0.0), v(60.0, 0.0)) }]).unwrap();
    assert_eq!(ends(&d, 2), (v(0.0, 0.0), v(40.0, 0.0)));
    assert!(h.undo(&mut d) && h.undo(&mut d));
    assert_eq!(d.dim_link(2), Some(&link()));
}

#[test]
fn link_validation() {
    let mut d = Document::new("t");
    let arc = Shape::Arc { center: v(0.0, 0.0), radius: 5.0, start: 0.0, sweep: 1.0 };
    d.apply(add_in(1, line(v(0.0, 0.0), v(40.0, 0.0)), 0)).unwrap();
    d.apply(add_in(2, dim(v(0.0, 0.0), v(40.0, 0.0)), 0)).unwrap();
    d.apply(add_in(3, Shape::Circle { center: v(9.0, 9.0), radius: 1.0 }, 0)).unwrap();
    d.apply(Command::AddSketch(crate::Sketch { id: 1, name: "s".into(), plane: crate::PlaneDef::Principal { base: crate::BasePlane::XY, offset: "0".into() }, level: None })).unwrap();
    d.apply(add_in(4, line(v(0.0, 0.0), v(1.0, 0.0)), 1)).unwrap();
    d.apply(add_in(5, arc, 0)).unwrap();
    let set = |id, a, b| Command::SetDimLink { id, link: Some((a, b)) };
    let (s, e) = (pt(1, PointKind::Start), pt(1, PointKind::End));
    assert!(matches!(d.apply(set(1, s, e)), Err(DocError::BadFeature("not a linear dimension"))));
    assert!(matches!(d.apply(set(9, s, e)), Err(DocError::NotFound(9))));
    assert!(matches!(d.apply(set(2, s, pt(8, PointKind::End))), Err(DocError::NotFound(8))));
    assert!(matches!(d.apply(set(2, s, pt(3, PointKind::Start))), Err(DocError::BadFeature(_))));
    assert!(matches!(d.apply(set(2, s, pt(1, PointKind::Center))), Err(DocError::BadFeature(_))));
    assert!(matches!(d.apply(set(2, s, pt(4, PointKind::End))), Err(DocError::MixedSketches)));
    assert!(matches!(d.apply(Command::SetDimLink { id: 9, link: None }), Err(DocError::NotFound(9))));
    assert_eq!(d.dim_links().count(), 0);
    // Circle centre to arc end is a valid pair.
    d.apply(set(2, pt(3, PointKind::Center), pt(5, PointKind::End))).unwrap();
    assert_eq!(d.dim_links().count(), 1);
}

#[test]
fn measured_line_and_dimension_cannot_be_removed_while_linked() {
    let (mut d, mut h) = linked();
    assert!(matches!(h.execute(&mut d, vec![Command::Remove(1)]), Err(DocError::EntityConstrained(1))));
    assert!(matches!(h.execute(&mut d, vec![Command::Remove(2)]), Err(DocError::EntityConstrained(2))));
    h.execute(&mut d, vec![Command::SetDimLink { id: 2, link: None }, Command::Remove(2), Command::Remove(1)]).unwrap();
    assert!(d.is_empty());
    assert!(h.undo(&mut d));
    assert_eq!((d.len(), d.dim_link(2)), (2, Some(&link())));
}

#[test]
fn links_survive_save_and_load() {
    let (d, _) = linked();
    let p = std::env::temp_dir().join(format!("cadtest_{}_dimlink.cadproj", std::process::id()));
    d.save(&p).unwrap();
    let l = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    assert_eq!(l.dim_link(2), Some(&link()));
    let dangling = r#"{"version":11,"name":"x","next_id":3,"dim_links":{"2":[{"entity":1,"kind":"Start"},{"entity":1,"kind":"End"}]},"entities":{}}"#;
    std::fs::write(&p, dangling).unwrap();
    let r = Document::load(&p);
    std::fs::remove_file(&p).unwrap();
    assert!(matches!(r, Err(DocError::NotFound(2))));
}
