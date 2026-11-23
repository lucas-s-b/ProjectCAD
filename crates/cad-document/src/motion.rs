//! Motion studies: one joint driven over a range in steps (the exact joint walk per sample) and the first collision.
use crate::bom::csv_field;
use crate::component::ComponentId;
use crate::document::Document;
use crate::feature::BODY_TOLERANCE;
use crate::joint::{JointId, JointKind, snap};
use crate::rebuild::{RebuildResult, eval};
use cad_geometry::{DAffine3, DVec3};
use cad_mech::Overlap;
use cad_modeler::Body;
use std::collections::BTreeMap;

/// Most steps of one study (the collision check runs once per sample).
pub const MAX_STEPS: usize = 360;

/// What a motion study drives and records. App state, not stored in the document.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionStudy {
    /// A revolute or slider joint.
    pub joint: JointId,
    /// Expressions in the joint's unit (angle or length; a bare number is degrees or millimetres).
    pub start: String,
    pub end: String,
    /// Equal steps from start to end, 1..=MAX_STEPS: steps + 1 samples.
    pub steps: usize,
    /// The component whose point is traced, and the point in its body's coordinates (length expressions).
    pub trace: ComponentId,
    pub point: [String; 3],
}

impl Default for MotionStudy {
    /// No joint or component yet (the app picks them), 0 to 90 in 36 steps, tracing the component origin.
    fn default() -> Self {
        let zero = || "0".to_string();
        MotionStudy { joint: 0, start: zero(), end: "90".into(), steps: 36, trace: 0, point: [zero(), zero(), zero()] }
    }
}

/// The samples of a motion study. Positions are exact (rigid motions from the joint walk).
#[derive(Debug, Clone, PartialEq)]
pub struct Motion {
    pub joint: JointId,
    /// True for a revolute joint: `values` are radians; else millimetres.
    pub angle: bool,
    /// The joint value of every sample, from start to end.
    pub values: Vec<f64>,
    /// The joint's child and every component placed through it, in id order: they move with the joint.
    pub moving: Vec<ComponentId>,
    /// Per sample, the motion of each moving component from its solid in the rebuild: a point q of that solid is at
    /// `moves[k][&c].transform_point3(q)` at sample k.
    pub moves: Vec<BTreeMap<ComponentId, DAffine3>>,
    /// The traced point in world coordinates per sample, mm.
    pub trace: Vec<DVec3>,
}

/// The first sample at which a moving component shares a volume with a component that does not move.
#[derive(Debug, Clone, PartialEq)]
pub struct Collision {
    pub sample: usize,
    pub moving: ComponentId,
    pub other: ComponentId,
    /// mm³
    pub volume: f64,
    /// The common solid at that sample, world coordinates.
    pub region: Body,
}

impl Document {
    /// `root` and every component placed through it by joints, in id order.
    fn subtree(&self, root: ComponentId) -> Vec<ComponentId> {
        let reaches = |c: ComponentId| {
            let (mut at, mut steps) = (c, 0);
            while at != root {
                match self.parent_joint(at) {
                    // The step bound stops on a loop in a file that skipped `load`'s checks.
                    Some(j) if steps <= self.joints().count() => {
                        at = j.parent;
                        steps += 1;
                    }
                    _ => return false,
                }
            }
            true
        };
        self.components().map(|c| c.id).filter(|&c| reaches(c)).collect()
    }

