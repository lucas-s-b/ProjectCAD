//! Engineering materials: density for masses in the bill of materials, elastic constants and yield strength for static studies.

/// A material of the table: handbook values for a common grade, good enough for a first estimate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    pub name: &'static str,
    /// g/cm³.
    pub density: f64,
    /// Young's modulus, MPa (N/mm²).
    pub young: f64,
    /// Poisson's ratio.
    pub poisson: f64,
    /// Yield strength (0.2 % proof stress) of the grade named in the table, MPa; for grey cast iron, which has no
    /// yield point, the tensile strength.
    pub yield_strength: f64,
}

const fn m(name: &'static str, density: f64, young: f64, poisson: f64, yield_strength: f64) -> Material {
    Material { name, density, young, poisson, yield_strength }
}

pub const MATERIALS: [Material; 11] = [
    m("Steel", 7.85, 210_000.0, 0.30, 250.0),          // structural, S235 / A36
    m("Stainless steel", 8.0, 193_000.0, 0.29, 215.0), // 304, annealed
    m("Cast iron", 7.2, 110_000.0, 0.26, 200.0),       // grey EN-GJL-200: tensile strength
    m("Aluminium", 2.7, 69_000.0, 0.33, 276.0),        // 6061-T6
    m("Brass", 8.5, 100_000.0, 0.34, 250.0),           // CW614N, R430
    m("Bronze", 8.8, 110_000.0, 0.34, 140.0),          // CuSn12-C, cast
    m("Copper", 8.96, 117_000.0, 0.34, 70.0),          // Cu-ETP, annealed
    m("Titanium", 4.43, 114_000.0, 0.34, 880.0),       // Ti-6Al-4V
    m("ABS", 1.04, 2_300.0, 0.35, 40.0),
    m("POM", 1.41, 2_900.0, 0.35, 65.0),               // POM-C
    m("Nylon", 1.14, 2_700.0, 0.39, 70.0),             // PA6, dry
];

/// The material called `name` (surrounding spaces and letter case are ignored); None when unknown.
pub fn material(name: &str) -> Option<&'static Material> {
    let name = name.trim();
    MATERIALS.iter().find(|m| m.name.eq_ignore_ascii_case(name))
}

/// Density in g/cm³ of the material called `name`; None when unknown.
pub fn density(name: &str) -> Option<f64> {
    material(name).map(|m| m.density)
}

#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;
