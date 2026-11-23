//! Sample documents for exporter tests, `cad-bench` and end-to-end scripts.
use crate::{BasePlane, Command, Document, Entity, Level, PlaneDef, Sketch};
use cad_geometry::{DVec2, Opening, OpeningKind, Roof, RoofKind, Room, Shape, Slab, Stair, Wall, WallAlign};

/// A two-storey house. Level 1 "Ground" (sketch 1: XY, offset "0", storey "3000 mm"): centred walls 200 thick, 3000
/// high, ids 1 (0,0)-(6000,0), 2 (6000,0)-(6000,4000), 3 (6000,4000)-(0,4000), 4 (0,4000)-(0,0); wall 1 has meta
/// `Pset_WallCommon.FireRating` = "EI60", `Pset_WallCommon.IsExternal` = "true", `note` = "not exported"; door 5 in
/// wall 1 (at 1500, 900 wide, 2100 high, sill 0; meta `Pset_DoorCommon.FireExit` = "true"); window 6 in wall 2 (at
/// 2000, 1200 x 1200, sill 900); room 7 "Living", number "1", seed (3000, 2000); slab 8 on (0,0)-(6000,4000), 200
/// thick; stair 9 from (1000, 800) to (1000, 3200), 900 wide, 3000 high, 17 risers. Level 2 "First" (sketch 2:
/// offset "3000 mm", storey "2800 mm"): hip roof 10 on (-100,-100)-(6100,4100), pitch 30, 200 thick, overhang 300.
pub fn house() -> Document {
    let mut d = Document::new("house");
    for (id, name, offset, height) in [(1, "Ground", "0", "3000 mm"), (2, "First", "3000 mm", "2800 mm")] {
        let plane = PlaneDef::Principal { base: BasePlane::XY, offset: offset.into() };
        d.apply(Command::AddSketch(Sketch { id, name: name.into(), plane, level: Some(Level { height: height.into() }) })).expect("valid sample");
    }
    let p = |x: f64, y: f64| DVec2::new(x, y);
    let wall = |a: DVec2, b: DVec2| Wall { a, b, thickness: 200.0, height: 3000.0, align: WallAlign::Center };
    let corners = [p(0.0, 0.0), p(6000.0, 0.0), p(6000.0, 4000.0), p(0.0, 4000.0)];
    let walls: Vec<Wall> = (0..4).map(|k| wall(corners[k], corners[(k + 1) % 4])).collect();
    let opening = |host: usize, at, width, height, sill, kind| {
        Shape::Opening(Opening { host: host as u64, wall: walls[host - 1], at, width, height, sill, kind, hinge_b: false, swing_left: true })
    };
    let meta = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let rect = |a: DVec2, b: DVec2| vec![a, p(b.x, a.y), b, p(a.x, b.y)];
    let items: Vec<(&str, u64, Shape, u32, _)> = vec![
        ("Wall", 1, Shape::Wall(walls[0]), 1, meta(&[("Pset_WallCommon.FireRating", "EI60"), ("Pset_WallCommon.IsExternal", "true"), ("note", "not exported")])),
        ("Wall", 2, Shape::Wall(walls[1]), 1, meta(&[])),
        ("Wall", 3, Shape::Wall(walls[2]), 1, meta(&[])),
        ("Wall", 4, Shape::Wall(walls[3]), 1, meta(&[])),
        ("Door", 5, opening(1, 1500.0, 900.0, 2100.0, 0.0, OpeningKind::Door), 1, meta(&[("Pset_DoorCommon.FireExit", "true")])),
        ("Window", 6, opening(2, 2000.0, 1200.0, 1200.0, 900.0, OpeningKind::Window), 1, meta(&[])),
        ("Room", 7, Shape::Room(Room { seed: p(3000.0, 2000.0), name: "Living".into(), number: "1".into() }), 1, meta(&[])),
        ("Slab", 8, Shape::Slab(Slab { outline: rect(p(0.0, 0.0), p(6000.0, 4000.0)), thickness: 200.0 }), 1, meta(&[])),
        ("Stair", 9, Shape::Stair(Stair { a: p(1000.0, 800.0), b: p(1000.0, 3200.0), width: 900.0, height: 3000.0, risers: 17 }), 1, meta(&[])),
        (
            "Roof",
            10,
            Shape::Roof(Roof { outline: rect(p(-100.0, -100.0), p(6100.0, 4100.0)), kind: RoofKind::Hip, pitch: 30.0, thickness: 200.0, overhang: 300.0 }),
            2,
            meta(&[]),
        ),
    ];
    for (kind, id, shape, sketch, meta) in items {
        d.apply(Command::Add(Entity { id, name: format!("{kind} {id}"), shape, layer: 0, sketch, construction: false, group: None, meta })).expect("valid sample");
    }
    d
}

#[cfg(test)]
#[path = "samples_tests.rs"]
mod tests;
