use crate::app::{CadApp, Tool};
use crate::view2d::pick;
use cad_document::{Command, EntityId};
use cad_geometry::edit::{chamfer, extend, fillet, offset, trim};
use cad_geometry::{DVec2, Shape};
use eframe::egui::{self, Color32, PointerButton};

/// Stored shapes of every visible entity except `skip`.
fn others(app: &CadApp, skip: EntityId) -> Vec<&Shape> {
    app.doc.entities_in(app.sketch).filter(|e| e.id != skip && app.visible(e)).map(|e| &e.shape).collect()
}

pub fn edit_tool(app: &mut CadApp, _ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect) {
    if !resp.clicked_by(PointerButton::Primary) {
        return;
    }
    let Some(pos) = resp.interact_pointer_pos() else { return };
    let world = app.cam2d.screen_to_world(rect, pos);
    let hit = pick(app, rect, pos);
    match run(app, world, hit) {
        Ok(cmds) if !cmds.is_empty() => app.exec(cmds),
        Ok(_) => {}
        Err(msg) => app.status = msg,
    }
}

/// Commands for one click, or the error text for the status bar.
fn run(app: &mut CadApp, world: DVec2, hit: Option<EntityId>) -> Result<Vec<Command>, String> {
    let err = |e: cad_geometry::GeomError| e.to_string();
    let Some(id) = hit.or(app.op_target.map(|(id, _)| id).filter(|_| app.tool == Tool::Offset)) else {
        return Ok(vec![]);
    };
    let mut cmds = Vec::new();
    match (app.tool, app.op_target) {
        (Tool::Trim, _) | (Tool::Extend, _) => {
            let Some(e) = app.doc.get(id) else { return Ok(vec![]) };
            let layer = e.layer;
            if app.tool == Tool::Extend {
                let shape = extend(&e.shape, world, &others(app, id)).map_err(err)?;
                return Ok(vec![Command::SetShape { id, shape }]);
            }
            let pieces = trim(&e.shape, world, &others(app, id)).map_err(err)?;
            let mut pieces = pieces.into_iter();
            match pieces.next() {
                None => cmds.push(Command::Remove(id)),
                Some(shape) => cmds.push(Command::SetShape { id, shape }),
            }
            for piece in pieces {
                cmds.push(Command::Add(app.new_entity(piece, layer)));
            }
        }
        (Tool::Offset, None) | (Tool::Fillet, None) | (Tool::Chamfer, None) => {
            if hit.is_some() {
                app.op_target = Some((id, world));
            }
        }
        (Tool::Offset, Some((first, _))) => {
            let Some(e) = app.doc.get(first) else { return Ok(vec![]) };
            let (layer, shape) = (e.layer, offset(&e.shape, app.offset_dist, world).map_err(err)?);
            cmds.push(Command::Add(app.new_entity(shape, layer)));
            app.op_target = None;
        }
        (Tool::Fillet | Tool::Chamfer, Some((first, pick_a))) => {
            let (Some(a), Some(b)) = (app.doc.get(first), app.doc.get(id)) else { return Ok(vec![]) };
            if first == id {
                return Ok(vec![]);
            }
            let (sa, sb, layer) = (&a.shape, &b.shape, a.layer);
            let (la, lb, third) = if app.tool == Tool::Fillet {
                fillet(sa, pick_a, sb, world, app.fillet_radius)
            } else {
                chamfer(sa, pick_a, sb, world, app.chamfer_dist)
            }
            .map_err(err)?;
            cmds.push(Command::SetShape { id: first, shape: la });
            cmds.push(Command::SetShape { id, shape: lb });
            if let Some(third) = third {
                cmds.push(Command::Add(app.new_entity(third, layer)));
            }
            app.op_target = None;
        }
        _ => {}
    }
    Ok(cmds)
}

pub fn draw_hint(app: &CadApp, painter: &egui::Painter, rect: egui::Rect) {
    let text = match app.tool {
        Tool::Trim => "Click the part to remove",
        Tool::Extend => "Click near the end to extend",
        Tool::Offset => "Click a curve, then the side",
        Tool::Fillet | Tool::Chamfer => "Click the first line, then the second",
        _ => return,
    };
    painter.text(
        rect.left_top() + egui::vec2(8.0, 8.0),
        egui::Align2::LEFT_TOP,
        text,
        egui::FontId::proportional(14.0),
        Color32::from_gray(200),
    );
}
