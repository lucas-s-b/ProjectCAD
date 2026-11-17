//! Groups and metadata: whole-group click selection and the Group / Metadata parts of the properties panel.
use crate::app::CadApp;
use crate::widgets as w;
use cad_document::{Command, EntityId};
use eframe::egui::{self, Button, TextEdit};
use std::collections::BTreeSet;

impl CadApp {
    /// `id` with the other members of its group that are in the active sketch and can be picked.
    pub fn with_group(&self, id: EntityId) -> Vec<EntityId> {
        let Some(group) = self.doc.get(id).and_then(|e| e.group) else { return vec![id] };
        let mut ids: Vec<EntityId> = self
            .doc
            .group_members(group)
            .into_iter()
            .filter(|m| self.doc.get(*m).is_some_and(|e| e.sketch == self.sketch && self.pickable(e)))
            .collect();
        if !ids.contains(&id) {
            ids.push(id);
        }
        ids
    }

    /// Selection by click: a group is selected as a whole; Shift toggles the whole group.
    pub fn click_select(&mut self, hit: Option<EntityId>, shift: bool) {
        match (hit, shift) {
            (Some(id), true) => {
                let mut ids = self.selection.clone();
                let group = self.with_group(id);
                if ids.contains(&id) {
                    ids.retain(|i| !group.contains(i));
                } else {
                    ids.extend(group);
                }
                self.set_selection(ids);
            }
            (Some(id), false) => {
                let group = self.with_group(id);
                self.set_selection(group);
            }
            (None, true) => {}
            (None, false) => self.set_selection([]),
        }
    }

    /// Puts the selection in one new group.
    pub fn group_selection(&mut self) {
        let group = self.doc.alloc_group_id();
        let ids = self.selection.iter().copied().collect();
        self.exec(vec![Command::SetGroup { ids, group: Some(group) }]);
    }

    /// Dissolves every group that has a selected member.
    pub fn ungroup_selection(&mut self) {
        let groups: BTreeSet<u32> = self.selection.iter().filter_map(|id| self.doc.get(*id)).filter_map(|e| e.group).collect();
        let ids: Vec<EntityId> = groups.into_iter().flat_map(|g| self.doc.group_members(g)).collect();
        self.exec(vec![Command::SetGroup { ids, group: None }]);
    }
}

pub fn group_buttons(app: &mut CadApp, ui: &mut egui::Ui) {
    let grouped = app.selection.iter().filter_map(|id| app.doc.get(*id)).any(|e| e.group.is_some());
    ui.horizontal(|ui| {
        if w::add_enabled(ui, app.selection.len() >= 2, "Group", Button::new("Group")).clicked() {
            app.group_selection();
        }
        if w::add_enabled(ui, grouped, "Ungroup", Button::new("Ungroup")).clicked() {
            app.ungroup_selection();
        }
    });
}

/// Key = value rows of the one selected entity, with a row to add one.
pub fn metadata(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some(id) = (app.selection.len() == 1).then(|| app.selection.iter().next().copied()).flatten() else { return };
    let Some(rows) = app.doc.get(id).map(|e| e.meta.clone()) else { return };
    ui.group(|ui| {
        ui.label("Metadata");
        for (key, value) in rows {
            ui.horizontal(|ui| {
                if w::button(ui, "x").clicked() {
                    app.exec(vec![Command::SetMeta { id, key: key.clone(), value: None }]);
                }
                ui.label(format!("{key} = {value}"));
            });
        }
        ui.horizontal(|ui| {
            w::add(ui, "Meta key", TextEdit::singleline(&mut app.meta_new.0).hint_text("key").desired_width(70.0));
            w::add(ui, "Meta value", TextEdit::singleline(&mut app.meta_new.1).hint_text("value").desired_width(90.0));
        });
        if w::button(ui, "Add meta").clicked() {
            let (key, value) = app.meta_new.clone();
            app.exec(vec![Command::SetMeta { id, key, value: Some(value) }]);
            if app.status.is_empty() {
                app.meta_new = Default::default();
            }
        }
    });
}
