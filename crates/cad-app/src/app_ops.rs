use crate::app::CadApp;
use cad_document::{Command, Entity, EntityId, LayerId, SketchId};
use cad_geometry::xform::rotation_about;
use cad_geometry::{DAffine2, DVec2, DVec3, Shape};

impl CadApp {
    /// Drops selected entities that are gone, hidden or locked; falls back to layer 0 when the current layer is gone.
    pub fn sanitize(&mut self) {
        let keep: Vec<EntityId> =
            self.selection.iter().copied().filter(|id| self.doc.get(*id).is_some_and(|e| self.pickable(e))).collect();
        if keep.len() != self.selection.len() {
            self.set_selection(keep);
        }
        let before = self.captures.clone();
        let exists = |id: &EntityId| self.doc.get(*id).is_some();
        let kept: Vec<Vec<EntityId>> = before.iter().map(|c| c.iter().copied().filter(exists).collect::<Vec<_>>()).filter(|c| !c.is_empty()).collect();
        if kept != before {
            self.captures = kept;
            self.sel_rev += 1;
        }
        if self.doc.layer(self.layer).is_none() {
            self.layer = 0;
        }
        if self.op_target.is_some_and(|(id, _)| self.doc.get(id).is_none()) {
            self.op_target = None;
        }
        if self.doc.sketch(self.sketch).is_none() {
            self.set_sketch(0);
        }
    }

    /// Makes `id` the active sketch and fits the 2D camera to its entities.
    pub fn set_sketch(&mut self, id: SketchId) {
        self.sketch = id;
        self.set_selection([]);
        self.points.clear();
        self.op_target = None;
        self.inspector = None;
        self.fit_sketch();
    }

    /// Fits the 2D camera to the entities of the active sketch.
    pub fn fit_sketch(&mut self) {
        let bounds = self
            .doc
            .entities_in(self.sketch)
            .map(|e| e.shape.bbox())
            .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
            .map_or((DVec2::splat(-50.0), DVec2::splat(50.0)), |(lo, hi)| (lo.truncate(), hi.truncate()));
        self.cam2d.fit(self.view_rect, bounds.0, bounds.1);
    }

    /// Maps a point of the sketch plane into world space; None when the plane does not resolve.
    pub fn sketch_to_world(&self, sketch: SketchId) -> Option<impl Fn(DVec2) -> DVec3 + use<>> {
        let plane = self.sketch_plane(sketch).ok()?;
        Some(move |p| plane.point(p))
    }

    /// World-space box of an entity; 2D shapes go through their sketch plane (the 4 corners of the 2D box).
    pub fn entity_bounds(&self, e: &Entity) -> Option<(DVec3, DVec3)> {
        let (lo, hi) = e.shape.bbox();
        if e.shape.is_solid() {
            return Some((lo, hi));
        }
        let to_world = self.sketch_to_world(e.sketch)?;
        let corners = [lo.truncate(), DVec2::new(hi.x, lo.y), hi.truncate(), DVec2::new(lo.x, hi.y)].map(to_world);
        corners.into_iter().map(|c| (c, c)).reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
    }

    pub fn new_entity(&mut self, shape: Shape, layer: LayerId) -> Entity {
        let id = self.doc.alloc_id();
        Entity { id, name: format!("{} {}", shape.kind(), id), shape, layer, sketch: self.sketch, construction: false, group: None, meta: Default::default() }
    }

    pub fn visible(&self, e: &Entity) -> bool {
        self.doc.layer(e.layer).is_some_and(|l| l.visible)
    }

    pub fn pickable(&self, e: &Entity) -> bool {
        self.doc.layer(e.layer).is_some_and(|l| l.visible && !l.locked)
    }

    pub fn color(&self, e: &Entity) -> [u8; 3] {
        self.doc.layer(e.layer).map_or([230, 230, 230], |l| l.color)
    }

    /// Centre of the union XY bbox of the selected entities.
    pub fn selection_center(&self) -> Option<DVec2> {
        self.selection
            .iter()
            .filter_map(|id| self.doc.get(*id))
            .map(|e| {
                let (lo, hi) = e.shape.bbox();
                (lo.truncate(), hi.truncate())
            })
            .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
            .map(|(lo, hi)| (lo + hi) / 2.0)
    }

