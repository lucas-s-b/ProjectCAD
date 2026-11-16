use super::*;
use crate::{sample::v2, to_dxf};
use ::dxf::entities::{self as de, Entity as DEntity, EntityType};
use ::dxf::{Drawing, Point};
use std::f64::consts::{FRAC_PI_2, PI};

fn shapes(doc: &Document) -> Vec<Shape> {
    doc.entities().map(|e| e.shape.clone()).collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn drawing_text(entities: Vec<EntityType>) -> String {
    let mut d = Drawing::new();
    // the default version (R12) has no LWPOLYLINE, ELLIPSE or SPLINE
    d.header.version = ::dxf::enums::AcadVersion::R2013;
    for e in entities {
        d.add_entity(DEntity::new(e));
    }
    let mut out = Vec::new();
    d.save(&mut out).unwrap();
    String::from_utf8(out).unwrap()
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y, 0.0)
}

#[test]
fn round_trip_of_the_exporter() {
    let mut d = Document::new("rt");
    let mut zero = d.layer(0).unwrap().clone();
    zero.color = [0, 255, 0];
    d.apply(Command::SetLayer(zero)).unwrap();
    let mut walls = Layer::new(1, "Walls");
    walls.color = [255, 0, 0];
    let mut dash = Layer::new(2, "Dash");
    dash.color = [0, 0, 255];
    dash.line_type = LineType::Dashed;
    for l in [walls, dash] {
        d.apply(Command::AddLayer(l)).unwrap();
    }
    let list = [
        (0, Shape::Line { a: v2(0.0, 0.0), b: v2(10.0, 5.0) }),
        (0, Shape::Circle { center: v2(5.0, 5.0), radius: 3.0 }),
        (0, Shape::Arc { center: v2(1.0, 2.0), radius: 4.0, start: 0.5, sweep: 1.5 }),
        (1, Shape::Polyline { points: vec![v2(0.0, 0.0), v2(5.0, 5.0), v2(9.0, 1.0)], closed: false }),
        (1, Shape::Polyline { points: vec![v2(0.0, 0.0), v2(5.0, 0.0), v2(5.0, 5.0), v2(0.0, 7.0)], closed: true }),
        (1, Shape::rect(v2(1.0, 1.0), v2(4.0, 6.0))),
        (1, Shape::Text { pos: v2(3.0, 4.0), height: 2.5, angle: 0.3, text: "Hi there".into() }),
        (2, Shape::Ellipse { center: v2(10.0, 20.0), rx: 6.0, ry: 3.0, angle: 0.0 }),
    ];
    for (layer, shape) in list {
        let id = d.alloc_id();
        d.apply(Command::Add(Entity { id, name: format!("e{id}"), shape, layer, sketch: 0 })).unwrap();
    }
    let (back, report) = from_dxf(&to_dxf(&d, 0), "back").unwrap();
    assert_eq!(report, ImportReport { imported: 8, skipped: BTreeMap::new() });
    assert_eq!(back.name, "back");
    let layers: Vec<_> = back.layers().map(|l| (l.name.clone(), l.color, l.line_type, l.visible)).collect();
    assert_eq!(
        layers,
        [
            ("0".into(), [0, 255, 0], LineType::Continuous, true),
            ("WALLS".into(), [255, 0, 0], LineType::Continuous, true),
            ("DASH".into(), [0, 0, 255], LineType::Dashed, true),
        ]
    );
    assert_eq!(back.entities().map(|e| e.layer).collect::<Vec<_>>(), [0, 0, 0, 1, 1, 1, 1, 2]);
    let s = shapes(&back);
    let Shape::Line { a, b } = &s[0] else { panic!("{:?}", s[0]) };
    assert!(a.distance(v2(0.0, 0.0)) < 1e-6 && b.distance(v2(10.0, 5.0)) < 1e-6);
    let Shape::Circle { center, radius } = &s[1] else { panic!() };
    assert!(center.distance(v2(5.0, 5.0)) < 1e-6 && near(*radius, 3.0));
    let Shape::Arc { center, radius, start, sweep } = &s[2] else { panic!() };
    assert!(center.distance(v2(1.0, 2.0)) < 1e-6 && near(*radius, 4.0) && near(*start, 0.5) && near(*sweep, 1.5));
    let Shape::Polyline { points, closed } = &s[3] else { panic!() };
    assert!(!closed && points.len() == 3 && points[2].distance(v2(9.0, 1.0)) < 1e-6);
    let Shape::Polyline { points, closed } = &s[4] else { panic!() };
    assert!(*closed && points.len() == 4 && points[3].distance(v2(0.0, 7.0)) < 1e-6);
    // the rectangle comes back as a closed polyline
    let Shape::Polyline { points, closed } = &s[5] else { panic!() };
    assert!(*closed && points.len() == 4 && points[0].distance(v2(1.0, 1.0)) < 1e-6 && points[2].distance(v2(4.0, 6.0)) < 1e-6);
    let Shape::Text { pos, height, angle, text } = &s[6] else { panic!() };
    assert!(pos.distance(v2(3.0, 4.0)) < 1e-6 && near(*height, 2.5) && near(*angle, 0.3) && text == "Hi there");
    // the exporter flattens an ellipse into a closed 96-point polyline
    let Shape::Polyline { points, closed } = &s[7] else { panic!() };
    assert!(*closed && points.len() == 96 && points[0].distance(v2(16.0, 20.0)) < 1e-6);
}

