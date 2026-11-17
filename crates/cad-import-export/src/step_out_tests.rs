use super::*;
use cad_geometry::{DVec2, Shape, profile::Profile};
use cad_modeler::{BoolOp, Plane3, Surface, boolean, extrude};
use std::collections::HashMap;

fn body(shape: Shape, z0: f64, height: f64) -> Body {
    let profile = Profile::from_shapes(&[&shape]).unwrap();
    let plane = Plane3 { origin: DVec3::new(0.0, 0.0, z0), ..Plane3::XY };
    Body::from_brep(&extrude(&profile, &plane, height).unwrap(), 0.01).unwrap()
}

fn cuboid() -> Body {
    body(Shape::rect(DVec2::ZERO, DVec2::new(40.0, 20.0)), 0.0, 10.0)
}

/// A clone whose faces are all free-form, so it is written as a faceted B-Rep.
fn faceted(b: &Body) -> Body {
    let mut b = b.clone();
    b.faces.iter_mut().for_each(|f| f.surface = Surface::Faceted);
    b
}

/// The cuboid with a 5 mm radius hole through it.
fn plate() -> Body {
    boolean(&cuboid(), &body(Shape::Circle { center: DVec2::new(20.0, 10.0), radius: 5.0 }, -1.0, 12.0), BoolOp::Difference).unwrap()
}

fn cylinder() -> Body {
    body(Shape::Circle { center: DVec2::new(5.0, 5.0), radius: 7.0 }, 0.0, 12.0)
}

/// Entity number -> text, for the lines of the DATA section.
fn entities(step: &str) -> HashMap<usize, String> {
    let data = step.split("DATA;\n").nth(1).unwrap().split("ENDSEC;").next().unwrap();
    data.lines()
        .map(|l| {
            let (k, text) = l.strip_prefix('#').unwrap().split_once('=').unwrap();
            (k.parse().unwrap(), text.strip_suffix(';').unwrap().to_string())
        })
        .collect()
}

/// Numbers that follow a `#` in `text`.
fn refs_in(text: &str) -> Vec<usize> {
    text.split('#').skip(1).map(|s| s.chars().take_while(char::is_ascii_digit).collect::<String>().parse().unwrap()).collect()
}

/// The coordinates of a `CARTESIAN_POINT('',(x,y,z))`.
fn coords(text: &str) -> DVec3 {
    let inner = text.rsplit_once('(').unwrap().1.split(')').next().unwrap();
    let v: Vec<f64> = inner.split(',').map(|v| v.parse().unwrap()).collect();
    DVec3::new(v[0], v[1], v[2])
}

#[test]
fn step_is_well_formed() {
    let (a, b) = (cuboid(), cylinder());
    let step = to_step(&[(1, &a), (2, &b)], "demo");
    assert!(step.starts_with("ISO-10303-21;"));
    assert!(step.ends_with("END-ISO-10303-21;\n"));
    let ents = entities(&step);
    for text in ents.values() {
        for r in refs_in(text) {
            assert!(ents.contains_key(&r), "#{r} is not defined");
        }
    }
}

#[test]
fn step_faces_match_triangles_and_volume() {
    let (a, b) = (faceted(&cuboid()), faceted(&cylinder()));
    let step = to_step(&[(1, &a), (2, &b)], "demo");
    let triangles = [&a, &b]
        .iter()
        .flat_map(|b| b.mesh.indices.as_chunks::<3>().0.iter().map(|t| t.map(|i| b.mesh.positions[i as usize])))
        .filter(|[p, q, r]| (*q - *p).cross(*r - *p).length() >= 1e-12)
        .count();
    assert_eq!(step.matches("FACE_SURFACE(").count(), triangles);
    let ents = entities(&step);
    let (mut volume, mut loops) = (0.0, 0);
    for text in ents.values().filter(|t| t.starts_with("POLY_LOOP(")) {
        let [p, q, r] = refs_in(text)[..] else { panic!("a loop has three points") };
        let [p, q, r] = [p, q, r].map(|k| coords(&ents[&k]));
        volume += p.dot(q.cross(r)) / 6.0;
        loops += 1;
    }
    assert_eq!(loops, triangles);
    let want = a.volume() + b.volume();
    assert!((volume - want).abs() / want < 1e-5, "{volume} vs {want}");
}

