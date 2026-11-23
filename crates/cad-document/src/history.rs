use crate::document::{Command, DocError, Document, EntityId};
use std::collections::BTreeSet;

#[derive(Default)]
pub struct History {
    undo: Vec<Vec<Command>>,
    redo: Vec<Vec<Command>>,
    merge_key: Option<u64>,
}

/// Applies `cmds` in order; returns the inverses in reverse order. Rolls back on error.
fn run(doc: &mut Document, cmds: Vec<Command>) -> Result<Vec<Command>, DocError> {
    let mut inverses: Vec<Command> = Vec::with_capacity(cmds.len());
    for cmd in cmds {
        match doc.apply(cmd) {
            Ok(inv) => inverses.insert(0, inv),
            Err(e) => {
                for inv in inverses {
                    // Inverses of commands that just succeeded cannot fail.
                    let _ = doc.apply(inv);
                }
                return Err(e);
            }
        }
    }
    Ok(inverses)
}

impl History {
    /// Runs `cmds`, then lets the constraint solver move geometry so the document satisfies its constraints.
    /// Both land in one undo step. On any error the document is exactly as before the call.
    pub fn execute(&mut self, doc: &mut Document, cmds: Vec<Command>) -> Result<(), DocError> {
        self.execute_pinned(doc, cmds, &BTreeSet::new())
    }

    /// Like `execute`, but the solve after the commands holds the pinned entities where the commands left them, so the rest of the sketch follows.
    /// When that solve fails the constraints win over the pins and it is repeated without them.
    pub fn execute_pinned(&mut self, doc: &mut Document, cmds: Vec<Command>, pinned: &BTreeSet<EntityId>) -> Result<(), DocError> {
        if cmds.is_empty() {
            return Ok(());
        }
        let mut inverses = run(doc, cmds)?;
        if doc.constraints().next().is_some() {
            let solved = doc.solve_commands_pinned(pinned).or_else(|e| if pinned.is_empty() { Err(e) } else { doc.solve_commands() });
            match solved.and_then(|extra| run(doc, extra)) {
                // Undo replays in list order: the solver's changes first, then the user's.
                Ok(mut solver_inverses) => {
                    solver_inverses.append(&mut inverses);
                    inverses = solver_inverses;
                }
                Err(e) => {
                    for inv in inverses {
                        let _ = doc.apply(inv);
                    }
                    return Err(e);
                }
            }
        }
        // Linked dimensions follow their points and openings their walls, after the solve moved them.
        let mut refresh = doc.dim_refresh_commands();
        refresh.extend(doc.opening_refresh_commands());
        match run(doc, refresh) {
            Ok(mut follow_inverses) => {
                follow_inverses.append(&mut inverses);
                inverses = follow_inverses;
            }
            Err(e) => {
                for inv in inverses {
                    let _ = doc.apply(inv);
                }
                return Err(e);
            }
        }
        self.undo.push(inverses);
        self.redo.clear();
        self.merge_key = None;
        Ok(())
    }

    /// Like `execute`, but a run of calls with the same key shares one undo step.
    /// Only for "set" commands on the same target: the first inverse must restore the state before the whole run.
    /// Does not solve constraints: only for commands that do not affect geometry or constraints.
    pub fn execute_merged(&mut self, doc: &mut Document, cmds: Vec<Command>, key: u64) -> Result<(), DocError> {
        if self.merge_key == Some(key) && self.redo.is_empty() && !self.undo.is_empty() {
            run(doc, cmds)?;
            return Ok(());
        }
        self.execute(doc, cmds)?;
        self.merge_key = Some(key);
        Ok(())
    }

    pub fn undo(&mut self, doc: &mut Document) -> bool {
        self.merge_key = None;
        Self::step(&mut self.undo, &mut self.redo, doc)
    }

    pub fn redo(&mut self, doc: &mut Document) -> bool {
        self.merge_key = None;
        Self::step(&mut self.redo, &mut self.undo, doc)
    }

