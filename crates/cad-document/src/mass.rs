//! Mass properties of the bodies, the placed components and the whole assembly.
use crate::component::ComponentId;
use crate::document::Document;
use crate::feature::FeatureId;
use crate::rebuild::RebuildResult;
use cad_analysis::{Accuracy, MassProps, TESSELLATION};

/// Density assumed when no known material is given, g/cm³.
pub const ASSUMED_DENSITY: f64 = 1.0;

/// Mass properties of one solid or of the assembly, with the density used and how exact they are.
#[derive(Debug, Clone, PartialEq)]
pub struct MassRow {
    pub props: MassProps,
    /// g/cm³: the material's density; for the assembly total the mean density, mass / volume.
    pub density: f64,
    /// True when `ASSUMED_DENSITY` stood in for a missing or unknown material (always for bodies; for the total when
    /// any component's density was assumed).
    pub assumed: bool,
    pub accuracy: Accuracy,
}

/// Mass properties of everything the last rebuild made. Derived on demand, never stored.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MassReport {
    /// Every body of the rebuild, at `ASSUMED_DENSITY` (bodies carry no material), in body coordinates.
    pub bodies: Vec<(FeatureId, MassRow)>,
    /// Every placed component (assembled placement, never exploded) at the density of its "material" metadata.
    pub components: Vec<(ComponentId, MassRow)>,
    /// All components together about the assembly centre of mass; None without placed components.
    pub assembly: Option<MassRow>,
}

impl Document {
    /// Mass properties of the bodies and placed components of `result`. Solids whose mesh gives no positive volume
    /// and components that failed to place are left out. A row is `Exact` when the solid has only planar faces,
    /// else approximate (tessellation); the total is exact only when every component is.
    pub fn mass(&self, result: &RebuildResult) -> MassReport {
        let row = |body: &cad_modeler::Body, density: f64, assumed: bool| {
            MassProps::of(&body.mesh, density).map(|props| MassRow { props, density, assumed, accuracy: Accuracy::of_body(body) })
        };
        let bodies = result.bodies.iter().filter_map(|(id, b)| row(b, ASSUMED_DENSITY, true).map(|r| (*id, r))).collect();
        let components: Vec<(ComponentId, MassRow)> = result
            .components
            .iter()
            .filter_map(|(id, placed)| {
                let body = placed.as_ref().ok()?;
                let known = self.component(*id).and_then(|c| c.meta.get("material")).and_then(|m| cad_mech::density(m));
                row(body, known.unwrap_or(ASSUMED_DENSITY), known.is_none()).map(|r| (*id, r))
            })
            .collect();
        let parts: Vec<MassProps> = components.iter().map(|(_, r)| r.props).collect();
        let assembly = MassProps::combine(&parts).map(|props| MassRow {
            density: props.mass * 1000.0 / props.volume,
            assumed: components.iter().any(|(_, r)| r.assumed),
            accuracy: if components.iter().all(|(_, r)| r.accuracy == Accuracy::Exact) { Accuracy::Exact } else { Accuracy::Approximate(TESSELLATION) },
            props,
        });
        MassReport { bodies, components, assembly }
    }
}

#[cfg(test)]
#[path = "mass_tests.rs"]
mod tests;
