//! Components: placed instances of bodies inside the document (an assembly without external files).
use crate::document::{Command, DocError, Document};
use crate::feature::FeatureId;
use crate::rebuild::{Bodies, eval};
use cad_geometry::{DAffine3, DQuat, DVec3};
use cad_modeler::Body;
use cad_params::parse;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

pub type ComponentId = u64;

/// `v` millimetres as an expression with at most three decimals: 54 gives "54 mm".
pub fn mm(v: f64) -> String {
    format!("{} mm", cad_mech::num(v))
}

/// One placed instance of a body. Its solid is the body turned about the world X, then Y, then Z axis through the
/// origin by `angles`, then moved by `at`: p' = Rz Ry Rx p + at. Several components may place the same body; a body
/// with components is drawn only through them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Component {
    pub id: ComponentId,
    pub name: String,
    /// The body placed: the id of the feature that made it.
    pub body: FeatureId,
    /// Translation (x, y, z), length expressions; a bare number is millimetres.
    pub at: [String; 3],
    /// Turns about X, Y and Z, angle expressions; a bare number is degrees.
    pub angles: [String; 3],
    /// Free key/value notes; the bill of materials reads "material" and "part_number".
    #[serde(default)]
    pub meta: BTreeMap<String, String>,
}

impl Document {
    /// In id order.
    pub fn components(&self) -> impl Iterator<Item = &Component> {
        self.components.values()
    }

    pub fn component(&self, id: ComponentId) -> Option<&Component> {
        self.components.get(&id)
    }

    pub fn alloc_component_id(&mut self) -> ComponentId {
        let id = self.next_component_id;
        self.next_component_id += 1;
        id
    }

    /// The rigid motion of `c` with its expressions evaluated.
    pub fn placement(&self, c: &Component) -> Result<DAffine3, String> {
        let mut v = [0.0; 6];
        for (i, src) in c.at.iter().chain(&c.angles).enumerate() {
            v[i] = eval(self, src, i >= 3)?;
        }
        // Spelled out: glam's EulerRot names changed meaning between versions.
        let turn = DQuat::from_rotation_z(v[5]) * DQuat::from_rotation_y(v[4]) * DQuat::from_rotation_x(v[3]);
        Ok(DAffine3::from_rotation_translation(turn, DVec3::new(v[0], v[1], v[2])))
    }

    /// `c` moved by `delta`; its translation becomes plain millimetre values (expressions there are replaced).
    pub fn component_moved(&self, c: &Component, delta: DVec3) -> Result<Component, String> {
        let at = self.placement(c)?.translation + delta;
        Ok(Component { at: [mm(at.x), mm(at.y), mm(at.z)], ..c.clone() })
    }

    /// The solid of `c`: its body in `bodies` moved by `placement`.
    pub(crate) fn place_component(&self, c: &Component, bodies: &Bodies) -> Result<Arc<Body>, String> {
        let body = bodies.iter().find(|(id, _)| *id == c.body).map(|(_, b)| b).ok_or_else(|| format!("body {} not found", c.body))?;
        body.transformed(&self.placement(c)?).map(Arc::new).map_err(|e| e.to_string())
    }

    fn check_component(&self, c: &Component) -> Result<(), DocError> {
        if c.name.trim().is_empty() {
            return Err(DocError::BadComponent("empty name"));
        }
        if self.feature(c.body).is_none() {
            return Err(DocError::FeatureNotFound(c.body));
        }
        c.at.iter().chain(&c.angles).try_for_each(|src| parse(src).map(drop))?;
        Ok(())
    }

    /// Validates first, so a failed apply changes nothing. Returns the inverse command.
    pub(crate) fn apply_component(&mut self, cmd: Command) -> Result<Command, DocError> {
        match cmd {
            Command::AddComponent(c) => {
                if self.components.contains_key(&c.id) {
                    return Err(DocError::ComponentExists(c.id));
                }
                self.check_component(&c)?;
                let id = c.id;
                self.next_component_id = self.next_component_id.max(id.saturating_add(1));
                self.components.insert(id, c);
                Ok(Command::RemoveComponent(id))
            }
            Command::RemoveComponent(id) => {
                let c = self.components.remove(&id).ok_or(DocError::ComponentNotFound(id))?;
                Ok(Command::AddComponent(c))
            }
            Command::SetComponent(c) => {
                if !self.components.contains_key(&c.id) {
                    return Err(DocError::ComponentNotFound(c.id));
                }
                self.check_component(&c)?;
                let old = self.components.insert(c.id, c).expect("checked above");
                Ok(Command::SetComponent(old))
            }
            _ => unreachable!("not a component command"),
        }
    }
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
