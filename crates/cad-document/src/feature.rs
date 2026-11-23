use crate::document::{Command, DocError, Document, EntityId};
use crate::part::PartDef;
use crate::refs::{FaceKey, FaceRef};
use crate::sketch::{BasePlane, SketchId};
use cad_geometry::DVec3;
use cad_params::parse;
use serde::{Deserialize, Serialize};

pub type FeatureId = u64;
/// Tessellation tolerance of feature bodies, mm.
pub const BODY_TOLERANCE: f64 = 0.02;
/// mm: how far a stored point may be from the edge it names.
pub const EDGE_PICK_TOLERANCE: f64 = 0.05;

/// Taper of extrudes from files before version 14.
fn zero_angle() -> String {
    "0 deg".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyOp {
    New,
    Add,
    Cut,
    Intersect,
}

/// Which bodies an `Add`, `Cut` or `Intersect` feature combines with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Target {
    /// `Add` and `Intersect`: the last body. `Cut`: every body.
    #[default]
    Auto,
    All,
    /// The body created by the feature with this id.
    Body(FeatureId),
}

/// A world axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Axis3 {
    X,
    Y,
    Z,
}

/// What a mirror copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MirrorSource {
    Feature(FeatureId),
    Bodies,
}

/// Sketch X axis, sketch Y axis, or a Line entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RevolveAxis {
    X,
    Y,
    Line(EntityId),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeatureKind {
    /// The solid runs from the sketch plane moved by `offset` along its normal to `distance` further (against it when `flip`).
    /// `taper` is the draft angle of the side walls about that start plane: positive narrows the solid away from it.
    Extrude {
        profile: Vec<EntityId>,
        distance: String,
        offset: String,
        #[serde(default = "zero_angle")]
        taper: String,
        op: BodyOp,
        #[serde(default)]
        flip: bool,
        #[serde(default)]
        target: Target,
    },
    Revolve {
        profile: Vec<EntityId>,
        axis: RevolveAxis,
        angle: String,
        op: BodyOp,
        #[serde(default)]
        target: Target,
    },
    /// Each point lies on an edge of a body, as picked. `faces[i]` holds which way the two faces of edge `i` pointed
    /// when it was picked: the rebuild takes the nearest edge between faces that still point that way, so the edge
    /// follows when earlier features move it. `faces` is empty in files before version 13; each point must then lie
    /// within `EDGE_PICK_TOLERANCE` of its edge.
    Fillet {
        edges: Vec<DVec3>,
        #[serde(default)]
        faces: Vec<[FaceKey; 2]>,
        radius: String,
    },
    /// Like `Fillet`, with a chamfer distance.
    Chamfer {
        edges: Vec<DVec3>,
        #[serde(default)]
        faces: Vec<[FaceKey; 2]>,
        distance: String,
    },
    /// Repeats the solid of `source` `count` times in total, each copy shifted by one more step (dx, dy, dz) in world axes.
    LinearPattern { source: FeatureId, count: String, dx: String, dy: String, dz: String },
    /// Repeats the solid of `source` `count` times in total around a world axis through (cx, cy, cz).
    CircularPattern { source: FeatureId, count: String, angle: String, axis: Axis3, cx: String, cy: String, cz: String },
    /// Mirrors the solid of a feature, or every body, across a principal plane moved by `offset` along its normal.
    Mirror { source: MirrorSource, plane: BasePlane, offset: String },
    /// The profile (in one sketch) is carried along the path (a chain of lines and arcs in one sketch, usually another one).
    Sweep {
        profile: Vec<EntityId>,
        path: Vec<EntityId>,
        op: BodyOp,
        #[serde(default)]
        target: Target,
    },
    /// A solid through the sections in order; each section is a closed profile in its own sketch. Ruled between
    /// sections, or with `smooth` (three or more sections) a Catmull-Rom surface through them. `false` before version 14.
    Loft {
        sections: Vec<Vec<EntityId>>,
        op: BodyOp,
        #[serde(default)]
        target: Target,
        #[serde(default)]
        smooth: bool,
    },
    /// Hollows the body whose surface passes through `at` (as picked; a snapshot like the points of a fillet), leaving walls
    /// of `thickness`. With `open`, the planar face nearest to that point is removed.
    Shell { thickness: String, open: Option<DVec3>, at: DVec3 },
    /// A generated part (`PartDef`) on the plane of `sketch`, its axis along the plane normal through the point (x, y)
    /// of the sketch. Combined with the bodies like an extrude.
    Part {
        def: PartDef,
        sketch: SketchId,
        x: String,
        y: String,
        op: BodyOp,
        #[serde(default)]
        target: Target,
    },
    /// Turns planar faces of body `body` about the neutral plane, the plane of `sketch` (pull direction: its normal),
    /// by `angle`; positive narrows the solid away from that plane. Each face is a reference found again on every
    /// rebuild, so the draft follows when earlier features move the faces. Faces next to a drafted face must be planes
    /// at right angles to it (`cad_modeler::draft_face`).
    Draft { body: FeatureId, faces: Vec<FaceRef>, sketch: SketchId, angle: String },
    /// A closed triangle mesh imported from STL or OBJ (mm): a new body. Points as written in the file (f32), shared
    /// between triangles; each triangle lists three point indices counter-clockwise seen from outside. Version 28.
    // shortcut: stored as plain JSON arrays (about 1.5x the binary STL size); store base64 if imported meshes make project files too large
    Mesh { positions: Vec<[f32; 3]>, triangles: Vec<[u32; 3]> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub id: FeatureId,
    pub name: String,
    pub suppressed: bool,
    pub kind: FeatureKind,
}

impl Feature {
    /// Profile entities, a Line axis, a sweep path or loft sections; none for a fillet, chamfer, pattern or mirror.
    pub fn entities(&self) -> Vec<EntityId> {
        match &self.kind {
            FeatureKind::Extrude { profile, .. } => profile.clone(),
            FeatureKind::Revolve { profile, axis, .. } => {
                profile.iter().copied().chain(if let RevolveAxis::Line(id) = axis { Some(*id) } else { None }).collect()
            }
            FeatureKind::Sweep { profile, path, .. } => profile.iter().chain(path).copied().collect(),
            FeatureKind::Loft { sections, .. } => sections.iter().flatten().copied().collect(),
            _ => Vec::new(),
        }
    }

    /// The sketch a part sits on or the neutral plane of a draft; None for other features (their sketches come from their entities).
    pub fn sketch(&self) -> Option<SketchId> {
        if let FeatureKind::Part { sketch, .. } | FeatureKind::Draft { sketch, .. } = &self.kind { Some(*sketch) } else { None }
    }

    /// The entities that must lie in one sketch each.
    fn groups(&self) -> Vec<Vec<EntityId>> {
        match &self.kind {
            FeatureKind::Sweep { profile, path, .. } => vec![profile.clone(), path.clone()],
            FeatureKind::Loft { sections, .. } => sections.clone(),
            _ => vec![self.entities()],
        }
    }

    /// Every expression of the feature, in the order of its fields.
    pub fn expressions(&self) -> Vec<&str> {
        match &self.kind {
            FeatureKind::Fillet { radius: size, .. } | FeatureKind::Chamfer { distance: size, .. } => vec![size.as_str()],
            FeatureKind::Shell { thickness, .. } => vec![thickness.as_str()],
            FeatureKind::Extrude { distance, offset, taper, .. } => vec![distance.as_str(), offset.as_str(), taper.as_str()],
            FeatureKind::Revolve { angle, .. } => vec![angle.as_str()],
            FeatureKind::LinearPattern { count, dx, dy, dz, .. } => [count, dx, dy, dz].map(String::as_str).to_vec(),
            FeatureKind::CircularPattern { count, angle, cx, cy, cz, .. } => [count, angle, cx, cy, cz].map(String::as_str).to_vec(),
            FeatureKind::Mirror { offset, .. } => vec![offset.as_str()],
            FeatureKind::Part { def, x, y, .. } => def.expressions().into_iter().chain([x.as_str(), y.as_str()]).collect(),
            FeatureKind::Draft { angle, .. } => vec![angle.as_str()],
            FeatureKind::Sweep { .. } | FeatureKind::Loft { .. } | FeatureKind::Mesh { .. } => Vec::new(),
        }
    }

    /// The feature whose solid a pattern or mirror copies.
    pub fn source(&self) -> Option<FeatureId> {
        match &self.kind {
            FeatureKind::LinearPattern { source, .. }
            | FeatureKind::CircularPattern { source, .. }
            | FeatureKind::Mirror { source: MirrorSource::Feature(source), .. } => Some(*source),
            _ => None,
        }
    }

    /// `None` unless the feature is an extrude or revolve.
    fn profile(&self) -> Option<&[EntityId]> {
        match &self.kind {
            FeatureKind::Extrude { profile, .. } | FeatureKind::Revolve { profile, .. } => Some(profile),
            _ => None,
        }
    }
}

fn bad(msg: &'static str) -> DocError {
    DocError::BadFeature(msg)
}

impl Document {
    pub fn params(&self) -> &cad_params::ParamSet {
        &self.params
    }

    pub fn features(&self) -> &[Feature] {
        &self.features
    }

    pub fn feature(&self, id: FeatureId) -> Option<&Feature> {
        self.features.iter().find(|f| f.id == id)
    }

    pub fn alloc_feature_id(&mut self) -> FeatureId {
        let id = self.next_feature_id;
        self.next_feature_id += 1;
        id
    }

    pub fn features_using_entity(&self, id: EntityId) -> Vec<FeatureId> {
        self.features.iter().filter(|f| f.entities().contains(&id)).map(|f| f.id).collect()
    }

    /// Features whose expressions use the parameter, directly or through other parameters.
    pub fn features_using_param(&self, name: &str) -> Vec<FeatureId> {
        let mut names = self.params.dependents(name);
        names.insert(name.to_string());
        let uses = |src: &&str| parse(src).is_ok_and(|e| e.variables().iter().any(|v| names.contains(v)));
        self.features.iter().filter(|f| f.expressions().iter().any(uses)).map(|f| f.id).collect()
    }

    fn check_feature(&self, f: &Feature) -> Result<(), DocError> {
        if f.name.trim().is_empty() {
            return Err(bad("empty name"));
        }
        if let Some(source) = f.source() {
            if source == f.id {
                return Err(bad("a feature cannot use itself"));
            }
            self.feature_index(source)?;
        }
        match (&f.kind, f.profile()) {
            (FeatureKind::Fillet { edges, faces, .. } | FeatureKind::Chamfer { edges, faces, .. }, _) => {
                if edges.is_empty() {
                    return Err(bad("no edges"));
                }
                if !edges.iter().all(|p| p.is_finite()) {
                    return Err(bad("edge point is not finite"));
                }
                if !faces.is_empty() && faces.len() != edges.len() {
                    return Err(bad("one face pair per edge"));
                }
            }
            (FeatureKind::Shell { open, at, .. }, _) => {
                if !at.is_finite() || open.is_some_and(|p| !p.is_finite()) {
                    return Err(bad("shell point is not finite"));
                }
            }
            (FeatureKind::Part { def, sketch, .. }, _) => {
                if self.sketch(*sketch).is_none() {
                    return Err(DocError::SketchNotFound(*sketch));
                }
                def.check().map_err(bad)?;
            }
            (FeatureKind::Draft { faces, sketch, .. }, _) => {
                if faces.is_empty() {
                    return Err(bad("no faces"));
                }
                if !faces.iter().all(|r| r.at.is_finite()) {
                    return Err(bad("face point is not finite"));
                }
                if self.sketch(*sketch).is_none() {
                    return Err(DocError::SketchNotFound(*sketch));
                }
            }
            (FeatureKind::Sweep { profile, path, .. }, _) if profile.is_empty() || path.is_empty() => {
                return Err(bad("empty profile or path"));
            }
            (FeatureKind::Loft { sections, .. }, _) if sections.len() < 2 => return Err(bad("loft needs at least two sections")),
            (FeatureKind::Loft { sections, .. }, _) if sections.iter().any(Vec::is_empty) => return Err(bad("empty section")),
            (FeatureKind::Mesh { positions, triangles }, _) => {
                if triangles.len() < 4 || triangles.iter().flatten().any(|&i| i as usize >= positions.len()) || positions.iter().flatten().any(|v| !v.is_finite()) {
                    return Err(bad("mesh needs at least 4 triangles with valid point indices"));
                }
            }
            (_, Some([])) => return Err(bad("empty profile")),
            _ => {}
        }
        let ids = f.entities();
        if let Some(missing) = ids.iter().find(|e| !self.entities.contains_key(e)) {
            return Err(DocError::NotFound(*missing));
        }
        f.groups().iter().try_for_each(|g| self.check_one_sketch(g))?;
        f.expressions().into_iter().try_for_each(|src| parse(src).map(drop))?;
        Ok(())
    }

    fn feature_index(&self, id: FeatureId) -> Result<usize, DocError> {
        self.features.iter().position(|f| f.id == id).ok_or(DocError::FeatureNotFound(id))
    }

    /// Validates first, so a failed apply changes nothing. Returns the inverse command.
    pub(crate) fn apply_feature(&mut self, cmd: Command) -> Result<Command, DocError> {
        match cmd {
            Command::AddFeature { index, feature } => {
                if self.feature(feature.id).is_some() {
                    return Err(DocError::FeatureExists(feature.id));
                }
                if index > self.features.len() {
                    return Err(bad("index out of range"));
                }
                self.check_feature(&feature)?;
                let id = feature.id;
                self.next_feature_id = self.next_feature_id.max(id.saturating_add(1));
                self.features.insert(index, feature);
                Ok(Command::RemoveFeature(id))
            }
            Command::RemoveFeature(id) => {
                let index = self.feature_index(id)?;
                if self.features.iter().any(|f| f.source() == Some(id)) {
                    return Err(DocError::FeatureInUse(id));
                }
                if self.components.values().any(|c| c.body == id) {
                    return Err(DocError::BodyInUse(id));
                }
                Ok(Command::AddFeature { index, feature: self.features.remove(index) })
            }
            Command::SetFeature(feature) => {
                let index = self.feature_index(feature.id)?;
                self.check_feature(&feature)?;
                Ok(Command::SetFeature(std::mem::replace(&mut self.features[index], feature)))
            }
            Command::MoveFeature { id, index } => {
                let old = self.feature_index(id)?;
                if index >= self.features.len() {
                    return Err(bad("index out of range"));
                }
                let f = self.features.remove(old);
                self.features.insert(index, f);
                Ok(Command::MoveFeature { id, index: old })
            }
            Command::SetParam { name, expr } => {
                let previous = self.params.get(&name).map(str::to_string);
                match expr {
                    Some(src) => self.params.set(&name, &src)?,
                    None if previous.is_some() => {
                        self.params.remove(&name);
                    }
                    None => return Err(cad_params::ParamError::UnknownName(name).into()),
                }
                Ok(Command::SetParam { name, expr: previous })
            }
            _ => unreachable!("not a feature command"),
        }
    }
}

#[cfg(test)]
#[path = "feature_tests.rs"]
mod tests;
