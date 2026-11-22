//! Part features: the generator parameters as expressions (`PartDef`), their evaluation into `cad_mech::Part`, and the
//! rebuild inputs of a part feature.
use crate::document::Document;
use crate::feature::{Feature, FeatureKind};
use crate::rebuild::{Inputs, Tool, eval};
use crate::sketch::SketchPlanes;
use cad_geometry::DVec2;
use cad_geometry::profile::Profile;
use cad_mech::{Bearing, Bolt, Gear, Nut, Part, Shaft, Spring, bearing, metric};
use cad_modeler::Plane3;
use cad_params::Dim;
use serde::{Deserialize, Serialize};

const BAD_TEETH: &str = "teeth must be a whole number";
const BAD_TURNS: &str = "turns must be a plain number";

/// The parameters of a generated part, each an expression (lengths, the pressure angle, plain numbers for teeth and
/// turns), except the thread size and the bearing size, which name rows of `cad_mech::METRIC` and
/// `cad_mech::BEARINGS`, and the modelled-thread flags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PartDef {
    /// Involute spur gear: module, number of teeth (whole), pressure angle, face width, bore diameter (0 for none).
    Gear { module: String, teeth: String, pressure: String, width: String, bore: String },
    /// Stepped shaft: [diameter, length] per step from the plane outwards; 45 degree chamfer at both ends (0 for none).
    Shaft { steps: Vec<[String; 2]>, chamfer: String },
    /// Helical compression spring: wire diameter, mean coil diameter, pitch, turns (need not be whole).
    Spring { wire: String, mean: String, pitch: String, turns: String },
    /// ISO 4017 hex head screw: thread size ("M8"), length under the head, modelled helical thread (false: cosmetic).
    Bolt {
        size: String,
        length: String,
        #[serde(default)]
        modelled: bool,
    },
    /// ISO 4032 hex nut; modelled helical thread (false: cosmetic).
    Nut {
        size: String,
        #[serde(default)]
        modelled: bool,
    },
    /// Deep groove ball bearing of ISO 15 ("6204").
    Bearing { size: String },
}

impl PartDef {
    /// One part of each kind with typical values, in the order of the "Add part" buttons.
    pub fn defaults() -> [PartDef; 6] {
        let s = |x: &str| x.to_string();
        [
            PartDef::Gear { module: s("2 mm"), teeth: s("20"), pressure: s("20 deg"), width: s("10 mm"), bore: s("8 mm") },
            PartDef::Shaft { steps: vec![[s("20 mm"), s("30 mm")], [s("16 mm"), s("50 mm")]], chamfer: s("1 mm") },
            PartDef::Spring { wire: s("2 mm"), mean: s("20 mm"), pitch: s("5 mm"), turns: s("6") },
            PartDef::Bolt { size: s("M8"), length: s("30 mm"), modelled: false },
            PartDef::Nut { size: s("M8"), modelled: false },
            PartDef::Bearing { size: s("6204") },
        ]
    }

    /// "Gear", "Shaft", "Spring", "Bolt", "Nut" or "Bearing".
    pub fn label(&self) -> &'static str {
        match self {
            PartDef::Gear { .. } => "Gear",
            PartDef::Shaft { .. } => "Shaft",
            PartDef::Spring { .. } => "Spring",
            PartDef::Bolt { .. } => "Bolt",
            PartDef::Nut { .. } => "Nut",
            PartDef::Bearing { .. } => "Bearing",
        }
    }

    /// Every expression, in the order of the fields (the thread size is not one).
    pub fn expressions(&self) -> Vec<&str> {
        match self {
            PartDef::Gear { module, teeth, pressure, width, bore } => [module, teeth, pressure, width, bore].map(String::as_str).to_vec(),
            PartDef::Shaft { steps, chamfer } => steps.iter().flatten().chain([chamfer]).map(String::as_str).collect(),
            PartDef::Spring { wire, mean, pitch, turns } => [wire, mean, pitch, turns].map(String::as_str).to_vec(),
            PartDef::Bolt { length, .. } => vec![length.as_str()],
            PartDef::Nut { .. } | PartDef::Bearing { .. } => Vec::new(),
        }
    }

    /// What can be checked without evaluating: a shaft has steps, a thread size is in the table.
    pub(crate) fn check(&self) -> Result<(), &'static str> {
        match self {
            PartDef::Shaft { steps, .. } if steps.is_empty() => Err("shaft needs at least one step"),
            PartDef::Bolt { size, .. } | PartDef::Nut { size, .. } if metric(size).is_none() => Err("unknown thread size"),
            PartDef::Bearing { size } if bearing(size).is_none() => Err("unknown bearing size"),
            _ => Ok(()),
        }
    }
}

impl Document {
    /// `def` with every expression evaluated: lengths in mm, the pressure angle in radians, teeth and turns as plain
    /// numbers. Ranges are checked when the solid is made (`Part::body`).
    pub fn part(&self, def: &PartDef) -> Result<Part, String> {
        let len = |s: &str| eval(self, s, false);
        let number = |s: &str| self.params().eval_expr(s).ok().filter(|v| v.dim == Dim::SCALAR).map(|v| v.v);
        let size = |s: &str| metric(s).ok_or_else(|| format!("unknown thread size {}", s.trim()));
        Ok(match def {
            PartDef::Gear { module, teeth, pressure, width, bore } => {
                let teeth = number(teeth).filter(|t| (t - t.round()).abs() <= 1e-9 && (0.0..=1e6).contains(&t.round())).ok_or(BAD_TEETH)?;
                Part::Gear(Gear {
                    module: len(module)?,
                    teeth: teeth.round() as u32,
                    pressure: eval(self, pressure, true)?,
                    width: len(width)?,
                    bore: len(bore)?,
                })
            }
            PartDef::Shaft { steps, chamfer } => Part::Shaft(Shaft {
                steps: steps.iter().map(|[d, l]| Ok((len(d)?, len(l)?))).collect::<Result<Vec<_>, String>>()?,
                chamfer: len(chamfer)?,
            }),
            PartDef::Spring { wire, mean, pitch, turns } => {
                Part::Spring(Spring { wire: len(wire)?, mean: len(mean)?, pitch: len(pitch)?, turns: number(turns).ok_or(BAD_TURNS)? })
            }
            PartDef::Bolt { size: s, length, modelled } => Part::Bolt(Bolt { size: size(s)?, length: len(length)?, modelled: *modelled }),
            PartDef::Nut { size: s, modelled } => Part::Nut(Nut { size: size(s)?, modelled: *modelled }),
            PartDef::Bearing { size: s } => Part::Bearing(Bearing { size: bearing(s).ok_or_else(|| format!("unknown bearing size {}", s.trim()))? }),
        })
    }
}

/// Rebuild inputs of a part feature: the evaluated part on its sketch plane, moved to (x, y) of the sketch.
pub(crate) fn resolve_part(doc: &Document, f: &Feature, planes: &SketchPlanes) -> Result<Inputs, String> {
    let FeatureKind::Part { def, sketch, x, y, op, target } = &f.kind else {
        return Err("not a part feature".to_string());
    };
    let part = doc.part(def)?;
    let plane = doc.plane_in(*sketch, planes)?;
    let at = DVec2::new(eval(doc, x, false)?, eval(doc, y, false)?);
    Ok(Inputs {
        profile: Profile { outer: Vec::new(), holes: Vec::new() },
        // The evaluated part is the whole geometric input: its Debug text keys the cache.
        shapes_json: format!("{part:?}"),
        plane: Plane3 { origin: plane.point(at), ..plane },
        tool: Tool::Part(part),
        op: *op,
        target: *target,
    })
}
