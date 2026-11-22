//! The bill of materials: the components grouped into rows, and their CSV text.
use crate::document::Document;
use crate::feature::FeatureKind;
use crate::rebuild::RebuildResult;

/// One row of the bill of materials.
#[derive(Debug, Clone, PartialEq)]
pub struct BomRow {
    /// 1, 2, ... in the order of each row's first component.
    pub item: usize,
    /// Name of the feature that made the body.
    pub name: String,
    /// Standard designation when a part feature made the body, else empty.
    pub designation: String,
    pub material: String,
    pub part_number: String,
    pub quantity: usize,
    /// Grams per piece: body volume times the density of `material`; None when the material is not in
    /// `cad_mech::MATERIALS` or the body is not in `result`.
    pub mass: Option<f64>,
}

impl Document {
    /// The components grouped into rows: components with the same designation (the same body when it has none),
    /// material and part number (component meta "material" and "part_number", trimmed) share a row; name and mass
    /// come from the row's first component.
    pub fn bom(&self, result: &RebuildResult) -> Vec<BomRow> {
        let mut rows: Vec<((String, String, String), BomRow)> = Vec::new();
        for c in self.components() {
            let feature = self.feature(c.body);
            let part = feature.and_then(|f| match &f.kind {
                FeatureKind::Part { def, .. } => self.part(def).ok(),
                _ => None,
            });
            let designation = part.map(|p| p.designation()).unwrap_or_default();
            let meta = |k: &str| c.meta.get(k).map_or_else(String::new, |s| s.trim().to_string());
            let (material, part_number) = (meta("material"), meta("part_number"));
            let group = if designation.is_empty() { format!("body {}", c.body) } else { designation.clone() };
            let key = (group, material.clone(), part_number.clone());
            if let Some((_, row)) = rows.iter_mut().find(|(k, _)| *k == key) {
                row.quantity += 1;
                continue;
            }
            let name = feature.map_or_else(String::new, |f| f.name.clone());
            // mm³ times g/cm³ is mg.
            let mass = result.body(c.body).zip(cad_mech::density(&material)).map(|(b, rho)| b.volume() * rho / 1000.0);
            let item = rows.len() + 1;
            rows.push((key, BomRow { item, name, designation, material, part_number, quantity: 1, mass }));
        }
        rows.into_iter().map(|(_, r)| r).collect()
    }
}

/// CSV of `rows`: a UTF-8 byte order mark (spreadsheets need it for "Ø"), the header line
/// `Item,Part,Designation,Material,Part number,Quantity,Mass (g)`, then one line per row, each ending in "\n". Fields
/// with a comma, quote or line break are quoted, quotes doubled; the mass has one decimal and is empty when unknown.
pub fn bom_csv(rows: &[BomRow]) -> String {
    let field = |s: &str| if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    let mut out = String::from("\u{feff}Item,Part,Designation,Material,Part number,Quantity,Mass (g)\n");
    for r in rows {
        let mass = r.mass.map_or_else(String::new, |m| format!("{m:.1}"));
        let cells = [r.item.to_string(), field(&r.name), field(&r.designation), field(&r.material), field(&r.part_number), r.quantity.to_string(), mass];
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[path = "bom_tests.rs"]
mod tests;
