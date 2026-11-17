use crate::*;
use cad_geometry::{DVec2, Shape};
use std::f64::consts::{FRAC_PI_4, FRAC_PI_6};

fn line(id: EntityId, a: (f64, f64), b: (f64, f64)) -> Command {
    let shape = Shape::Line { a: DVec2::new(a.0, a.1), b: DVec2::new(b.0, b.1) };
    Command::Add(Entity { id, name: format!("e{id}"), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })
}

fn pt(entity: EntityId, kind: PointKind) -> PointRef {
    PointRef { entity, kind }
}

fn set_param(name: &str, expr: &str) -> Command {
    Command::SetParam { name: name.into(), expr: Some(expr.into()) }
}

fn ends(d: &Document, id: EntityId) -> (DVec2, DVec2) {
    match &d.get(id).unwrap().shape {
        Shape::Line { a, b } => (*a, *b),
        other => panic!("{other:?}"),
    }
}

fn length(d: &Document, id: EntityId) -> f64 {
    let (a, b) = ends(d, id);
    a.distance(b)
}

fn state(d: &Document) -> String {
    serde_json::to_string(d).unwrap()
}

fn value(d: &Document, id: ConstraintId) -> f64 {
    d.constraint(id).and_then(Constraint::value).unwrap()
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} vs {b}");
}

fn temp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("cadtest_{}_{name}.cadproj", std::process::id()))
}

/// One slanted line with Horizontal (constraint 1) and a Distance (constraint 2) driven by `w` = 40 mm.
fn driven_line() -> (Document, History) {
    let (mut d, mut h) = (Document::new("t"), History::default());
    h.execute(&mut d, vec![set_param("w", "40 mm"), line(1, (0.0, 0.0), (30.0, 5.0))]).unwrap();
    let distance = Constraint::Distance(pt(1, PointKind::Start), pt(1, PointKind::End), 1.0);
    let cmds = vec![
        Command::AddConstraint(1, Constraint::Horizontal(1)),
        Command::AddConstraint(2, distance),
        Command::SetConstraintExpr { id: 2, expr: Some("w".into()) },
    ];
    h.execute(&mut d, cmds).unwrap();
    (d, h)
}

#[test]
fn a_distance_takes_its_value_from_a_parameter() {
    let (d, _) = driven_line();
    near(length(&d, 1), 40.0);
    near(value(&d, 2), 40.0);
    assert_eq!(d.constraint_expr(2), Some("w"));
}

#[test]
fn changing_the_parameter_moves_the_sketch_in_one_undo_step() {
    let (mut d, mut h) = driven_line();
    let before = state(&d);
    h.execute(&mut d, vec![set_param("w", "60 mm")]).unwrap();
    near(length(&d, 1), 60.0);
    near(value(&d, 2), 60.0);
    assert!(h.undo(&mut d));
    assert_eq!(d.params().get("w"), Some("40 mm"));
    near(length(&d, 1), 40.0);
    near(value(&d, 2), 40.0);
    assert_eq!(state(&d), before);
    assert!(h.redo(&mut d));
    near(length(&d, 1), 60.0);
}

#[test]
fn an_angle_takes_its_value_from_a_parameter() {
    let (mut d, mut h) = (Document::new("t"), History::default());
    h.execute(&mut d, vec![set_param("a", "30 deg"), line(1, (0.0, 0.0), (10.0, 0.0)), line(2, (0.0, 0.0), (8.0, 8.0))]).unwrap();
    let cmds = vec![
        Command::AddConstraint(1, Constraint::Horizontal(1)),
        Command::AddConstraint(2, Constraint::Angle(1, 2, 1.0)),
        Command::SetConstraintExpr { id: 2, expr: Some("a".into()) },
    ];
    h.execute(&mut d, cmds).unwrap();
    near(value(&d, 2), FRAC_PI_6);
    let (a, b) = ends(&d, 2);
    near((b - a).to_angle(), FRAC_PI_6);
    h.execute(&mut d, vec![set_param("a", "45 deg")]).unwrap();
    let (a, b) = ends(&d, 2);
    near((b - a).to_angle(), FRAC_PI_4);
}

