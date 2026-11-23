//! Static linear-elastic studies: stored in the document (body, element size, material, supports, loads), run on
//! demand from the rebuild result; results are never stored.
use crate::document::{Command, DocError, Document};
use crate::feature::FeatureId;
use crate::refs::FaceRef;
use crate::rebuild::{RebuildResult, eval};
use cad_analysis::{Accuracy, Elastic, FEA, FeaResult, MeshStats, TetMesh, gravity_loads, solve_static, traction_loads};
use cad_geometry::DVec3;
use cad_params::parse;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// What acts on a load face. Numbers are expressions (plain numbers or parameters), evaluated when the study runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LoadKind {
    /// Total force on the face, N, in body coordinates; spread evenly over the face area.
    Force([String; 3]),
    /// MPa (N/mm²), pushing into the face.
    Pressure(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Load {
    pub face: FaceRef,
    pub kind: LoadKind,
}

/// A static linear-elastic study of one body. Number fields are expressions: `h` a length, `young` MPa, `poisson`
/// plain, `density` g/cm³ (used by gravity only), `yield_strength` MPa (empty: not given).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Study {
    pub name: String,
    pub body: FeatureId,
    /// Element size of the mesh.
    pub h: String,
    /// The `cad_mech::MATERIALS` name the numbers came from, or "Custom".
    pub material: String,
    pub young: String,
    pub poisson: String,
    pub density: String,
    /// Yield strength, MPa, for the safety factor; empty when not given (studies saved before format 27).
    #[serde(default)]
    pub yield_strength: String,
    /// Add the body's weight along −Z (9.81 m/s²).
    #[serde(default)]
    pub gravity: bool,
    /// Faces held in place (every displacement 0).
    #[serde(default)]
    pub fixed: Vec<FaceRef>,
    #[serde(default)]
    pub loads: Vec<Load>,
}

impl Study {
    /// A steel study of `body` with no supports or loads.
    pub fn new(name: String, body: FeatureId, h: String) -> Study {
        let mut s = Study { name, body, h, material: String::new(), young: String::new(), poisson: String::new(), density: String::new(), yield_strength: String::new(), gravity: false, fixed: Vec::new(), loads: Vec::new() };
        s.set_material("Steel");
        s
    }

    /// Copies name, E, ν, density and yield strength of the table material `name` ("210000", "0.3", "7.85", "250");
    /// false (and no change) when unknown.
    pub fn set_material(&mut self, name: &str) -> bool {
        let Some(m) = cad_mech::material(name) else { return false };
        (self.material, self.young, self.poisson, self.density, self.yield_strength) =
            (m.name.into(), m.young.to_string(), m.poisson.to_string(), m.density.to_string(), m.yield_strength.to_string());
        true
    }

    /// Renames the material "Custom" when E, ν, density or a given yield strength no longer read as the table's values
    /// for its name (an empty yield strength is "not given" and keeps the name).
    pub fn custom_if_edited(&mut self) {
        let same = |text: &str, v: f64| text.trim().parse::<f64>() == Ok(v);
        if cad_mech::material(&self.material).is_some_and(|m| {
            !(same(&self.young, m.young) && same(&self.poisson, m.poisson) && same(&self.density, m.density) && (self.yield_strength.trim().is_empty() || same(&self.yield_strength, m.yield_strength)))
        }) {
            self.material = "Custom".into();
        }
    }

    /// Every expression parses (values are checked when the study runs).
    pub(crate) fn check(&self) -> Result<(), DocError> {
        let forces = self.loads.iter().flat_map(|l| match &l.kind {
            LoadKind::Force(c) => c.iter().collect::<Vec<_>>(),
            LoadKind::Pressure(p) => vec![p],
        });
        [&self.h, &self.young, &self.poisson, &self.density]
            .into_iter()
            .chain(Some(&self.yield_strength).filter(|y| !y.trim().is_empty()))
            .chain(forces)
            .try_for_each(|src| parse(src).map(drop))?;
        Ok(())
    }
}

impl Document {
    pub fn studies(&self) -> &[Study] {
        &self.studies
    }

    /// `Command::SetStudies`: checks every study, replaces the list, returns the inverse.
    pub(crate) fn apply_studies(&mut self, studies: Vec<Study>) -> Result<Command, DocError> {
        studies.iter().try_for_each(Study::check)?;
        Ok(Command::SetStudies(std::mem::replace(&mut self.studies, studies)))
    }
}

/// The result of a study run: the mesh it ran on and the solution. Never stored.
#[derive(Debug, Clone, PartialEq)]
pub struct StudyReport {
    /// Index of the study in `Document::studies`.
    pub study: usize,
    pub body: FeatureId,
    /// Element size, mm.
    pub h: f64,
    pub mesh: TetMesh,
    pub stats: MeshStats,
    pub result: FeaResult,
    /// MPa, as the study gave it; None when not given.
    pub yield_strength: Option<f64>,
    /// Always `Approximate(FEA)`; `label` adds the element size.
    pub accuracy: Accuracy,
    /// Milliseconds spent meshing, and on loads, assembly, solve and stresses.
    pub mesh_ms: f64,
    pub solve_ms: f64,
}

