use super::*;
use crate::sample;
use cad_document::{Command, Entity, Layer, LineType};
use cad_geometry::{HatchPattern, Shape};

/// "%PDF-1.4", a line feed and the comment line with four bytes above 127.
const HEADER: usize = "%PDF-1.4\n".len() + "%".len() + 4 + "\n".len();

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("ASCII after the header")
}

/// Page box and content stream of a PDF, after checking the xref table, the object headers and /Length.
fn check(pdf: &[u8]) -> ([f64; 4], String) {
    assert!(pdf.starts_with(b"%PDF-1.4\n") && pdf.ends_with(b"%%EOF"));
    let tail = text(&pdf[pdf.len() - 40..]);
    let start: usize = tail.split("startxref\n").nth(1).unwrap().split('\n').next().unwrap().parse().unwrap();
    let table = text(&pdf[start..]);
    let mut lines = table.lines();
    assert_eq!(lines.next(), Some("xref"));
    let count: usize = lines.next().unwrap().strip_prefix("0 ").unwrap().parse().unwrap();
    assert_eq!(lines.next(), Some("0000000000 65535 f "));
    for n in 1..count {
        let entry = lines.next().unwrap();
        let (off, rest) = entry.split_at(10);
        assert_eq!(rest, " 00000 n ");
        let off: usize = off.parse().unwrap();
        assert!(pdf[off..].starts_with(format!("{n} 0 obj\n").as_bytes()), "object {n} is not at {off}");
    }
    assert!(table.contains(&format!("<< /Size {count} /Root 1 0 R >>")));
    // every entry is 20 bytes: ten digits, space, five digits, space, letter, space, line feed
    assert_eq!(table.split("trailer").next().unwrap().len(), "xref\n".len() + format!("0 {count}\n").len() + 20 * count);
    let body = text(&pdf[HEADER..]);
    let page = body.split("/MediaBox [").nth(1).unwrap().split(']').next().unwrap();
    let b: Vec<f64> = page.split(' ').map(|v| v.parse().unwrap()).collect();
    let stream_obj = body.split("4 0 obj\n").nth(1).unwrap();
    let length: usize = stream_obj.strip_prefix("<< /Length ").unwrap().split(' ').next().unwrap().parse().unwrap();
    let data = stream_obj.split_once(">>\nstream\n").unwrap().1;
    assert_eq!(&data[length..length + "\nendstream\nendobj".len()], "\nendstream\nendobj");
    ([b[0], b[1], b[2], b[3]], data[..length].to_string())
}

#[test]
fn structure_xref_length_and_media_box() {
    let d = sample::doc();
    let (media, content) = check(&to_pdf(&d, 0));
    let (lo, hi) = d
        .entities()
        .filter(|e| e.layer < 2 && !e.shape.is_solid())
        .map(|e| e.shape.bbox())
        .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
        .unwrap();
    let want = [0.0, 0.0, (hi.x - lo.x + 20.0) * PT, (hi.y - lo.y + 20.0) * PT];
    assert!(media.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-3), "{media:?} {want:?}");
    // the text and the dimension value; the hidden layer and the box add nothing
    assert_eq!(content.matches("BT /F1 ").count(), 2);
    // every drawn point lies on the page
    for line in content.lines().filter(|l| l.ends_with(" S") || l.ends_with(" f")) {
        let n: Vec<f64> = line.split(' ').filter_map(|t| t.parse().ok()).collect();
        assert!(n.chunks(2).all(|p| p[0] >= -1e-3 && p[0] <= media[2] + 1e-3 && p[1] >= -1e-3 && p[1] <= media[3] + 1e-3), "{line}");
    }
}

