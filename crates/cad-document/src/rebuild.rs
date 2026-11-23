use crate::component::ComponentId;
use crate::document::{Document, EntityId};
use crate::feature::{BODY_TOLERANCE, BodyOp, Feature, FeatureId, FeatureKind, RevolveAxis, Target};
use crate::joint::JointId;
use crate::rebuild_blend::Blend;
use crate::rebuild_draft::Draft;
use crate::rebuild_pattern::{Pattern, resolve_pattern};
use crate::rebuild_shell::Shell;
use crate::rebuild_sweep::resolve_sweep;
use crate::refs::face_plane;
use crate::sketch::{PlaneDef, Sketch, SketchPlanes};
use cad_geometry::profile::Profile;
use cad_geometry::{DVec2, Shape};
use cad_modeler::{Body, BoolOp, ModelError, PathPiece, Plane3, Section, boolean, extrude, extrude_tapered, loft, revolve, sweep};
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
    /// For each feature that resolved, the features it read from, ascending: the last writer of every body it reads,
    /// the last writer of the body under each sketch-on-face it uses, and its pattern source.
    pub depends: BTreeMap<FeatureId, Vec<FeatureId>>,
    /// Planes of the sketches on faces, as this rebuild placed them.
    pub planes: SketchPlanes,
    /// Wall, floor slab, roof and stair solids by entity id (`rebuild_walls`).
    pub walls: Vec<(EntityId, Arc<Body>)>,
    /// The enclosed area of each room whose seed lies in one, by room entity id (`rebuild_walls`).
    pub rooms: Vec<(EntityId, cad_arch::Region)>,
    /// The solid of every component in id order, or why it could not be placed (`Document::placements`, `Document::place_component`).
    pub components: Vec<(ComponentId, Result<Arc<Body>, String>)>,
    /// Solved values (radians or mm) of the free joints by joint id (`Document::solve_joints`); empty without loops
    /// or when the solve failed (the components' errors say why).
    pub joint_values: BTreeMap<JointId, f64>,
}

impl RebuildResult {
    pub fn total_volume(&self) -> f64 {
        self.bodies.iter().map(|(_, b)| b.volume()).sum()
    }

    /// The placed solid of component `id`; None when the component is missing or failed.
    pub fn component(&self, id: ComponentId) -> Option<&Arc<Body>> {
        self.components.iter().find(|(i, _)| *i == id).and_then(|(_, r)| r.as_ref().ok())
    }

    pub fn body(&self, id: FeatureId) -> Option<&Arc<Body>> {
        self.bodies.iter().find(|(i, _)| *i == id).map(|(_, b)| b)
    }
}

/// What one feature changed in the body list: the bodies it created or replaced and the ids it removed. Cached
/// instead of the whole list, so a feature whose key is unchanged applies the same change to the bodies as they are now.
#[derive(Clone, Default)]
pub(crate) struct Delta {
    removed: Vec<FeatureId>,
    put: Vec<(FeatureId, Arc<Body>)>,
}

impl Delta {
    /// The change from `before` to `after`; `after` keeps the order of the bodies it shares with `before` and
    /// appends new ones (every feature does).
    fn between(before: &Bodies, after: &Bodies) -> Delta {
        let removed = before.iter().filter(|(id, _)| !after.iter().any(|(j, _)| j == id)).map(|(id, _)| *id).collect();
        let put = after.iter().filter(|(id, b)| !before.iter().any(|(j, c)| j == id && Arc::ptr_eq(b, c))).cloned().collect();
        Delta { removed, put }
    }

    /// `bodies` without the removed ids, each put body replacing the body with its id in place or appended.
    fn apply(&self, bodies: &Bodies) -> Bodies {
        let mut out: Bodies = bodies.iter().filter(|(id, _)| !self.removed.contains(id)).cloned().collect();
        for (id, b) in &self.put {
            match out.iter_mut().find(|(j, _)| j == id) {
                Some(slot) => slot.1 = b.clone(),
                None => out.push((*id, b.clone())),
            }
        }
        out
    }
}

/// Key, change to the body list, state.
type Cached = (u64, Delta, FeatureState);

/// Keeps what every feature changed in the body list, keyed by a hash of its evaluated inputs and the versions of the bodies it reads.
#[derive(Default)]
pub struct Rebuilder {
    cache: HashMap<FeatureId, Cached>,
}

