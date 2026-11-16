use crate::document::{Command, ConstraintId, DocError, Document, EntityId};
use cad_constraints::{Constraint, Solution, SolveError, solve};
use std::collections::BTreeMap;

impl Document {
    pub fn constraints(&self) -> impl Iterator<Item = (ConstraintId, &Constraint)> {
        self.constraints.iter().map(|(id, c)| (*id, c))
    }

    pub fn constraint(&self, id: ConstraintId) -> Option<&Constraint> {
        self.constraints.get(&id)
    }

    pub fn alloc_constraint_id(&mut self) -> ConstraintId {
        let id = self.next_constraint_id;
        self.next_constraint_id += 1;
        id
    }

    pub fn constraints_of(&self, entity: EntityId) -> Vec<ConstraintId> {
        let uses = |c: &Constraint| c.entities().contains(&entity);
        self.constraints.iter().filter(|(_, c)| uses(c)).map(|(id, _)| *id).collect()
    }

    fn check_entities(&self, c: &Constraint) -> Result<(), DocError> {
        let ids = c.entities();
        match ids.iter().find(|e| !self.entities.contains_key(e)) {
            Some(missing) => Err(DocError::NotFound(*missing)),
            None => self.check_one_sketch(&ids),
        }
    }

    /// Validates first, so a failed apply changes nothing. Returns the inverse command.
    pub(crate) fn apply_constraint(&mut self, cmd: Command) -> Result<Command, DocError> {
        match cmd {
            Command::AddConstraint(id, c) => {
                if self.constraints.contains_key(&id) {
                    return Err(DocError::ConstraintExists(id));
                }
                self.check_entities(&c)?;
                self.next_constraint_id = self.next_constraint_id.max(id.saturating_add(1));
                self.constraints.insert(id, c);
                Ok(Command::RemoveConstraint(id))
            }
            Command::RemoveConstraint(id) => {
                let old = self.constraints.remove(&id).ok_or(DocError::ConstraintNotFound(id))?;
                Ok(Command::AddConstraint(id, old))
            }
            Command::SetConstraint(id, c) => {
                if !self.constraints.contains_key(&id) {
                    return Err(DocError::ConstraintNotFound(id));
                }
                self.check_entities(&c)?;
                let old = self.constraints.insert(id, c).expect("checked above");
                Ok(Command::SetConstraint(id, old))
            }
            _ => unreachable!("not a constraint command"),
        }
    }

    fn solve_all(&self) -> Result<Solution, SolveError> {
        let shapes = self.entities.iter().map(|(id, e)| (*id, e.shape.clone())).collect::<BTreeMap<_, _>>();
        let list: Vec<Constraint> = self.constraints.values().cloned().collect();
        solve(&shapes, &list)
    }

    /// SetShape commands that make the geometry satisfy the constraints. Empty when nothing has to move.
    pub fn solve_commands(&self) -> Result<Vec<Command>, DocError> {
        let solution = self.solve_all()?;
        Ok(solution.shapes.into_iter().map(|(id, shape)| Command::SetShape { id, shape }).collect())
    }

    /// Degrees of freedom and redundancy of the constrained geometry. None without constraints.
    pub fn constraint_status(&self) -> Option<Result<Solution, SolveError>> {
        if self.constraints.is_empty() { None } else { Some(self.solve_all()) }
    }
}
