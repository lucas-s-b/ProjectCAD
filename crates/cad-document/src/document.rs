use crate::component::{Component, ComponentId};
use crate::joint::{ExplodeStep, Joint, JointId};
use crate::linetype::LineType;
use crate::feature::{Feature, FeatureId};
use crate::sketch::{Sketch, SketchId, default_sketches, first_sketch_id};
use crate::units::Settings;
use cad_constraints::{Constraint, PointRef, SolveError};
use cad_geometry::{GeomError, Shape, Tolerance};
use cad_params::{ParamError, ParamSet};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type EntityId = u64;
pub type LayerId = u32;
pub type ConstraintId = u64;
pub use crate::sketch::FORMAT_VERSION;

pub const MIN_LINE_WEIGHT: f64 = 0.05;
pub const MAX_LINE_WEIGHT: f64 = 2.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub color: [u8; 3],
    pub visible: bool,
    pub locked: bool,
    #[serde(default)]
    pub line_type: LineType,
    /// Millimetres on paper.
    #[serde(default = "default_weight")]
    pub line_weight: f64,
}

fn default_weight() -> f64 {
    0.25
}

impl Layer {
    pub fn new(id: LayerId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            color: [230, 230, 230],
            visible: true,
            locked: false,
            line_type: LineType::Continuous,
            line_weight: default_weight(),
        }
    }

    pub fn check_weight(&self) -> Result<(), DocError> {
        if (MIN_LINE_WEIGHT..=MAX_LINE_WEIGHT).contains(&self.line_weight) { Ok(()) } else { Err(DocError::BadLineWeight) }
    }
}

fn default_layers() -> BTreeMap<LayerId, Layer> {
    BTreeMap::from([(0, Layer::new(0, "0"))])
}

fn first_constraint_id() -> ConstraintId {
    1
}

fn first_layer_id() -> LayerId {
    1
}

fn first_feature_id() -> FeatureId {
    1
}

fn first_component_id() -> ComponentId {
    1
}

fn first_joint_id() -> JointId {
    1
}

