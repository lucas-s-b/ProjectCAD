use crate::{exported, flattened, label_origin, num};
use cad_document::{Document, LayerId, LineType, SketchId};
use cad_geometry::annot::{fills, labels, strokes};
use cad_geometry::{DVec2, Shape};
use std::f64::consts::TAU;
use std::fmt::{Display, Write};

/// AutoCAD colour index candidates: (index, rgb).
pub(crate) const ACI: [(u8, [i32; 3]); 9] = [
    (1, [255, 0, 0]),
    (2, [255, 255, 0]),
    (3, [0, 255, 0]),
    (4, [0, 255, 255]),
    (5, [0, 0, 255]),
    (6, [255, 0, 255]),
    (7, [255, 255, 255]),
    (8, [128, 128, 128]),
    (9, [192, 192, 192]),
];

fn aci(c: [u8; 3]) -> u8 {
    let dist = |rgb: &[i32; 3]| (0..3).map(|i| (rgb[i] - i32::from(c[i])).pow(2)).sum::<i32>();
    ACI.iter().min_by_key(|(_, rgb)| dist(rgb)).map_or(7, |(i, _)| *i)
}

fn layer_names(doc: &Document) -> Vec<(LayerId, String)> {
    let mut used: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for l in doc.layers() {
        let mut name: String = l
            .name
            .to_uppercase()
            .chars()
            .map(|c| if c.is_ascii_uppercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '$') { c } else { '_' })
            .collect();
        if name.is_empty() {
            name = format!("LAYER_{}", l.id);
        } else if used.contains(&name) {
            name = format!("{name}_{}", l.id);
        }
        used.push(name.clone());
        out.push((l.id, name));
    }
    out
}

struct W(String);

impl W {
    fn g(&mut self, code: u16, v: impl Display) {
        let _ = write!(self.0, "{code}\n{v}\n");
    }
    fn n(&mut self, code: u16, v: f64) {
        self.g(code, num(v));
    }
    /// Groups code, code + 10, code + 20 with Z = 0.
    fn pt(&mut self, code: u16, p: DVec2) {
        self.n(code, p.x);
        self.n(code + 10, p.y);
        self.n(code + 20, 0.0);
    }
    fn ent(&mut self, kind: &str, layer: &str) {
        self.g(0, kind);
        self.g(8, layer);
    }
    fn line(&mut self, layer: &str, a: DVec2, b: DVec2) {
        self.ent("LINE", layer);
        self.pt(10, a);
        self.pt(11, b);
    }
    /// A triangle: the fourth corner repeats the third.
    fn solid(&mut self, layer: &str, t: &[DVec2; 3]) {
        self.ent("SOLID", layer);
        for (code, p) in [(10, t[0]), (11, t[1]), (12, t[2]), (13, t[2])] {
            self.pt(code, p);
        }
    }
    fn circle(&mut self, layer: &str, c: DVec2, r: f64) {
        self.ent("CIRCLE", layer);
        self.pt(10, c);
        self.n(40, r);
    }
    fn polyline(&mut self, layer: &str, pts: &[DVec2], closed: bool) {
        self.ent("POLYLINE", layer);
        self.g(66, 1);
        self.g(70, u8::from(closed));
        for p in pts {
            self.ent("VERTEX", layer);
            self.pt(10, *p);
        }
        self.ent("SEQEND", layer);
    }
    fn text(&mut self, layer: &str, pos: DVec2, height: f64, angle: f64, s: &str) {
        self.ent("TEXT", layer);
        self.pt(10, pos);
        self.n(40, height);
        self.g(1, s.replace(['\r', '\n'], " "));
        self.n(50, angle.to_degrees());
    }
}

fn degrees(a: f64) -> f64 {
    let d = a.to_degrees().rem_euclid(360.0);
    if d >= 360.0 { 0.0 } else { d }
}

