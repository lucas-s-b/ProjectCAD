use super::*;
use crate::to_svg;
use cad_geometry::{DVec2, Shape};
use std::f64::consts::{FRAC_PI_2, PI};

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn near(a: DVec2, b: DVec2, tol: f64) -> bool {
    a.distance(b) <= tol
}
fn shapes(d: &Document) -> Vec<Shape> {
    d.entities().map(|e| e.shape.clone()).collect()
}
fn import(svg: &str) -> (Document, ImportReport) {
    from_svg(svg, "t").unwrap()
}
/// Distance from `p` to the nearest segment of the path.
fn dist_to_path(points: &[DVec2], p: DVec2) -> f64 {
    points.windows(2).map(|w| cad_geometry::point_segment_distance(p, w[0], w[1])).fold(f64::INFINITY, f64::min)
}

#[test]
fn our_own_export_comes_back() {
    let mut d = Document::new("rt");
    let mut walls = Layer::new(1, "Walls");
    walls.color = [10, 20, 200];
    d.apply(Command::AddLayer(walls)).unwrap();
    let list = [
        (0, Shape::Line { a: v2(1.0, 2.0), b: v2(30.5, -4.25) }),
        (0, Shape::Circle { center: v2(50.0, 20.0), radius: 7.5 }),
        (1, Shape::rect(v2(-5.0, 3.0), v2(20.0, 30.0))),
        (1, Shape::Polyline { points: vec![v2(0.0, 0.0), v2(5.0, 5.5), v2(10.0, 0.0)], closed: true }),
        (1, Shape::Polyline { points: vec![v2(0.0, 10.0), v2(5.0, 15.5), v2(10.0, 10.0)], closed: false }),
        (1, Shape::Arc { center: v2(0.0, 0.0), radius: 10.0, start: 0.0, sweep: FRAC_PI_2 }),
        (1, Shape::Arc { center: v2(5.0, 5.0), radius: 10.0, start: FRAC_PI_2, sweep: 3.0 * FRAC_PI_2 }),
        (1, Shape::Ellipse { center: v2(10.0, 20.0), rx: 6.0, ry: 3.0, angle: 0.7 }),
        (1, Shape::Text { pos: v2(10.0, -20.0), height: 5.0, angle: 0.5, text: "Hello".into() }),
    ];
    for (layer, shape) in &list {
        let id = d.alloc_id();
        d.apply(Command::Add(Entity { id, name: "e".into(), shape: shape.clone(), layer: *layer, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
    }
    let (back, report) = import(&to_svg(&d, 0));
    assert_eq!((report.imported, report.skipped.len()), (list.len(), 0), "{report:?}");
    let got: Vec<(String, Shape)> = back.entities().map(|e| (back.layer(e.layer).unwrap().name.clone(), e.shape.clone())).collect();
    for ((layer, want), (name, shape)) in list.iter().zip(&got) {
        assert_eq!(name, if *layer == 0 { "0" } else { "Walls" });
        match (want, shape) {
            (Shape::Line { a, b }, Shape::Line { a: a2, b: b2 }) => assert!(near(*a, *a2, 1e-6) && near(*b, *b2, 1e-6)),
            (Shape::Circle { center, radius }, Shape::Circle { center: c2, radius: r2 }) => {
                assert!(near(*center, *c2, 1e-6) && (radius - r2).abs() < 1e-6);
            }
            (Shape::Rect { min, max }, Shape::Rect { min: m2, max: x2 }) => assert!(near(*min, *m2, 1e-6) && near(*max, *x2, 1e-6)),
            (Shape::Polyline { points, closed }, Shape::Polyline { points: p2, closed: c2 }) => {
                assert_eq!(closed, c2);
                assert!(points.iter().zip(p2).all(|(a, b)| near(*a, *b, 1e-6)));
            }
            (Shape::Arc { center, radius, start, sweep }, Shape::Arc { center: c2, radius: r2, start: s2, sweep: w2 }) => {
                assert!(near(*center, *c2, 1e-6) && (radius - r2).abs() < 1e-6, "{shape:?}");
                assert!((start - s2).abs() < 1e-6 && (sweep - w2).abs() < 1e-6, "{shape:?}");
            }
            (Shape::Ellipse { center, rx, ry, angle }, Shape::Ellipse { center: c2, rx: x2, ry: y2, angle: a2 }) => {
                assert!(near(*center, *c2, 1e-6) && (rx - x2).abs() < 1e-6 && (ry - y2).abs() < 1e-6);
                assert!((angle - a2).abs() < 1e-6, "{angle} {a2}");
            }
            (Shape::Text { pos, height, angle, text }, Shape::Text { pos: p2, height: h2, angle: a2, text: t2 }) => {
                assert!(near(*pos, *p2, 1e-6) && (height - h2).abs() < 1e-6 && (angle - a2).abs() < 1e-6 && text == t2);
            }
            other => panic!("{other:?}"),
        }
    }
    let walls = back.layers().find(|l| l.name == "Walls").unwrap();
    assert_eq!(walls.color, [10, 20, 200]);
    assert_eq!(back.layers().count(), 2);
}

const NESTED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200mm" height="200mm" viewBox="0 0 200 200">
  <g id="outline" stroke="#ff8000" transform="translate(10 20)">
    <g transform="scale(2)"><line x1="0" y1="0" x2="5" y2="0"/></g>
    <path d="m 0 0 l 10 0 v 10 h -10 z"/>
    <path d="M 0 0 A 5 5 0 0 1 10 0"/>
    <path d="M 0 0 C 0 10 10 10 10 0"/>
    <path d="M 0 0 10 0 10 10"/>
  </g>
  <g transform="matrix(2 0 0 1 0 0)"><circle cx="10" cy="10" r="3"/><line x1="0" y1="0" x2="1" y2="1"/></g>
  <text x="0" y="0" font-size="4" transform="rotate(30)">Hi <tspan>there</tspan></text>
  <rect x="1" y="1" width="4" height="2"/>
  <rect x="0" y="0" width="4" height="2" transform="rotate(45)"/>
  <rect x="0" y="0" width="4" height="2" rx="1"/>
</svg>"##;

#[test]
fn nested_transforms_relative_paths_arcs_and_curves() {
    let (d, report) = import(NESTED);
    assert_eq!(report.skipped.iter().map(|(k, n)| (k.as_str(), *n)).collect::<Vec<_>>(), [("rect", 1)]);
    let all = shapes(&d);
    assert_eq!(all.len(), 10, "{all:?}");
    // scale(2) inside translate(10 20), y flipped
    assert!(matches!(&all[0], Shape::Line { a, b } if near(*a, v2(10.0, -20.0), 1e-9) && near(*b, v2(20.0, -20.0), 1e-9)), "{:?}", all[0]);
    // relative path with z: a closed square
    let Shape::Polyline { points, closed: true } = &all[1] else { panic!("{:?}", all[1]) };
    let want = [v2(10.0, -20.0), v2(20.0, -20.0), v2(20.0, -30.0), v2(10.0, -30.0)];
    assert!(points.len() == 4 && points.iter().zip(want).all(|(a, b)| near(*a, b, 1e-9)), "{points:?}");
    // the half circle over the top: centre (15, -20), radius 5, through (15, -15)
    let Shape::Arc { center, radius, start, sweep } = &all[2] else { panic!("{:?}", all[2]) };
    assert!(near(*center, v2(15.0, -20.0), 1e-9) && (radius - 5.0).abs() < 1e-9 && (sweep - PI).abs() < 1e-9);
    let mid = *center + DVec2::from_angle(start + sweep / 2.0) * *radius;
    assert!(near(mid, v2(15.0, -15.0), 1e-9), "{mid:?}");
    // cubic: end points, and the curve's middle (15, -27.5) lies within 0.02 of the flattened path
    let Shape::Polyline { points, closed: false } = &all[3] else { panic!("{:?}", all[3]) };
    assert!(near(points[0], v2(10.0, -20.0), 1e-9) && near(*points.last().unwrap(), v2(20.0, -20.0), 1e-9));
    assert!(dist_to_path(points, v2(15.0, -27.5)) < 0.02);
    assert!(points.len() > 8);
    // a move followed by bare coordinates is a polyline
    assert!(matches!(&all[4], Shape::Polyline { points, closed: false } if points.len() == 3));
    // non-uniform scale: the circle becomes a polyline of an ellipse 12 wide and 6 high, the line stays exact
    let Shape::Polyline { points, closed: true } = &all[5] else { panic!("{:?}", all[5]) };
    assert!(points.iter().all(|p| (((p.x - 20.0) / 6.0).powi(2) + ((p.y + 10.0) / 3.0).powi(2) - 1.0).abs() < 1e-9));
    assert!(points.iter().map(|p| p.x).fold(0.0_f64, f64::max) > 25.99 && points.len() > 20);
    assert!(matches!(&all[6], Shape::Line { a, b } if near(*a, v2(0.0, 0.0), 1e-9) && near(*b, v2(2.0, -1.0), 1e-9)));
    let Shape::Text { pos, height, angle, text } = &all[7] else { panic!("{:?}", all[7]) };
    assert!(near(*pos, DVec2::ZERO, 1e-9) && (height - 4.0).abs() < 1e-9 && (angle + PI / 6.0).abs() < 1e-9, "{angle}");
    assert_eq!(text, "Hi there");
    assert!(matches!(&all[8], Shape::Rect { min, max } if near(*min, v2(1.0, -3.0), 1e-9) && near(*max, v2(5.0, -1.0), 1e-9)));
    assert!(matches!(&all[9], Shape::Polyline { points, closed: true } if points.len() == 4));
    let layer = d.layers().find(|l| l.name == "outline").unwrap();
    assert_eq!(layer.color, [255, 128, 0]);
    // the first five shapes are in the group's layer, the rest on layer 0
    assert_eq!(d.entities().map(|e| e.layer).collect::<Vec<_>>(), [layer.id, layer.id, layer.id, layer.id, layer.id, 0, 0, 0, 0, 0]);
}

#[test]
fn width_and_view_box_set_the_scale() {
    let line = |attrs: &str| {
        let (d, _) = import(&format!(r#"<svg xmlns="http://www.w3.org/2000/svg" {attrs}><line x1="0" y1="0" x2="100" y2="50"/></svg>"#));
        let Shape::Line { b, .. } = shapes(&d)[0] else { panic!() };
        b
    };
    assert!(near(line(r#"width="50mm" viewBox="0 0 100 100""#), v2(50.0, -25.0), 1e-9));
    assert!(near(line(r#"width="1in" viewBox="0 0 10 10""#), v2(254.0, -127.0), 1e-9));
    assert!(near(line(r#"width="5cm" viewBox="0 0 100 100""#), v2(50.0, -25.0), 1e-9));
    assert!(near(line(r#"width="96px" viewBox="0 0 96 96""#), v2(100.0 * 25.4 / 96.0, -50.0 * 25.4 / 96.0), 1e-9));
    assert!(near(line(r#"width="72pt" viewBox="0 0 1 1""#), v2(2540.0, -1270.0), 1e-9));
    assert!(near(line(r#"viewBox="0 0 10 10""#), v2(100.0, -50.0), 1e-9));
    assert!(near(line(r#"width="50mm""#), v2(100.0, -50.0), 1e-9));
    assert!(near(line(""), v2(100.0, -50.0), 1e-9));
}

#[test]
fn unknown_and_invalid_elements_are_counted() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
      <image href="a.png"/><use href="#x"/><use href="#y"/>
      <linearGradient id="g"/>
      <circle cx="0" cy="0" r="-1"/>
      <line x1="1" y1="1" x2="1" y2="1"/>
      <path d="M 0 0 L"/>
      <g transform="skewX(10)"><line x1="0" y1="0" x2="1" y2="0"/></g>
      <circle cx="3" cy="4" r="2"/>
    </svg>"##;
    let (d, report) = import(svg);
    assert_eq!(d.len(), 1);
    assert_eq!(report.imported, 1);
    let skipped: Vec<_> = report.skipped.iter().map(|(k, n)| (k.as_str(), *n)).collect();
    assert_eq!(skipped, [("image", 1), ("invalid", 4), ("linearGradient", 1), ("use", 2)]);
}

#[test]
fn malformed_input_is_a_format_error() {
    for bad in ["<svg", "", "not xml at all", "<svg><g></svg>", "<html></html>"] {
        assert!(matches!(from_svg(bad, "x"), Err(ImportError::Format(_))), "{bad:?}");
    }
    assert!(from_svg("<html/>", "x").unwrap_err().to_string().starts_with("not an SVG file"));
    let (d, report) = import(r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#);
    assert!(d.is_empty() && report == ImportReport::default());
}

#[test]
fn path_commands_smooth_curves_and_arc_flags() {
    let (d, report) = import(
        r#"<svg xmlns="http://www.w3.org/2000/svg">
        <path d="M0,0 Q 5,10 10,0 T 20,0"/>
        <path d="M0 0 C 0 5 5 5 5 0 S 10 -5 10 0"/>
        <path d="M0 0 a5 5 0 1 0 10 0"/>
        <path d="M0 0 A 5 3 0 0 1 10 0"/>
        <path d="M 0 0 H 10 V 10 L 0 10 Z M 20 0 L 30 0"/>
      </svg>"#,
    );
    assert_eq!(report.skipped.len(), 0, "{report:?}");
    let all = shapes(&d);
    assert_eq!(all.len(), 6);
    let Shape::Polyline { points, .. } = &all[0] else { panic!() };
    // the smooth quadratic mirrors the control point: the humps lie on opposite sides of the axis
    assert!(points.iter().any(|p| p.y < -2.0 && p.x < 10.0) && points.iter().any(|p| p.y > 2.0 && p.x > 10.0));
    assert!(near(*points.last().unwrap(), v2(20.0, 0.0), 1e-9));
    let Shape::Polyline { points, .. } = &all[1] else { panic!() };
    assert!(near(*points.last().unwrap(), v2(10.0, 0.0), 1e-9));
    // a1 1 0: radius 5 from (0,0) to (10,0) is a half circle either way; flag 0 turns it the other way round than flag 1
    assert!(matches!(&all[2], Shape::Arc { radius, .. } if (radius - 5.0).abs() < 1e-9));
    // an elliptical arc is flattened
    assert!(matches!(&all[3], Shape::Polyline { closed: false, .. }));
    assert!(matches!(&all[4], Shape::Polyline { points, closed: true } if points.len() == 4));
    assert!(matches!(&all[5], Shape::Line { .. }));
}
