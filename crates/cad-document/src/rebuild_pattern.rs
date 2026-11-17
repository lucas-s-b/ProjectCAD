use crate::document::Document;
use crate::feature::{Axis3, Feature, FeatureId, FeatureKind, MirrorSource};
use crate::sketch::SketchPlanes;
use crate::rebuild::{Bodies, FeatureState, Inputs, eval, hash_of, resolve_solid};
use crate::sketch::BasePlane;
use cad_geometry::{DAffine3, DQuat, DVec3};
use cad_modeler::{BoolOp, boolean, union_all};
use cad_params::Dim;
use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::sync::Arc;

const BAD_COUNT: &str = "count must be a whole number from 1 to 500";
const NOT_SOURCE: &str = "this feature cannot be used as a source";

/// A pattern or mirror with its inputs evaluated.
pub(crate) enum Pattern {
    /// The tool of `root` copied by each of `forms`; the root's own tool is not repeated.
    Copies { root: Inputs, forms: Vec<DAffine3> },
    /// Every body united with its mirror image.
    Bodies(DAffine3),
}

/// The tool solids of a feature, all as the root feature's tool moved by a transform.
struct Forms {
    root: FeatureId,
    all: Vec<DAffine3>,
    /// The ones this feature adds: instances after the first, or every mirrored tool.
    fresh: Vec<DAffine3>,
}

fn count(doc: &Document, src: &str) -> Result<usize, String> {
    let n = doc.params().eval_expr(src).ok().filter(|v| v.dim == Dim::SCALAR).map(|v| v.v);
    n.filter(|n| (n - n.round()).abs() <= 1e-9 && (1.0..=500.0).contains(&n.round()))
        .map(|n| n.round() as usize)
        .ok_or_else(|| BAD_COUNT.to_string())
}

fn mirror_matrix(plane: BasePlane, offset: f64) -> DAffine3 {
    let n = plane.plane().normal();
    DAffine3::from_translation(2.0 * offset * n) * DAffine3::from_scale(DVec3::ONE - 2.0 * n.abs())
}

fn forms(doc: &Document, f: &Feature, states: &BTreeMap<FeatureId, FeatureState>) -> Result<Forms, String> {
    let len = |s: &str| eval(doc, s, false);
    let (source, instances, skip) = match &f.kind {
        FeatureKind::Extrude { .. } | FeatureKind::Revolve { .. } | FeatureKind::Sweep { .. } | FeatureKind::Loft { .. } => {
            return Ok(Forms { root: f.id, all: vec![DAffine3::IDENTITY], fresh: Vec::new() });
        }
        FeatureKind::LinearPattern { source, count: n, dx, dy, dz } => {
            let (n, step) = (count(doc, n)?, DVec3::new(len(dx)?, len(dy)?, len(dz)?));
            (*source, (0..n).map(|k| DAffine3::from_translation(step * k as f64)).collect::<Vec<_>>(), 1)
        }
        FeatureKind::CircularPattern { source, count: n, angle, axis, cx, cy, cz } => {
            let (n, total, c) = (count(doc, n)?, eval(doc, angle, true)?, DVec3::new(len(cx)?, len(cy)?, len(cz)?));
            let axis = match axis {
                Axis3::X => DVec3::X,
                Axis3::Y => DVec3::Y,
                Axis3::Z => DVec3::Z,
            };
            // A full turn closes the circle, so the last copy must not land on the first.
            let parts = if (total.abs() - TAU).abs() <= 1e-9 { n } else { n.max(2) - 1 };
            let turn = |k: usize| {
                let q = DQuat::from_axis_angle(axis, total / parts as f64 * k as f64);
                DAffine3::from_rotation_translation(q, c - q * c)
            };
            (*source, (0..n).map(turn).collect(), 1)
        }
        FeatureKind::Mirror { source: MirrorSource::Feature(source), plane, offset } => {
            (*source, vec![mirror_matrix(*plane, len(offset)?)], 0)
        }
        FeatureKind::Fillet { .. } | FeatureKind::Chamfer { .. } | FeatureKind::Shell { .. } | FeatureKind::Mirror { source: MirrorSource::Bodies, .. } => {
            return Err(NOT_SOURCE.to_string());
        }
    };
    match states.get(&source) {
        None => return Err("source feature must come before this feature".to_string()),
        Some(FeatureState::Ok) => {}
        Some(_) => return Err("source feature is not active".to_string()),
    }
    let inner = forms(doc, doc.feature(source).ok_or("source feature not found")?, states)?;
    let moved = |skip: usize| -> Vec<DAffine3> {
        instances.iter().skip(skip).flat_map(|m| inner.all.iter().map(move |t| *m * *t)).collect()
    };
    Ok(Forms { root: inner.root, all: moved(0), fresh: moved(skip) })
}

/// `states` holds the features before `f`, so a source that is missing from it comes later.
pub(crate) fn resolve_pattern(doc: &Document, f: &Feature, states: &BTreeMap<FeatureId, FeatureState>, planes: &SketchPlanes) -> Result<Pattern, String> {
    if let FeatureKind::Mirror { source: MirrorSource::Bodies, plane, offset } = &f.kind {
        return Ok(Pattern::Bodies(mirror_matrix(*plane, eval(doc, offset, false)?)));
    }
    let fr = forms(doc, f, states)?;
    let root = resolve_solid(doc, doc.feature(fr.root).ok_or("source feature not found")?, planes)?;
    Ok(Pattern::Copies { root, forms: fr.fresh })
}

fn bits(m: &DAffine3) -> Vec<u64> {
    m.to_cols_array().iter().map(|x| x.to_bits()).collect()
}

impl Pattern {
    /// The root's own key covers editing the source; the copies cover every parameter in between.
    pub fn key(&self, prev: u64) -> u64 {
        match self {
            Pattern::Copies { root, forms } => hash_of((prev, "pattern", root.key(0), forms.iter().map(bits).collect::<Vec<_>>())),
            Pattern::Bodies(m) => hash_of((prev, "mirror", bits(m))),
        }
    }

    /// The body list after the feature; `bodies` is not touched, so an error leaves it as it was.
    pub fn apply(&self, id: FeatureId, bodies: &Bodies) -> Result<Bodies, String> {
        match self {
            Pattern::Copies { root, forms } => {
                let tool = root.tool_body()?;
                let copies = forms.iter().map(|m| tool.transformed(m)).collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
                match union_all(&copies).map_err(|e| e.to_string())? {
                    Some(copies) => root.place(id, bodies, copies),
                    None => Ok(bodies.clone()),
                }
            }
            Pattern::Bodies(m) => bodies
                .iter()
                .map(|(i, b)| {
                    let both = b.transformed(m).and_then(|mirrored| boolean(b, &mirrored, BoolOp::Union));
                    both.map(|b| (*i, Arc::new(b))).map_err(|e| e.to_string())
                })
                .collect(),
        }
    }
}

#[cfg(test)]
#[path = "rebuild_pattern_tests.rs"]
pub(crate) mod tests;
