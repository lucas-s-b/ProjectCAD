use crate::app::CadApp;
use crate::app::Tool;
use cad_document::{Command, Entity, EntityId, LayerId, SketchId};
use cad_geometry::xform::rotation_about;
use cad_geometry::{DAffine2, DVec2, DVec3, GeomError, Opening, Room, Shape, Stair, Wall, distance_2d, point_segment_distance};
use std::collections::BTreeMap;

impl CadApp {
    /// Adds a wall from `a` to `b` on the active sketch with the Wall tool settings; its height is the level height.
    pub fn add_wall(&mut self, a: DVec2, b: DVec2) {
        match self.doc.level_height(self.sketch) {
            Ok(height) => self.add_shape(Shape::Wall(Wall { a, b, thickness: self.wall_thickness, height, align: self.wall_align })),
            Err(e) => {
                self.points.clear();
                self.status = format!("Walls go on a level ({e}): add one in the Model tab");
            }
        }
    }

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
        if self.component_sel.is_some_and(|id| self.doc.component(id).is_none()) {
            self.component_sel = None;
            self.component_edit = None;
            self.sel_rev += 1;
        }
        if self.joint_sel.is_some_and(|id| self.doc.joint(id).is_none()) {
            self.joint_sel = None;
            self.joint_edit = None;
        }
        if self.component_face.is_some_and(|(c, _)| self.doc.component(c).is_none()) {
            self.component_face = None;
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

    /// Adds a copy of the selection placed by `m` as one undo step; the copies become the selection when `select`.
    pub fn copy_transformed(&mut self, m: &DAffine2, select: bool) {
        let src = self.copies();
        let cmds = match self.copy_commands(&src, |s| s.transformed(m)) {
            Ok(c) => c,
            Err(e) => {
                self.status = e.to_string();
                return;
            }
        };
        let ids: Vec<EntityId> = cmds.iter().filter_map(|c| if let Command::Add(e) = c { Some(e.id) } else { None }).collect();
        self.exec(cmds);
        if select {
            let made: Vec<EntityId> = ids.into_iter().filter(|id| self.doc.get(*id).is_some()).collect();
            self.set_selection(made);
        }
    }

    pub fn duplicate_selection(&mut self) {
        let src = self.copies();
        let cmds = match self.copy_commands(&src, |s| Ok(s.clone())) {
            Ok(c) => c,
            Err(e) => {
                self.status = e.to_string();
                return;
            }
        };
        let ids: Vec<EntityId> = cmds.iter().filter_map(|c| if let Command::Add(e) = c { Some(e.id) } else { None }).collect();
        self.exec(cmds);
        let made: Vec<EntityId> = ids.into_iter().filter(|id| self.doc.get(*id).is_some()).collect();
        self.set_selection(made);
    }

    /// Puts the active tool's door or window on the wall of the active sketch that contains `p`, centred where `p`
    /// projects on its baseline (the wall with the nearest baseline when several contain it).
    pub fn add_opening(&mut self, p: DVec2) {
        let host = self
            .doc
            .entities_in(self.sketch)
            .filter(|x| self.pickable(x) && distance_2d(&x.shape, p) <= 1e-9)
            .filter_map(|x| match &x.shape {
                Shape::Wall(w) => Some((x.id, *w)),
                _ => None,
            })
            .min_by(|x, y| point_segment_distance(p, x.1.a, x.1.b).total_cmp(&point_segment_distance(p, y.1.a, y.1.b)));
        let Some((host, wall)) = host else {
            self.status = "Click inside a wall to place a door or window".into();
            return;
        };
        let template = if self.tool == Tool::Window { self.window } else { self.door };
        let at = (p - wall.a).dot((wall.b - wall.a).normalize());
        self.add_shape(Shape::Opening(Opening { host, wall, at, ..template }));
    }
}

impl CadApp {
    /// Puts a room at `p` when the walls of the active level enclose it, named "Room" and numbered one past the level's
    /// rooms (the user renames it).
    pub fn add_room(&mut self, p: DVec2) {
        let walls: Vec<Wall> = self
            .doc
            .entities_in(self.sketch)
            .filter(|x| !x.construction)
            .filter_map(|x| match &x.shape {
                Shape::Wall(w) => Some(*w),
                _ => None,
            })
            .collect();
        if cad_arch::regions(&walls, &[p])[0].is_none() {
            self.status = "Click inside an area enclosed by walls".into();
            return;
        }
        // Highest numeric room number + 1, so numbers do not repeat after a deletion.
        let n = self
            .doc
            .entities_in(self.sketch)
            .filter_map(|x| match &x.shape {
                Shape::Room(r) => r.number.parse::<u32>().ok(),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1;
        self.add_shape(Shape::Room(Room { seed: p, name: "Room".into(), number: n.to_string() }));
    }

    /// A stair on the active level from `a` towards `towards` with the Stair tool settings: as many risers as the level
    /// height needs at `stair_max_riser` each, `stair_tread` deep treads. None off a level or for a zero direction.
    pub fn stair_from(&self, a: DVec2, towards: DVec2) -> Option<Stair> {
        let height = self.doc.level_height(self.sketch).ok()?;
        let d = (towards - a).try_normalize()?;
        let risers = Stair::risers_for(height, self.stair_max_riser);
        Some(Stair { a, b: a + d * (f64::from(risers - 1) * self.stair_tread), width: self.stair_width, height, risers })
    }

    /// Adds `stair_from(a, towards)`, or says why not.
    pub fn add_stair(&mut self, a: DVec2, towards: DVec2) {
        match self.stair_from(a, towards) {
            Some(s) => self.add_shape(Shape::Stair(s)),
            None => self.status = "Stairs go on a level (add one in the Model tab); click a second point for the direction".into(),
        }
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
    fn copies(&self) -> Vec<(EntityId, Shape, LayerId)> {
        self.selection.iter().filter_map(|id| self.doc.get(*id)).map(|e| (e.id, e.shape.clone(), e.layer)).collect()
    }

    /// Add commands for one copy of `src`, each shape placed by `place`. A copied opening goes onto the copy of its wall;
    /// one whose wall is not copied is left out (it would sit on the original wall, on top of the original opening).
    fn copy_commands(
        &mut self,
        src: &[(EntityId, Shape, LayerId)],
        place: impl Fn(&Shape) -> Result<Shape, GeomError>,
    ) -> Result<Vec<Command>, GeomError> {
        let new_ids: BTreeMap<EntityId, EntityId> = src.iter().map(|(id, ..)| (*id, self.doc.alloc_id())).collect();
        let mut cmds = Vec::new();
        for (old, shape, layer) in src {
            let mut shape = place(shape)?;
            if let Shape::Opening(o) = &mut shape {
                let Some(host) = new_ids.get(&o.host) else { continue };
                o.host = *host;
            }
            let id = new_ids[old];
            let name = format!("{} {}", shape.kind(), id);
            cmds.push(Command::Add(Entity { id, name, shape, layer: *layer, sketch: self.sketch, construction: false, group: None, meta: Default::default() }));
        }
        Ok(cmds)
    }

    /// Copies of the selection at (i * dx, j * dy), as one undo step. The selection stays as it is.
    pub fn array_rect(&mut self, nx: u32, ny: u32, dx: f64, dy: f64) {
        if nx.saturating_mul(ny) > MAX_COPIES {
            self.status = "array too large".into();
            return;
        }
        let src = self.copies();
        let mut cmds = Vec::new();
        for off in array_rect_offsets(nx, ny, dx, dy) {
            let placed = self.copy_commands(&src, |s| {
                let mut c = s.clone();
                c.translate(off.extend(0.0));
                Ok(c)
            });
            match placed {
                Ok(c) => cmds.extend(c),
                Err(e) => {
                    self.status = e.to_string();
                    return;
                }
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
        let src = self.copies();
        let mut cmds = Vec::new();
        for a in array_polar_angles(count, total_deg) {
            match self.copy_commands(&src, |s| s.transformed(&rotation_about(center, a.to_radians()))) {
                Ok(c) => cmds.extend(c),
                Err(e) => {
                    self.status = e.to_string();
                    return;
                }
            }
        }
        self.exec(cmds);
    }
}

#[cfg(test)]
#[path = "app_ops_tests.rs"]
mod tests;
