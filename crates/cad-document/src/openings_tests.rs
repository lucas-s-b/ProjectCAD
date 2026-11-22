use crate::*;
use cad_geometry::{DVec2, Opening, OpeningKind, Shape, Wall, WallAlign};

fn wall(a: (f64, f64), b: (f64, f64)) -> Wall {
    Wall { a: DVec2::new(a.0, a.1), b: DVec2::new(b.0, b.1), thickness: 200.0, height: 2500.0, align: WallAlign::Center }
}
fn entity(id: EntityId, sketch: SketchId, shape: Shape) -> Entity {
    Entity { id, name: format!("x{id}"), shape, layer: 0, sketch, construction: false, group: None, meta: Default::default() }
}
/// A 900 x 2100 door at 1000 on `host`, with a placeholder wall copy (the history refreshes it).
fn door(host: EntityId) -> Shape {
    let o = Opening { host, wall: wall((0.0, 0.0), (1.0, 0.0)), at: 1000.0, width: 900.0, height: 2100.0, sill: 0.0, kind: OpeningKind::Door, hinge_b: false, swing_left: true };
    Shape::Opening(o)
}
fn copy_of(d: &Document, id: EntityId) -> Wall {
    match &d.get(id).unwrap().shape {
        Shape::Opening(o) => o.wall,
        s => panic!("not an opening: {s:?}"),
    }
}
/// Levels 1 and 2 at 0; wall 1 (0,0)-(4000,0) on level 1, wall 2 the same on level 2, line 3 on level 1.
fn two_levels() -> Document {
    let mut d = Document::new("two");
    for id in [1, 2] {
        let plane = PlaneDef::Principal { base: BasePlane::XY, offset: "0".into() };
        d.apply(Command::AddSketch(Sketch { id, name: format!("L{id}"), plane, level: Some(Level { height: "3000 mm".into() }) })).unwrap();
    }
    d.apply(Command::Add(entity(1, 1, Shape::Wall(wall((0.0, 0.0), (4000.0, 0.0)))))).unwrap();
    d.apply(Command::Add(entity(2, 2, Shape::Wall(wall((0.0, 0.0), (4000.0, 0.0)))))).unwrap();
    d.apply(Command::Add(entity(3, 1, Shape::Line { a: DVec2::ZERO, b: DVec2::new(1.0, 0.0) }))).unwrap();
    d
}

#[test]
fn openings_need_a_wall_on_their_level() {
    let mut d = two_levels();
    d.apply(Command::Add(entity(4, 1, door(1)))).unwrap();
    assert!(matches!(d.apply(Command::Add(entity(5, 1, door(3)))), Err(DocError::NotAHost(3))));
    assert!(matches!(d.apply(Command::Add(entity(5, 1, door(2)))), Err(DocError::NotAHost(2))));
    assert!(matches!(d.apply(Command::Add(entity(5, 1, door(99)))), Err(DocError::NotAHost(99))));
    assert!(matches!(d.apply(Command::SetShape { id: 4, shape: door(3) }), Err(DocError::NotAHost(3))));
}

#[test]
fn walls_with_openings_stay() {
    let mut d = two_levels();
    d.apply(Command::Add(entity(4, 1, door(1)))).unwrap();
    assert!(matches!(d.apply(Command::Remove(1)), Err(DocError::HostInUse(1))));
    let line = Shape::Line { a: DVec2::ZERO, b: DVec2::new(1.0, 0.0) };
    assert!(matches!(d.apply(Command::SetShape { id: 1, shape: line }), Err(DocError::HostInUse(1))));
    d.apply(Command::SetShape { id: 1, shape: Shape::Wall(wall((0.0, 0.0), (3000.0, 0.0))) }).unwrap();
    assert_eq!(d.openings_of(1).collect::<Vec<_>>(), [4]);
    d.apply(Command::Remove(4)).unwrap();
    d.apply(Command::Remove(1)).unwrap();
}

#[test]
fn openings_follow_their_wall_in_one_undo_step() {
    let mut d = two_levels();
    let mut h = History::default();
    h.execute(&mut d, vec![Command::Add(entity(4, 1, door(1)))]).unwrap();
    assert_eq!(copy_of(&d, 4), wall((0.0, 0.0), (4000.0, 0.0)));
    h.execute(&mut d, vec![Command::SetShape { id: 1, shape: Shape::Wall(wall((0.0, 500.0), (4000.0, 500.0))) }]).unwrap();
    assert_eq!(copy_of(&d, 4), wall((0.0, 500.0), (4000.0, 500.0)));
    h.undo(&mut d);
    assert_eq!(copy_of(&d, 4), wall((0.0, 0.0), (4000.0, 0.0)));
    h.undo(&mut d);
    assert!(d.get(4).is_none());
}
