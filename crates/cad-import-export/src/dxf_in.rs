//! DXF import: LINE, CIRCLE, ARC, POLYLINE, LWPOLYLINE, TEXT and full ELLIPSE entities plus the layer table.
use crate::dxf::ACI;
use cad_document::{Command, Document, Entity, Layer, LayerId, LineType};
use cad_geometry::{DVec2, Shape};
use std::collections::BTreeMap;
use std::f64::consts::TAU;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ImportReport {
    pub imported: usize,
    /// Key = entity type, or "invalid" for geometry that failed validation.
    pub skipped: BTreeMap<String, usize>,
}

// shortcut: no thiserror here (it is not a dependency of this crate), Display and Error are written by hand
#[derive(Debug)]
pub enum ImportError {
    Format(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self::Format(m) = self;
        write!(f, "not a DXF file: {m}")
    }
}

impl std::error::Error for ImportError {}

/// One entity or table record: its type (group 0) and the groups that follow.
struct Rec {
    kind: String,
    groups: Vec<(i32, String)>,
}

impl Rec {
    fn s(&self, code: i32) -> Option<&str> {
        self.groups.iter().find(|g| g.0 == code).map(|g| g.1.as_str())
    }
    fn f(&self, code: i32) -> Option<f64> {
        self.s(code)?.parse().ok().filter(|v: &f64| v.is_finite())
    }
    fn pt(&self, x: i32) -> Option<DVec2> {
        Some(DVec2::new(self.f(x)?, self.f(x + 10)?))
    }
    fn closed(&self) -> bool {
        self.f(70).is_some_and(|f| f as i32 & 1 == 1)
    }
}

fn format_err(m: &str) -> ImportError {
    ImportError::Format(m.into())
}

/// Records per section name, and whether an ENTITIES section or EOF was seen.
fn parse(text: &str) -> Result<(BTreeMap<String, Vec<Rec>>, bool), ImportError> {
    if text.starts_with("AutoCAD Binary DXF") {
        return Err(format_err("binary DXF is not supported"));
    }
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    if !lines.len().is_multiple_of(2) {
        return Err(format_err("odd number of lines"));
    }
    let mut pairs = Vec::with_capacity(lines.len() / 2);
    for c in lines.as_chunks::<2>().0 {
        pairs.push((c[0].parse::<i32>().map_err(|_| format_err("group code is not an integer"))?, c[1]));
    }
    let mut sections: BTreeMap<String, Vec<Rec>> = BTreeMap::new();
    let (mut current, mut seen) = (None::<String>, false);
    let mut it = pairs.into_iter();
    while let Some((code, value)) = it.next() {
        match (code, value) {
            (0, "EOF") => {
                seen = true;
                break;
            }
            (0, "SECTION") => {
                let Some((2, name)) = it.next() else { return Err(format_err("section without a name")) };
                seen |= name == "ENTITIES";
                sections.entry(name.to_string()).or_default();
                current = Some(name.to_string());
            }
            (0, "ENDSEC") => current = None,
            (code, value) => {
                let Some(list) = current.as_ref().and_then(|n| sections.get_mut(n)) else { continue };
                if code == 0 {
                    list.push(Rec { kind: value.to_string(), groups: Vec::new() });
                } else if let Some(r) = list.last_mut() {
                    r.groups.push((code, value.to_string()));
                }
            }
        }
    }
    Ok((sections, seen))
}

fn rgb(index: i32) -> [u8; 3] {
    ACI.iter().find(|(i, _)| i32::from(*i) == index).map_or(Layer::new(0, "x").color, |(_, c)| c.map(|v| v as u8))
}

fn line_type(name: &str) -> LineType {
    LineType::ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(name)).unwrap_or_default()
}

struct Builder {
    doc: Document,
    ids: BTreeMap<String, LayerId>,
    report: ImportReport,
}

impl Builder {
    fn layer(&mut self, name: &str) -> LayerId {
        if let Some(id) = self.ids.get(name) {
            return *id;
        }
        let id = self.doc.alloc_layer_id();
        let _ = self.doc.apply(Command::AddLayer(Layer::new(id, if name.trim().is_empty() { "Layer" } else { name })));
        self.ids.insert(name.to_string(), id);
        id
    }

    fn table(&mut self, r: &Rec) {
        let Some(name) = r.s(2) else { return };
        let id = self.layer(name);
        let mut layer = self.doc.layer(id).cloned().unwrap_or_else(|| Layer::new(id, name));
        let colour = r.f(62).map_or(0, |v| v as i32);
        layer.visible = colour >= 0;
        layer.color = rgb(colour.abs());
        layer.line_type = line_type(r.s(6).unwrap_or(""));
        let _ = self.doc.apply(Command::SetLayer(layer));
    }

