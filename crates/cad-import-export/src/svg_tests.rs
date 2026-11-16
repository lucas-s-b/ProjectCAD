use super::*;
use std::f64::consts::FRAC_PI_2;
use crate::sample;

fn names<'a>(g: roxmltree::Node<'a, 'a>) -> Vec<&'a str> {
    g.children().filter(|n| n.is_element()).map(|n| n.tag_name().name()).collect()
}

#[test]
fn groups_children_and_view_box() {
    let doc = sample::doc();
    let svg = to_svg(&doc, 0);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let root = xml.root_element();
    let groups: Vec<_> = root.children().filter(|n| n.is_element()).collect();
    assert_eq!(groups.len(), 2);
    assert_eq!(names(groups[0]), ["line", "circle"]);
    // rect, arc, polygon, text, then the dimension: 3 lines, 2 arrows, value
    let want = ["rect", "path", "polygon", "text", "line", "line", "line", "polygon", "polygon", "text"];
    assert_eq!(names(groups[1]), want);
    let (lo, hi) = doc
        .entities()
        .filter(|e| e.layer < 2 && !e.shape.is_solid())
        .map(|e| e.shape.bbox())
        .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
        .unwrap();
    let vb: Vec<f64> = root.attribute("viewBox").unwrap().split(' ').map(|v| v.parse().unwrap()).collect();
    assert!((vb[0] - (lo.x - 5.0)).abs() < 1e-5 && (vb[1] - (-hi.y - 5.0)).abs() < 1e-5);
    assert!((vb[2] - (hi.x - lo.x + 10.0)).abs() < 1e-5 && (vb[3] - (hi.y - lo.y + 10.0)).abs() < 1e-5);
    assert_eq!(root.attribute("width").unwrap(), format!("{}mm", num(vb[2])));
}

#[test]
fn layer_colour_name_and_hidden() {
    let svg = to_svg(&sample::doc(), 0);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let g: Vec<_> = xml.root_element().children().filter(|n| n.is_element()).collect();
    assert_eq!(g[0].attribute("stroke"), Some("#000000"));
    assert_eq!(g[1].attribute("stroke"), Some("#0a14c8"));
    assert_eq!(g[1].attribute("data-name"), Some("Walls & <Co>"));
    assert!(!svg.contains("500") && !svg.contains("900"));
}

#[test]
fn quarter_arc_path() {
    let svg = to_svg(&sample::doc(), 0);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let path = xml.descendants().find(|n| n.has_tag_name("path")).unwrap();
    assert_eq!(path.attribute("d"), Some("M 10 0 A 10 10 0 0 0 0 -10"));
}

#[test]
fn text_escapes_and_round_trips() {
    let svg = to_svg(&sample::doc(), 0);
    assert!(svg.contains("a&lt;&amp;&gt;&quot;b"));
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let t = xml.descendants().find(|n| n.has_tag_name("text")).unwrap();
    assert_eq!(t.text(), Some("a<&>\"b"));
    assert_eq!(t.attribute("y"), Some("20"));
    assert!(t.attribute("transform").unwrap().starts_with("rotate(-28.64789 10 20)"));
    let dim = xml.descendants().filter(|n| n.has_tag_name("text")).nth(1).unwrap();
    assert_eq!(dim.text(), Some("100.00"));
    assert_eq!(dim.attribute("text-anchor"), Some("middle"));
}

#[test]
fn empty_document_is_valid() {
    let svg = to_svg(&Document::new("e"), 0);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(xml.root_element().attribute("viewBox"), Some("0 0 100 100"));
    assert_eq!(xml.root_element().children().filter(|n| n.is_element()).count(), 0);
}

