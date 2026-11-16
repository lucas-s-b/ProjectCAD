use crate::{Command, Constraint, DocError, Document, Entity, EntityId, History, PointKind, PointRef, SolveError, Status};
use cad_geometry::{DVec2, Shape};

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn line(a: (f64, f64), b: (f64, f64)) -> Shape {
    Shape::Line { a: v(a.0, a.1), b: v(b.0, b.1) }
}
fn circle(x: f64, r: f64) -> Shape {
    Shape::Circle { center: v(x, 0.0), radius: r }
}
fn add(id: EntityId, shape: Shape) -> Command {
    Command::Add(Entity { id, name: format!("e{id}"), shape, layer: 0, sketch: 0 })
}
fn pt(entity: EntityId, kind: PointKind) -> PointRef {
    PointRef { entity, kind }
}
fn shape_of(d: &Document, id: EntityId) -> Shape {
    d.get(id).unwrap().shape.clone()
}
fn ends(d: &Document, id: EntityId) -> (DVec2, DVec2) {
    match shape_of(d, id) {
        Shape::Line { a, b } => (a, b),
        other => panic!("{other:?}"),
    }
}
fn state(d: &Document) -> (Vec<Entity>, Vec<(u64, Constraint)>) {
    (d.entities().cloned().collect(), d.constraints().map(|(i, c)| (i, c.clone())).collect())
}
fn slanted() -> (Document, History) {
    let mut d = Document::new("t");
    let mut h = History::default();
    h.execute(&mut d, vec![add(1, line((0.0, 0.0), (40.0, 10.0)))]).unwrap();
    (d, h)
}

#[test]
fn constraint_commands_invert() {
    let (mut d, _) = slanted();
    let id = d.alloc_constraint_id();
    assert_eq!(id, 1);
    assert_eq!(d.apply(Command::AddConstraint(id, Constraint::Horizontal(1))).unwrap(), Command::RemoveConstraint(1));
    let inv = d.apply(Command::SetConstraint(1, Constraint::Vertical(1))).unwrap();
    assert_eq!(inv, Command::SetConstraint(1, Constraint::Horizontal(1)));
    assert_eq!(d.constraint(1), Some(&Constraint::Vertical(1)));
    let inv = d.apply(Command::RemoveConstraint(1)).unwrap();
    assert_eq!(inv, Command::AddConstraint(1, Constraint::Vertical(1)));
    assert_eq!(d.constraints().count(), 0);
    assert_eq!(d.alloc_constraint_id(), 2);
}

#[test]
fn constraint_commands_reject_bad_input() {
    let (mut d, _) = slanted();
    let c = Constraint::Horizontal(1);
    assert!(matches!(d.apply(Command::AddConstraint(1, Constraint::Horizontal(9))), Err(DocError::NotFound(9))));
    d.apply(Command::AddConstraint(1, c.clone())).unwrap();
    assert!(matches!(d.apply(Command::AddConstraint(1, c.clone())), Err(DocError::ConstraintExists(1))));
    assert!(matches!(d.apply(Command::RemoveConstraint(5)), Err(DocError::ConstraintNotFound(5))));
    assert!(matches!(d.apply(Command::SetConstraint(5, c)), Err(DocError::ConstraintNotFound(5))));
    assert!(matches!(d.apply(Command::SetConstraint(1, Constraint::Vertical(9))), Err(DocError::NotFound(9))));
    assert_eq!(d.constraint(1), Some(&Constraint::Horizontal(1)));
}

#[test]
fn constrained_entity_cannot_be_removed() {
    let (mut d, mut h) = slanted();
    h.execute(&mut d, vec![Command::AddConstraint(1, Constraint::Horizontal(1))]).unwrap();
    assert!(matches!(d.apply(Command::Remove(1)), Err(DocError::EntityConstrained(1))));
    assert!(matches!(h.execute(&mut d, vec![Command::Remove(1)]), Err(DocError::EntityConstrained(1))));
    assert_eq!(d.constraints_of(1), [1]);
    h.execute(&mut d, vec![Command::RemoveConstraint(1), Command::Remove(1)]).unwrap();
    assert!(d.is_empty() && d.constraints_of(1).is_empty());
    assert!(h.undo(&mut d));
    assert_eq!((d.len(), d.constraints().count()), (1, 1));
}

#[test]
fn adding_horizontal_solves_in_one_undo_step() {
    let (mut d, mut h) = slanted();
    let before = state(&d).0;
    h.execute(&mut d, vec![Command::AddConstraint(1, Constraint::Horizontal(1))]).unwrap();
    let (a, b) = ends(&d, 1);
    assert!((a.y - b.y).abs() < 1e-6);
    let solved = shape_of(&d, 1);
    assert!(h.undo(&mut d));
    assert_eq!(state(&d), (before, vec![]));
    assert!(h.redo(&mut d));
    assert_eq!(shape_of(&d, 1), solved);
    assert_eq!(d.constraints().count(), 1);
}

#[test]
fn set_shape_that_breaks_a_constraint_is_repaired() {
    let (mut d, mut h) = slanted();
    h.execute(&mut d, vec![Command::AddConstraint(1, Constraint::Horizontal(1))]).unwrap();
    h.execute(&mut d, vec![Command::SetShape { id: 1, shape: line((0.0, 0.0), (30.0, 12.0)) }]).unwrap();
    let (a, b) = ends(&d, 1);
    assert!((a.y - b.y).abs() < 1e-6);
}