    pub fn transform_selection(&mut self, m: DAffine2) {
        let mut cmds = Vec::new();
        for e in self.selection.iter().filter_map(|id| self.doc.get(*id)) {
            match e.shape.transformed(&m) {
                Ok(shape) => cmds.push(Command::SetShape { id: e.id, shape }),
                Err(err) => {
                    self.status = err.to_string();
                    return;
                }
            }
        }
        self.exec(cmds);
    }

    pub fn duplicate_selection(&mut self) {
        let src: Vec<(Shape, LayerId)> =
            self.selection.iter().filter_map(|id| self.doc.get(*id)).map(|e| (e.shape.clone(), e.layer)).collect();
        let mut ids = Vec::new();
        let mut cmds = Vec::new();
        for (shape, layer) in src {
            let id = self.doc.alloc_id();
            ids.push(id);
            cmds.push(Command::Add(Entity { id, name: format!("{} {}", shape.kind(), id), shape, layer, sketch: self.sketch, construction: false, group: None, meta: Default::default() }));
        }
        self.exec(cmds);
        let made: Vec<EntityId> = ids.into_iter().filter(|id| self.doc.get(*id).is_some()).collect();
        self.set_selection(made);
    }
}

const MAX_COPIES: u32 = 1000;

/// Inputs of the Array group in the properties panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrayParams {
    pub nx: u32,
    pub ny: u32,
    pub dx: f64,
    pub dy: f64,
    pub count: u32,
    pub angle_deg: f64,
    pub center: DVec2,
}

impl Default for ArrayParams {
    fn default() -> Self {
        Self { nx: 2, ny: 1, dx: 10.0, dy: 10.0, count: 4, angle_deg: 360.0, center: DVec2::ZERO }
    }
}

/// Offsets of every grid cell except the original one at (0, 0).
fn array_rect_offsets(nx: u32, ny: u32, dx: f64, dy: f64) -> Vec<DVec2> {
    (0..nx)
        .flat_map(|i| (0..ny).map(move |j| (i, j)))
        .filter(|ij| *ij != (0, 0))
        .map(|(i, j)| DVec2::new(f64::from(i) * dx, f64::from(j) * dy))
        .collect()
}

/// Rotation angles in degrees of the copies; a full circle is divided into `count` steps, a partial one into `count - 1`.
fn array_polar_angles(count: u32, total_deg: f64) -> Vec<f64> {
    if count < 2 {
        return Vec::new();
    }
    let steps = if (total_deg - 360.0).abs() < 1e-9 { count } else { count - 1 };
    let step = total_deg / f64::from(steps);
    (1..count).map(|k| step * f64::from(k)).collect()
}

impl CadApp {
    fn copies(&self) -> Vec<(Shape, LayerId)> {
        self.selection.iter().filter_map(|id| self.doc.get(*id)).map(|e| (e.shape.clone(), e.layer)).collect()
    }

    /// Copies of the selection at (i * dx, j * dy), as one undo step. The selection stays as it is.
    pub fn array_rect(&mut self, nx: u32, ny: u32, dx: f64, dy: f64) {
        if nx.saturating_mul(ny) > MAX_COPIES {
            self.status = "array too large".into();
            return;
        }
        let mut cmds = Vec::new();
        for (shape, layer) in self.copies() {
            for off in array_rect_offsets(nx, ny, dx, dy) {
                let mut copy = shape.clone();
                copy.translate(off.extend(0.0));
                cmds.push(Command::Add(self.new_entity(copy, layer)));
            }
        }
        self.exec(cmds);
    }

    /// `count - 1` copies of the selection rotated about `center`, as one undo step.
    pub fn array_polar(&mut self, count: u32, total_deg: f64, center: DVec2) {
        if count > MAX_COPIES {
            self.status = "array too large".into();
            return;
        }
        let mut cmds = Vec::new();
        for (shape, layer) in self.copies() {
            for a in array_polar_angles(count, total_deg) {
                match shape.transformed(&rotation_about(center, a.to_radians())) {
                    Ok(copy) => cmds.push(Command::Add(self.new_entity(copy, layer))),
                    Err(e) => {
                        self.status = e.to_string();
                        return;
                    }
                }
            }
        }
        self.exec(cmds);
    }
}

#[cfg(test)]
#[path = "app_ops_tests.rs"]
mod tests;
