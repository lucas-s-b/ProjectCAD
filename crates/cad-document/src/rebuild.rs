use crate::document::{Document, EntityId};
use crate::feature::{BODY_TOLERANCE, BodyOp, Feature, FeatureId, FeatureKind, RevolveAxis, Target};
use crate::rebuild_blend::Blend;
use crate::rebuild_pattern::{Pattern, resolve_pattern};
use crate::rebuild_shell::Shell;
use crate::rebuild_sweep::resolve_sweep;
use crate::refs::face_plane;
use crate::sketch::{PlaneDef, Sketch, SketchPlanes};
use cad_geometry::profile::Profile;
use cad_geometry::{DVec2, Shape};
use cad_modeler::{Body, BoolOp, ModelError, PathPiece, Plane3, Section, boolean, extrude, loft, revolve, sweep};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum FeatureState {
    Ok,
    Suppressed,
    Error(String),
}

/// In creation order; a body's id is the id of the feature that created it.
pub(crate) type Bodies = Vec<(FeatureId, Arc<Body>)>;

#[derive(Debug, Clone)]
pub struct RebuildResult {
    pub bodies: Bodies,
    pub states: BTreeMap<FeatureId, FeatureState>,
    /// Features whose geometry was computed in this call (not taken from the cache).
    pub recomputed: usize,
    /// Planes of the sketches on faces, as this rebuild placed them.
    pub planes: SketchPlanes,
}

impl RebuildResult {
    pub fn total_volume(&self) -> f64 {
        self.bodies.iter().map(|(_, b)| b.volume()).sum()
    }

    pub fn body(&self, id: FeatureId) -> Option<&Arc<Body>> {
        self.bodies.iter().find(|(i, _)| *i == id).map(|(_, b)| b)
    }
}

/// Key, bodies after the feature, state.
type Cached = (u64, Bodies, FeatureState);

/// Keeps the bodies after every feature, keyed by a hash of everything that feature and the ones before it depend on.
#[derive(Default)]
pub struct Rebuilder {
    cache: HashMap<FeatureId, Cached>,
}

/// Feature inputs with every expression evaluated.
pub(crate) enum Tool {
    Extrude { distance: f64, offset: f64, flip: bool },
    Revolve { axis_point: DVec2, axis_dir: DVec2, angle: f64 },
    /// The inputs' profile and plane are carried along the path.
    Sweep { path: Vec<PathPiece>, closed: bool },
    Loft { sections: Vec<(Profile, Plane3)> },
}

/// For a loft the profile and plane are those of the first section.
pub(crate) struct Inputs {
    pub(crate) profile: Profile,
    /// Everything the shapes and extra planes of the feature contribute to its key.
    pub(crate) shapes_json: String,
    pub(crate) plane: Plane3,
    pub(crate) tool: Tool,
    pub(crate) op: BodyOp,
    pub(crate) target: Target,
}

