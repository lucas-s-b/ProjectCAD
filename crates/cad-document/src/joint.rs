//! Joints between components (placements by a walk up the joint tree) and exploded-view steps.
use crate::component::{ComponentId, mm};
use crate::feature::FeatureId;
use crate::document::{Command, DocError, Document};
use crate::rebuild::{RebuildResult, eval};
use crate::refs::{FACE_ERROR, FaceRef, find_face};
use cad_geometry::{DAffine3, DVec3};
use cad_modeler::Body;
use cad_params::parse;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

pub type JointId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JointKind {
    /// The child keeps the mated pose; `value` is not used.
    Fixed,
    /// The child turns about the parent frame's axis by `value`, an angle.
    Revolute,
    /// The child moves along the parent frame's axis by `value`, a length.
    Slider,
}

/// A frame in a component's own coordinates (those of its body). Its X direction is derived from the axis (`cad_mech::frame`).
/// A frame taken from a face follows that face (`face`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Length expressions; a bare number is millimetres.
    pub origin: [String; 3],
    /// Direction, plain numbers (normalised; it must not be zero).
    pub axis: [String; 3],
    /// The face the frame was taken from; when set, the rebuild takes origin and axis from that face as it is now
    /// (`cad_mech::face_frame` of the face `find_face` finds in the component's body) and `origin`/`axis` are only
    /// the values at pick time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<FaceRef>,
}

/// Values closer to zero than half the last written decimal become 0, so no "-0" is written.
pub(crate) fn snap(v: f64) -> f64 {
    if v.abs() < 5e-7 { 0.0 } else { v }
}

impl Frame {
    /// `origin` with three decimals and `axis` with six, as expressions: (1/3, 0, -1e-9) and (0, 0.6, -0.8) give
    /// ["0.333 mm", "0 mm", "0 mm"] and ["0", "0.6", "-0.8"].
    pub fn from_vectors(origin: DVec3, axis: DVec3) -> Frame {
        let unit = |v: f64| {
            let s = format!("{:.6}", snap(v));
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        };
        Frame { origin: origin.to_array().map(|v| mm(snap(v))), axis: axis.to_array().map(unit), face: None }
    }

    /// The frame of face `face` of `body` (`cad_mech::face_frame`), remembering the face so the frame follows it;
    /// None unless the face is a plane or a cylinder.
    pub fn from_face(body: &Body, face: usize) -> Option<Frame> {
        cad_mech::face_frame(body, face).map(|(o, a)| Frame { face: FaceRef::of(body, face), ..Frame::from_vectors(o, a) })
    }
}

/// Places `child` relative to `parent`: child placement = parent placement x `cad_mech::mate(parent frame, motion, child frame)`.
/// A value outside `min`..`max` fails the child's placement (`Document::check_limits`); limits refuse, never clamp.
/// A closing joint (`closes_loop`) is not walked: the loop solver (`Document::solve_joints`) sets the free joints' values so that its frames meet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Joint {
    pub id: JointId,
    pub kind: JointKind,
    pub parent: ComponentId,
    pub child: ComponentId,
    pub parent_frame: Frame,
    pub child_frame: Frame,
    /// Angle (revolute) or length (slider) expression; parsed but not evaluated for a fixed joint.
    pub value: String,
    /// Lowest allowed value, an expression in the joint's unit (angle or length); None: no limit. Fixed joints ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<String>,
    /// Highest allowed value, like `min`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,
    /// Closes a loop: not part of the joint tree (its child keeps its tree parent joint or its own placement); the
    /// loop solver makes its frames meet as its kind allows. Its value and limits are not used.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub closes_loop: bool,
    /// The loop solver sets this joint's value; `value` is only the start guess. Fixed and closing joints ignore it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub free: bool,
}

impl Joint {
    /// `v` (radians for a revolute joint, mm otherwise) as an expression in the joint's unit: "62.72 deg", "12.5 mm"
    /// (three decimals at most).
    pub fn value_text(&self, v: f64) -> String {
        if self.kind == JointKind::Revolute { format!("{} deg", cad_mech::num(v.to_degrees())) } else { mm(v) }
    }

