//! PDF export: one page, vector strokes, filled triangles and Helvetica text, at scale 1:1. PDF 1.4 without compression.
use crate::{exported, label_origin, num, paper_color};
use cad_document::{Document, SketchId};
use cad_geometry::DVec2;
use cad_geometry::annot::{fills, labels, strokes};
use std::fmt::Write;

const MARGIN_MM: f64 = 10.0;
/// Points per millimetre.
const PT: f64 = 72.0 / 25.4;
const A4_MM: DVec2 = DVec2::new(210.0, 297.0);

fn rgb(c: [u8; 3]) -> String {
    c.map(|v| num(f64::from(v) / 255.0)).join(" ")
}

/// A PDF string in WinAnsi: `(`, `)` and `\` escaped, bytes above 126 as octal escapes.
fn pdf_string(s: &str) -> String {
    let mut out = String::from("(");
    for c in s.chars() {
        let code = match c as u32 {
            0x20..=0x7e | 0xa0..=0xff => c as u32,
            // the diameter sign has no WinAnsi code
            0x2300 => u32::from(b'D'),
            0..=0x1f | 0x7f..=0x9f => u32::from(b' '),
            _ => u32::from(b'?'),
        };
        match code {
            0x28 | 0x29 | 0x5c => {
                out.push('\\');
                out.push(code as u8 as char);
            }
            0x20..=0x7e => out.push(code as u8 as char),
            _ => {
                let _ = write!(out, "\\{code:03o}");
            }
        }
    }
    out.push(')');
    out
}

/// Appends `n 0 obj ... endobj` and remembers where it starts.
fn object(out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &str) {
    offsets.push(out.len());
    out.extend(format!("{} 0 obj\n{body}\nendobj\n", offsets.len()).as_bytes());
}

pub fn to_pdf(doc: &Document, sketch: SketchId) -> Vec<u8> {
    let layers = exported(doc, sketch);
    let bounds = layers
        .iter()
        .flat_map(|(_, es)| es.iter().map(|e| e.shape.bbox()))
        .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)));
    let (lo, size) = match bounds {
        Some((lo, hi)) => (lo.truncate(), (hi - lo).truncate()),
        None => (DVec2::ZERO, A4_MM - DVec2::splat(2.0 * MARGIN_MM)),
    };
    let page = (size + DVec2::splat(2.0 * MARGIN_MM)) * PT;
    // PDF y points up, like the drawing
    let at = |p: DVec2| (p - lo + DVec2::splat(MARGIN_MM)) * PT;
    let xy = |p: DVec2| {
        let q = at(p);
        format!("{} {}", num(q.x), num(q.y))
    };
    let mut content = String::new();
    for (layer, entities) in layers {
        let col = rgb(paper_color(layer.color));
        let dash: Vec<String> = layer.line_type.pattern_mm().iter().map(|v| num(v * PT)).collect();
        let _ = writeln!(content, "{col} RG {col} rg {} w\n[{}] 0 d", num(layer.line_weight * PT), dash.join(" "));
        for e in entities {
            for path in strokes(&e.shape).iter().filter(|p| p.len() >= 2) {
                // shortcut: curves are written as polylines, use Bezier segments for true arcs later
                let pts: Vec<String> = path.iter().enumerate().map(|(i, p)| format!("{} {}", xy(*p), if i == 0 { "m" } else { "l" })).collect();
                let _ = writeln!(content, "{} S", pts.join(" "));
            }
            for t in fills(&e.shape) {
                let _ = writeln!(content, "{} m {} l {} l h f", xy(t[0]), xy(t[1]), xy(t[2]));
            }
            for l in labels(&e.shape) {
                let value = doc.settings().label_text(&l.text);
                let (s, c) = l.angle.sin_cos();
                let _ = writeln!(
                    content,
                    "BT /F1 {} Tf {} {} {} {} {} Tm {} Tj ET",
                    num(l.height * PT),
                    num(c),
                    num(s),
                    num(-s),
                    num(c),
                    xy(label_origin(&l, &value)),
                    pdf_string(&value)
                );
            }
        }
    }
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    object(&mut out, &mut offsets, "<< /Type /Catalog /Pages 2 0 R >>");
    object(&mut out, &mut offsets, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    let media = format!("[0 0 {} {}]", num(page.x), num(page.y));
    object(&mut out, &mut offsets, &format!("<< /Type /Page /Parent 2 0 R /MediaBox {media} /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"));
    object(&mut out, &mut offsets, &format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()));
    object(&mut out, &mut offsets, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>");
    let xref = out.len();
    let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1);
    for off in &offsets {
        let _ = writeln!(table, "{off:010} 00000 n ");
    }
    let _ = write!(table, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF", offsets.len() + 1);
    out.extend(table.as_bytes());
    out
}

#[cfg(test)]
#[path = "pdf_out_tests.rs"]
mod tests;