pub(crate) fn hash_of(v: impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

pub(crate) fn eval(doc: &Document, src: &str, angle: bool) -> Result<f64, String> {
    let value = doc.params().eval_expr(src).map_err(|e| e.to_string())?;
    let number = if angle { value.as_angle_rad() } else { value.as_length_mm() };
    number.ok_or_else(|| if angle { "expected an angle" } else { "expected a length" }.to_string())
}

const NOT_SOLID: &str = "not a solid feature";

/// A feature with its inputs evaluated.
enum Step {
    Solid(Inputs),
    Blend(Blend),
    Pattern(Pattern),
    Shell(Shell),
}

fn resolve(doc: &Document, f: &Feature, states: &BTreeMap<FeatureId, FeatureState>, planes: &SketchPlanes) -> Result<Step, String> {
    match &f.kind {
        FeatureKind::Fillet { edges, faces, radius: size } | FeatureKind::Chamfer { edges, faces, distance: size } => {
            let fillet = matches!(f.kind, FeatureKind::Fillet { .. });
            Ok(Step::Blend(Blend { edges: edges.clone(), faces: faces.clone(), size: eval(doc, size, false)?, fillet }))
        }
        FeatureKind::LinearPattern { .. } | FeatureKind::CircularPattern { .. } | FeatureKind::Mirror { .. } => {
            resolve_pattern(doc, f, states, planes).map(Step::Pattern)
        }
        FeatureKind::Shell { thickness, open, at } => {
            Ok(Step::Shell(Shell { thickness: eval(doc, thickness, false)?, open: *open, at: *at }))
        }
        _ => resolve_solid(doc, f, planes).map(Step::Solid),
    }
}

pub(crate) fn resolve_solid(doc: &Document, f: &Feature, planes: &SketchPlanes) -> Result<Inputs, String> {
    if matches!(f.kind, FeatureKind::Sweep { .. } | FeatureKind::Loft { .. }) {
        return resolve_sweep(doc, f, planes);
    }
    let (profile_ids, op, target, numbers) = match &f.kind {
        FeatureKind::Extrude { profile, distance, offset, op, target, .. } => {
            (profile, *op, *target, (eval(doc, distance, false)?, eval(doc, offset, false)?))
        }
        FeatureKind::Revolve { profile, angle, op, target, .. } => (profile, *op, *target, (eval(doc, angle, true)?, 0.0)),
        _ => return Err(NOT_SOLID.to_string()),
    };
    let shape = |id| doc.get(id).map(|e| &e.shape).ok_or_else(|| format!("entity {id} not found"));
    let shapes = profile_shapes(doc, profile_ids)?;
    let profile = Profile::from_shapes(&shapes).map_err(|e| e.to_string())?;
    let shapes_json = serde_json::to_string(&shapes).map_err(|e| e.to_string())?;
    // A profile is never empty and all its entities share a sketch (both checked when the feature is added).
    let sketch = profile_ids.first().and_then(|id| doc.get(*id)).map_or(0, |e| e.sketch);
    let plane = doc.plane_in(sketch, planes)?;
    let tool = match &f.kind {
        FeatureKind::Extrude { flip, .. } => Tool::Extrude { distance: numbers.0, offset: numbers.1, flip: *flip },
        FeatureKind::Revolve { axis, .. } => {
            let (axis_point, axis_dir) = match axis {
                RevolveAxis::X => (DVec2::ZERO, DVec2::X),
                RevolveAxis::Y => (DVec2::ZERO, DVec2::Y),
                RevolveAxis::Line(id) => match shape(*id)? {
                    Shape::Line { a, b } => (*a, *b - *a),
                    _ => return Err("axis is not a line".to_string()),
                },
            };
            Tool::Revolve { axis_point, axis_dir, angle: numbers.0 }
        }
        _ => return Err(NOT_SOLID.to_string()),
    };
    Ok(Inputs { profile, shapes_json, plane, tool, op, target })
}

/// The shapes of the entities that make up a profile or path; construction entities are left out.
pub(crate) fn profile_shapes<'a>(doc: &'a Document, ids: &[EntityId]) -> Result<Vec<&'a Shape>, String> {
    let mut shapes = Vec::new();
    for &id in ids {
        let e = doc.get(id).ok_or_else(|| format!("entity {id} not found"))?;
        if !e.construction {
            shapes.push(&e.shape);
        }
    }
    Ok(shapes)
}

