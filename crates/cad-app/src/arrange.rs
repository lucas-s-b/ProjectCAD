use crate::app::CadApp;
use crate::widgets as w;
use cad_document::{Command, ConstraintId, EntityId};
use cad_geometry::edit::join;
use cad_geometry::{DVec2, DVec3};
use eframe::egui;
use std::collections::BTreeSet;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AlignMode {
    Left,
    Right,
    Top,
    Bottom,
    CentreX,
    CentreY,
    DistributeX,
    DistributeY,
}

/// How far each box (min, max) moves so that it is aligned with the union of all boxes, or the centres are evenly spaced.
/// The two outermost boxes of a distribution stay; fewer than 3 boxes do not move.
pub fn align_offsets(boxes: &[(DVec2, DVec2)], mode: AlignMode) -> Vec<DVec2> {
    let Some((lo, hi)) = boxes.iter().copied().reduce(|(a, b), (c, d)| (a.min(c), b.max(d))) else { return Vec::new() };
    let centre = |b: &(DVec2, DVec2)| (b.0 + b.1) / 2.0;
    match mode {
        AlignMode::Left => boxes.iter().map(|b| DVec2::new(lo.x - b.0.x, 0.0)).collect(),
        AlignMode::Right => boxes.iter().map(|b| DVec2::new(hi.x - b.1.x, 0.0)).collect(),
        AlignMode::Top => boxes.iter().map(|b| DVec2::new(0.0, hi.y - b.1.y)).collect(),
        AlignMode::Bottom => boxes.iter().map(|b| DVec2::new(0.0, lo.y - b.0.y)).collect(),
        AlignMode::CentreX => boxes.iter().map(|b| DVec2::new((lo.x + hi.x) / 2.0 - centre(b).x, 0.0)).collect(),
        AlignMode::CentreY => boxes.iter().map(|b| DVec2::new(0.0, (lo.y + hi.y) / 2.0 - centre(b).y)).collect(),
        AlignMode::DistributeX | AlignMode::DistributeY => {
            let axis = |v: DVec2| if mode == AlignMode::DistributeX { v.x } else { v.y };
            let mut order: Vec<usize> = (0..boxes.len()).collect();
            order.sort_by(|&i, &j| axis(centre(&boxes[i])).total_cmp(&axis(centre(&boxes[j]))));
            let mut out = vec![DVec2::ZERO; boxes.len()];
            if let (Some(&first), Some(&last)) = (order.first(), order.last()) {
                let (c0, c1) = (axis(centre(&boxes[first])), axis(centre(&boxes[last])));
                for (k, &i) in order.iter().enumerate() {
                    let d = c0 + (c1 - c0) * k as f64 / (order.len() - 1) as f64 - axis(centre(&boxes[i]));
                    out[i] = if mode == AlignMode::DistributeX { DVec2::new(d, 0.0) } else { DVec2::new(0.0, d) };
                }
            }
            out
        }
    }
}

impl CadApp {
    /// Moves the selected entities as `align_offsets` says, as one undo step.
    pub fn align_selection(&mut self, mode: AlignMode) {
        let items: Vec<_> = self.selection.iter().filter_map(|id| self.doc.get(*id)).collect();
        let boxes: Vec<_> = items.iter().map(|e| {
            let (lo, hi) = e.shape.bbox();
            (lo.truncate(), hi.truncate())
        }).collect();
        let cmds = items
            .iter()
            .zip(align_offsets(&boxes, mode))
            .filter(|(_, d)| d.length() > 0.0)
            .map(|(e, d)| {
                let mut shape = e.shape.clone();
                shape.translate(DVec3::new(d.x, d.y, 0.0));
                Command::SetShape { id: e.id, shape }
            })
            .collect();
        self.exec(cmds);
    }

    /// Replaces the selected lines and open polylines by one polyline that keeps the lowest id.
    pub fn join_selection(&mut self) {
        let shapes: Vec<_> = self.selection.iter().filter_map(|id| self.doc.get(*id)).map(|e| &e.shape).collect();
        let shape = match join(&shapes) {
            Ok(s) => s,
            Err(e) => {
                self.status = e.to_string();
                return;
            }
        };
        let Some(&keep) = self.selection.iter().next() else { return };
        let gone: Vec<EntityId> = self.selection.iter().copied().filter(|id| *id != keep).collect();
        let constraints: BTreeSet<ConstraintId> = gone.iter().flat_map(|id| self.doc.constraints_of(*id)).collect();
        let cmds = constraints
            .into_iter()
            .map(Command::RemoveConstraint)
            .chain(gone.into_iter().map(Command::Remove))
            .chain([Command::SetShape { id: keep, shape }])
            .collect();
        self.exec(cmds);
        self.set_selection([keep]);
    }
}

/// Align and distribute buttons (2 or more selected, distribute 3 or more) and Join.
pub fn arrange_group(app: &mut CadApp, ui: &mut egui::Ui) {
    let n = app.selection.len();
    if n < 2 {
        return;
    }
    ui.group(|ui| {
        ui.label("Arrange");
        let modes = [
            ("Align left", AlignMode::Left),
            ("Align right", AlignMode::Right),
            ("Align top", AlignMode::Top),
            ("Align bottom", AlignMode::Bottom),
            ("Align centre X", AlignMode::CentreX),
            ("Align centre Y", AlignMode::CentreY),
            ("Distribute X", AlignMode::DistributeX),
            ("Distribute Y", AlignMode::DistributeY),
        ];
        ui.horizontal_wrapped(|ui| {
            for (label, mode) in modes {
                let distribute = matches!(mode, AlignMode::DistributeX | AlignMode::DistributeY);
                if (!distribute || n >= 3) && w::button(ui, label).clicked() {
                    app.align_selection(mode);
                }
            }
            if w::button(ui, "Join").clicked() {
                app.join_selection();
            }
        });
    });
}

#[cfg(test)]
#[path = "arrange_tests.rs"]
mod tests;
