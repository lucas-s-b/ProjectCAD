use super::*;
use crate::fixtures::*;
use crate::ifc_class;
use cad_geometry::{Room, Shape, Slab};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}
/// `got` is `want` up to the starting vertex, each point within 1e-6 (crossing points carry rounding).
fn ring6(got: &[DVec2], want: &[(f64, f64)]) {
    let n = want.len();
    let ok = got.len() == n && (0..n).any(|s| (0..n).all(|k| got[(s + k) % n].distance(v(want[k].0, want[k].1)) <= 1e-6));
    assert!(ok, "{got:?} vs {want:?}");
}
/// Centred walls 200 thick around (0,0)-(4000,3000), L joins at the corners.
fn box_walls() -> Vec<Wall> {
    vec![
        wall((0.0, 0.0), (4000.0, 0.0), 200.0),
        wall((4000.0, 0.0), (4000.0, 3000.0), 200.0),
        wall((4000.0, 3000.0), (0.0, 3000.0), 200.0),
        wall((0.0, 3000.0), (0.0, 0.0), 200.0),
    ]
}

#[test]
fn four_walls_enclose_one_room() {
    let r = regions(&box_walls(), &[v(2000.0, 1500.0), v(5000.0, 5000.0), v(2000.0, 0.0)]);
    let g = r[0].as_ref().unwrap();
    ring6(&g.outline, &[(100.0, 100.0), (3900.0, 100.0), (3900.0, 2900.0), (100.0, 2900.0)]);
    assert!(g.islands.is_empty());
    close(g.area, 10640000.0);
    close(g.perimeter, 13200.0);
    assert!(r[1].is_none());
    assert!(r[2].is_none());
}

#[test]
fn partition_splits_the_room() {
    let mut walls = box_walls();
    walls.push(wall((2000.0, 0.0), (2000.0, 3000.0), 100.0));
    let r = regions(&walls, &[v(1000.0, 1500.0), v(3000.0, 1500.0)]);
    let (l, rt) = (r[0].as_ref().unwrap(), r[1].as_ref().unwrap());
    ring6(&l.outline, &[(100.0, 100.0), (1950.0, 100.0), (1950.0, 2900.0), (100.0, 2900.0)]);
    ring6(&rt.outline, &[(2050.0, 100.0), (3900.0, 100.0), (3900.0, 2900.0), (2050.0, 2900.0)]);
    for g in [l, rt] {
        close(g.area, 5180000.0);
        close(g.perimeter, 9300.0);
    }
}

#[test]
fn crossing_walls_bound_rooms() {
    let mut walls = box_walls();
    walls.push(wall((2000.0, -500.0), (2000.0, 3500.0), 200.0));
    walls.push(wall((-500.0, 1500.0), (4500.0, 1500.0), 200.0));
    let r = regions(&walls, &[v(1000.0, 700.0)]);
    let g = r[0].as_ref().unwrap();
    ring6(&g.outline, &[(100.0, 100.0), (1900.0, 100.0), (1900.0, 1400.0), (100.0, 1400.0)]);
    close(g.area, 2340000.0);
    close(g.perimeter, 6200.0);
}

#[test]
fn free_wall_is_an_island() {
    let mut walls = box_walls();
    walls.push(wall((1000.0, 1500.0), (3000.0, 1500.0), 200.0));
    let r = regions(&walls, &[v(500.0, 500.0), v(2000.0, 1500.0)]);
    let g = r[0].as_ref().unwrap();
    assert_eq!(g.islands.len(), 1);
    ring6(&g.islands[0], &[(1000.0, 1400.0), (3000.0, 1400.0), (3000.0, 1600.0), (1000.0, 1600.0)]);
    close(g.area, 10240000.0);
    close(g.perimeter, 17600.0);
    assert!(r[1].is_none());
}

#[test]
fn open_side_is_no_room() {
    assert_eq!(regions(&box_walls()[..3], &[v(2000.0, 1500.0)]), vec![None]);
    assert_eq!(regions(&[], &[v(0.0, 0.0)]), vec![None]);
}

#[test]
fn ifc_classes_of_rooms_and_floors() {
    assert_eq!(ifc_class(&Shape::Room(Room { seed: v(0.0, 0.0), name: "R".into(), number: "1".into() })), Some("IfcSpace"));
    let slab = Slab { outline: vec![v(0.0, 0.0), v(1.0, 0.0), v(0.0, 1.0)], thickness: 1.0 };
    assert_eq!(ifc_class(&Shape::Slab(slab)), Some("IfcSlab"));
}