    fn step(from: &mut Vec<Vec<Command>>, to: &mut Vec<Vec<Command>>, doc: &mut Document) -> bool {
        let Some(cmds) = from.pop() else { return false };
        match run(doc, cmds.clone()) {
            Ok(inv) => {
                to.push(inv);
                true
            }
            Err(_) => {
                from.push(cmds);
                false
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The commands the next undo applies (the inverse of the last step, in application order); None when there is none.
    pub fn last_undo(&self) -> Option<&[Command]> {
        self.undo.last().map(Vec::as_slice)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.merge_key = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Entity, Layer};
    use cad_geometry::{DVec2, Shape};

    fn circle(r: f64) -> Shape {
        Shape::Circle { center: DVec2::ZERO, radius: r }
    }
    fn add(id: u64, r: f64) -> Command {
        Command::Add(Entity { id, name: format!("c{id}"), shape: circle(r), layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })
    }
    fn snapshot(d: &Document) -> Vec<Entity> {
        d.entities().cloned().collect()
    }

    #[test]
    fn last_undo_is_the_inverse_step() {
        let mut d = Document::new("t");
        let mut h = History::default();
        assert!(h.last_undo().is_none());
        h.execute(&mut d, vec![add(1, 1.0), add(2, 2.0)]).unwrap();
        let l = h.last_undo().unwrap();
        assert_eq!(l.len(), 2);
        assert!(matches!(l[0], Command::Remove(2)));
        assert!(matches!(l[1], Command::Remove(1)));
        h.execute(&mut d, vec![]).unwrap();
        assert_eq!(h.last_undo().unwrap().len(), 2);
    }

    #[test]
    fn failing_transaction_rolls_back() {
        let mut d = Document::new("t");
        let mut h = History::default();
        h.execute(&mut d, vec![add(1, 1.0)]).unwrap();
        let before = snapshot(&d);
        let r = h.execute(&mut d, vec![add(2, 1.0), Command::Remove(1), Command::Remove(7)]);
        assert!(r.is_err());
        assert_eq!(snapshot(&d), before);
        h.undo(&mut d);
        assert!(d.is_empty());
        assert!(!h.can_undo());
    }

    #[test]
    fn undo_redo_roundtrip_over_mixed_transactions() {
        let mut d = Document::new("t");
        let mut h = History::default();
        let mut states = vec![snapshot(&d)];
        let txs = vec![
            vec![add(1, 1.0), add(2, 2.0)],
            vec![Command::SetShape { id: 1, shape: circle(5.0) }],
            vec![Command::Remove(2), add(3, 3.0)],
            vec![Command::SetShape { id: 3, shape: circle(4.0) }, Command::SetShape { id: 1, shape: circle(6.0) }],
            vec![Command::Remove(1), Command::Remove(3)],
        ];
        for tx in txs {
            h.execute(&mut d, tx).unwrap();
            states.push(snapshot(&d));
        }
        for i in (0..states.len() - 1).rev() {
            assert!(h.undo(&mut d));
            assert_eq!(snapshot(&d), states[i]);
        }
        assert!(!h.undo(&mut d));
        for s in &states[1..] {
            assert!(h.redo(&mut d));
            assert_eq!(&snapshot(&d), s);
        }
        assert!(!h.redo(&mut d));
    }

    #[test]
    fn execute_after_undo_clears_redo() {
        let mut d = Document::new("t");
        let mut h = History::default();
        h.execute(&mut d, vec![add(1, 1.0)]).unwrap();
        h.undo(&mut d);
        assert!(h.can_redo());
        h.execute(&mut d, vec![add(2, 1.0)]).unwrap();
        assert!(!h.can_redo());
        h.execute(&mut d, vec![]).unwrap();
        assert_eq!(d.len(), 1);
    }

    fn set_layer(visible: bool) -> Vec<Command> {
        vec![Command::SetLayer(Layer { visible, ..Layer::new(0, "0") })]
    }

    #[test]
    fn merged_commands_share_one_undo_step() {
        let mut d = Document::new("t");
        let mut h = History::default();
        h.execute_merged(&mut d, set_layer(false), 7).unwrap();
        h.execute_merged(&mut d, vec![Command::SetLayer(Layer { color: [1, 2, 3], ..Layer::new(0, "0") })], 7).unwrap();
        h.execute_merged(&mut d, vec![Command::SetLayer(Layer { color: [4, 5, 6], ..Layer::new(0, "0") })], 7).unwrap();
        let last = d.layer(0).unwrap().clone();
        assert!(h.undo(&mut d));
        assert_eq!(d.layer(0), Some(&Layer::new(0, "0")));
        assert!(!h.can_undo());
        assert!(h.redo(&mut d));
        assert_eq!(d.layer(0), Some(&last));
    }

    #[test]
    fn other_key_or_execute_starts_new_step() {
        let mut d = Document::new("t");
        let mut h = History::default();
        h.execute_merged(&mut d, set_layer(false), 1).unwrap();
        h.execute_merged(&mut d, set_layer(true), 2).unwrap();
        h.execute(&mut d, set_layer(false)).unwrap();
        h.execute_merged(&mut d, set_layer(true), 2).unwrap();
        for _ in 0..4 {
            assert!(h.undo(&mut d));
        }
        assert!(!h.undo(&mut d));
    }

    #[test]
    fn revision_increases_on_every_change() {
        let mut d = Document::new("t");
        let mut h = History::default();
        h.execute(&mut d, vec![add(1, 1.0)]).unwrap();
        let r1 = d.revision();
        h.undo(&mut d);
        let r2 = d.revision();
        h.redo(&mut d);
        assert!(r1 < r2 && r2 < d.revision());
    }
}
