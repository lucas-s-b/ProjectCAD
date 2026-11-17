use crate::document::{Command, DocError, Document, EntityId};
use cad_constraints::{PointKind, PointRef};
use cad_geometry::{DVec2, Shape};

/// A linked dimension follows its points when they move further than this, mm.
const FOLLOW_EPS: f64 = 1e-12;

impl Document {
    /// Linked dimensions: dimension entity and the two points it measures.
    pub fn dim_links(&self) -> impl Iterator<Item = (EntityId, &(PointRef, PointRef))> {
        self.dim_links.iter().map(|(id, l)| (*id, l))
    }

    pub fn dim_link(&self, id: EntityId) -> Option<&(PointRef, PointRef)> {
        self.dim_links.get(&id)
    }

    /// Line Start/End, Circle/Arc Center, Arc Start/End.
    pub fn point_of(&self, p: &PointRef) -> Option<DVec2> {
        match (&self.entities.get(&p.entity)?.shape, p.kind) {
            (Shape::Line { a, .. }, PointKind::Start) => Some(*a),
            (Shape::Line { b, .. }, PointKind::End) => Some(*b),
            (Shape::Circle { center, .. } | Shape::Arc { center, .. }, PointKind::Center) => Some(*center),
            (Shape::Arc { center, radius, start, .. }, PointKind::Start) => Some(*center + DVec2::from_angle(*start) * *radius),
            (Shape::Arc { center, radius, start, sweep }, PointKind::End) => Some(*center + DVec2::from_angle(start + sweep) * *radius),
            _ => None,
        }
    }

    /// SetShape commands that move every linked dimension onto its points. Empty when all are in place.
    pub fn dim_refresh_commands(&self) -> Vec<Command> {
        let mut out = Vec::new();
        for (id, (p, q)) in &self.dim_links {
            let (Some(pa), Some(pb), Some(e)) = (self.point_of(p), self.point_of(q), self.entities.get(id)) else { continue };
            if let Shape::Dimension { a, b, offset, text_height } = &e.shape
                && (a.distance(pa) > FOLLOW_EPS || b.distance(pb) > FOLLOW_EPS)
            {
                out.push(Command::SetShape { id: *id, shape: Shape::Dimension { a: pa, b: pb, offset: *offset, text_height: *text_height } });
            }
        }
        out
    }

    fn check_link(&self, id: EntityId, link: &(PointRef, PointRef)) -> Result<(), DocError> {
        if !matches!(self.entities.get(&id).ok_or(DocError::NotFound(id))?.shape, Shape::Dimension { .. }) {
            return Err(DocError::BadFeature("not a linear dimension"));
        }
        for p in [&link.0, &link.1] {
            if !self.entities.contains_key(&p.entity) {
                return Err(DocError::NotFound(p.entity));
            }
        }
        self.check_one_sketch(&[id, link.0.entity, link.1.entity])?;
        if self.point_of(&link.0).is_none() || self.point_of(&link.1).is_none() {
            return Err(DocError::BadFeature("bad dimension point"));
        }
        Ok(())
    }

    /// Validates first, so a failed apply changes nothing. Returns the inverse command.
    pub(crate) fn apply_dim_link(&mut self, cmd: Command) -> Result<Command, DocError> {
        let Command::SetDimLink { id, link } = cmd else { unreachable!("not a dimension link command") };
        match link {
            Some(l) => {
                self.check_link(id, &l)?;
                Ok(Command::SetDimLink { id, link: self.dim_links.insert(id, l) })
            }
            None => {
                if !self.entities.contains_key(&id) {
                    return Err(DocError::NotFound(id));
                }
                Ok(Command::SetDimLink { id, link: self.dim_links.remove(&id) })
            }
        }
    }
}