#[test]
fn step_quotes_name() {
    assert!(to_step(&[(1, &cuboid())], "it's").contains("PRODUCT('it''s'"));
}

#[test]
fn real_has_a_decimal_point() {
    assert_eq!(real(1.0), "1.000000");
    assert_eq!(real(-0.0000001), "0.");
    assert_eq!(real(0.0), "0.");
}

/// Number of entity lines of the given type.
fn count(step: &str, name: &str) -> usize {
    step.matches(&format!("={name}(")).count()
}

fn all_refs_defined(step: &str) {
    let ents = entities(step);
    for text in ents.values() {
        for r in refs_in(text) {
            assert!(ents.contains_key(&r), "#{r} is not defined");
        }
    }
}

#[test]
fn plate_exports_analytic_faces() {
    let step = to_step(&[(1, &plate())], "p");
    let want = [
        ("ADVANCED_FACE", 7),
        ("PLANE", 6),
        ("CYLINDRICAL_SURFACE", 1),
        ("CIRCLE", 2),
        ("LINE", 12),
        ("EDGE_CURVE", 14),
        ("VERTEX_POINT", 10),
        ("ORIENTED_EDGE", 28),
        ("EDGE_LOOP", 10),
        ("FACE_OUTER_BOUND", 6),
        ("FACE_BOUND", 4),
        ("MANIFOLD_SOLID_BREP", 1),
        ("ADVANCED_BREP_SHAPE_REPRESENTATION", 1),
        ("FACETED_BREP", 0),
        ("FACE_SURFACE", 0),
    ];
    for (name, n) in want {
        assert_eq!(count(&step, name), n, "{name}");
    }
    all_refs_defined(&step);
}

#[test]
fn oriented_edges_use_each_edge_once_each_way() {
    let ents = entities(&to_step(&[(1, &plate())], "p"));
    let mut uses: HashMap<usize, (u32, u32)> = HashMap::new();
    for text in ents.values().filter(|t| t.starts_with("ORIENTED_EDGE(")) {
        let u = uses.entry(*refs_in(text).last().unwrap()).or_default();
        if text.ends_with(".T.)") { u.0 += 1 } else { u.1 += 1 }
    }
    assert_eq!(uses.len(), 14);
    assert!(uses.values().all(|&u| u == (1, 1)), "{uses:?}");
}

#[test]
fn circle_vertices_lie_on_their_circles() {
    let ents = entities(&to_step(&[(1, &plate())], "p"));
    let mut circles = 0;
    for text in ents.values().filter(|t| t.starts_with("EDGE_CURVE(")) {
        let [v0, v1, curve] = refs_in(text)[..] else { panic!("an edge has two vertices and a curve") };
        let curve = &ents[&curve];
        if !curve.starts_with("CIRCLE(") {
            continue;
        }
        circles += 1;
        let place = refs_in(curve)[0];
        let [o, z, _] = refs_in(&ents[&place])[..] else { panic!("a placement has a point and two directions") };
        let (center, axis) = (coords(&ents[&o]), coords(&ents[&z]));
        let radius: f64 = curve.trim_end_matches(')').rsplit(',').next().unwrap().parse().unwrap();
        for v in [v0, v1] {
            let p = coords(&ents[&refs_in(&ents[&v])[0]]);
            assert!(((p - center).length() - radius).abs() < 1e-6);
            assert!((p - center).dot(axis).abs() < 1e-6);
        }
    }
    assert_eq!(circles, 2);
}

#[test]
fn mixed_bodies_link_both_representations() {
    let step = to_step(&[(1, &plate()), (2, &faceted(&cuboid()))], "m");
    assert_eq!(count(&step, "SHAPE_REPRESENTATION"), 1);
    assert_eq!(count(&step, "ADVANCED_BREP_SHAPE_REPRESENTATION"), 1);
    assert_eq!(count(&step, "FACETED_BREP_SHAPE_REPRESENTATION"), 1);
    assert_eq!(count(&step, "SHAPE_REPRESENTATION_RELATIONSHIP"), 2);
    all_refs_defined(&step);
}
