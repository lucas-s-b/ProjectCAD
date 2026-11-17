//! Export of a document to SVG, DXF and PDF, and of solid bodies to STL and OBJ; import of DXF and SVG. Model coordinates are millimetres.
mod dxf;
mod dxf_in;
mod mesh_out;
mod pdf_out;
mod svg;
mod svg_curve;
mod svg_in;
mod svg_path;
mod svg_xform;

pub use dxf::to_dxf;
pub use dxf_in::{ImportError, ImportReport, from_dxf};
pub use mesh_out::{to_obj, to_stl};
pub use pdf_out::to_pdf;
pub use svg::to_svg;
pub use svg_in::from_svg;

use cad_document::{Document, Entity, Layer, SketchId};

/// At most 6 decimals, trailing zeros trimmed.
pub(crate) fn num(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".into() } else { s.into() }
}

/// Light screen colours would vanish on white paper.
pub(crate) fn paper_color(c: [u8; 3]) -> [u8; 3] {
    if c.iter().all(|v| *v >= 200) { [0, 0, 0] } else { c }
}

/// Baseline-left start of a label drawn in a format that anchors text there (DXF TEXT, PDF): the estimated box is moved so the label anchor lands where it belongs.
pub(crate) fn label_origin(l: &cad_geometry::annot::Label, value: &str) -> cad_geometry::DVec2 {
    use cad_geometry::annot::{Align, text_box};
    let b = text_box(cad_geometry::DVec2::ZERO, l.height, l.angle, value);
    match l.align {
        Align::Left => l.anchor,
        Align::Center => l.anchor - (b[0] + b[2]) / 2.0,
        Align::Right => l.anchor - (b[1] - b[0]),
    }
}

/// First outline path of a curve as 2D points; a closed path loses its repeated last point.
pub(crate) fn flattened(shape: &cad_geometry::Shape, closed: bool) -> Vec<cad_geometry::DVec2> {
    let mut path = cad_geometry::outline_paths(shape).swap_remove(0);
    if closed {
        path.pop();
    }
    path.iter().map(|p| p.truncate()).collect()
}

/// Visible layers that hold at least one non-solid, non-construction entity of the sketch, with those entities.
/// Entity groups and metadata are not part of SVG or DXF, so the exporters never read them.
pub(crate) fn exported(doc: &Document, sketch: SketchId) -> Vec<(&Layer, Vec<&Entity>)> {
    doc.layers()
        .filter(|l| l.visible)
        .map(|l| (l, doc.entities().filter(|e| e.sketch == sketch && e.layer == l.id && !e.shape.is_solid() && !e.construction).collect::<Vec<_>>()))
        .filter(|(_, es)| !es.is_empty())
        .collect()
}

#[cfg(test)]
pub(crate) mod sample {
    use cad_document::{Command, Document, Entity, Layer};
    use cad_geometry::{DVec2, DVec3, Shape};
    use std::f64::consts::FRAC_PI_2;

    pub fn v2(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }

    /// Hatch (5 lines), leader, radial dimension and angular dimension on layer 0.
    pub fn annot_doc() -> Document {
        let mut d = Document::new("annot");
        let shapes = [
            Shape::Hatch { boundary: vec![v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0), v2(0.0, 10.0)], angle: 0.0, spacing: 2.0, pattern: cad_geometry::HatchPattern::Lines },
            Shape::Leader { points: vec![v2(20.0, 0.0), v2(30.0, 10.0), v2(50.0, 10.0)], text: "Note".into(), text_height: 2.0 },
            Shape::RadialDim { center: v2(0.0, 30.0), radius: 20.0, angle: 0.5, text_height: 2.0, diameter: false },
            Shape::AngularDim { center: v2(60.0, 0.0), start: 0.0, sweep: FRAC_PI_2, radius: 15.0, text_height: 2.0 },
        ];
        for shape in shapes {
            let id = d.alloc_id();
            d.apply(Command::Add(Entity { id, name: format!("e{id}"), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
        }
        d
    }

    /// Layer 0 (light grey, 2 shapes + a box), layer 1 "Walls & <Co>" (blue, 5 shapes), hidden layer 2.
    pub fn doc() -> Document {
        let mut d = Document::new("sample");
        let mut walls = Layer::new(1, "Walls & <Co>");
        walls.color = [10, 20, 200];
        let mut hidden = Layer::new(2, "Hidden");
        hidden.visible = false;
        for l in [walls, hidden] {
            d.apply(Command::AddLayer(l)).unwrap();
        }
        let shapes = [
            (0, Shape::Line { a: v2(0.0, 0.0), b: v2(100.0, 0.0) }),
            (0, Shape::Circle { center: v2(50.0, 50.0), radius: 10.0 }),
            (1, Shape::rect(v2(0.0, 0.0), v2(20.0, 30.0))),
            (1, Shape::Arc { center: v2(0.0, 0.0), radius: 10.0, start: 0.0, sweep: FRAC_PI_2 }),
            (1, Shape::Polyline { points: vec![v2(0.0, 0.0), v2(5.0, 5.0), v2(10.0, 0.0)], closed: true }),
            (1, Shape::Text { pos: v2(10.0, -20.0), height: 5.0, angle: 0.5, text: "a<&>\"b".into() }),
            (1, Shape::Dimension { a: v2(0.0, 0.0), b: v2(100.0, 0.0), offset: 20.0, text_height: 4.0 }),
            (2, Shape::Line { a: v2(500.0, 500.0), b: v2(600.0, 600.0) }),
            (0, Shape::Box { center: DVec3::new(900.0, 900.0, 0.0), size: DVec3::splat(10.0) }),
        ];
        for (layer, shape) in shapes {
            let id = d.alloc_id();
            d.apply(Command::Add(Entity { id, name: format!("e{id}"), shape, layer, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
        }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_format() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(-0.0000001), "0");
        assert_eq!(num(0.1234567), "0.123457");
        assert_eq!(num(-2.5), "-2.5");
        assert_eq!(num(100.0), "100");
    }

    #[test]
    fn export_skips_construction_entities() {
        let mut d = sample::doc();
        let before = (exported(&d, 0).iter().map(|(_, es)| es.len()).sum::<usize>(), to_svg(&d, 0), to_dxf(&d, 0));
        let id = d.alloc_id();
        let line = cad_geometry::Shape::Line { a: sample::v2(0.0, 0.0), b: sample::v2(777.0, 888.0) };
        d.apply(cad_document::Command::Add(Entity { id, name: "c".into(), shape: line, layer: 0, sketch: 0, construction: true, group: None, meta: Default::default() })).unwrap();
        assert_eq!((exported(&d, 0).iter().map(|(_, es)| es.len()).sum::<usize>(), to_svg(&d, 0), to_dxf(&d, 0)), before);
    }

    #[test]
    fn exported_skips_hidden_and_solids() {
        let d = sample::doc();
        let ex = exported(&d, 0);
        assert_eq!(ex.iter().map(|(l, es)| (l.id, es.len())).collect::<Vec<_>>(), [(0, 2), (1, 5)]);
    }
}
