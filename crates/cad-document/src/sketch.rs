use crate::document::{Command, DocError, Document, Entity};
use crate::feature::FeatureId;
use cad_geometry::DVec3;
use cad_modeler::Plane3;
use cad_params::parse;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type SketchId = u32;
pub const FORMAT_VERSION: u32 = 17;

/// Axes (x, y, normal): XY = (X, Y, +Z); XZ = (X, Z, -Y); YZ = (Y, Z, +X).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BasePlane {
    XY,
    XZ,
    YZ,
}

impl BasePlane {
    pub(crate) fn plane(self) -> Plane3 {
        let (x, y) = match self {
            BasePlane::XY => (DVec3::X, DVec3::Y),
            BasePlane::XZ => (DVec3::X, DVec3::Z),
            BasePlane::YZ => (DVec3::Y, DVec3::Z),
        };
        Plane3 { origin: DVec3::ZERO, x, y }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlaneDef {
    /// `offset` is a length expression along the base plane's normal.
    Principal { base: BasePlane, offset: String },
    /// Captured from a face of a body; it does not follow later changes (files before version 13).
    Fixed { origin: DVec3, x: DVec3, y: DVec3 },
    /// On a planar face of a body: the plane as picked (normal `x × y` pointing out of the body), the picked point
    /// `at` on it, and the id of the body. The rebuild moves the plane along its normal onto that face as it is now
    /// (`refs::face_plane`); `resolve_plane` gives the plane as picked.
    Face { origin: DVec3, x: DVec3, y: DVec3, at: DVec3, body: FeatureId },
}

/// Planes of the sketches on faces as one rebuild placed them; an error when the face was not found.
pub type SketchPlanes = BTreeMap<SketchId, Result<Plane3, String>>;

impl Document {
    /// The plane of `sketch`: from `planes` when the rebuild placed it there, else `resolve_plane`.
    pub(crate) fn plane_in(&self, sketch: SketchId, planes: &SketchPlanes) -> Result<Plane3, String> {
        planes.get(&sketch).cloned().unwrap_or_else(|| self.resolve_plane(sketch))
    }
}

/// A storey. Its sketch lies on the XY plane: the sketch name is the storey name, the plane offset its elevation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Level {
    /// Floor-to-floor height, a length expression; new walls on the level take it as their height.
    pub height: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sketch {
    pub id: SketchId,
    pub name: String,
    pub plane: PlaneDef,
    /// Present when the sketch is a level; walls live only on levels. None before version 15.
    #[serde(default)]
    pub level: Option<Level>,
}

pub(crate) fn first_sketch_id() -> SketchId {
    1
}

pub(crate) fn default_sketches() -> BTreeMap<SketchId, Sketch> {
    let plane = PlaneDef::Principal { base: BasePlane::XY, offset: "0".into() };
    BTreeMap::from([(0, Sketch { id: 0, name: "Sketch 0".into(), plane, level: None })])
}

fn check_sketch(s: &Sketch) -> Result<(), DocError> {
    if s.name.trim().is_empty() {
        return Err(DocError::BadFeature("empty name"));
    }
    match &s.plane {
        PlaneDef::Principal { offset, .. } => parse(offset).map(drop)?,
        PlaneDef::Fixed { origin, x, y } | PlaneDef::Face { origin, x, y, .. } => {
            let unit = |v: &DVec3| (v.length() - 1.0).abs() <= 1e-9;
            let on_plane = match &s.plane {
                PlaneDef::Face { at, .. } => at.is_finite() && x.cross(*y).dot(*at - *origin).abs() <= 1e-6,
                _ => true,
            };
            if !(origin.is_finite() && x.is_finite() && y.is_finite() && unit(x) && unit(y) && x.dot(*y).abs() <= 1e-9 && on_plane) {
                return Err(DocError::BadPlane);
            }
        }
    }
    if let Some(level) = &s.level {
        if !matches!(s.plane, PlaneDef::Principal { base: BasePlane::XY, .. }) {
            return Err(DocError::BadPlane);
        }
        parse(&level.height).map(drop)?;
    }
    Ok(())
}

impl Document {
    /// The sketch exists and is a level.
    pub fn is_level(&self, sketch: SketchId) -> bool {
        self.sketch(sketch).is_some_and(|s| s.level.is_some())
    }

    /// The evaluated storey height of a level.
    pub fn level_height(&self, sketch: SketchId) -> Result<f64, String> {
        let level = self.sketch(sketch).and_then(|s| s.level.as_ref()).ok_or("not a level")?;
        let h = crate::rebuild::eval(self, &level.height, false)?;
        if h > 0.0 { Ok(h) } else { Err("level height must be positive".into()) }
    }

    pub fn sketches(&self) -> impl Iterator<Item = &Sketch> {
        self.sketches.values()
    }

    pub fn sketch(&self, id: SketchId) -> Option<&Sketch> {
        self.sketches.get(&id)
    }

    pub fn alloc_sketch_id(&mut self) -> SketchId {
        let id = self.next_sketch_id;
        self.next_sketch_id += 1;
        id
    }

    pub fn entities_in(&self, sketch: SketchId) -> impl Iterator<Item = &Entity> {
        self.entities.values().filter(move |e| e.sketch == sketch)
    }

    /// The sketch plane with its offset evaluated against the parameters.
    pub fn resolve_plane(&self, sketch: SketchId) -> Result<Plane3, String> {
        match &self.sketch(sketch).ok_or("sketch not found")?.plane {
            PlaneDef::Principal { base, offset } => {
                let plane = base.plane();
                let d = crate::rebuild::eval(self, offset, false)?;
                Ok(Plane3 { origin: plane.normal() * d, ..plane })
            }
            PlaneDef::Fixed { origin, x, y } | PlaneDef::Face { origin, x, y, .. } => Ok(Plane3 { origin: *origin, x: *x, y: *y }),
        }
    }

    /// Fails with `MixedSketches` unless every entity is in one sketch. The entities must exist.
    pub(crate) fn check_one_sketch(&self, ids: &[u64]) -> Result<(), DocError> {
        let mut sketches = ids.iter().filter_map(|id| self.entities.get(id)).map(|e| e.sketch);
        match sketches.next() {
            Some(first) if sketches.any(|s| s != first) => Err(DocError::MixedSketches),
            _ => Ok(()),
        }
    }

    /// Validates first, so a failed apply changes nothing. Returns the inverse command.
    pub(crate) fn apply_sketch(&mut self, cmd: Command) -> Result<Command, DocError> {
        match cmd {
            Command::AddSketch(s) => {
                if self.sketches.contains_key(&s.id) {
                    return Err(DocError::SketchExists(s.id));
                }
                check_sketch(&s)?;
                let id = s.id;
                self.next_sketch_id = self.next_sketch_id.max(id.saturating_add(1));
                self.sketches.insert(id, s);
                Ok(Command::RemoveSketch(id))
            }
            Command::RemoveSketch(id) => {
                if id == 0 {
                    return Err(DocError::ProtectedSketch);
                }
                if !self.sketches.contains_key(&id) {
                    return Err(DocError::SketchNotFound(id));
                }
                if self.entities_in(id).next().is_some() {
                    return Err(DocError::SketchInUse(id));
                }
                Ok(Command::AddSketch(self.sketches.remove(&id).expect("checked above")))
            }
            Command::SetSketch(s) => {
                if !self.sketches.contains_key(&s.id) {
                    return Err(DocError::SketchNotFound(s.id));
                }
                check_sketch(&s)?;
                if s.level.is_none() && self.entities_in(s.id).any(|e| e.shape.needs_level()) {
                    return Err(DocError::NotALevel(s.id));
                }
                let old = self.sketches.insert(s.id, s).expect("checked above");
                Ok(Command::SetSketch(old))
            }
            _ => unreachable!("not a sketch command"),
        }
    }
}

#[cfg(test)]
#[path = "sketch_tests.rs"]
mod tests;
