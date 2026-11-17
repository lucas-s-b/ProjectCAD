use super::*;
use crate::sample;
use ::dxf::entities::EntityType;
use cad_document::{Command, Entity, Layer};

fn load(doc: &Document) -> ::dxf::Drawing {
    ::dxf::Drawing::load(&mut to_dxf(doc, 0).as_bytes()).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn structure() {
    let s = to_dxf(&sample::doc(), 0);
    assert!(s.ends_with("0\nEOF\n"));
    let lines: Vec<&str> = s.lines().collect();
    let count = |v: &str| lines.chunks(2).filter(|c| *c == ["0", v]).count();
    assert_eq!(count("SECTION"), 3);
    assert_eq!(count("ENDSEC"), 3);
    assert_eq!(count("TABLE"), count("ENDTAB"));
    assert!(s.starts_with("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n"));
    let empty = load(&Document::new("e"));
    assert_eq!(empty.entities().count(), 0);
}

#[test]
fn entities_read_back() {
    let d = load(&sample::doc());
    // line, circle | rect, arc, polyline, text, dimension = 3 ext/line + 6 arrow edges + text
    assert_eq!(d.entities().count(), 2 + 4 + 3 + 6 + 1);
    let es: Vec<_> = d.entities().collect();
    let EntityType::Line(l) = &es[0].specific else { panic!() };
    assert!(close(l.p1.x, 0.0) && close(l.p2.x, 100.0) && close(l.p2.y, 0.0));
    assert_eq!(es[0].common.layer, "0");
    let EntityType::Circle(c) = &es[1].specific else { panic!() };
    assert!(close(c.center.x, 50.0) && close(c.center.y, 50.0) && close(c.radius, 10.0));
    let EntityType::Polyline(p) = &es[2].specific else { panic!() };
    assert!(p.is_closed() && p.vertices().count() == 4);
    assert_eq!(es[2].common.layer, "WALLS____CO_");
    let EntityType::Arc(a) = &es[3].specific else { panic!() };
    assert!(close(a.start_angle, 0.0) && close(a.end_angle, 90.0) && close(a.radius, 10.0));
    let EntityType::Polyline(p) = &es[4].specific else { panic!() };
    assert!(p.is_closed() && p.vertices().count() == 3);
    let EntityType::Text(t) = &es[5].specific else { panic!() };
    assert_eq!(t.value, "a<&>\"b");
    assert!(close(t.text_height, 5.0) && close(t.rotation, 0.5f64.to_degrees()));
    assert!(close(t.location.x, 10.0) && close(t.location.y, -20.0));
    let EntityType::Text(v) = &es.last().unwrap().specific else { panic!() };
    assert_eq!(v.value, "100.00");
    // box 6 * 0.6 * 4 = 14.4 wide, centred on (50, 23.6)
    assert!(close(v.location.x, 50.0 - 7.2) && close(v.location.y, 23.6 - 2.0));
}

#[test]
fn layers_are_sanitised_unique_and_coloured() {
    let mut d = Document::new("l");
    for (id, name, color) in [(1, "a b", [250, 10, 10]), (2, "A-B", [0, 0, 250]), (3, "x y", [100, 100, 100]), (4, "A_B", [0, 0, 0])] {
        let mut l = Layer::new(id, name);
        l.color = color;
        d.apply(Command::AddLayer(l)).unwrap();
    }
    let n = layer_names(&d);
    let names: Vec<&str> = n.iter().map(|(_, s)| s.as_str()).collect();
    assert_eq!(names, ["0", "A_B", "A-B", "X_Y", "A_B_4"]);
    let id = d.alloc_id();
    d.apply(Command::Add(Entity { id, name: "x".into(), shape: Shape::Line { a: DVec2::ZERO, b: DVec2::X }, layer: 1, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
    let dr = load(&d);
    let layers: Vec<_> = dr.layers().map(|l| (l.name.clone(), l.color.index())).collect();
    assert_eq!(layers[1], ("A_B".to_string(), Some(1)));
    assert_eq!(layers[2], ("A-B".to_string(), Some(5)));
    assert_eq!(aci([0, 0, 0]), 8);
}

#[test]
fn hidden_layers_and_solids_are_skipped() {
    let s = to_dxf(&sample::doc(), 0);
    assert!(!s.contains("500") && !s.contains("900"));
    assert!(s.contains("2\nHIDDEN\n"));
}

#[test]
fn full_arc_is_a_circle() {
    let mut d = Document::new("a");
    let arc = Shape::Arc { center: DVec2::ZERO, radius: 3.0, start: 1.0, sweep: TAU };
    d.apply(Command::Add(Entity { id: 1, name: "a".into(), shape: arc, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
    let dr = load(&d);
    assert!(matches!(dr.entities().next().unwrap().specific, EntityType::Circle(_)));
}

#[test]
fn layer_line_type_is_written_and_read_back() {
    let mut d = Document::new("l");
    let l = Layer { line_type: cad_document::LineType::Dashed, ..Layer::new(1, "A") };
    d.apply(Command::AddLayer(l)).unwrap();
    let dr = load(&d);
    let layers: Vec<_> = dr.layers().map(|l| l.line_type_name.clone()).collect();
    assert_eq!(layers, ["CONTINUOUS", "DASHED"]);
    let lts: Vec<_> = dr.line_types().map(|t| (t.name.clone(), t.element_count)).collect();
    assert_eq!(lts, [("CONTINUOUS".to_string(), 0), ("DASHED".into(), 2), ("HIDDEN".into(), 2), ("CENTER".into(), 4)]);
    let dashed = dr.line_types().find(|t| t.name == "DASHED").unwrap();
    assert_eq!(dashed.dash_dot_space_lengths, [3.0, -1.5]);
    assert!(close(dashed.total_pattern_length, 4.5));
}

#[test]
fn ellipse_and_spline_become_polylines() {
    let mut d = Document::new("c");
    let shapes = [
        Shape::Ellipse { center: DVec2::new(10.0, 20.0), rx: 6.0, ry: 3.0, angle: 0.0 },
        Shape::Spline { points: vec![DVec2::ZERO, DVec2::new(5.0, 5.0), DVec2::new(10.0, 0.0)], closed: false },
    ];
    for shape in shapes {
        let id = d.alloc_id();
        d.apply(Command::Add(Entity { id, name: "e".into(), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
    }
    let dr = load(&d);
    let es: Vec<_> = dr.entities().collect();
    let EntityType::Polyline(e) = &es[0].specific else { panic!() };
    assert!(e.is_closed() && e.vertices().count() == 96);
    let first = e.vertices().next().unwrap();
    assert!(close(first.location.x, 16.0) && close(first.location.y, 20.0));
    let EntityType::Polyline(s) = &es[1].specific else { panic!() };
    assert!(!s.is_closed() && s.vertices().count() == 33);
}

#[test]
fn annotation_shapes_read_back() {
    let d = load(&sample::annot_doc());
    let es: Vec<_> = d.entities().collect();
    let count = |f: fn(&EntityType) -> bool| es.iter().filter(|e| f(&e.specific)).count();
    // hatch: ring + 5 lines; leader: line + 3 arrow edges; radial: 1 + 3; angular: arc + 6
    assert_eq!(count(|e| matches!(e, EntityType::Polyline(_))), 3);
    assert_eq!(count(|e| matches!(e, EntityType::Line(_))), 5 + 3 + 4 + 6);
    let texts: Vec<_> = es.iter().filter_map(|e| if let EntityType::Text(t) = &e.specific { Some(t.value.clone()) } else { None }).collect();
    assert_eq!(texts, ["Note", "R20.00", "90.00%%d"]);
}

#[test]
fn solid_hatch_is_written_as_solid_triangles() {
    use cad_geometry::HatchPattern;
    let l = [DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0), DVec2::new(10.0, 4.0), DVec2::new(4.0, 4.0), DVec2::new(4.0, 10.0), DVec2::new(0.0, 10.0)];
    let mut d = Document::new("c");
    let shape = Shape::Hatch { boundary: l.to_vec(), angle: 0.0, spacing: 2.0, pattern: HatchPattern::Solid };
    let id = d.alloc_id();
    d.apply(Command::Add(Entity { id, name: "h".into(), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
    let dr = load(&d);
    let tris: Vec<_> = dr
        .entities()
        .filter_map(|e| if let EntityType::Solid(s) = &e.specific { Some([&s.first_corner, &s.second_corner, &s.third_corner]) } else { None })
        .collect();
    assert_eq!(tris.len(), 4);
    let area: f64 = tris.iter().map(|t| ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[2].x - t[0].x) * (t[1].y - t[0].y)).abs() / 2.0).sum();
    // the L: 10 x 10 minus the 6 x 6 notch
    assert!(close(area, 64.0), "{area}");
    // one outline ring, no hatch lines
    assert_eq!(dr.entities().filter(|e| matches!(e.specific, EntityType::Polyline(_))).count(), 1);
    assert_eq!(dr.entities().filter(|e| matches!(e.specific, EntityType::Line(_))).count(), 0);
}