    /// Set by the loop solver: marked free, revolute or slider, and not closing a loop.
    pub fn is_free(&self) -> bool {
        self.free && self.kind != JointKind::Fixed && !self.closes_loop
    }
}

/// One exploded-view step: `component` is drawn `distance` further along `direction` (display only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplodeStep {
    pub component: ComponentId,
    /// Plain numbers (normalised).
    pub direction: [String; 3],
    /// Length expression.
    pub distance: String,
}

/// Slack of the limit comparison (radians or mm), so a value typed equal to a limit passes.
const LIMIT_SLACK: f64 = 1e-9;

impl Document {
    /// In id order.
    pub fn joints(&self) -> impl Iterator<Item = &Joint> {
        self.joints.values()
    }

    pub fn joint(&self, id: JointId) -> Option<&Joint> {
        self.joints.get(&id)
    }

    pub fn alloc_joint_id(&mut self) -> JointId {
        let id = self.next_joint_id;
        self.next_joint_id += 1;
        id
    }

    /// The tree joint whose child is `child` (closing joints are not in the tree).
    pub fn parent_joint(&self, child: ComponentId) -> Option<&Joint> {
        self.joints.values().find(|j| j.child == child && !j.closes_loop)
    }

    /// Where joint `j` moves its child in `result`: the origin and unit Z axis of its parent frame under the parent's
    /// placement (a revolute joint turns the child about this line by its value, a slider moves it along the axis).
    /// Err when the parent is not placed or the frame does not resolve.
    pub fn joint_axis(&self, result: &RebuildResult, j: &Joint) -> Result<(DVec3, DVec3), String> {
        let parent = result.placements.get(&j.parent).ok_or_else(|| format!("component {} is not placed", j.parent))?;
        let f = *parent * self.frame(&j.parent_frame, j.parent, &result.bodies)?;
        Ok((f.translation, f.matrix3.z_axis.normalize()))
    }

    pub fn explode_steps(&self) -> &[ExplodeStep] {
        &self.explode
    }

    /// The vector of three expressions evaluated as lengths (a bare number counts as itself).
    pub(crate) fn vec3(&self, v: &[String; 3]) -> Result<DVec3, String> {
        Ok(DVec3::new(eval(self, &v[0], false)?, eval(self, &v[1], false)?, eval(self, &v[2], false)?))
    }

    /// Ok when `v` (radians for a revolute joint, mm for a slider) lies within `j`'s limits, with `LIMIT_SLACK`; fixed
    /// joints have none. Errors: "the minimum is above the maximum", "120 deg is above the maximum 90 deg",
    /// "-1 mm is below the minimum 0 mm", or a limit's expression error ("expected an angle").
    pub fn check_limits(&self, j: &Joint, v: f64) -> Result<(), String> {
        let angle = match j.kind {
            JointKind::Fixed => return Ok(()),
            JointKind::Revolute => true,
            JointKind::Slider => false,
        };
        let bound = |s: &Option<String>| s.as_deref().map(|s| eval(self, s, angle)).transpose();
        let (lo, hi) = (bound(&j.min)?, bound(&j.max)?);
        let show = |v: f64| j.value_text(v);
        if let (Some(lo), Some(hi)) = (lo, hi)
            && lo > hi
        {
            return Err("the minimum is above the maximum".into());
        }
        if let Some(lo) = lo
            && v < lo - LIMIT_SLACK
        {
            return Err(format!("{} is below the minimum {}", show(v), show(lo)));
        }
        if let Some(hi) = hi
            && v > hi + LIMIT_SLACK
        {
            return Err(format!("{} is above the maximum {}", show(v), show(hi)));
        }
        Ok(())
    }

    /// Frame `f` of component `comp` as a rigid motion (`cad_mech::frame`); a frame on a face is found in `bodies`.
    pub(crate) fn frame(&self, f: &Frame, comp: ComponentId, bodies: &[(FeatureId, Arc<Body>)]) -> Result<DAffine3, String> {
        let (origin, axis) = match &f.face {
            Some(r) => {
                let body = self.component(comp).and_then(|c| bodies.iter().find(|(id, _)| *id == c.body)).map(|(_, b)| b).ok_or("body not built")?;
                find_face(body, r).and_then(|i| cad_mech::face_frame(body, i)).ok_or(FACE_ERROR)?
            }
            None => (self.vec3(&f.origin)?, self.vec3(&f.axis)?),
        };
        cad_mech::frame(origin, axis).ok_or_else(|| "zero-length axis".to_string())
    }