/// Feature inputs with every expression evaluated.
pub(crate) enum Tool {
    Extrude {
        distance: f64,
        offset: f64,
        flip: bool,
        /// Draft angle in radians; 0 uses the B-Rep extrude.
        taper: f64,
    },
    Revolve { axis_point: DVec2, axis_dir: DVec2, angle: f64 },
    /// The inputs' profile and plane are carried along the path.
    Sweep { path: Vec<PathPiece>, closed: bool },
    Loft { sections: Vec<(Profile, Plane3)>, smooth: bool },
    /// A generated part; the plane is its placement.
    Part(cad_mech::Part),
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
    Draft(Draft),
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
        FeatureKind::Draft { body, faces, sketch, angle } => {
            Ok(Step::Draft(Draft { body: *body, faces: faces.clone(), plane: doc.plane_in(*sketch, planes)?, angle: eval(doc, angle, true)? }))
        }
        _ => resolve_solid(doc, f, planes).map(Step::Solid),
    }
}

pub(crate) fn resolve_solid(doc: &Document, f: &Feature, planes: &SketchPlanes) -> Result<Inputs, String> {
    if matches!(f.kind, FeatureKind::Part { .. }) {
        return crate::part::resolve_part(doc, f, planes);
    }
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
        FeatureKind::Extrude { flip, taper, .. } => {
            Tool::Extrude { distance: numbers.0, offset: numbers.1, flip: *flip, taper: eval(doc, taper, true)? }
        }
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
            Tool::Extrude { distance, offset, flip, taper } => {
                ("extrude", vec![distance.to_bits(), offset.to_bits(), u64::from(*flip), taper.to_bits()])
            }
            Tool::Revolve { axis_point, axis_dir, angle } => {
                let bits = [axis_point.x, axis_point.y, axis_dir.x, axis_dir.y, *angle].map(f64::to_bits);
                ("revolve", bits.to_vec())
            }
            Tool::Sweep { closed, .. } => ("sweep", vec![u64::from(*closed)]),
            Tool::Loft { smooth, .. } => ("loft", vec![u64::from(*smooth)]),
            Tool::Part(_) => ("part", Vec::new()),
        };
        let Plane3 { origin, x, y } = self.plane;
        numbers.extend([origin, x, y].iter().flat_map(|v| v.to_array()).map(f64::to_bits));
        hash_of((prev, tag, self.op, numbers, self.target, &self.shapes_json))
    }

    pub(crate) fn tool_body(&self) -> Result<Body, String> {
        let solid = |brep: Result<_, ModelError>| brep.and_then(|b| Body::from_brep(&b, BODY_TOLERANCE));
        let body = match &self.tool {
            Tool::Extrude { distance, offset, flip, taper } => {
                let plane = Plane3 { origin: self.plane.origin + self.plane.normal() * *offset, ..self.plane };
                let distance = if *flip { -distance } else { *distance };
                if *taper == 0.0 {
                    solid(extrude(&self.profile, &plane, distance))
                } else {
                    extrude_tapered(&self.profile, &plane, distance, *taper, BODY_TOLERANCE)
                }
            }
            Tool::Revolve { axis_point, axis_dir, angle } => solid(revolve(&self.profile, &self.plane, *axis_point, *axis_dir, *angle)),
            Tool::Sweep { path, closed } => sweep(&self.profile, &self.plane, path, *closed, BODY_TOLERANCE),
            Tool::Loft { sections, smooth } => {
                let sections: Vec<Section> = sections.iter().map(|(profile, plane)| Section { profile, plane }).collect();
                loft(&sections, *smooth, BODY_TOLERANCE)
            }
            Tool::Part(part) => part.body(&self.plane, BODY_TOLERANCE),
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

    /// Ids of the bodies the feature combines with; a `New` feature reads none. A missing `Body(id)` target is still read (as missing).
    fn reads(&self, bodies: &Bodies) -> Vec<FeatureId> {
        match (self.op, self.target) {
            (BodyOp::New, _) => Vec::new(),
            (_, Target::Body(id)) => vec![id],
            _ => self.targets(bodies).unwrap_or_default().into_iter().map(|i| bodies[i].0).collect(),
        }
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
    /// Ids of the bodies the feature reads; their versions are the rest of its key. Fillets, chamfers, shells and
    /// mirrors of all bodies search every body.
    fn reads(&self, bodies: &Bodies) -> Vec<FeatureId> {
        match self {
            Step::Solid(i) | Step::Pattern(Pattern::Copies { root: i, .. }) => i.reads(bodies),
            Step::Draft(d) => vec![d.body],
            Step::Blend(_) | Step::Shell(_) | Step::Pattern(Pattern::Bodies(_)) => bodies.iter().map(|(id, _)| *id).collect(),
        }
    }

    fn key(&self, prev: u64) -> u64 {
        match self {
            Step::Solid(i) => i.key(prev),
            Step::Blend(b) => b.key(prev),
            Step::Pattern(p) => p.key(prev),
            Step::Shell(s) => s.key(prev),
            Step::Draft(d) => d.key(prev),
        }
    }

    fn apply(&self, id: FeatureId, bodies: &Bodies) -> Result<Bodies, String> {
        match self {
            Step::Solid(i) => i.apply(id, bodies),
            Step::Blend(b) => b.apply(bodies),
            Step::Pattern(p) => p.apply(id, bodies),
            Step::Shell(s) => s.apply(bodies),
            Step::Draft(d) => d.apply(bodies),
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
    /// Walks the features in order; a feature is recomputed only when its own evaluated inputs or a body it reads
    /// changed, otherwise its change comes from the cache. A suppressed or failed feature leaves the bodies as they were and never stops the features after it.
    pub fn rebuild(&mut self, doc: &Document) -> RebuildResult {
        self.rebuild_until(doc, &|| false).expect("never stopped")
    }

    /// Like `rebuild`, but returns `None` as soon as `stop` says so; `stop` is checked before every feature.
    /// Features finished before the stop stay cached.
    pub fn rebuild_until(&mut self, doc: &Document, stop: &dyn Fn() -> bool) -> Option<RebuildResult> {
        let mut bodies: Bodies = Vec::new();
        // Body id -> (its version: the key of the feature that last wrote it, that feature's id).
        let mut written: HashMap<FeatureId, (u64, FeatureId)> = HashMap::new();
        let mut recomputed = 0;
        let (mut states, mut depends) = (BTreeMap::new(), BTreeMap::new());
        let mut planes = SketchPlanes::new();
        let ids = |b: &Bodies| b.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        for f in doc.features() {
            if stop() {
                return None;
            }
            if f.suppressed {
                states.insert(f.id, FeatureState::Suppressed);
                continue;
            }
            let sketches: Vec<&Sketch> =
                f.entities().iter().filter_map(|id| doc.get(*id)).map(|e| e.sketch).chain(f.sketch()).filter_map(|id| doc.sketch(id)).collect();
            for s in &sketches {
                place_sketch(s, &bodies, &mut planes);
            }
            let step = match resolve(doc, f, &states, &planes) {
                Ok(s) => s,
                Err(msg) => {
                    states.insert(f.id, FeatureState::Error(msg));
                    continue;
                }
            };
            let reads = step.reads(&bodies);
            let on_faces = sketches.iter().filter_map(|s| match s.plane {
                PlaneDef::Face { body, .. } => Some(body),
                _ => None,
            });
            let mut uses: Vec<FeatureId> = reads.iter().copied().chain(on_faces).filter_map(|b| written.get(&b).map(|w| w.1)).chain(f.source()).collect();
            uses.sort_unstable();
            uses.dedup();
            depends.insert(f.id, uses);
            // The key covers the feature's own evaluated inputs and the versions of the bodies it reads; features
            // before it that only touched other bodies leave it unchanged.
            let versions: Vec<(FeatureId, Option<u64>)> = reads.iter().map(|b| (*b, written.get(b).map(|w| w.0))).collect();
            let key = step.key(hash_of(versions));
            let delta = match self.cache.get(&f.id).filter(|c| c.0 == key) {
                Some((_, delta, state)) => {
                    states.insert(f.id, state.clone());
                    delta.clone()
                }
                None => {
                    recomputed += 1;
                    let applied = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| step.apply(f.id, &bodies)))
                        .unwrap_or_else(|p| Err(format!("internal error: {}", panic_message(p.as_ref()))));
                    let (delta, state) = match applied {
                        Ok(after) => {
                            let delta = Delta::between(&bodies, &after);
                            debug_assert_eq!(ids(&delta.apply(&bodies)), ids(&after), "feature {} reordered the bodies", f.id);
                            (delta, FeatureState::Ok)
                        }
                        Err(msg) => (Delta::default(), FeatureState::Error(msg)),
                    };
                    self.cache.insert(f.id, (key, delta.clone(), state.clone()));
                    states.insert(f.id, state);
                    delta
                }
            };
            bodies = delta.apply(&bodies);
            for id in &delta.removed {
                written.remove(id);
            }
            for (id, _) in &delta.put {
                written.insert(*id, (key, f.id));
            }
        }
        for s in doc.sketches() {
            place_sketch(s, &bodies, &mut planes);
        }
        self.cache.retain(|id, _| doc.feature(*id).is_some());
        let walls = crate::rebuild_walls::wall_bodies(doc);
        let rooms = crate::rebuild_walls::room_regions(doc);
        // Not cached: a placement is a mesh copy, and features stay cached.
        let mut joint_values = BTreeMap::new();
        let placements = doc.placements_driven(&bodies, None, &mut joint_values);
        let components = doc
            .components()
            .map(|c| (c.id, placements[&c.id].clone().and_then(|p| doc.place_component(c, &p, &bodies))))
            .collect();
        Some(RebuildResult { bodies, states, recomputed, depends, planes, walls, rooms, components, joint_values })
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