/// `a` combined with `b`; an empty result is `None`.
fn combine(a: &Body, b: &Body, op: BoolOp) -> Result<Option<Arc<Body>>, String> {
    match boolean(a, b, op) {
        Ok(body) => Ok(Some(Arc::new(body))),
        Err(ModelError::Invalid("boolean result is empty")) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

impl Inputs {
    pub(crate) fn key(&self, prev: u64) -> u64 {
        let (tag, mut numbers) = match &self.tool {
            Tool::Extrude { distance, offset, flip } => ("extrude", vec![distance.to_bits(), offset.to_bits(), u64::from(*flip)]),
            Tool::Revolve { axis_point, axis_dir, angle } => {
                let bits = [axis_point.x, axis_point.y, axis_dir.x, axis_dir.y, *angle].map(f64::to_bits);
                ("revolve", bits.to_vec())
            }
            Tool::Sweep { closed, .. } => ("sweep", vec![u64::from(*closed)]),
            Tool::Loft { .. } => ("loft", Vec::new()),
        };
        let Plane3 { origin, x, y } = self.plane;
        numbers.extend([origin, x, y].iter().flat_map(|v| v.to_array()).map(f64::to_bits));
        hash_of((prev, tag, self.op, numbers, self.target, &self.shapes_json))
    }

    pub(crate) fn tool_body(&self) -> Result<Body, String> {
        let solid = |brep: Result<_, ModelError>| brep.and_then(|b| Body::from_brep(&b, BODY_TOLERANCE));
        let body = match &self.tool {
            Tool::Extrude { distance, offset, flip } => {
                let plane = Plane3 { origin: self.plane.origin + self.plane.normal() * *offset, ..self.plane };
                solid(extrude(&self.profile, &plane, if *flip { -distance } else { *distance }))
            }
            Tool::Revolve { axis_point, axis_dir, angle } => solid(revolve(&self.profile, &self.plane, *axis_point, *axis_dir, *angle)),
            Tool::Sweep { path, closed } => sweep(&self.profile, &self.plane, path, *closed, BODY_TOLERANCE),
            Tool::Loft { sections } => {
                let sections: Vec<Section> = sections.iter().map(|(profile, plane)| Section { profile, plane }).collect();
                loft(&sections, BODY_TOLERANCE)
            }
        };
        body.map_err(|e| e.to_string())
    }

    /// Positions in `bodies` (ascending) that the feature combines with.
    fn targets(&self, bodies: &Bodies) -> Result<Vec<usize>, String> {
        Ok(match self.target {
            Target::Body(id) => vec![bodies.iter().position(|(i, _)| *i == id).ok_or("target body not found")?],
            Target::Auto if self.op != BodyOp::Cut => bodies.len().checked_sub(1).into_iter().collect(),
            Target::Auto | Target::All => (0..bodies.len()).collect(),
        })
    }

    fn apply(&self, id: FeatureId, bodies: &Bodies) -> Result<Bodies, String> {
        self.place(id, bodies, self.tool_body()?)
    }

    /// The body list after feature `id` combined `tool` with the bodies; `bodies` is not touched, so an error leaves it as it was.
    pub(crate) fn place(&self, id: FeatureId, bodies: &Bodies, tool: Body) -> Result<Bodies, String> {
        let mut out = bodies.clone();
        if self.op == BodyOp::New {
            out.push((id, Arc::new(tool)));
            return Ok(out);
        }
        let targets = self.targets(bodies)?;
        if self.op == BodyOp::Add {
            let Some((&first, rest)) = targets.split_first() else {
                out.push((id, Arc::new(tool)));
                return Ok(out);
            };
            let mut merged = combine(&bodies[first].1, &tool, BoolOp::Union)?.ok_or("boolean result is empty")?;
            for &i in rest {
                merged = combine(&merged, &bodies[i].1, BoolOp::Union)?.ok_or("boolean result is empty")?;
            }
            out[first].1 = merged;
            for &i in rest.iter().rev() {
                out.remove(i);
            }
            return Ok(out);
        }
        let (op, missing) = match self.op {
            BodyOp::Cut => (BoolOp::Difference, "nothing to cut from"),
            _ => (BoolOp::Intersection, "nothing to intersect with"),
        };
        if targets.is_empty() {
            return Err(missing.to_string());
        }
        let mut gone = Vec::new();
        for &i in &targets {
            match combine(&bodies[i].1, &tool, op)? {
                Some(b) => out[i].1 = b,
                None => gone.push(i),
            }
        }
        for &i in gone.iter().rev() {
            out.remove(i);
        }
        Ok(out)
    }
}

impl Step {
    fn key(&self, prev: u64) -> u64 {
        match self {
            Step::Solid(i) => i.key(prev),
            Step::Blend(b) => b.key(prev),
            Step::Pattern(p) => p.key(prev),
            Step::Shell(s) => s.key(prev),
        }
    }

    fn apply(&self, id: FeatureId, bodies: &Bodies) -> Result<Bodies, String> {
        match self {
            Step::Solid(i) => i.apply(id, bodies),
            Step::Blend(b) => b.apply(bodies),
            Step::Pattern(p) => p.apply(id, bodies),
            Step::Shell(s) => s.apply(bodies),
        }
    }
}

/// Puts the plane of `sketch` into `planes` when it lies on a face and this rebuild has not placed it yet.
fn place_sketch(sketch: &Sketch, bodies: &Bodies, planes: &mut SketchPlanes) {
    if let PlaneDef::Face { origin, x, y, at, body } = &sketch.plane
        && !planes.contains_key(&sketch.id)
    {
        planes.insert(sketch.id, face_plane(Plane3 { origin: *origin, x: *x, y: *y }, *at, *body, bodies));
    }
}

impl Rebuilder {
    /// Walks the features in order; features whose inputs and predecessors are unchanged come from the cache.
    /// A suppressed or failed feature leaves the bodies as they were and never stops the features after it.
    pub fn rebuild(&mut self, doc: &Document) -> RebuildResult {
        self.rebuild_until(doc, &|| false).expect("never stopped")
    }

    /// Like `rebuild`, but returns `None` as soon as `stop` says so; `stop` is checked before every feature.
    /// Features finished before the stop stay cached.
    pub fn rebuild_until(&mut self, doc: &Document, stop: &dyn Fn() -> bool) -> Option<RebuildResult> {
        let mut bodies: Bodies = Vec::new();
        let (mut prev, mut recomputed) = (0u64, 0);
        let mut states = BTreeMap::new();
        let mut planes = SketchPlanes::new();
        for f in doc.features() {
            if stop() {
                return None;
            }
            if f.suppressed {
                states.insert(f.id, FeatureState::Suppressed);
                prev = hash_of((prev, f.id, "suppressed"));
                continue;
            }
            for s in f.entities().iter().filter_map(|id| doc.get(*id)).filter_map(|e| doc.sketch(e.sketch)) {
                place_sketch(s, &bodies, &mut planes);
            }
            let step = match resolve(doc, f, &states, &planes) {
                Ok(i) => i,
                Err(msg) => {
                    prev = hash_of((prev, f.id, "error", &msg));
                    states.insert(f.id, FeatureState::Error(msg));
                    continue;
                }
            };
            let key = step.key(prev);
            prev = key;
            if let Some((_, cached, state)) = self.cache.get(&f.id).filter(|c| c.0 == key) {
                bodies = cached.clone();
                states.insert(f.id, state.clone());
                continue;
            }
            recomputed += 1;
            let applied = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| step.apply(f.id, &bodies)))
                .unwrap_or_else(|p| Err(format!("internal error: {}", panic_message(p.as_ref()))));
            let state = match applied {
                Ok(b) => {
                    bodies = b;
                    FeatureState::Ok
                }
                Err(msg) => FeatureState::Error(msg),
            };
            self.cache.insert(f.id, (key, bodies.clone(), state.clone()));
            states.insert(f.id, state);
        }
        for s in doc.sketches() {
            place_sketch(s, &bodies, &mut planes);
        }
        self.cache.retain(|id, _| doc.feature(*id).is_some());
        Some(RebuildResult { bodies, states, recomputed, planes })
    }
}

/// Text of a panic payload (`panic!` gives `&str` or `String`).
pub fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown panic".into())
}

#[cfg(test)]
#[path = "rebuild_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "rebuild_bodies_tests.rs"]
mod bodies_tests;
