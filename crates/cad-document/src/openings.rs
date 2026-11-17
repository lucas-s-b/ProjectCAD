//! Doors and windows on their host walls: the host check, and the refresh that keeps each opening's copy of its wall
//! current.
use crate::document::{Command, DocError, Document, EntityId};
use crate::sketch::SketchId;
use cad_geometry::{Opening, Shape};

impl Document {
    /// The opening's host exists, is a wall, and is in `sketch`.
    pub(crate) fn check_host(&self, sketch: SketchId, o: &Opening) -> Result<(), DocError> {
        match self.entities.get(&o.host) {
            Some(h) if h.sketch == sketch && matches!(h.shape, Shape::Wall(_)) => Ok(()),
            _ => Err(DocError::NotAHost(o.host)),
        }
    }

    /// Ids of the doors and windows hosted by `wall`.
    pub fn openings_of(&self, wall: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.entities.values().filter(move |x| matches!(&x.shape, Shape::Opening(o) if o.host == wall)).map(|x| x.id)
    }

    /// SetShape commands that copy each host wall into its openings. Empty when all are current.
    pub fn opening_refresh_commands(&self) -> Vec<Command> {
        self.entities
            .values()
            .filter_map(|x| {
                let Shape::Opening(o) = &x.shape else { return None };
                let Some(Shape::Wall(w)) = self.entities.get(&o.host).map(|h| &h.shape) else { return None };
                (o.wall != *w).then_some(Command::SetShape { id: x.id, shape: Shape::Opening(Opening { wall: *w, ..*o }) })
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "openings_tests.rs"]
mod tests;