    /// The motion of `j`'s child relative to its parent (`cad_mech::mate`). A value in `set` (radians or mm) replaces
    /// the joint's expression. With `limits`, the value must lie within the joint's limits (the loop solver walks
    /// without them and checks afterwards).
    fn mate(&self, j: &Joint, bodies: &[(FeatureId, Arc<Body>)], set: &BTreeMap<JointId, f64>, limits: bool) -> Result<DAffine3, String> {
        let value = |angle: bool| -> Result<f64, String> {
            let v = match set.get(&j.id) {
                Some(v) => *v,
                None => eval(self, &j.value, angle)?,
            };
            if limits {
                self.check_limits(j, v)?;
            }
            Ok(v)
        };
        let motion = match j.kind {
            JointKind::Fixed => DAffine3::IDENTITY,
            JointKind::Revolute => DAffine3::from_rotation_z(value(true)?),
            JointKind::Slider => DAffine3::from_translation(DVec3::Z * value(false)?),
        };
        Ok(cad_mech::mate(self.frame(&j.parent_frame, j.parent, bodies)?, motion, self.frame(&j.child_frame, j.child, bodies)?))
    }

    /// The rigid motion of every component: its own `at`/`angles` (`placement`) when no joint has it as child, else
    /// its parent's placement times the joint's mate. A joint error fails the child and its whole subtree with
    /// "joint N: ..."; an error of a root is passed down unchanged. `bodies` are the rebuilt bodies, for frames on faces.
    /// Closed loops are solved first (`solve_joints`), from the free joints' expressions.
    pub fn placements(&self, bodies: &[(FeatureId, Arc<Body>)]) -> BTreeMap<ComponentId, Result<DAffine3, String>> {
        self.placements_driven(bodies, None, &mut BTreeMap::new())
    }

    /// The joint walk alone (closing joints are not walked): every component's placement with the values in `set`
    /// replacing those joints' expressions.
    pub(crate) fn walk(&self, bodies: &[(FeatureId, Arc<Body>)], set: &BTreeMap<JointId, f64>, limits: bool) -> BTreeMap<ComponentId, Result<DAffine3, String>> {
        let mut out = BTreeMap::new();
        for id in self.components.keys() {
            // Fills `out` for `id` and its ancestors; the result is read from `out`.
            let _ = self.place_in(*id, &mut out, 0, bodies, set, limits);
        }
        out
    }

    fn place_in(&self, id: ComponentId, out: &mut BTreeMap<ComponentId, Result<DAffine3, String>>, depth: usize, bodies: &[(FeatureId, Arc<Body>)], set: &BTreeMap<JointId, f64>, limits: bool) -> Result<DAffine3, String> {
        if let Some(r) = out.get(&id) {
            return r.clone();
        }
        let r = match self.parent_joint(id) {
            // Only a file that skipped `load`'s checks can hold a loop; stop instead of recursing forever.
            Some(_) if depth > self.joints.len() => Err("joints form a loop".to_string()),
            Some(j) => self.place_in(j.parent, out, depth + 1, bodies, set, limits).and_then(|p| Ok(p * self.mate(j, bodies, set, limits).map_err(|e| format!("joint {}: {e}", j.id))?)),
            None => self.component(id).ok_or_else(|| format!("component {id} not found")).and_then(|c| self.placement(c)),
        };
        out.insert(id, r.clone());
        r
    }

    /// The offset of step `s` at full explode: `distance` along `direction` normalised.
    pub fn explode_vector(&self, s: &ExplodeStep) -> Result<DVec3, String> {
        let dir = self.vec3(&s.direction)?.try_normalize().ok_or_else(|| "zero-length direction".to_string())?;
        Ok(dir * eval(self, &s.distance, false)?)
    }

