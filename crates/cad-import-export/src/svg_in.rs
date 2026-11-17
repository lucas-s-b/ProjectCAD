//! SVG import: line, rect, circle, ellipse, polyline, polygon, path and text, with nested transforms and top-level groups as layers.
use crate::svg_curve::{ArcGeom, sample_arc};
use crate::svg_path::path_shapes;
use crate::svg_xform::{Scan, parse_transform, user_unit_mm};
use crate::{ImportError, ImportReport};
use cad_document::{Command, Document, Entity, Layer, LayerId};
use cad_geometry::{DAffine2, DVec2, Shape};
use roxmltree::Node;
use std::collections::BTreeMap;
use std::f64::consts::TAU;

struct Builder {
    doc: Document,
    layers: BTreeMap<String, LayerId>,
    report: ImportReport,
}

/// A numeric attribute; absent means 0, an unreadable value is an error.
fn attr(n: Node, name: &str) -> Result<f64, ()> {
    n.attribute(name).map_or(Ok(0.0), |v| Scan::new(v).number().ok_or(()))
}

/// `#rrggbb` as a colour.
fn hex_color(text: &str) -> Option<[u8; 3]> {
    let h = text.strip_prefix('#').filter(|h| h.len() == 6 && h.is_ascii())?;
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// The outline of an ellipse under a transform that does not keep it an ellipse.
fn flattened_ellipse(center: DVec2, rx: f64, ry: f64, m: &DAffine2) -> Shape {
    let mut points = sample_arc(&ArcGeom { center, rx, ry, phi: 0.0, theta: 0.0, delta: TAU }, m);
    points.pop();
    Shape::Polyline { points, closed: true }
}

fn polygon(corners: &[DVec2], m: &DAffine2, closed: bool) -> Shape {
    Shape::Polyline { points: corners.iter().map(|p| m.transform_point2(*p)).collect(), closed }
}

impl Builder {
    fn skip(&mut self, key: &str) {
        *self.report.skipped.entry(key.to_string()).or_default() += 1;
    }

    fn add(&mut self, layer: LayerId, shape: Option<Shape>) {
        let id = self.doc.alloc_id();
        let ok = shape.is_some_and(|shape| {
            let name = format!("{} {id}", shape.kind());
            self.doc.apply(Command::Add(Entity { id, name, shape, layer, sketch: 0, construction: false, group: None, meta: Default::default() })).is_ok()
        });
        if ok {
            self.report.imported += 1;
        } else {
            self.skip("invalid");
        }
    }

    /// The layer a top-level group stands for: its `data-name` or `id`, coloured by its stroke.
    fn layer_of(&mut self, g: Node) -> LayerId {
        let Some(name) = g.attribute("data-name").or(g.attribute("id")).filter(|n| !n.trim().is_empty()) else { return 0 };
        if name == "0" {
            return 0;
        }
        if let Some(id) = self.layers.get(name) {
            return *id;
        }
        let id = self.doc.alloc_layer_id();
        let mut layer = Layer::new(id, name);
        if let Some(c) = g.attribute("stroke").and_then(hex_color) {
            layer.color = c;
        }
        let _ = self.doc.apply(Command::AddLayer(layer));
        self.layers.insert(name.to_string(), id);
        id
    }

    fn element(&mut self, n: Node, parent: DAffine2, layer: LayerId) {
        let tag = n.tag_name().name();
        let m = match n.attribute("transform").map(parse_transform) {
            None => parent,
            Some(Some(t)) => parent * t,
            Some(None) => return self.add(layer, None),
        };
        let shapes = match tag {
            "g" => {
                for child in n.children().filter(Node::is_element) {
                    self.element(child, m, layer);
                }
                return;
            }
            "path" => match n.attribute("d").and_then(|d| path_shapes(d, &m)) {
                Some(shapes) => shapes,
                None => vec![None],
            },
            "line" | "rect" | "circle" | "ellipse" | "polyline" | "polygon" | "text" => {
                if tag == "rect" && (attr(n, "rx").is_ok_and(|v| v > 0.0) || attr(n, "ry").is_ok_and(|v| v > 0.0)) {
                    return self.skip("rect");
                }
                vec![primitive(n, tag, &m)]
            }
            other => return self.skip(other),
        };
        for shape in shapes {
            self.add(layer, shape);
        }
    }
}

/// The shape of a basic element, or None when its attributes are unusable.
fn primitive(n: Node, tag: &str, m: &DAffine2) -> Option<Shape> {
    let a = |name| attr(n, name).ok();
    let point = |x, y| Some(DVec2::new(a(x)?, a(y)?));
    match tag {
        "line" => Some(Shape::Line { a: m.transform_point2(point("x1", "y1")?), b: m.transform_point2(point("x2", "y2")?) }),
        "polyline" | "polygon" => {
            let nums = Scan::new(n.attribute("points")?).all_numbers().filter(|v| v.len() % 2 == 0)?;
            let pts: Vec<DVec2> = nums.chunks(2).map(|c| DVec2::new(c[0], c[1])).collect();
            Some(polygon(&pts, m, tag == "polygon"))
        }
        "rect" => {
            let (p, w, h) = (point("x", "y")?, a("width")?, a("height")?);
            if w <= 0.0 || h <= 0.0 {
                return None;
            }
            let rect = Shape::rect(p, p + DVec2::new(w, h));
            // a turned or skewed rectangle is a closed polyline
            rect.transformed(m).ok().or_else(|| Some(polygon(&[p, p + DVec2::new(w, 0.0), p + DVec2::new(w, h), p + DVec2::new(0.0, h)], m, true)))
        }
        "circle" => {
            let (c, r) = (point("cx", "cy")?, a("r")?);
            Shape::Circle { center: c, radius: r }.transformed(m).ok().or_else(|| Some(flattened_ellipse(c, r, r, m)))
        }
        "ellipse" => {
            let (c, rx, ry) = (point("cx", "cy")?, a("rx")?, a("ry")?);
            Shape::Ellipse { center: c, rx, ry, angle: 0.0 }.transformed(m).ok().or_else(|| Some(flattened_ellipse(c, rx, ry, m)))
        }
        _ => {
            let text: String = n.descendants().filter(Node::is_text).filter_map(|t| t.text()).collect();
            let size = Scan::new(n.attribute("font-size")?).number()?;
            let first = |name: &str| n.attribute(name).map_or(Some(0.0), |v| Scan::new(v).number());
            Shape::Text { pos: DVec2::new(first("x")?, first("y")?), height: size, angle: 0.0, text: text.trim().to_string() }.transformed(m).ok()
        }
    }
}

/// Imports an SVG drawing into a new document; one unit is a millimetre unless `width` and `viewBox` say otherwise.
pub fn from_svg(text: &str, name: &str) -> Result<(Document, ImportReport), ImportError> {
    let options = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let xml = roxmltree::Document::parse_with_options(text, options).map_err(|e| ImportError::Format(format!("not an SVG file: {e}")))?;
    let root = xml.root_element();
    if root.tag_name().name() != "svg" {
        return Err(ImportError::Format(format!("not an SVG file: the root element is <{}>", root.tag_name().name())));
    }
    let unit = user_unit_mm(root.attribute("width"), root.attribute("viewBox"));
    // user space has y down: the flip and the unit scale come last
    let base = DAffine2::from_scale(DVec2::new(unit, -unit));
    let mut b = Builder { doc: Document::new(name), layers: BTreeMap::new(), report: ImportReport::default() };
    for child in root.children().filter(Node::is_element) {
        let layer = if child.tag_name().name() == "g" { b.layer_of(child) } else { 0 };
        b.element(child, base, layer);
    }
    Ok((b.doc, b.report))
}

#[cfg(test)]
#[path = "svg_in_tests.rs"]
mod tests;