#[test]
fn bad_expressions_are_refused_and_nothing_changes() {
    let (mut d, mut h) = driven_line();
    let before = state(&d);
    for src in ["30 deg", "nope", "w + 1 deg"] {
        let r = h.execute(&mut d, vec![Command::SetConstraintExpr { id: 2, expr: Some(src.into()) }]);
        assert!(matches!(r, Err(DocError::ConstraintExpr { id: 2, .. })), "{src}: {r:?}");
        assert_eq!(state(&d), before, "{src}");
    }
    let r = h.execute(&mut d, vec![Command::SetConstraintExpr { id: 2, expr: Some("(".into()) }]);
    assert!(matches!(r, Err(DocError::Param(_))), "{r:?}");
    let r = h.execute(&mut d, vec![Command::SetConstraintExpr { id: 1, expr: Some("w".into()) }]);
    assert!(matches!(r, Err(DocError::BadFeature("this constraint has no value"))), "{r:?}");
    let r = h.execute(&mut d, vec![Command::SetConstraintExpr { id: 9, expr: None }]);
    assert!(matches!(r, Err(DocError::ConstraintNotFound(9))), "{r:?}");
    assert_eq!(state(&d), before);
    // A parameter change that breaks the dimension is refused too.
    let r = h.execute(&mut d, vec![set_param("w", "5 deg")]);
    assert!(matches!(r, Err(DocError::ConstraintExpr { id: 2, .. })), "{r:?}");
    assert_eq!(state(&d), before);
}

#[test]
fn a_parameter_used_by_a_constraint_cannot_be_removed() {
    let (mut d, mut h) = driven_line();
    let before = state(&d);
    let r = h.execute(&mut d, vec![Command::SetParam { name: "w".into(), expr: None }]);
    // The refusal is the constraint error, not a parameter error.
    assert!(matches!(r, Err(DocError::ConstraintExpr { id: 2, .. })), "{r:?}");
    assert_eq!(d.params().get("w"), Some("40 mm"));
    assert_eq!(state(&d), before);
    // Dropping the expression first frees the parameter.
    h.execute(&mut d, vec![Command::SetConstraintExpr { id: 2, expr: None }, Command::SetParam { name: "w".into(), expr: None }]).unwrap();
    assert_eq!(d.params().get("w"), None);
    near(value(&d, 2), 40.0);
}

#[test]
fn removing_a_constraint_and_undoing_restores_its_expression() {
    let (mut d, mut h) = driven_line();
    let before = state(&d);
    let inverse = d.apply(Command::RemoveConstraint(2)).unwrap();
    assert!(matches!(inverse, Command::AddConstraintWithExpr(2, _, ref src) if src == "w"));
    assert_eq!(d.constraint_expr(2), None);
    d.apply(inverse).unwrap();
    assert_eq!(state(&d), before);
    h.execute(&mut d, vec![Command::RemoveConstraint(2)]).unwrap();
    assert_eq!(d.constraint_expr(2), None);
    assert!(h.undo(&mut d));
    assert_eq!(d.constraint_expr(2), Some("w"));
    assert_eq!(state(&d), before);
    assert!(h.redo(&mut d));
    assert_eq!(d.constraint_expr(2), None);
}

#[test]
fn undoing_a_new_expression_restores_the_previous_one() {
    let (mut d, mut h) = driven_line();
    h.execute(&mut d, vec![set_param("v", "25 mm"), Command::SetConstraintExpr { id: 2, expr: Some("v + w".into()) }]).unwrap();
    near(length(&d, 1), 65.0);
    assert!(h.undo(&mut d));
    assert_eq!(d.constraint_expr(2), Some("w"));
    near(length(&d, 1), 40.0);
}

/// Four rough lines, each end meeting the next start; `w` and `h` set the bottom and left lengths.
fn parametric_rectangle() -> (Document, History) {
    let (mut d, mut h) = (Document::new("t"), History::default());
    let corners = [(0.0, 0.0), (38.0, 1.0), (39.0, 19.0), (1.0, 21.0)];
    let mut cmds = vec![set_param("w", "40 mm"), set_param("h", "20 mm")];
    cmds.extend((0..4).map(|i| line(i as u64 + 1, corners[i], corners[(i + 1) % 4])));
    h.execute(&mut d, cmds).unwrap();
    let mut cs = vec![Constraint::Horizontal(1), Constraint::Vertical(2), Constraint::Horizontal(3), Constraint::Vertical(4)];
    cs.extend((1..=4u64).map(|i| Constraint::Coincident(pt(i, PointKind::End), pt(i % 4 + 1, PointKind::Start))));
    cs.push(Constraint::Distance(pt(1, PointKind::Start), pt(1, PointKind::End), 1.0));
    cs.push(Constraint::Distance(pt(4, PointKind::End), pt(4, PointKind::Start), 1.0));
    let mut cmds: Vec<Command> = cs.into_iter().enumerate().map(|(i, c)| Command::AddConstraint(i as u64 + 1, c)).collect();
    cmds.push(Command::SetConstraintExpr { id: 9, expr: Some("w".into()) });
    cmds.push(Command::SetConstraintExpr { id: 10, expr: Some("h".into()) });
    h.execute(&mut d, cmds).unwrap();
    let kind = FeatureKind::Extrude { profile: vec![1, 2, 3, 4], distance: "5 mm".into(), offset: "0".into(), taper: "0 deg".into(), op: BodyOp::Add, flip: false, target: Target::Auto };
    let feature = Feature { id: 1, name: "Extrude".into(), suppressed: false, kind };
    h.execute(&mut d, vec![Command::AddFeature { index: 0, feature }]).unwrap();
    (d, h)
}

