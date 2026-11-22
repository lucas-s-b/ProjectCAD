use crate::*;
use cad_geometry::{DVec2, Opening, OpeningKind, Room, Shape, Slab, Wall, WallAlign};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

/// Parameter `elev` = 3000 mm, level sketch 1 at offset "elev", centred walls 200 thick and 2500 high: 1 (0,0)-(4000,0),
/// 2 (4000,0)-(4000,3000) (an L with 1), 3 (2000,0)-(2000,3000) (a T on 1).
fn house() -> Document {
    let mut d = Document::new("house");
    d.apply(Command::SetParam { name: "elev".into(), expr: Some("3000 mm".into()) }).unwrap();
    d.apply(Command::AddSketch(Sketch {
        id: 1,
        name: "Ground".into(),
        plane: PlaneDef::Principal { base: BasePlane::XY, offset: "elev".into() },
        level: Some(Level { height: "2500 mm".into() }),
    }))
    .unwrap();
    for (id, a, b) in [(1, (0.0, 0.0), (4000.0, 0.0)), (2, (4000.0, 0.0), (4000.0, 3000.0)), (3, (2000.0, 0.0), (2000.0, 3000.0))] {
        let w = Wall { a: DVec2::new(a.0, a.1), b: DVec2::new(b.0, b.1), thickness: 200.0, height: 2500.0, align: WallAlign::Center };
        let e = Entity { id, name: format!("w{id}"), shape: Shape::Wall(w), layer: 0, sketch: 1, construction: false, group: None, meta: Default::default() };
        d.apply(Command::Add(e)).unwrap();
    }
    d
}

#[test]
fn walls_become_joined_solids_on_their_level() {
    let mut d = house();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.walls.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [1, 2, 3]);
    close(r.walls[0].1.volume(), 2.0e9);
    close(r.walls[1].1.volume(), 1.5e9);
    close(r.walls[2].1.volume(), 1.45e9);
    assert!(r.bodies.is_empty());
    let (lo, hi) = r.walls[0].1.bbox();
    close(lo.z, 3000.0);
    close(hi.z, 5500.0);
    d.apply(Command::SetParam { name: "elev".into(), expr: Some("0".into()) }).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    let (lo, hi) = r.walls[0].1.bbox();
    close(lo.z, 0.0);
    close(hi.z, 2500.0);
}

#[test]
fn construction_walls_have_no_solid() {
    let mut d = house();
    d.apply(Command::SetConstruction { id: 3, on: true }).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.walls.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [1, 2]);
    close(r.walls[0].1.volume(), 2.0e9);
    close(r.walls[1].1.volume(), 1.5e9);
}

fn close6(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-6 * b.abs().max(1.0), "{a} vs {b}");
}

/// An opening on `host` in level 1 whose wall copy is stale on purpose: the rebuild must place it on the host.
fn opening(id: EntityId, host: EntityId, at: f64, (width, height, sill): (f64, f64, f64), kind: OpeningKind) -> Entity {
    let wall = Wall { a: DVec2::ZERO, b: DVec2::new(1.0, 0.0), thickness: 1.0, height: 1.0, align: WallAlign::Center };
    let o = Opening { host, wall, at, width, height, sill, kind, hinge_b: false, swing_left: true };
    Entity { id, name: format!("o{id}"), shape: Shape::Opening(o), layer: 0, sketch: 1, construction: false, group: None, meta: Default::default() }
}

#[test]
fn doors_and_windows_cut_their_walls() {
    let mut d = house();
    d.apply(Command::Add(opening(4, 1, 1000.0, (900.0, 2100.0, 0.0), OpeningKind::Door))).unwrap();
    d.apply(Command::Add(opening(5, 2, 1500.0, (1200.0, 1200.0, 900.0), OpeningKind::Window))).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.walls.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [1, 2, 3]);
    close6(r.walls[0].1.volume(), 1.622e9);
    close6(r.walls[1].1.volume(), 1.212e9);
    close6(r.walls[2].1.volume(), 1.45e9);
    let (lo, hi) = r.walls[0].1.bbox();
    close(lo.z, 3000.0);
    close(hi.z, 5500.0);
    d.apply(Command::SetConstruction { id: 4, on: true }).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    close6(r.walls[0].1.volume(), 2.0e9);
}

fn in_level(id: EntityId, shape: Shape) -> Entity {
    Entity { id, name: format!("x{id}"), shape, layer: 0, sketch: 1, construction: false, group: None, meta: Default::default() }
}

fn wall_shape(a: (f64, f64), b: (f64, f64)) -> Shape {
    Shape::Wall(Wall { a: DVec2::new(a.0, a.1), b: DVec2::new(b.0, b.1), thickness: 200.0, height: 2500.0, align: WallAlign::Center })
}

fn room_shape(x: f64, y: f64) -> Shape {
    Shape::Room(Room { seed: DVec2::new(x, y), name: "R".into(), number: "1".into() })
}

#[test]
fn rooms_and_floors_on_rebuild() {
    let mut d = house();
    d.apply(Command::Add(in_level(4, wall_shape((4000.0, 3000.0), (0.0, 3000.0))))).unwrap();
    d.apply(Command::Add(in_level(5, wall_shape((0.0, 3000.0), (0.0, 0.0))))).unwrap();
    d.apply(Command::Add(opening(6, 3, 1500.0, (900.0, 2100.0, 0.0), OpeningKind::Door))).unwrap();
    for (id, x, y) in [(7, 1000.0, 1500.0), (8, 3000.0, 1500.0), (9, 9000.0, 9000.0)] {
        d.apply(Command::Add(in_level(id, room_shape(x, y)))).unwrap();
    }
    let outline = vec![DVec2::ZERO, DVec2::new(0.0, 3000.0), DVec2::new(4000.0, 3000.0), DVec2::new(4000.0, 0.0)];
    d.apply(Command::Add(in_level(10, Shape::Slab(Slab { outline, thickness: 200.0 })))).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.rooms.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [7, 8]);
    for (_, g) in &r.rooms {
        close(g.area, 5040000.0);
        close(g.perimeter, 9200.0);
    }
    assert_eq!(r.walls.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [1, 2, 3, 4, 5, 10]);
    let slab = &r.walls[5].1;
    close6(slab.volume(), 2.4e9);
    let (lo, hi) = slab.bbox();
    close(lo.z, 2800.0);
    close(hi.z, 3000.0);
    d.apply(Command::SetConstruction { id: 3, on: true }).unwrap();
    let r = Rebuilder::default().rebuild(&d);
    assert_eq!(r.rooms.len(), 2);
    for (_, g) in &r.rooms {
        close(g.area, 10640000.0);
        close(g.perimeter, 13200.0);
    }
}