fn first_group_id() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub shape: Shape,
    #[serde(default)]
    pub layer: LayerId,
    #[serde(default)]
    pub sketch: SketchId,
    /// Reference geometry: drawn dashed, snaps and takes constraints, but is not exported or used as a profile.
    #[serde(default)]
    pub construction: bool,
    /// Entities sharing a group id are selected together. Not exported to DXF or SVG.
    #[serde(default)]
    pub group: Option<u32>,
    /// Free key/value notes. Not exported to DXF or SVG.
    #[serde(default)]
    pub meta: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub(crate) version: u32,
    pub name: String,
    pub(crate) entities: BTreeMap<EntityId, Entity>,
    pub(crate) next_id: EntityId,
    #[serde(default = "default_layers")]
    pub(crate) layers: BTreeMap<LayerId, Layer>,
    #[serde(default = "first_layer_id")]
    pub(crate) next_layer_id: LayerId,
    #[serde(default)]
    pub(crate) settings: Settings,
    #[serde(default)]
    pub(crate) constraints: BTreeMap<ConstraintId, Constraint>,
    #[serde(default = "first_constraint_id")]
    pub(crate) next_constraint_id: ConstraintId,
    /// Constraint id -> parameter expression that drives its value (distance, angle, radius, diameter).
    #[serde(default)]
    pub(crate) constraint_exprs: BTreeMap<ConstraintId, String>,
    #[serde(default)]
    pub(crate) params: ParamSet,
    #[serde(default)]
    pub(crate) features: Vec<Feature>,
    #[serde(default = "first_feature_id")]
    pub(crate) next_feature_id: FeatureId,
    #[serde(default = "default_sketches")]
    pub(crate) sketches: BTreeMap<SketchId, Sketch>,
    #[serde(default = "first_sketch_id")]
    pub(crate) next_sketch_id: SketchId,
    #[serde(default = "first_group_id")]
    pub(crate) next_group_id: u32,
    /// Linear dimension entity -> the two points it measures; the dimension follows them.
    #[serde(default)]
    pub(crate) dim_links: BTreeMap<EntityId, (PointRef, PointRef)>,
    #[serde(default)]
    pub(crate) components: BTreeMap<ComponentId, Component>,
    #[serde(default = "first_component_id")]
    pub(crate) next_component_id: ComponentId,
    #[serde(default)]
    pub(crate) joints: BTreeMap<JointId, Joint>,
    #[serde(default = "first_joint_id")]
    pub(crate) next_joint_id: JointId,
    #[serde(default)]
    pub(crate) explode: Vec<ExplodeStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) studies: Vec<crate::study::Study>,
    #[serde(skip)]
    pub(crate) revision: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Add(Entity),
    Remove(EntityId),
    SetShape { id: EntityId, shape: Shape },
    AddLayer(Layer),
    RemoveLayer(LayerId),
    SetLayer(Layer),
    SetEntityLayer { id: EntityId, layer: LayerId },
    SetConstruction { id: EntityId, on: bool },
    SetSettings(Settings),
    AddConstraint(ConstraintId, Constraint),
    RemoveConstraint(ConstraintId),
    SetConstraint(ConstraintId, Constraint),
    /// `Some` makes the expression drive the constraint value, `None` removes it. The constraint must have a value.
    SetConstraintExpr { id: ConstraintId, expr: Option<String> },
    /// The inverse of removing a constraint that had an expression.
    AddConstraintWithExpr(ConstraintId, Constraint, String),
    AddFeature { index: usize, feature: Feature },
    RemoveFeature(FeatureId),
    /// Replaces the feature with the same id.
    SetFeature(Feature),
    MoveFeature { id: FeatureId, index: usize },
    /// `Some` sets or replaces the parameter, `None` removes it.
    SetParam { name: String, expr: Option<String> },
    AddSketch(Sketch),
    RemoveSketch(SketchId),
    /// Replaces the sketch with the same id.
    SetSketch(Sketch),
    /// `None` removes the entities from their groups.
    SetGroup { ids: Vec<EntityId>, group: Option<u32> },
    /// The inverse of `SetGroup`: sets each entity's own previous group.
    RestoreGroups(Vec<(EntityId, Option<u32>)>),
    /// `None` removes the key.
    SetMeta { id: EntityId, key: String, value: Option<String> },
    /// `None` removes the link. The entity must be a linear dimension.
    SetDimLink { id: EntityId, link: Option<(PointRef, PointRef)> },
    AddComponent(Component),
    RemoveComponent(ComponentId),
    /// Replaces the component with the same id.
    SetComponent(Component),
    AddJoint(Joint),
    RemoveJoint(JointId),
    /// Replaces the joint with the same id.
    SetJoint(Joint),
    /// Replaces the whole list of exploded-view steps.
    SetExplode(Vec<ExplodeStep>),
    /// Replaces the whole list of static studies.
    SetStudies(Vec<crate::study::Study>),
}

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error(transparent)]
    Geometry(#[from] GeomError),
    #[error("entity {0} not found")]
    NotFound(EntityId),
    #[error("entity {0} already exists")]
    DuplicateId(EntityId),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid project file: {0}")]
    Format(#[from] serde_json::Error),
    #[error("project file version {0} is newer than this application supports")]
    UnsupportedVersion(u32),
    #[error("layer {0} not found")]
    LayerNotFound(LayerId),
    #[error("layer {0} already exists")]
    LayerExists(LayerId),
    #[error("layer {0} is in use")]
    LayerInUse(LayerId),
    #[error("layer 0 cannot be removed")]
    ProtectedLayer,
    #[error("layer name is empty")]
    EmptyLayerName,
    #[error("line weight must be between 0.05 and 2 mm")]
    BadLineWeight,
    #[error("precision must be 0 to 8")]
    BadPrecision,
    #[error("constraint {0} not found")]
    ConstraintNotFound(ConstraintId),
    #[error("constraint {0} already exists")]
    ConstraintExists(ConstraintId),
    #[error("entity {0} is used by a constraint")]
    EntityConstrained(EntityId),
    #[error(transparent)]
    Solve(#[from] SolveError),
    #[error("constraint {id}: {message}")]
    ConstraintExpr { id: ConstraintId, message: String },
    #[error("feature {0} not found")]
    FeatureNotFound(FeatureId),
    #[error("feature {0} already exists")]
    FeatureExists(FeatureId),
    #[error("feature {0} is used by another feature")]
    FeatureInUse(FeatureId),
    #[error("entity {0} is used by a feature")]
    EntityInFeature(EntityId),
    #[error("invalid feature: {0}")]
    BadFeature(&'static str),
    #[error(transparent)]
    Param(#[from] ParamError),
    #[error("sketch {0} not found")]
    SketchNotFound(SketchId),
    #[error("sketch {0} already exists")]
    SketchExists(SketchId),
    #[error("sketch {0} is not empty")]
    SketchInUse(SketchId),
    #[error("sketch 0 cannot be removed")]
    ProtectedSketch,
    #[error("invalid sketch plane")]
    BadPlane,
    #[error("entities are in different sketches")]
    MixedSketches,
    #[error("primitive solids can only be in sketch 0")]
    SolidOutsideBaseSketch,
    #[error("sketch {0} is not a level")]
    NotALevel(SketchId),
    #[error("entity {0} is not a wall on this level")]
    NotAHost(EntityId),
    #[error("wall {0} has doors or windows")]
    HostInUse(EntityId),
    #[error("component {0} not found")]
    ComponentNotFound(ComponentId),
    #[error("component {0} already exists")]
    ComponentExists(ComponentId),
    #[error("invalid component: {0}")]
    BadComponent(&'static str),
    #[error("body {0} is placed by a component")]
    BodyInUse(FeatureId),
    #[error("joint {0} not found")]
    JointNotFound(JointId),
    #[error("joint {0} already exists")]
    JointExists(JointId),
    #[error("invalid joint: {0}")]
    BadJoint(&'static str),
    #[error("joint would give component {0} a second parent or close a loop")]
    JointLoop(ComponentId),
    #[error("component {0} is used by a joint")]
    ComponentInUse(ComponentId),
}

impl Document {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: FORMAT_VERSION,
            name: name.into(),
            entities: BTreeMap::new(),
            next_id: 1,
            layers: default_layers(),
            next_layer_id: first_layer_id(),
            settings: Settings::default(),
            constraints: BTreeMap::new(),
            next_constraint_id: first_constraint_id(),
            constraint_exprs: BTreeMap::new(),
            params: ParamSet::default(),
            features: Vec::new(),
            next_feature_id: first_feature_id(),
            sketches: default_sketches(),
            next_sketch_id: first_sketch_id(),
            next_group_id: first_group_id(),
            dim_links: BTreeMap::new(),
            components: BTreeMap::new(),
            next_component_id: first_component_id(),
            joints: BTreeMap::new(),
            next_joint_id: first_joint_id(),
            explode: Vec::new(),
            studies: Vec::new(),
            revision: 0,
        }
    }

    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        self.layers.values()
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(&id)
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn alloc_layer_id(&mut self) -> LayerId {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        id
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn alloc_id(&mut self) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Validates first, so a failed apply changes nothing. Returns the inverse command.
    pub fn apply(&mut self, cmd: Command) -> Result<Command, DocError> {
        let tol = Tolerance::default();
        let inverse = match cmd {
            Command::Add(entity) => {
                entity.shape.validate(&tol)?;
                if !self.layers.contains_key(&entity.layer) {
                    return Err(DocError::LayerNotFound(entity.layer));
                }
                if !self.sketches.contains_key(&entity.sketch) {
                    return Err(DocError::SketchNotFound(entity.sketch));
                }
                if entity.shape.needs_level() && !self.is_level(entity.sketch) {
                    return Err(DocError::NotALevel(entity.sketch));
                }
                if let Shape::Opening(o) = &entity.shape {
                    self.check_host(entity.sketch, o)?;
                }
                if entity.sketch != 0 && entity.shape.is_solid() {
                    return Err(DocError::SolidOutsideBaseSketch);
                }
                if self.entities.contains_key(&entity.id) {
                    return Err(DocError::DuplicateId(entity.id));
                }
                let id = entity.id;
                self.next_id = self.next_id.max(id.saturating_add(1));
                self.entities.insert(id, entity);
                Command::Remove(id)
            }
            Command::Remove(id) => {
                if self.openings_of(id).next().is_some() {
                    return Err(DocError::HostInUse(id));
                }
                if self.constraints.values().any(|c| c.entities().contains(&id)) {
                    return Err(DocError::EntityConstrained(id));
                }
                if self.features.iter().any(|f| f.entities().contains(&id)) {
                    return Err(DocError::EntityInFeature(id));
                }
                if self.dim_links.contains_key(&id) || self.dim_links.values().any(|(p, q)| p.entity == id || q.entity == id) {
                    return Err(DocError::EntityConstrained(id));
                }
                let removed = self.entities.remove(&id).ok_or(DocError::NotFound(id))?;
                Command::Add(removed)
            }
            Command::SetShape { id, shape } => {
                shape.validate(&tol)?;
                let sketch = self.entities.get(&id).ok_or(DocError::NotFound(id))?.sketch;
                if shape.needs_level() && !self.is_level(sketch) {
                    return Err(DocError::NotALevel(sketch));
                }
                if let Shape::Opening(o) = &shape {
                    self.check_host(sketch, o)?;
                }
                if !matches!(shape, Shape::Wall(_)) && self.openings_of(id).next().is_some() {
                    return Err(DocError::HostInUse(id));
                }
                let e = self.entities.get_mut(&id).ok_or(DocError::NotFound(id))?;
                let old = std::mem::replace(&mut e.shape, shape);
                Command::SetShape { id, shape: old }
            }
            Command::AddLayer(layer) => {
                if self.layers.contains_key(&layer.id) {
                    return Err(DocError::LayerExists(layer.id));
                }
                if layer.name.trim().is_empty() {
                    return Err(DocError::EmptyLayerName);
                }
                layer.check_weight()?;
                let id = layer.id;
                self.next_layer_id = self.next_layer_id.max(id.saturating_add(1));
                self.layers.insert(id, layer);
                Command::RemoveLayer(id)
            }
            Command::RemoveLayer(id) => {
                if id == 0 {
                    return Err(DocError::ProtectedLayer);
                }
                if !self.layers.contains_key(&id) {
                    return Err(DocError::LayerNotFound(id));
                }
                if self.entities.values().any(|e| e.layer == id) {
                    return Err(DocError::LayerInUse(id));
                }
                Command::AddLayer(self.layers.remove(&id).expect("checked above"))
            }
            Command::SetLayer(layer) => {
                if layer.name.trim().is_empty() {
                    return Err(DocError::EmptyLayerName);
                }
                layer.check_weight()?;
                let slot = self.layers.get_mut(&layer.id).ok_or(DocError::LayerNotFound(layer.id))?;
                Command::SetLayer(std::mem::replace(slot, layer))
            }
            Command::SetEntityLayer { id, layer } => {
                if !self.layers.contains_key(&layer) {
                    return Err(DocError::LayerNotFound(layer));
                }
                let e = self.entities.get_mut(&id).ok_or(DocError::NotFound(id))?;
                let old = std::mem::replace(&mut e.layer, layer);
                Command::SetEntityLayer { id, layer: old }
            }
            Command::SetConstruction { id, on } => {
                let e = self.entities.get_mut(&id).ok_or(DocError::NotFound(id))?;
                let old = std::mem::replace(&mut e.construction, on);
                Command::SetConstruction { id, on: old }
            }
            Command::SetSettings(s) => {
                if s.precision > 8 {
                    return Err(DocError::BadPrecision);
                }
                Command::SetSettings(std::mem::replace(&mut self.settings, s))
            }
            Command::AddConstraint(..)
            | Command::AddConstraintWithExpr(..)
            | Command::RemoveConstraint(_)
            | Command::SetConstraint(..)
            | Command::SetConstraintExpr { .. } => {
                self.apply_constraint(cmd)?
            }
            Command::AddFeature { .. }
            | Command::RemoveFeature(_)
            | Command::SetFeature(_)
            | Command::MoveFeature { .. }
            | Command::SetParam { .. } => self.apply_feature(cmd)?,
            Command::AddSketch(_) | Command::RemoveSketch(_) | Command::SetSketch(_) => self.apply_sketch(cmd)?,
            Command::SetGroup { .. } | Command::RestoreGroups(_) | Command::SetMeta { .. } => self.apply_group(cmd)?,
            Command::SetDimLink { .. } => self.apply_dim_link(cmd)?,
            Command::AddComponent(_) | Command::RemoveComponent(_) | Command::SetComponent(_) => self.apply_component(cmd)?,
            Command::AddJoint(_) | Command::RemoveJoint(_) | Command::SetJoint(_) | Command::SetExplode(_) => self.apply_joint(cmd)?,
            Command::SetStudies(s) => self.apply_studies(s)?,
        };
        self.revision += 1;
        Ok(inverse)
    }
}

