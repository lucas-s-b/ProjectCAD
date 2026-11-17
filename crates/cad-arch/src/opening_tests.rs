use super::*;
use crate::fixtures::*;
use cad_geometry::DVec3;

fn close6(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-6 * b.abs().max(1.0), "{a} vs {b}");
}
/// On the centred wall (0,0)-(4000,0), 200 thick, 2500 high; door hinged at the `a` side, swinging left.
fn opening(at: f64, width: f64, height: f64, sill: f64, kind: OpeningKind) -> Opening {
    Opening { host: 1, wall: wall((0.0, 0.0), (4000.0, 0.0), 200.0), at, width, height, sill, kind, hinge_b: false, swing_left: true }
}
fn body(plane: &Plane3) -> Body {
    wall_body(&wall((0.0, 0.0), (4000.0, 0.0), 200.0).corners(), plane, 2500.0, 0.02).unwrap()
}
fn near(p: DVec2, x: f64, y: f64) -> bool {
    p.distance(v(x, y)) <= 1e-9
}
const DOOR: OpeningKind = OpeningKind::Door;
const WINDOW: OpeningKind = OpeningKind::Window;

#[test]
fn gap_reaches_through_wall_ends() {
    assert_eq!(gap(&opening(2000.0, 900.0, 2100.0, 0.0, DOOR)), (1550.0, 2450.0));
    assert_eq!(gap(&opening(100.0, 900.0, 2100.0, 0.0, DOOR)), (-200.0, 900.0));
    assert_eq!(gap(&opening(5000.0, 900.0, 2100.0, 0.0, DOOR)), (3100.0, 4200.0));
    assert_eq!(gap(&opening(2000.0, 5000.0, 2100.0, 0.0, DOOR)), (-200.0, 4200.0));
}

#[test]
fn door_and_window_cut_their_boxes() {
    let os = [opening(1000.0, 900.0, 2100.0, 0.0, DOOR), opening(3000.0, 1200.0, 1200.0, 900.0, WINDOW)];
    let b = cut_openings(body(&Plane3::XY), &os, &Plane3::XY, 0.02);
    assert!(b.mesh.is_closed());
    close6(b.volume(), 1.334e9);
    let (lo, hi) = b.bbox();
    assert!(lo.z.abs() <= 1e-9 && (hi.z - 2500.0).abs() <= 1e-9);
}

#[test]
fn full_height_opening_splits_the_wall() {
    let b = cut_openings(body(&Plane3::XY), &[opening(2000.0, 900.0, 3000.0, 0.0, DOOR)], &Plane3::XY, 0.02);
    assert!(b.mesh.is_closed());
    close6(b.volume(), 1.55e9);
}

#[test]
fn cut_follows_the_level_plane() {
    let plane = Plane3 { origin: DVec3::new(0.0, 0.0, 3000.0), ..Plane3::XY };
    let b = cut_openings(body(&plane), &[opening(2000.0, 1200.0, 1200.0, 900.0, WINDOW)], &plane, 0.02);
    close6(b.volume(), 1.712e9);
    let (lo, hi) = b.bbox();
    assert!((lo.z - 3000.0).abs() <= 1e-9 && (hi.z - 5500.0).abs() <= 1e-9);
}

#[test]
fn door_symbol_is_leaf_and_quarter_arc() {
    let s = opening_symbol(&opening(2000.0, 900.0, 2100.0, 0.0, DOOR));
    assert_eq!(s.len(), 17);
    assert!(near(s[0][0], 1550.0, 100.0) && near(s[0][1], 1550.0, 1000.0));
    assert!(near(s[1][0], 1550.0, 1000.0));
    assert!(near(s[16][1], 2450.0, 100.0));
    let h = v(1550.0, 100.0);
    for l in &s[1..] {
        assert!((l[0].distance(h) - 900.0).abs() <= 1e-9 && (l[1].distance(h) - 900.0).abs() <= 1e-9);
    }
    let mut o = opening(2000.0, 900.0, 2100.0, 0.0, DOOR);
    (o.hinge_b, o.swing_left) = (true, false);
    let s = opening_symbol(&o);
    assert!(near(s[0][0], 2450.0, -100.0) && near(s[0][1], 2450.0, -1000.0));
    assert!(near(s[16][1], 1550.0, -100.0));
}

#[test]
fn window_symbol_is_three_lines() {
    let s = opening_symbol(&opening(2000.0, 1200.0, 1200.0, 900.0, WINDOW));
    assert_eq!(s.len(), 3);
    for (l, y) in s.iter().zip([100.0, -100.0, 0.0]) {
        assert!(near(l[0], 1400.0, y) && near(l[1], 2600.0, y));
    }
}

#[test]
fn ifc_classes() {
    assert_eq!(ifc_class(&Shape::Wall(wall((0.0, 0.0), (4000.0, 0.0), 200.0))), Some("IfcWall"));
    assert_eq!(ifc_class(&Shape::Opening(opening(2000.0, 900.0, 2100.0, 0.0, DOOR))), Some("IfcDoor"));
    assert_eq!(ifc_class(&Shape::Opening(opening(2000.0, 900.0, 2100.0, 0.0, WINDOW))), Some("IfcWindow"));
    assert_eq!(ifc_class(&Shape::Line { a: v(0.0, 0.0), b: v(1.0, 0.0) }), None);
}