#[test]
fn entities_from_the_dxf_crate_and_skips() {
    let text = drawing_text(vec![
        EntityType::Line(de::Line::new(p(1.0, 2.0), p(3.0, 4.0))),
        EntityType::Circle(de::Circle::new(p(5.0, 5.0), 2.0)),
        EntityType::Arc(de::Arc::new(p(0.0, 0.0), 3.0, 350.0, 10.0)),
        EntityType::LwPolyline(de::LwPolyline {
            flags: 1,
            vertices: vec![
                ::dxf::LwPolylineVertex { x: 0.0, y: 0.0, ..Default::default() },
                ::dxf::LwPolylineVertex { x: 4.0, y: 0.0, bulge: 0.5, ..Default::default() },
                ::dxf::LwPolylineVertex { x: 4.0, y: 3.0, ..Default::default() },
            ],
            ..Default::default()
        }),
        EntityType::Text(de::Text { location: p(7.0, 8.0), text_height: 2.0, value: "abc".into(), rotation: 90.0, ..Default::default() }),
        EntityType::Ellipse(de::Ellipse { center: p(1.0, 1.0), major_axis: ::dxf::Vector::new(0.0, 4.0, 0.0), minor_axis_ratio: 0.5, ..Default::default() }),
        EntityType::Ellipse(de::Ellipse { end_parameter: 1.0, ..Default::default() }),
        EntityType::ModelPoint(de::ModelPoint::new(p(1.0, 1.0))),
        EntityType::Spline(de::Spline::default()),
    ]);
    let (doc, report) = from_dxf(&text, "x").unwrap();
    assert_eq!(report.imported, 6, "{report:?}
{}", text.lines().skip_while(|l| *l != "ENTITIES").collect::<Vec<_>>().join(" "));
    let skipped: Vec<_> = report.skipped.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(skipped, [("ELLIPSE", 1), ("POINT", 1), ("SPLINE", 1), ("bulge", 1)]);
    let s = shapes(&doc);
    assert!(matches!(&s[0], Shape::Line { a, b } if *a == v2(1.0, 2.0) && *b == v2(3.0, 4.0)));
    assert!(matches!(&s[1], Shape::Circle { radius, .. } if *radius == 2.0));
    let Shape::Polyline { points, closed } = &s[3] else { panic!() };
    assert!(*closed && points.len() == 3 && points[2] == v2(4.0, 3.0));
    let Shape::Text { pos, height, angle, text } = &s[4] else { panic!() };
    assert!(*pos == v2(7.0, 8.0) && *height == 2.0 && near(*angle, FRAC_PI_2) && text == "abc");
    let Shape::Ellipse { center, rx, ry, angle } = &s[5] else { panic!() };
    assert!(*center == v2(1.0, 1.0) && near(*rx, 4.0) && near(*ry, 2.0) && near(*angle, FRAC_PI_2));
}

#[test]
fn arc_across_zero_has_a_short_sweep() {
    let text = drawing_text(vec![
        EntityType::Arc(de::Arc::new(p(0.0, 0.0), 3.0, 350.0, 10.0)),
        EntityType::Arc(de::Arc::new(p(0.0, 0.0), 3.0, 90.0, 90.0)),
    ]);
    let (doc, _) = from_dxf(&text, "x").unwrap();
    let s = shapes(&doc);
    let Shape::Arc { start, sweep, .. } = &s[0] else { panic!() };
    assert!(near(*start, 350f64.to_radians()) && near(*sweep, 20f64.to_radians()));
    let Shape::Arc { sweep, .. } = &s[1] else { panic!() };
    assert!(near(*sweep, 2.0 * PI));
}

#[test]
fn crlf_and_invalid_geometry() {
    let mut d = Document::new("c");
    let id = d.alloc_id();
    d.apply(Command::Add(Entity { id, name: "l".into(), shape: Shape::Line { a: v2(0.0, 0.0), b: v2(1.0, 1.0) }, layer: 0, sketch: 0 })).unwrap();
    let crlf = to_dxf(&d, 0).replace('\n', "\r\n");
    let (doc, report) = from_dxf(&crlf, "x").unwrap();
    assert_eq!((doc.len(), report.imported), (1, 1));
    let zero = "0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n8\n0\n10\n0\n20\n0\n40\n0\n0\nENDSEC\n0\nEOF\n";
    let (doc, report) = from_dxf(zero, "x").unwrap();
    assert!(doc.is_empty() && report.imported == 0);
    assert_eq!(report.skipped.get("invalid"), Some(&1));
}

#[test]
fn layers_hidden_colours_and_unknown_layers() {
    let text = "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n0\nLAYER\n2\nA\n62\n-1\n6\nhidden\n0\nLAYER\n2\nB\n62\n40\n6\nWeird\n0\nENDTAB\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nC\n10\n0\n20\n0\n11\n1\n21\n1\n0\nENDSEC\n0\nEOF\n";
    let (doc, report) = from_dxf(text, "x").unwrap();
    assert_eq!(report.imported, 1);
    let l: Vec<_> = doc.layers().map(|l| (l.id, l.name.clone(), l.visible, l.color, l.line_type)).collect();
    assert_eq!(
        l,
        [
            (0, "0".to_string(), true, [230, 230, 230], LineType::Continuous),
            (1, "A".to_string(), false, [255, 0, 0], LineType::Hidden),
            (2, "B".to_string(), true, [230, 230, 230], LineType::Continuous),
            (3, "C".to_string(), true, [230, 230, 230], LineType::Continuous),
        ]
    );
    assert_eq!(doc.entities().next().unwrap().layer, 3);
}

#[test]
fn not_a_dxf_file() {
    for bad in ["hello world\nthis is not dxf\n", "0\nSECTION\n2\nHEADER\n0\nENDSEC\n", "0\nSECTION\n2\nENTITIES\n0\n", "AutoCAD Binary DXF\r\n\u{1a}\0", "", "x\ny\n"] {
        assert!(matches!(from_dxf(bad, "x"), Err(ImportError::Format(_))), "{bad:?}");
    }
    assert!(from_dxf("garbage", "x").unwrap_err().to_string().starts_with("not a DXF file"));
}
