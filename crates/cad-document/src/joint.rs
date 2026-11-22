//! Joints between components (placements by a walk up the joint tree) and exploded-view steps.
use crate::component::{ComponentId, mm};
use crate::document::{Command, DocError, Document};
use crate::rebuild::eval;
use cad_geometry::{DAffine3, DVec3};
use cad_modeler::Body;
use cad_params::parse;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Length expressions; a bare number is millimetres.
    pub origin: [String; 3],
    /// Direction, plain numbers (normalised; it must not be zero).
    pub axis: [String; 3],
}

/// Values closer to zero than half the last written decimal become 0, so no "-0" is written.
fn snap(v: f64) -> f64 {
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
        Frame { origin: origin.to_array().map(|v| mm(snap(v))), axis: axis.to_array().map(unit) }
    }

    /// The frame of face `face` of `body` (`cad_mech::face_frame`); None unless the face is a plane or a cylinder.
    pub fn from_face(body: &Body, face: usize) -> Option<Frame> {
        cad_mech::face_frame(body, face).map(|(o, a)| Frame::from_vectors(o, a))
    }
}

/// Places `child` relative to `parent`: child placement = parent placement x `cad_mech::mate(parent frame, motion, child frame)`.
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

    /// The joint whose child is `child`.
    pub fn parent_joint(&self, child: ComponentId) -> Option<&Joint> {
        self.joints.values().find(|j| j.child == child)
    }

    pub fn explode_steps(&self) -> &[ExplodeStep] {
        &self.explode
    }

    /// The vector of three expressions evaluated as lengths (a bare number counts as itself).
    fn vec3(&self, v: &[String; 3]) -> Result<DVec3, String> {
        Ok(DVec3::new(eval(self, &v[0], false)?, eval(self, &v[1], false)?, eval(self, &v[2], false)?))
    }

    /// The motion of `j`'s child relative to its parent (`cad_mech::mate`).
    fn mate(&self, j: &Joint) -> Result<DAffine3, String> {
        let frame = |f: &Frame| -> Result<DAffine3, String> {
            cad_mech::frame(self.vec3(&f.origin)?, self.vec3(&f.axis)?).ok_or_else(|| "zero-length axis".to_string())
        };
        let motion = match j.kind {
            JointKind::Fixed => DAffine3::IDENTITY,
            JointKind::Revolute => DAffine3::from_rotation_z(eval(self, &j.value, true)?),
            JointKind::Slider => DAffine3::from_translation(DVec3::Z * eval(self, &j.value, false)?),
        };
        Ok(cad_mech::mate(frame(&j.parent_frame)?, motion, frame(&j.child_frame)?))
    }

    /// The rigid motion of every component: its own `at`/`angles` (`placement`) when no joint has it as child, else
    /// its parent's placement times the joint's mate. A joint error fails the child and its whole subtree with
    /// "joint N: ..."; an error of a root is passed down unchanged.
    pub fn placements(&self) -> BTreeMap<ComponentId, Result<DAffine3, String>> {
        let mut out = BTreeMap::new();
        for id in self.components.keys() {
            // Fills `out` for `id` and its ancestors; the result is read from `out`.
            let _ = self.place_in(*id, &mut out, 0);
        }
        out
    }

    fn place_in(&self, id: ComponentId, out: &mut BTreeMap<ComponentId, Result<DAffine3, String>>, depth: usize) -> Result<DAffine3, String> {
        if let Some(r) = out.get(&id) {
            return r.clone();
        }
        let r = match self.parent_joint(id) {
            // Only a file that skipped `load`'s checks can hold a loop; stop instead of recursing forever.
            Some(_) if depth > self.joints.len() => Err("joints form a loop".to_string()),
            Some(j) => self.place_in(j.parent, out, depth + 1).and_then(|p| Ok(p * self.mate(j).map_err(|e| format!("joint {}: {e}", j.id))?)),
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

    /// Both components exist, the child is not grounded, has no other parent joint and is not an ancestor of the
    /// parent; every expression parses. Joints with the id of `j` are ignored (so `SetJoint` and `load` can use it).
    pub(crate) fn check_joint(&self, j: &Joint) -> Result<(), DocError> {
        for id in [j.parent, j.child] {
            if self.component(id).is_none() {
                return Err(DocError::ComponentNotFound(id));
            }
        }
        if self.component(j.child).is_some_and(|c| c.grounded) {
            return Err(DocError::BadJoint("the child is grounded"));
        }
        let others = || self.joints.values().filter(|o| o.id != j.id);
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
        let frames = [&j.parent_frame, &j.child_frame];
        frames.iter().flat_map(|f| f.origin.iter().chain(&f.axis)).chain(std::iter::once(&j.value)).try_for_each(|src| parse(src).map(drop))?;
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
mod tests;