fn shape(w: &mut W, s: &Shape, layer: &str, doc: &Document) {
    match s {
        Shape::Line { a, b } => w.line(layer, *a, *b),
        Shape::Circle { center, radius } => w.circle(layer, *center, *radius),
        Shape::Arc { center, radius, .. } if matches!(s, Shape::Arc { sweep, .. } if (sweep - TAU).abs() < 1e-9) => {
            w.circle(layer, *center, *radius)
        }
        Shape::Arc { center, radius, start, sweep } => {
            w.ent("ARC", layer);
            w.pt(10, *center);
            w.n(40, *radius);
            w.n(50, degrees(*start));
            w.n(51, degrees(start + sweep));
        }
        Shape::Rect { min, max } => {
            let c = [*min, DVec2::new(max.x, min.y), *max, DVec2::new(min.x, max.y)];
            w.polyline(layer, &c, true);
        }
        Shape::Polyline { points, closed } => w.polyline(layer, points, *closed),
        Shape::Wall(wall) => w.polyline(layer, &wall.corners(), true),
        // shortcut: openings are not exported; draw gaps and symbols when DXF plans are needed
        Shape::Opening(_) => {}
        Shape::Slab(s) => w.polyline(layer, &s.outline, true),
        // shortcut: room tags are not exported; write TEXT when DXF plans need them
        Shape::Room(_) => {}
        // shortcut: R12 has no ELLIPSE or SPLINE entity, curves are exported as a flattened POLYLINE, upgrade with a newer DXF version
        Shape::Ellipse { .. } => w.polyline(layer, &flattened(s, true), true),
        Shape::Spline { closed, .. } => w.polyline(layer, &flattened(s, *closed), *closed),
        // shortcut: dimensions are exported exploded, real DIMENSION entities need block definitions
        Shape::Text { .. }
        | Shape::Dimension { .. }
        | Shape::Hatch { .. }
        | Shape::Leader { .. }
        | Shape::RadialDim { .. }
        | Shape::AngularDim { .. } => annotation(w, s, layer, doc),
        Shape::Box { .. } | Shape::Cylinder { .. } | Shape::Sphere { .. } => {}
    }
}

/// Strokes as LINE or open POLYLINE, arrow heads as 3 lines, solid hatch triangles as SOLID, labels as TEXT.
fn annotation(w: &mut W, s: &Shape, layer: &str, doc: &Document) {
    for p in strokes(s) {
        if let [a, b] = p[..] {
            w.line(layer, a, b);
        } else {
            w.polyline(layer, &p, false);
        }
    }
    for t in fills(s) {
        if matches!(s, Shape::Hatch { .. }) {
            w.solid(layer, &t);
            continue;
        }
        for i in 0..3 {
            w.line(layer, t[i], t[(i + 1) % 3]);
        }
    }
    for l in labels(s) {
        let value = doc.settings().label_text(&l.text)
            // R12 has no Unicode: AutoCAD control codes for the degree and diameter signs
            .replace('\u{b0}', "%%d")
            .replace('\u{d8}', "%%c");
        // DXF TEXT is anchored at its baseline-left
        w.text(layer, label_origin(&l, &value), l.height, l.angle, &value);
    }
}

pub fn to_dxf(doc: &Document, sketch: SketchId) -> String {
    let names = layer_names(doc);
    let name_of = |id: LayerId| names.iter().find(|(i, _)| *i == id).map_or("0", |(_, n)| n.as_str()).to_owned();
    let mut w = W(String::new());
    for g in [(0, "SECTION"), (2, "HEADER"), (9, "$ACADVER"), (1, "AC1009"), (0, "ENDSEC")] {
        w.g(g.0, g.1);
    }
    w.g(0, "SECTION");
    w.g(2, "TABLES");
    w.g(0, "TABLE");
    w.g(2, "LTYPE");
    w.g(70, LineType::ALL.len());
    for lt in LineType::ALL {
        let pat = lt.pattern_mm();
        w.g(0, "LTYPE");
        w.g(2, lt.name().to_uppercase());
        w.g(70, 64);
        w.g(3, lt.name());
        w.g(72, 65);
        w.g(73, pat.len());
        w.n(40, pat.iter().sum());
        for (i, v) in pat.iter().enumerate() {
            w.n(49, if i % 2 == 0 { *v } else { -*v });
        }
    }
    w.g(0, "ENDTAB");
    w.g(0, "TABLE");
    w.g(2, "LAYER");
    w.g(70, names.len());
    for l in doc.layers() {
        w.g(0, "LAYER");
        w.g(2, name_of(l.id));
        w.g(70, 0);
        w.g(62, aci(l.color));
        w.g(6, l.line_type.name().to_uppercase());
    }
    w.g(0, "ENDTAB");
    w.g(0, "ENDSEC");
    w.g(0, "SECTION");
    w.g(2, "ENTITIES");
    for (layer, entities) in exported(doc, sketch) {
        let name = name_of(layer.id);
        for e in entities {
            shape(&mut w, &e.shape, &name, doc);
        }
    }
    w.g(0, "ENDSEC");
    w.g(0, "EOF");
    w.0
}

#[cfg(test)]
#[path = "dxf_tests.rs"]
mod tests;
