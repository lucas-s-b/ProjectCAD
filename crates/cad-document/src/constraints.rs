use crate::document::{Command, ConstraintId, DocError, Document, EntityId};
use crate::rebuild::eval;
use cad_constraints::{Constraint, Solution, SolveError, solve_pinned};
use cad_params::parse;
use std::collections::{BTreeMap, BTreeSet};

fn check_expr(c: &Constraint, src: Option<&str>) -> Result<(), DocError> {
    if c.value().is_none() {
        return Err(DocError::BadFeature("this constraint has no value"));
    }
    src.map_or(Ok(()), |s| parse(s).map(drop).map_err(DocError::from))
}

impl Document {
    pub fn constraints(&self) -> impl Iterator<Item = (ConstraintId, &Constraint)> {
        self.constraints.iter().map(|(id, c)| (*id, c))
    }

    pub fn constraint(&self, id: ConstraintId) -> Option<&Constraint> {
        self.constraints.get(&id)
    }

    /// The expression that drives the value of the constraint, if any.
    pub fn constraint_expr(&self, id: ConstraintId) -> Option<&str> {
        self.constraint_exprs.get(&id).map(String::as_str)
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
            Command::AddConstraint(id, c) => self.add_constraint(id, c, None),
            Command::AddConstraintWithExpr(id, c, src) => self.add_constraint(id, c, Some(src)),
            Command::RemoveConstraint(id) => {
                let old = self.constraints.remove(&id).ok_or(DocError::ConstraintNotFound(id))?;
                Ok(match self.constraint_exprs.remove(&id) {
                    Some(src) => Command::AddConstraintWithExpr(id, old, src),
                    None => Command::AddConstraint(id, old),
                })
            }
            Command::SetConstraint(id, c) => {
                if !self.constraints.contains_key(&id) {
                    return Err(DocError::ConstraintNotFound(id));
                }
                self.check_entities(&c)?;
                if self.constraint_exprs.contains_key(&id) {
                    check_expr(&c, None)?;
                }
                let old = self.constraints.insert(id, c).expect("checked above");
                Ok(Command::SetConstraint(id, old))
            }
            Command::SetConstraintExpr { id, expr } => {
                check_expr(self.constraints.get(&id).ok_or(DocError::ConstraintNotFound(id))?, expr.as_deref())?;
                let previous = match expr {
                    Some(src) => self.constraint_exprs.insert(id, src),
                    None => self.constraint_exprs.remove(&id),
                };
                Ok(Command::SetConstraintExpr { id, expr: previous })
            }
            _ => unreachable!("not a constraint command"),
        }
    }

    fn add_constraint(&mut self, id: ConstraintId, c: Constraint, src: Option<String>) -> Result<Command, DocError> {
        if self.constraints.contains_key(&id) {
            return Err(DocError::ConstraintExists(id));
        }
        self.check_entities(&c)?;
        if src.is_some() {
            check_expr(&c, src.as_deref())?;
        }
        self.next_constraint_id = self.next_constraint_id.max(id.saturating_add(1));
        self.constraints.insert(id, c);
        if let Some(src) = src {
            self.constraint_exprs.insert(id, src);
        }
        Ok(Command::RemoveConstraint(id))
    }

    /// The value of every expression, in constraint order. Distance, radius and diameter are lengths, angles are angles.
    fn expr_values(&self) -> Vec<(ConstraintId, Result<f64, DocError>)> {
        let value = |id: ConstraintId, src: &str| {
            let angle = matches!(self.constraints.get(&id), Some(Constraint::Angle(..)));
            eval(self, src, angle).map_err(|message| DocError::ConstraintExpr { id, message })
        };
        self.constraint_exprs.iter().filter(|(id, _)| self.constraints.contains_key(id)).map(|(id, src)| (*id, value(*id, src))).collect()
    }

    /// The constraints with the given values in place of the stored ones.
    fn with_values(&self, values: &BTreeMap<ConstraintId, f64>) -> Vec<Constraint> {
        let new = |(id, c): (&ConstraintId, &Constraint)| values.get(id).and_then(|v| c.with_value(*v)).unwrap_or_else(|| c.clone());
        self.constraints.iter().map(new).collect()
    }

    fn solve_all(&self, list: &[Constraint], pinned: &BTreeSet<EntityId>) -> Result<Solution, SolveError> {
        let shapes = self.entities.iter().map(|(id, e)| (*id, e.shape.clone())).collect::<BTreeMap<_, _>>();
        solve_pinned(&shapes, list, pinned)
    }

    /// Commands that make the geometry satisfy the constraints: first the new values of constraints driven by expressions,
    /// then SetShape. Empty when nothing has to move. Fails when an expression cannot be evaluated.
    pub fn solve_commands(&self) -> Result<Vec<Command>, DocError> {
        self.solve_commands_pinned(&BTreeSet::new())
    }

    /// Like `solve_commands`, but the pinned entities stay where they are.
    pub fn solve_commands_pinned(&self, pinned: &BTreeSet<EntityId>) -> Result<Vec<Command>, DocError> {
        let mut values = BTreeMap::new();
        for (id, v) in self.expr_values() {
            values.insert(id, v?);
        }
        let list = self.with_values(&values);
        let solution = self.solve_all(&list, pinned)?;
        let mut cmds: Vec<Command> = (self.constraints.iter().zip(&list))
            .filter(|((_, old), new)| old != new)
            .map(|((id, _), new)| Command::SetConstraint(*id, new.clone()))
            .collect();
        cmds.extend(solution.shapes.into_iter().map(|(id, shape)| Command::SetShape { id, shape }));
        Ok(cmds)
    }

    /// Degrees of freedom and redundancy of the constrained geometry. None without constraints.
    /// An expression that cannot be evaluated counts with its stored value.
    pub fn constraint_status(&self) -> Option<Result<Solution, SolveError>> {
        if self.constraints.is_empty() {
            return None;
        }
        let values = self.expr_values().into_iter().filter_map(|(id, v)| v.ok().map(|v| (id, v))).collect();
        Some(self.solve_all(&self.with_values(&values), &BTreeSet::new()))
    }
}
