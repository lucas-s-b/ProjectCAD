//! Densities of common engineering materials, for masses in the bill of materials.

/// Name and density in g/cm³.
pub const MATERIALS: [(&str, f64); 11] = [
    ("Steel", 7.85),
    ("Stainless steel", 8.0),
    ("Cast iron", 7.2),
    ("Aluminium", 2.7),
    ("Brass", 8.5),
    ("Bronze", 8.8),
    ("Copper", 8.96),
    ("Titanium", 4.43),
    ("ABS", 1.04),
    ("POM", 1.41),
    ("Nylon", 1.14),
];

/// Density in g/cm³ of the material called `name` (surrounding spaces and letter case are ignored); None when unknown.
pub fn density(name: &str) -> Option<f64> {
    let name = name.trim();
    MATERIALS.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, d)| *d)
}

#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;