#[test]
fn layer_weight_and_dash_pattern() {
    use cad_document::{Command, Layer, LineType};
    let mut d = sample::doc();
    let dashed = Layer { line_type: LineType::Center, line_weight: 0.5, ..Layer::new(3, "D") };
    d.apply(Command::AddLayer(dashed)).unwrap();
    let id = d.alloc_id();
    let line = Shape::Line { a: DVec2::ZERO, b: DVec2::X };
    d.apply(Command::Add(cad_document::Entity { id, name: "l".into(), shape: line, layer: 3, sketch: 0 })).unwrap();
    let svg = to_svg(&d, 0);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let g: Vec<_> = xml.root_element().children().filter(|n| n.is_element()).collect();
    assert_eq!(g[0].attribute("stroke-width"), Some("0.25"));
    assert_eq!(g[0].attribute("stroke-dasharray"), None);
    assert_eq!(g[2].attribute("stroke-width"), Some("0.5"));
    assert_eq!(g[2].attribute("stroke-dasharray"), Some("8 1.5 1.5 1.5"));
}

#[test]
fn ellipse_and_spline_elements() {
    use cad_document::{Command, Entity};
    let mut d = Document::new("c");
    let shapes = [
        Shape::Ellipse { center: DVec2::new(10.0, 20.0), rx: 6.0, ry: 3.0, angle: FRAC_PI_2 },
        Shape::Spline { points: vec![DVec2::ZERO, DVec2::new(5.0, 5.0), DVec2::new(10.0, 0.0)], closed: false },
    ];
    for shape in shapes {
        let id = d.alloc_id();
        d.apply(Command::Add(Entity { id, name: "e".into(), shape, layer: 0, sketch: 0 })).unwrap();
    }
    let svg = to_svg(&d, 0);
    assert!(svg.contains("<ellipse cx=\"10\" cy=\"-20\" rx=\"6\" ry=\"3\" transform=\"rotate(-90 10 -20)\"/>"), "{svg}");
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let pl = xml.descendants().find(|n| n.has_tag_name("polyline")).unwrap();
    assert_eq!(pl.attribute("points").unwrap().split(' ').count(), 2 * 16 + 1);
}

#[test]
fn annotation_shapes_elements() {
    let svg = to_svg(&sample::annot_doc(), 0);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let g = xml.root_element().children().find(|n| n.is_element()).unwrap();
    let count = |tag: &str| names(g).iter().filter(|n| **n == tag).count();
    // hatch: ring + 5 lines; leader: polyline, arrow, text; radial: line, arrow, text; angular: arc, 2 arrows, text
    assert_eq!((count("polyline"), count("line"), count("polygon"), count("text")), (3, 6, 4, 3));
    let texts: Vec<_> = g.children().filter(|n| n.has_tag_name("text")).map(|n| n.text().unwrap()).collect();
    assert_eq!(texts, ["Note", "R20.00", "90.00\u{b0}"]);
    let angular = g.children().filter(|n| n.has_tag_name("text")).nth(2).unwrap();
    assert_eq!(angular.attribute("text-anchor"), Some("middle"));
    let radial = g.children().filter(|n| n.has_tag_name("text")).nth(1).unwrap();
    assert_eq!(radial.attribute("text-anchor"), None);
}

#[test]
fn only_the_requested_sketch_is_exported() {
    use cad_document::{BasePlane, Command, Entity, PlaneDef, Sketch};
    let mut d = sample::doc();
    let plane = PlaneDef::Principal { base: BasePlane::XZ, offset: "0".into() };
    d.apply(Command::AddSketch(Sketch { id: 1, name: "S1".into(), plane })).unwrap();
    let id = d.alloc_id();
    let circle = Shape::Circle { center: DVec2::new(7.0, 8.0), radius: 3.0 };
    d.apply(Command::Add(Entity { id, name: "c".into(), shape: circle, layer: 0, sketch: 1 })).unwrap();
    let svg = to_svg(&d, 1);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let children: Vec<_> = xml.descendants().filter(|n| n.is_element() && n.has_tag_name("circle")).collect();
    assert_eq!(children.len(), 1);
    assert_eq!(xml.descendants().filter(|n| n.is_element() && n.has_tag_name("g")).count(), 1);
    assert!(!to_svg(&d, 0).contains("cx=\"7\""));
    let dxf = crate::to_dxf(&d, 1);
    assert_eq!(dxf.matches("CIRCLE").count(), 1);
    assert_eq!(dxf.matches("\nLINE\n").count(), 0);
}
