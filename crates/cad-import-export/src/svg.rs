use crate::{exported, flattened, num};
use cad_document::{Document, SketchId};
use cad_geometry::annot::{Align, fills, labels, strokes};
use cad_geometry::{DVec2, Shape};
use std::f64::consts::{PI, TAU};
use std::fmt::Write;

const MARGIN: f64 = 5.0;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn color(c: [u8; 3]) -> String {
    let c = crate::paper_color(c);
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// World point as "x y" with Y flipped.
fn xy(p: DVec2) -> String {
    format!("{} {}", num(p.x), num(-p.y))
}

fn line(out: &mut String, p: DVec2, q: DVec2) {
    let _ = writeln!(out, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>", num(p.x), num(-p.y), num(q.x), num(-q.y));
}

fn circle(out: &mut String, c: DVec2, r: f64) {
    let _ = writeln!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>", num(c.x), num(-c.y), num(r));
}

fn points(pts: &[DVec2]) -> String {
    pts.iter().map(|p| format!("{},{}", num(p.x), num(-p.y))).collect::<Vec<_>>().join(" ")
}

fn text(out: &mut String, at: DVec2, size: f64, angle: f64, fill: &str, extra: &str, s: &str) {
    let (x, y) = (num(at.x), num(-at.y));
    let _ = writeln!(
        out,
        "<text x=\"{x}\" y=\"{y}\" font-size=\"{}\" font-family=\"sans-serif\" fill=\"{fill}\" stroke=\"none\"{extra} transform=\"rotate({} {x} {y})\">{}</text>",
        num(size),
        num(-angle.to_degrees()),
        escape(s)
    );
}

fn shape(out: &mut String, s: &Shape, col: &str, doc: &Document) {
    match s {
        Shape::Line { a, b } => line(out, *a, *b),
        Shape::Circle { center, radius } => circle(out, *center, *radius),
        Shape::Rect { min, max } => {
            let _ = writeln!(
                out,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                num(min.x),
                num(-max.y),
                num(max.x - min.x),
                num(max.y - min.y)
            );
        }
        Shape::Polyline { points: p, closed } => {
            let _ = writeln!(out, "<{} points=\"{}\"/>", if *closed { "polygon" } else { "polyline" }, points(p));
        }
        Shape::Wall(wall) => {
            let _ = writeln!(out, "<polygon points=\"{}\"/>", points(&wall.corners()));
        }
        // shortcut: openings are not exported; draw gaps and symbols when DXF plans are needed
        Shape::Opening(_) => {}
        Shape::Slab(s) => {
            let _ = writeln!(out, "<polygon points=\"{}\"/>", points(&s.outline));
        }
        Shape::Roof(r) => {
            for p in r.pieces() {
                let _ = writeln!(out, "<polygon points=\"{}\"/>", points(&p.outline));
            }
        }
        Shape::Stair(st) => {
            for [p, q] in st.symbol() {
                line(out, p, q);
            }
        }
        // shortcut: room tags are not exported; write TEXT when SVG plans need them
        Shape::Room(_) => {}
        Shape::Arc { center, radius, .. } if shape_is_full(s) => circle(out, *center, *radius),
        Shape::Arc { center, radius, start, sweep } => {
            let at = |a: f64| *center + DVec2::from_angle(a) * *radius;
            let large = u8::from(*sweep > PI);
            // Y is flipped, so a counter-clockwise world arc is sweep-flag 0
            let _ = writeln!(out, "<path d=\"M {} A {r} {r} 0 {large} 0 {}\"/>", xy(at(*start)), xy(at(start + sweep)), r = num(*radius));
        }
        Shape::Ellipse { center, rx, ry, angle } => {
            let (x, y) = (num(center.x), num(-center.y));
            let _ = writeln!(
                out,
                "<ellipse cx=\"{x}\" cy=\"{y}\" rx=\"{}\" ry=\"{}\" transform=\"rotate({} {x} {y})\"/>",
                num(*rx),
                num(*ry),
                num(-angle.to_degrees())
            );
        }
        Shape::Spline { closed, .. } => {
            let tag = if *closed { "polygon" } else { "polyline" };
            let _ = writeln!(out, "<{tag} points=\"{}\"/>", points(&flattened(s, *closed)));
        }
        Shape::Text { .. }
        | Shape::Dimension { .. }
        | Shape::Hatch { .. }
        | Shape::Leader { .. }
        | Shape::RadialDim { .. }
        | Shape::AngularDim { .. } => annotation(out, s, col, doc),
        Shape::Box { .. } | Shape::Cylinder { .. } | Shape::Sphere { .. } => {}
    }
}

/// Strokes, filled arrows and labels of an annotation shape.
fn annotation(out: &mut String, s: &Shape, col: &str, doc: &Document) {
    for p in strokes(s) {
        if let [a, b] = p[..] {
            line(out, a, b);
        } else {
            let _ = writeln!(out, "<polyline points=\"{}\"/>", points(&p));
        }
    }
    for t in fills(s) {
        let _ = writeln!(out, "<polygon points=\"{}\" fill=\"{col}\" stroke=\"none\"/>", points(&t));
    }
    for l in labels(s) {
        let extra = match l.align {
            Align::Left => "",
            Align::Center => " text-anchor=\"middle\" dominant-baseline=\"middle\"",
            Align::Right => " text-anchor=\"end\"",
        };
        text(out, l.anchor, l.height, l.angle, col, extra, &doc.settings().label_text(&l.text));
    }
}

fn shape_is_full(s: &Shape) -> bool {
    matches!(s, Shape::Arc { sweep, .. } if (sweep - TAU).abs() < 1e-9)
}

pub fn to_svg(doc: &Document, sketch: SketchId) -> String {
    let layers = exported(doc, sketch);
    let bounds = layers
        .iter()
        .flat_map(|(_, es)| es.iter().map(|e| e.shape.bbox()))
        .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)));
    let (min, size) = match bounds {
        Some((lo, hi)) => (DVec2::new(lo.x - MARGIN, -hi.y - MARGIN), (hi - lo).truncate() + DVec2::splat(2.0 * MARGIN)),
        None => (DVec2::ZERO, DVec2::splat(100.0)),
    };
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"{} {} {w} {h}\">",
        num(min.x),
        num(min.y),
        w = num(size.x),
        h = num(size.y)
    );
    for (layer, entities) in layers {
        let col = color(layer.color);
        let dash = match layer.line_type.pattern_mm() {
            [] => String::new(),
            p => format!(" stroke-dasharray=\"{}\"", p.iter().map(|v| num(*v)).collect::<Vec<_>>().join(" ")),
        };
        let _ = writeln!(
            out,
            "<g id=\"layer-{}\" data-name=\"{}\" stroke=\"{col}\" fill=\"none\" stroke-width=\"{}\"{dash}>",
            layer.id,
            escape(&layer.name),
            num(layer.line_weight)
        );
        for e in entities {
            shape(&mut out, &e.shape, &col, doc);
        }
        out.push_str("</g>\n");
    }
    out.push_str("</svg>\n");
    out
}

#[cfg(test)]
#[path = "svg_tests.rs"]
mod tests;