    fn add(&mut self, r: &Rec, shape: Option<Shape>) {
        let layer = self.layer(r.s(8).unwrap_or("0"));
        let id = self.doc.alloc_id();
        let ok = shape.is_some_and(|shape| {
            let name = format!("{} {id}", shape.kind());
            self.doc.apply(Command::Add(Entity { id, name, shape, layer, sketch: 0 })).is_ok()
        });
        if ok {
            self.report.imported += 1;
        } else {
            self.skip("invalid", 1);
        }
    }

    fn skip(&mut self, key: &str, n: usize) {
        if n > 0 {
            *self.report.skipped.entry(key.to_string()).or_default() += n;
        }
    }

    fn bulges(&mut self, recs: &[&Rec]) {
        let nonzero = |g: &&(i32, String)| g.0 == 42 && g.1.parse::<f64>().is_ok_and(|b| b != 0.0);
        let n = recs.iter().flat_map(|r| r.groups.iter()).filter(nonzero).count();
        self.skip("bulge", n);
    }
}

fn lw_polyline(r: &Rec) -> Option<Shape> {
    let mut points: Vec<DVec2> = Vec::new();
    for (code, v) in &r.groups {
        match code {
            10 => points.push(DVec2::new(v.parse().ok()?, 0.0)),
            20 => points.last_mut()?.y = v.parse().ok()?,
            _ => {}
        }
    }
    Some(Shape::Polyline { points, closed: r.closed() })
}

fn shape(r: &Rec) -> Option<Shape> {
    Some(match r.kind.as_str() {
        "LINE" => Shape::Line { a: r.pt(10)?, b: r.pt(11)? },
        "CIRCLE" => Shape::Circle { center: r.pt(10)?, radius: r.f(40)? },
        "ARC" => {
            let (a, b) = (r.f(50)?, r.f(51)?);
            let sweep = (b - a).rem_euclid(360.0);
            Shape::Arc {
                center: r.pt(10)?,
                radius: r.f(40)?,
                start: a.to_radians().rem_euclid(TAU),
                sweep: if sweep < 1e-9 { TAU } else { sweep.to_radians() },
            }
        }
        "TEXT" => Shape::Text {
            pos: r.pt(10)?,
            height: r.f(40)?,
            angle: r.f(50).unwrap_or(0.0).to_radians(),
            text: r.s(1)?.to_string(),
        },
        "LWPOLYLINE" => return lw_polyline(r),
        _ => return None,
    })
}

/// None for a partial ellipse (skipped); Some(None) for a full one with unusable numbers.
fn ellipse(r: &Rec) -> Option<Option<Shape>> {
    let (start, end) = (r.f(41).unwrap_or(0.0), r.f(42).unwrap_or(TAU));
    if start.abs() > 1e-6 || (end - TAU).abs() > 1e-6 {
        return None;
    }
    let shape = || {
        let major = r.pt(11)?;
        let rx = major.length();
        Some(Shape::Ellipse { center: r.pt(10)?, rx, ry: rx * r.f(40)?, angle: major.to_angle() })
    };
    Some(shape())
}

pub fn from_dxf(text: &str, name: &str) -> Result<(Document, ImportReport), ImportError> {
    let (sections, seen) = parse(text)?;
    if !seen {
        return Err(format_err("no ENTITIES section"));
    }
    let mut b = Builder { doc: Document::new(name), ids: BTreeMap::from([("0".to_string(), 0)]), report: ImportReport::default() };
    for r in sections.get("TABLES").into_iter().flatten().filter(|r| r.kind == "LAYER") {
        b.table(r);
    }
    let ents = sections.get("ENTITIES").map_or(&[][..], Vec::as_slice);
    let mut i = 0;
    while i < ents.len() {
        let r = &ents[i];
        i += 1;
        match r.kind.as_str() {
            "POLYLINE" => {
                let n = ents[i..].iter().take_while(|v| v.kind == "VERTEX").count();
                let verts: Vec<&Rec> = ents[i..i + n].iter().collect();
                i += n;
                if ents.get(i).is_some_and(|v| v.kind == "SEQEND") {
                    i += 1;
                }
                b.bulges(&verts);
                let points: Option<Vec<DVec2>> = verts.iter().map(|v| v.pt(10)).collect();
                b.add(r, points.map(|points| Shape::Polyline { points, closed: r.closed() }));
            }
            "LWPOLYLINE" => {
                b.bulges(&[r]);
                b.add(r, shape(r));
            }
            "ELLIPSE" => match ellipse(r) {
                Some(s) => b.add(r, s),
                None => b.skip("ELLIPSE", 1),
            },
            "LINE" | "CIRCLE" | "ARC" | "TEXT" => b.add(r, shape(r)),
            other => b.skip(other, 1),
        }
    }
    Ok((b.doc, b.report))
}

#[cfg(test)]
#[path = "dxf_in_tests.rs"]
mod tests;
