use crate::document::{Command, DocError, Document, EntityId};
use std::collections::BTreeSet;

impl Document {
    pub fn alloc_group_id(&mut self) -> u32 {
        let id = self.next_group_id;
        self.next_group_id += 1;
        id
    }

    /// Ids of the entities in `group`, ascending.
    pub fn group_members(&self, group: u32) -> Vec<EntityId> {
        self.entities.values().filter(|e| e.group == Some(group)).map(|e| e.id).collect()
    }

    fn set_groups(&mut self, items: &[(EntityId, Option<u32>)]) -> Result<Vec<(EntityId, Option<u32>)>, DocError> {
        if let Some((missing, _)) = items.iter().find(|(id, _)| !self.entities.contains_key(id)) {
            return Err(DocError::NotFound(*missing));
        }
        let mut seen = BTreeSet::new();
        let mut previous = Vec::new();
        for (id, group) in items {
            let e = self.entities.get_mut(id).expect("checked above");
            // The first mention of an id holds its real previous group.
            if seen.insert(*id) {
                previous.push((*id, e.group));
            }
            e.group = *group;
            if let Some(g) = group {
                self.next_group_id = self.next_group_id.max(g.saturating_add(1));
            }
        }
        Ok(previous)
    }

    /// Validates first, so a failed apply changes nothing. Returns the inverse command.
    pub(crate) fn apply_group(&mut self, cmd: Command) -> Result<Command, DocError> {
        match cmd {
            Command::SetGroup { ids, group } => {
                let items: Vec<_> = ids.into_iter().map(|id| (id, group)).collect();
                self.set_groups(&items).map(Command::RestoreGroups)
            }
            Command::RestoreGroups(items) => self.set_groups(&items).map(Command::RestoreGroups),
            Command::SetMeta { id, key, value } => {
                if key.trim().is_empty() {
                    return Err(DocError::BadFeature("empty key"));
                }
                let e = self.entities.get_mut(&id).ok_or(DocError::NotFound(id))?;
                let old = match &value {
                    Some(v) => e.meta.insert(key.clone(), v.clone()),
                    None => e.meta.remove(&key),
                };
                Ok(Command::SetMeta { id, key, value: old })
            }
            _ => unreachable!("not a group command"),
        }
    }
}