    /// Display offset of every component named by a step whose component exists and whose expressions evaluate, at
    /// explode `factor` (0 assembled, 1 fully exploded). With n steps, step i moves over factor i/n..(i+1)/n; steps of
    /// the same component add up.
    pub fn explode_offsets(&self, factor: f64) -> BTreeMap<ComponentId, DVec3> {
        let n = self.explode.len() as f64;
        let mut out = BTreeMap::new();
        for (i, s) in self.explode.iter().enumerate() {
            let Ok(v) = self.explode_vector(s) else { continue };
            if self.component(s.component).is_none() {
                continue;
            }
            let t = (factor * n - i as f64).clamp(0.0, 1.0);
            *out.entry(s.component).or_insert(DVec3::ZERO) += v * t;
        }
        out
    }

    /// Both components exist. A tree joint's child is not grounded, has no other tree parent joint and is not an
    /// ancestor of the parent; a closing joint (`closes_loop`) only needs two different components. Every expression
    /// parses. Joints with the id of `j` are ignored (so `SetJoint` and `load` can use it), and closing joints take no
    /// part in the tree checks.
    pub(crate) fn check_joint(&self, j: &Joint) -> Result<(), DocError> {
        for id in [j.parent, j.child] {
            if self.component(id).is_none() {
                return Err(DocError::ComponentNotFound(id));
            }
        }
        if j.closes_loop {
            if j.parent == j.child {
                return Err(DocError::JointLoop(j.child));
            }
        } else {
            if self.component(j.child).is_some_and(|c| c.grounded) {
                return Err(DocError::BadJoint("the child is grounded"));
            }
            let others = || self.joints.values().filter(|o| o.id != j.id && !o.closes_loop);
            if others().any(|o| o.child == j.child) {
                return Err(DocError::JointLoop(j.child));
            }
            // Walk up from the parent; meeting the child closes a loop. The step bound stops on a loop already in a file.
            let (mut at, mut steps) = (j.parent, 0);
            loop {
                if at == j.child || steps > self.joints.len() {
                    return Err(DocError::JointLoop(j.child));
                }
                let Some(up) = others().find(|o| o.child == at) else { break };
                at = up.parent;
                steps += 1;
            }
        }
        let frames = [&j.parent_frame, &j.child_frame];
        frames.iter().flat_map(|f| f.origin.iter().chain(&f.axis)).chain(std::iter::once(&j.value)).chain(j.min.iter()).chain(j.max.iter()).try_for_each(|src| parse(src).map(drop))?;
        Ok(())
    }

    /// Validates first, so a failed apply changes nothing. Returns the inverse command.
    pub(crate) fn apply_joint(&mut self, cmd: Command) -> Result<Command, DocError> {
        match cmd {
            Command::AddJoint(j) => {
                if self.joints.contains_key(&j.id) {
                    return Err(DocError::JointExists(j.id));
                }
                self.check_joint(&j)?;
                let id = j.id;
                self.next_joint_id = self.next_joint_id.max(id.saturating_add(1));
                self.joints.insert(id, j);
                Ok(Command::RemoveJoint(id))
            }
            Command::RemoveJoint(id) => {
                let j = self.joints.remove(&id).ok_or(DocError::JointNotFound(id))?;
                Ok(Command::AddJoint(j))
            }
            Command::SetJoint(j) => {
                if !self.joints.contains_key(&j.id) {
                    return Err(DocError::JointNotFound(j.id));
                }
                self.check_joint(&j)?;
                let old = self.joints.insert(j.id, j).expect("checked above");
                Ok(Command::SetJoint(old))
            }
            Command::SetExplode(steps) => {
                // Steps of a missing component are kept and skipped when drawn (see `explode_offsets`).
                steps.iter().flat_map(|s| s.direction.iter().chain(std::iter::once(&s.distance))).try_for_each(|src| parse(src).map(drop))?;
                Ok(Command::SetExplode(std::mem::replace(&mut self.explode, steps)))
            }
            _ => unreachable!("not a joint command"),
        }
    }
}

#[cfg(test)]
#[path = "joint_tests.rs"]
pub(crate) mod tests;