#[test]
fn contradicting_constraint_is_rejected_and_rolled_back() {
    let mut d = Document::new("t");
    let mut h = History::default();
    h.execute(&mut d, vec![add(1, circle(0.0, 1.0)), add(2, circle(10.0, 1.0))]).unwrap();
    let dist = |len| Constraint::Distance(pt(1, PointKind::Center), pt(2, PointKind::Center), len);
    h.execute(&mut d, vec![Command::AddConstraint(1, dist(10.0))]).unwrap();
    let before = state(&d);
    let r = h.execute(&mut d, vec![Command::AddConstraint(2, dist(20.0))]);
    assert!(matches!(r, Err(DocError::Solve(SolveError::NotConverged { .. }))));
    assert_eq!(state(&d), before);
    assert!(h.undo(&mut d));
    assert_eq!(d.constraints().count(), 0);
}

#[test]
fn failed_execute_keeps_undo_state() {
    let mut d = Document::new("t");
    let mut h = History::default();
    h.execute(&mut d, vec![add(1, circle(0.0, 1.0)), add(2, circle(10.0, 1.0))]).unwrap();
    let dist = |len| Constraint::Distance(pt(1, PointKind::Center), pt(2, PointKind::Center), len);
    h.execute(&mut d, vec![Command::AddConstraint(1, dist(10.0))]).unwrap();
    assert!(h.execute(&mut d, vec![Command::AddConstraint(2, dist(20.0))]).is_err());
    assert!(h.can_undo() && !h.can_redo());
    // exactly two undo steps remain: the constraint, then the two circles
    assert!(h.undo(&mut d) && h.undo(&mut d) && !h.undo(&mut d));
    assert!(d.is_empty());
}

#[test]
fn set_constraint_moves_geometry() {
    let mut d = Document::new("t");
    let mut h = History::default();
    h.execute(&mut d, vec![add(1, line((0.0, 0.0), (40.0, 0.0)))]).unwrap();
    let cs = [
        Constraint::Fixed { point: pt(1, PointKind::Start), at: v(0.0, 0.0) },
        Constraint::Horizontal(1),
        Constraint::Distance(pt(1, PointKind::Start), pt(1, PointKind::End), 40.0),
    ];
    for (i, c) in cs.into_iter().enumerate() {
        h.execute(&mut d, vec![Command::AddConstraint(i as u64 + 1, c)]).unwrap();
    }
    let longer = Constraint::Distance(pt(1, PointKind::Start), pt(1, PointKind::End), 60.0);
    h.execute(&mut d, vec![Command::SetConstraint(3, longer)]).unwrap();
    let (a, b) = ends(&d, 1);
    assert!(a.distance(v(0.0, 0.0)) < 1e-6 && b.distance(v(60.0, 0.0)) < 1e-6, "{a} {b}");
    assert_eq!(d.constraint_status().unwrap().unwrap().status(), Status::FullyConstrained);
}

#[test]
fn constraint_status_of_a_rectangle() {
    let mut d = Document::new("t");
    let sides = [((0.0, 0.0), (40.0, 0.0)), ((40.0, 0.0), (40.0, 20.0)), ((40.0, 20.0), (0.0, 20.0)), ((0.0, 20.0), (0.0, 0.0))];
    assert!(d.constraint_status().is_none());
    for (i, (a, b)) in sides.into_iter().enumerate() {
        d.apply(add(i as u64 + 1, line(a, b))).unwrap();
    }
    use PointKind::{End, Start};
    let cs = [
        Constraint::Coincident(pt(1, End), pt(2, Start)),
        Constraint::Coincident(pt(2, End), pt(3, Start)),
        Constraint::Coincident(pt(3, End), pt(4, Start)),
        Constraint::Coincident(pt(4, End), pt(1, Start)),
        Constraint::Horizontal(1),
        Constraint::Horizontal(3),
        Constraint::Vertical(2),
        Constraint::Vertical(4),
        Constraint::Fixed { point: pt(1, Start), at: v(0.0, 0.0) },
        Constraint::Distance(pt(1, Start), pt(1, End), 40.0),
        Constraint::Distance(pt(2, Start), pt(2, End), 20.0),
    ];
    for c in cs {
        let id = d.alloc_constraint_id();
        d.apply(Command::AddConstraint(id, c)).unwrap();
    }
    let sol = d.constraint_status().unwrap().unwrap();
    assert_eq!((sol.dof, sol.redundant), (0, 0));
    assert!(d.solve_commands().unwrap().is_empty());
}

fn temp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("cadconstraint_{}_{name}.cadproj", std::process::id()))
}

fn load_text(name: &str, text: &str) -> Result<Document, DocError> {
    let p = temp(name);
    std::fs::write(&p, text).unwrap();
    let r = Document::load(&p);
    std::fs::remove_file(&p).unwrap();
    r
}

#[test]
fn constraints_survive_save_and_load() {
    let (mut d, mut h) = slanted();
    h.execute(&mut d, vec![Command::AddConstraint(4, Constraint::Horizontal(1))]).unwrap();
    let p = temp("roundtrip");
    d.save(&p).unwrap();
    let mut l = Document::load(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    assert_eq!(state(&l), state(&d));
    assert_eq!(l.alloc_constraint_id(), 5);
}

#[test]
fn version_3_file_loads_without_constraints() {
    let text = r#"{"version":3,"name":"x","next_id":2,"entities":{"1":{"id":1,"name":"c",
        "shape":{"Circle":{"center":[0.0,0.0],"radius":1.0}}}}}"#;
    let mut d = load_text("v3", text).unwrap();
    assert_eq!(d.constraints().count(), 0);
    assert_eq!(d.alloc_constraint_id(), 1);
}

#[test]
fn load_rejects_constraint_on_missing_entity() {
    let text = r#"{"version":4,"name":"x","next_id":1,"entities":{},"constraints":{"1":{"Horizontal":5}}}"#;
    assert!(matches!(load_text("missing", text), Err(DocError::NotFound(5))));
}