impl StudyReport {
    /// Yield strength over the largest nodal von Mises stress: how many times the loads could grow before the most
    /// stressed node of this linear-elastic model yields (peaks at sharp re-entrant edges depend on the mesh). None
    /// without a yield strength or without stress.
    pub fn safety_factor(&self) -> Option<f64> {
        let s = self.result.max_von_mises().0;
        self.yield_strength.filter(|_| s > 0.0).map(|y| y / s)
    }

    /// "approximate (FEA, linear elastic, mesh h=5 mm)": the accuracy label with the element size.
    pub fn label(&self) -> String {
        format!("{}, mesh h={} mm)", self.accuracy.to_string().trim_end_matches(')'), self.h)
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

impl Document {
    /// Runs study `index` on the bodies of `result`: meshes the body (`mesh_body`), holds every node of the fixed
    /// faces (`face_nodes`), loads the boundary triangles whose three corners lie on a load face, adds gravity, and
    /// solves (`cad_analysis::solve_static`). Errors, in this order: "study 3 not found", expression errors,
    /// `Elastic::check`'s, "density must not be negative" (with gravity), "yield strength must be positive", the mesher's, "face not found; pick it
    /// again", "a support face has no mesh nodes; use a smaller element size", "a load face has no mesh faces; use a
    /// smaller element size", the solver's.
    pub fn run_study(&self, result: &RebuildResult, index: usize) -> Result<StudyReport, String> {
        let s = self.studies.get(index).ok_or_else(|| format!("study {index} not found"))?;
        let num = |src: &str| eval(self, src, false);
        let mat = Elastic { young: num(&s.young)?, poisson: num(&s.poisson)? };
        mat.check()?;
        let density = if s.gravity { num(&s.density)? } else { 0.0 };
        if density < 0.0 {
            return Err("density must not be negative".into());
        }
        let yield_strength = match s.yield_strength.trim() {
            "" => None,
            src => Some(num(src)?),
        };
        if yield_strength.is_some_and(|y| y.is_nan() || y <= 0.0) {
            return Err("yield strength must be positive".into());
        }
        let start = Instant::now();
        let rep = self.mesh_body(result, s.body, &s.h)?;
        let mesh_ms = ms(start);
        let start = Instant::now();
        let n = rep.mesh.nodes.len();
        let mut fixed = Vec::new();
        for face in &s.fixed {
            let nodes = self.face_nodes(result, &rep, face)?;
            if nodes.is_empty() {
                return Err("a support face has no mesh nodes; use a smaller element size".into());
            }
            fixed.extend(nodes);
        }
        fixed.sort_unstable();
        fixed.dedup();
        let boundary = rep.mesh.boundary();
        let mut loads = vec![DVec3::ZERO; n];
        for load in &s.loads {
            let mut on = vec![false; n];
            for v in self.face_nodes(result, &rep, &load.face)? {
                on[v as usize] = true;
            }
            let tris: Vec<[u32; 3]> = boundary.iter().filter(|t| t.iter().all(|&v| on[v as usize])).copied().collect();
            if tris.is_empty() {
                return Err("a load face has no mesh faces; use a smaller element size".into());
            }
            let f = match &load.kind {
                LoadKind::Force(c) => {
                    let total = DVec3::new(num(&c[0])?, num(&c[1])?, num(&c[2])?);
                    let area: f64 = tris
                        .iter()
                        .map(|t| {
                            let [a, b, c] = t.map(|v| rep.mesh.nodes[v as usize]);
                            (b - a).cross(c - a).length() / 2.0
                        })
                        .sum();
                    traction_loads(&rep.mesh, &tris, |_| total / area)
                }
                LoadKind::Pressure(p) => {
                    let p = num(p)?;
                    traction_loads(&rep.mesh, &tris, |normal| -normal * p)
                }
            };
            loads.iter_mut().zip(f).for_each(|(sum, add)| *sum += add);
        }
        if s.gravity {
            loads.iter_mut().zip(gravity_loads(&rep.mesh, density)).for_each(|(sum, add)| *sum += add);
        }
        let solution = solve_static(&rep.mesh, mat, &fixed, &loads)?;
        Ok(StudyReport { study: index, body: s.body, h: rep.h, stats: rep.stats, mesh: rep.mesh, result: solution, yield_strength, accuracy: Accuracy::Approximate(FEA), mesh_ms, solve_ms: ms(start) })
    }
}

#[cfg(test)]
#[path = "study_tests.rs"]
mod tests;