fn volume(d: &Document) -> f64 {
    let r = Rebuilder::default().rebuild(d);
    assert_eq!(r.bodies.len(), 1, "{:?}", r.states);
    r.total_volume()
}

#[test]
fn a_parametric_rectangle_drives_the_solid() {
    let (mut d, mut h) = parametric_rectangle();
    near(length(&d, 1), 40.0);
    near(length(&d, 4), 20.0);
    near(volume(&d), 4000.0);
    h.execute(&mut d, vec![set_param("w", "60 mm")]).unwrap();
    near(length(&d, 1), 60.0);
    near(volume(&d), 6000.0);
    h.execute(&mut d, vec![set_param("h", "w / 2")]).unwrap();
    near(volume(&d), 60.0 * 30.0 * 5.0);
    assert!(h.undo(&mut d) && h.undo(&mut d));
    near(volume(&d), 4000.0);
}

#[test]
fn expressions_survive_save_and_load() {
    let (d, _) = parametric_rectangle();
    let path = temp("exprs");
    d.save(&path).unwrap();
    let loaded = Document::load(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!((loaded.constraint_expr(9), loaded.constraint_expr(10)), (Some("w"), Some("h")));
    // serde_json parses floats without the exact round-trip feature, so compare shapes by value elsewhere.
    let constraints = |d: &Document| d.constraints().map(|(i, c)| (i, c.name(), c.value().map(|v| (v * 1e6).round()))).collect::<Vec<_>>();
    assert_eq!(constraints(&loaded), constraints(&d));
    near(length(&loaded, 1), 40.0);
    near(volume(&loaded), 4000.0);
}

const LITERAL: &str = r#"{"version":11,"name":"x","next_id":2,"next_constraint_id":2,"entities":{"1":{"id":1,"name":"l","shape":{"Line":{"a":[0.0,0.0],"b":[1.0,0.0]}}}},
    "constraints":{"1":{"Distance":[{"entity":1,"kind":"Start"},{"entity":1,"kind":"End"},3.0]}}}"#;

#[test]
fn a_version_11_file_loads_with_literal_values() {
    let path = temp("v11");
    std::fs::write(&path, LITERAL).unwrap();
    let mut d = Document::load(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!((d.constraint_expr(1), value(&d, 1)), (None, 3.0));
    let mut h = History::default();
    h.execute(&mut d, vec![Command::SetConstraintExpr { id: 1, expr: Some("2 mm".into()) }]).unwrap();
    near(length(&d, 1), 2.0);
}

#[test]
fn load_keeps_a_file_whose_expression_does_not_evaluate() {
    let body = LITERAL.replace(r#""version":11"#, r#""version":12"#);
    let text = format!(r#"{},"constraint_exprs":{{"1":"missing"}}}}"#, body.trim_end().strip_suffix('}').unwrap());
    let path = temp("broken");
    std::fs::write(&path, &text).unwrap();
    let d = Document::load(&path).unwrap();
    assert_eq!((d.constraint_expr(1), value(&d, 1)), (Some("missing"), 3.0));
    assert!(d.constraint_status().unwrap().is_ok());
    assert!(matches!(d.solve_commands(), Err(DocError::ConstraintExpr { id: 1, .. })));
    std::fs::write(&path, text.replace(r#""1":"missing""#, r#""7":"missing""#)).unwrap();
    let stray = Document::load(&path);
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(stray, Err(DocError::ConstraintNotFound(7))));
}