    /// Drives `study.joint` from `study.start` to `study.end` in `study.steps` equal steps; each sample runs the joint
    /// walk with the driven joint's value replaced (`placements_driven`). The last sample is exactly the end value.
    /// `result` must be the rebuild of this document revision. Errors: "joint 9 not found", "a fixed joint cannot be
    /// driven", "steps must be 1 to 360", "start: …" / "end: …" (expression error or outside the limits, e.g.
    /// "end: 120 deg is above the maximum 90 deg"), "trace point: …", a moving component without a rebuilt placement
    /// (its placement error), "step k: …" (a moving or traced component that cannot be placed at sample k).
    pub fn drive(&self, result: &RebuildResult, study: &MotionStudy) -> Result<Motion, String> {
        let j = self.joint(study.joint).ok_or_else(|| format!("joint {} not found", study.joint))?;
        let angle = match j.kind {
            JointKind::Fixed => return Err("a fixed joint cannot be driven".into()),
            JointKind::Revolute => true,
            JointKind::Slider => false,
        };
        if !(1..=MAX_STEPS).contains(&study.steps) {
            return Err(format!("steps must be 1 to {MAX_STEPS}"));
        }
        let bound = |src: &str, what: &str| {
            eval(self, src, angle).and_then(|v| self.check_limits(j, v).map(|()| v)).map_err(|err| format!("{what}: {err}"))
        };
        let (start, end) = (bound(&study.start, "start")?, bound(&study.end, "end")?);
        let point = self.vec3(&study.point).map_err(|err| format!("trace point: {err}"))?;
        let base = self.placements(&result.bodies);
        let moving = self.subtree(j.child);
        let mut undo = BTreeMap::new();
        for c in &moving {
            let p = base.get(c).cloned().unwrap_or_else(|| Err(format!("component {c} not found")))?;
            undo.insert(*c, p.inverse());
        }
        let n = study.steps;
        let mut out = Motion { joint: j.id, angle, values: Vec::new(), moving: moving.clone(), moves: Vec::new(), trace: Vec::new() };
        for k in 0..=n {
            let v = if k == n { end } else { start + (end - start) * k as f64 / n as f64 };
            let placed = self.placements_driven(&result.bodies, Some((j.id, v)));
            let at = |c: ComponentId| {
                placed.get(&c).cloned().unwrap_or_else(|| Err(format!("component {c} not found"))).map_err(|err| format!("step {k}: {err}"))
            };
            let mut moves = BTreeMap::new();
            for c in &moving {
                moves.insert(*c, at(*c)? * undo[c]);
            }
            out.trace.push(at(study.trace)?.transform_point3(point));
            out.values.push(v);
            out.moves.push(moves);
        }
        Ok(out)
    }

    /// The first sample of `motion` at which a moving component shares a volume with a component that does not move
    /// (`cad_mech::overlap` with the thickness `Document::check` uses; contact does not count), samples in order, then
    /// moving and other components in id order. Moving components keep their relative poses, so their pairs are not
    /// checked; components that failed to place are skipped. Err when a boolean fails ("step 4: …"). A collision
    /// between two samples is not seen: label results `Accuracy::Approximate(SAMPLED)`.
    pub fn first_collision(&self, result: &RebuildResult, motion: &Motion) -> Result<Option<Collision>, String> {
        let still: Vec<(ComponentId, &Body)> = result
            .components
            .iter()
            .filter(|(id, _)| !motion.moving.contains(id))
            .filter_map(|(id, r)| r.as_ref().ok().map(|b| (*id, &**b)))
            .collect();
        // shortcut: `overlap` rebuilds the parry meshes of both solids on every call; cache those of `still` if studies get slow.
        for (k, moves) in motion.moves.iter().enumerate() {
            for (c, m) in moves {
                let Some(body) = result.component(*c) else { continue };
                let moved = body.transformed(m).map_err(|err| format!("step {k}: {err}"))?;
                for (o, other) in &still {
                    match cad_mech::overlap(&moved, other, 2.0 * BODY_TOLERANCE) {
                        Ok(Some(Overlap::Interference { volume, region })) => {
                            return Ok(Some(Collision { sample: k, moving: *c, other: *o, volume, region }));
                        }
                        Ok(_) => {}
                        Err(err) => return Err(format!("step {k}: {err}")),
                    }
                }
            }
        }
        Ok(None)
    }

    /// CSV of `motion`: header "Step,Value (deg),X (mm),Y (mm),Z (mm),Collision" ("Value (mm)" for a slider), then one
    /// row per sample with the joint value and the traced point, six decimals, values within 5e-7 of 0 written as 0;
    /// the row of `collision` names the pair and the volume ("c2 / c1 interference 535.898 mm³"). Starts with a UTF-8
    /// byte order mark, like `bom_csv`.
    pub fn motion_csv(&self, motion: &Motion, collision: Option<&Collision>) -> String {
        let name = |id: ComponentId| self.component(id).map_or_else(|| format!("component {id}"), |c| c.name.clone());
        let unit = if motion.angle { "deg" } else { "mm" };
        let n = |x: f64| format!("{:.6}", snap(x));
        let mut out = format!("\u{feff}Step,Value ({unit}),X (mm),Y (mm),Z (mm),Collision\n");
        for (k, (v, p)) in motion.values.iter().zip(&motion.trace).enumerate() {
            let v = if motion.angle { v.to_degrees() } else { *v };
            let hit = collision
                .filter(|c| c.sample == k)
                .map_or_else(String::new, |c| csv_field(&format!("{} / {} interference {:.3} mm\u{b3}", name(c.moving), name(c.other), c.volume)));
            out.push_str(&format!("{k},{},{},{},{},{hit}\n", n(v), n(p.x), n(p.y), n(p.z)));
        }
        out
    }
}

#[cfg(test)]
#[path = "motion_tests.rs"]
mod tests;