fn one_layer_doc() -> Document {
    let mut d = Document::new("p");
    let dashed = Layer { line_type: LineType::Center, line_weight: 0.5, color: [10, 20, 200], ..Layer::new(1, "D") };
    d.apply(Command::AddLayer(dashed)).unwrap();
    let list = [
        Shape::Line { a: v2(0.0, 0.0), b: v2(50.0, 0.0) },
        Shape::Line { a: v2(0.0, 10.0), b: v2(50.0, 20.0) },
        Shape::Text { pos: v2(5.0, 5.0), height: 4.0, angle: 0.0, text: "a(b)\\c \u{b0} \u{2300}".into() },
        Shape::Hatch { boundary: vec![v2(0.0, 30.0), v2(10.0, 30.0), v2(10.0, 40.0), v2(0.0, 40.0)], angle: 0.0, spacing: 2.0, pattern: HatchPattern::Solid },
    ];
    for shape in list {
        let id = d.alloc_id();
        d.apply(Command::Add(Entity { id, name: "e".into(), shape, layer: 1, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
    }
    d
}

#[test]
fn paths_dash_width_colour_fills_and_escaped_text() {
    let (media, content) = check(&to_pdf(&one_layer_doc(), 0));
    let lines: Vec<&str> = content.lines().collect();
    let paths: Vec<&&str> = lines.iter().filter(|l| l.ends_with(" l S")).collect();
    // two line entities and the ring of the hatch
    assert_eq!(paths.len(), 3, "{content}");
    // the drawing starts at (10, 10) mm on the page, y up
    assert_eq!(*paths[0], format!("{0} {0} m {1} {0} l S", num(10.0 * PT), num(60.0 * PT)));
    assert!(paths.iter().all(|p| p.matches(" m ").count() == 1 && p.starts_with(|c: char| c.is_ascii_digit())));
    assert!(lines.contains(&"0.039216 0.078431 0.784314 RG 0.039216 0.078431 0.784314 rg 1.417323 w"), "{content}");
    assert!(lines.contains(&"[22.677165 4.251969 4.251969 4.251969] 0 d"), "{content}");
    assert_eq!(lines.iter().filter(|l| l.ends_with(" h f")).count(), 2);
    let t = lines.iter().find(|l| l.starts_with("BT ")).unwrap();
    assert!(t.ends_with(r"(a\(b\)\\c \260 D) Tj ET"), "{t}");
    assert!(t.starts_with(&format!("BT /F1 {} Tf 1 0 0 1 ", num(4.0 * PT))), "{t}");
    assert!((media[2] - 70.0 * PT).abs() < 1e-3 && (media[3] - 60.0 * PT).abs() < 1e-3);
}

#[test]
fn rotated_text_uses_a_text_matrix_and_continuous_layers_an_empty_dash_array() {
    let mut d = Document::new("r");
    let id = d.alloc_id();
    let shape = Shape::Text { pos: v2(0.0, 0.0), height: 5.0, angle: std::f64::consts::FRAC_PI_2, text: "up".into() };
    d.apply(Command::Add(Entity { id, name: "t".into(), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
    let (_, content) = check(&to_pdf(&d, 0));
    // light layer colours print black
    assert!(content.starts_with("0 0 0 RG 0 0 0 rg 0.708661 w\n[] 0 d\n"), "{content}");
    assert!(content.contains(" Tf 0 1 -1 0 "), "{content}");
}

#[test]
fn empty_document_is_an_a4_page() {
    let (media, content) = check(&to_pdf(&Document::new("e"), 0));
    assert!((media[2] - 595.2756).abs() < 1e-3 && (media[3] - 841.8898).abs() < 1e-3, "{media:?}");
    assert!(content.is_empty());
}

#[test]
fn string_escapes_and_winansi() {
    assert_eq!(pdf_string("a(b)\\c"), r"(a\(b\)\\c)");
    assert_eq!(pdf_string("90\u{b0}"), r"(90\260)");
    assert_eq!(pdf_string("\u{d8}12"), r"(\33012)");
    assert_eq!(pdf_string("\u{2300}5 \u{4e2d}\n"), "(D5 ? )");
}
