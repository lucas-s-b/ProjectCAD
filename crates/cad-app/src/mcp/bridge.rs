//! Runs MCP jobs on the UI thread by injecting egui events.
use super::{Job, Reply, Request};
use crate::app::{CadApp, ViewMode};
use cad_geometry::DVec2;
use eframe::egui::{self, Event, Modifiers, Pos2};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender};

type ReplyTx = Sender<Result<Reply, String>>;

/// Frames to wait after asking a widget to scroll into view.
const SCROLL_FRAMES: u8 = 3;
const WARMUP_FRAMES: u8 = 2;

/// A click on a named widget; `text` is typed into it afterwards.
struct WidgetJob {
    name: String,
    index: usize,
    mods: Modifiers,
    text: Option<String>,
}

/// Frames to wait after the last event so egui and the app have reacted.
const SETTLE_FRAMES: u8 = 2;
const DRAG_STEPS: u8 = 8;

pub struct Bridge {
    rx: Receiver<Job>,
    /// One batch of events per UI frame.
    frames: VecDeque<Vec<Event>>,
    mods: Modifiers,
    /// Input job in progress, settle frames left.
    pending: Option<(ReplyTx, u8)>,
    /// Widget job waiting for its widget to scroll into view, frames left.
    scrolling: Option<(WidgetJob, ReplyTx, u8)>,
    /// A request that waits for the rebuild to catch up with the document.
    held: Option<Job>,
    /// Frames still to draw before the first job; named widgets exist only after a drawn frame.
    warmup: u8,
}

impl Bridge {
    pub fn new(rx: Receiver<Job>) -> Self {
        Self { rx, frames: VecDeque::new(), mods: Modifiers::NONE, pending: None, scrolling: None, held: None, warmup: WARMUP_FRAMES }
    }

    pub fn feed(&mut self, raw: &mut egui::RawInput) {
        // The MCP window is usually not the OS foreground window; without this, text fields never get focus.
        raw.focused = true;
        // The MCP window takes input only from MCP: a real mouse moving over it would turn an injected click into a drag.
        raw.events.clear();
        raw.hovered_files.clear();
        raw.dropped_files.clear();
        if let Some(batch) = self.frames.pop_front() {
            // egui 0.37 has no RawInput::modifiers; an empty batch is the trailing release of the modifiers.
            if batch.is_empty() {
                self.mods = Modifiers::NONE;
            }
            raw.events.push(Event::ModifiersChanged(self.mods));
            raw.events.extend(batch);
        }
    }

    pub fn poll(&mut self, app: &mut CadApp, ctx: &egui::Context) {
        if self.warmup > 0 {
            self.warmup -= 1;
            ctx.request_repaint();
            return;
        }
        if let Some((_, _, left)) = &mut self.scrolling {
            ctx.request_repaint();
            if *left > 0 {
                *left -= 1;
                return;
            }
            if let Some((job, tx, _)) = self.scrolling.take() {
                self.start_widget(job, tx, app, ctx, true);
            }
            return;
        }
        if let Some((tx, left)) = &mut self.pending {
            ctx.request_repaint();
            // egui spreads a wheel scroll over several frames; the reply waits until the zoom has finished.
            let scrolling = ctx.input(|i| i.smooth_scroll_delta() != egui::Vec2::ZERO);
            if self.frames.is_empty() && !scrolling {
                if *left == 0 {
                    // Keep repainting until the worker has caught up.
                    if !app.model_current() {
                        return;
                    }
                    let _ = tx.send(Ok(Reply { text: summary(app).to_string(), png: None }));
                    self.pending = None;
                } else {
                    *left -= 1;
                }
            }
            return;
        }
        let Some((req, tx)) = self.held.take().or_else(|| self.rx.try_recv().ok()) else { return };
        if matches!(req, Request::State | Request::Screenshot | Request::Export(_) | Request::ExportMotion(_)) && !app.model_current() {
            self.held = Some((req, tx));
            ctx.request_repaint();
            return;
        }
        match req {
            Request::State => {
                let text = serde_json::to_string_pretty(&state_json(app, ctx)).unwrap_or_default();
                let _ = tx.send(Ok(Reply { text, png: None }));
            }
            Request::Screenshot => {
                ctx.request_screenshot(move |img| {
                    let _ = tx.send(encode_png(&img).map(|png| Reply {
                        text: format!("{}x{}", img.size[0], img.size[1]),
                        png: Some(png),
                    }));
                });
                ctx.request_repaint();
            }
            Request::Widgets => {
                let text = serde_json::to_string_pretty(&super::widget_req::list(app, ctx)).unwrap_or_default();
                let _ = tx.send(Ok(Reply { text, png: None }));
            }
            Request::ClickWidget { name, index, mods } => {
                self.start_widget(WidgetJob { name, index, mods, text: None }, tx, app, ctx, false);
            }
            Request::SetText { name, index, text } => {
                let job = WidgetJob { name, index, mods: Modifiers::NONE, text: Some(text) };
                self.start_widget(job, tx, app, ctx, false);
            }
            Request::New => {
                app.new_project();
                let _ = tx.send(Ok(Reply { text: summary(app).to_string(), png: None }));
            }
            Request::Open(p) => {
                let r = app.open_path(p).map(|()| Reply { text: summary(app).to_string(), png: None });
                let _ = tx.send(r);
            }
            Request::Import(p) => {
                let r = app.import_path(p).map(|_| Reply { text: summary(app).to_string(), png: None });
                let _ = tx.send(r);
            }
            Request::Save(p) => {
                let r = app.save_path(p).map(|()| Reply { text: summary(app).to_string(), png: None });
                let _ = tx.send(r);
            }
            Request::Export(p) => {
                let r = app.export_path(p).map(|()| Reply { text: summary(app).to_string(), png: None });
                let _ = tx.send(r);
            }
            Request::ExportMotion(p) => {
                let r = app.export_motion(p).map(|()| Reply { text: summary(app).to_string(), png: None });
                let _ = tx.send(r);
            }
            input => match self.queue(input, app, ctx) {
                Ok(()) => {
                    self.pending = Some((tx, SETTLE_FRAMES));
                    ctx.request_repaint();
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                }
            },
        }
    }

    /// Finds the widget and queues the click; scrolls it into view once when it is hidden.
    fn start_widget(&mut self, job: WidgetJob, tx: ReplyTx, app: &CadApp, ctx: &egui::Context, retried: bool) {
        let hit = match super::widget_req::find(&app.widgets, &job.name, job.index) {
            Ok(h) => (h.rect.center(), h.visible),
            Err(e) => {
                let _ = tx.send(Err(e));
                return;
            }
        };
        if !hit.1 {
            if retried {
                let _ = tx.send(Err(format!("widget '{}' is not visible", job.name)));
            } else {
                crate::widgets::request_scroll(ctx, &job.name, job.index);
                self.scrolling = Some((job, tx, SCROLL_FRAMES));
                ctx.request_repaint();
            }
            return;
        }
        self.click_at(hit.0, egui::PointerButton::Primary, job.mods);
        if let Some(text) = job.text {
            // One idle frame so a drag value has switched to its text editor.
            self.frames.push_back(vec![Event::PointerMoved(hit.0)]);
            let ctrl = Modifiers { ctrl: true, command: true, ..Default::default() };
            self.key_frames(egui::Key::A, ctrl);
            self.frames.push_back(vec![Event::Text(text)]);
            self.key_frames(egui::Key::Enter, Modifiers::NONE);
        }
        self.pending = Some((tx, SETTLE_FRAMES));
        ctx.request_repaint();
    }

    fn click_at(&mut self, p: Pos2, button: egui::PointerButton, mods: Modifiers) {
        let ev = |pressed| Event::PointerButton { pos: p, button, pressed, modifiers: mods };
        self.mods = mods;
        self.frames.extend([vec![Event::PointerMoved(p)], vec![Event::PointerMoved(p)], vec![ev(true)], vec![ev(false)]]);
        self.release_mods(mods);
    }

    fn key_frames(&mut self, key: egui::Key, mods: Modifiers) {
        self.mods = mods;
        let k = |pressed| Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: mods };
        self.frames.extend([vec![k(true)], vec![k(false)]]);
        self.release_mods(mods);
    }

    fn release_mods(&mut self, mods: Modifiers) {
        if mods != Modifiers::NONE {
            self.frames.push_back(Vec::new());
        }
    }

    fn queue(&mut self, req: Request, app: &CadApp, ctx: &egui::Context) -> Result<(), String> {
        let pos = |x: f32, y: f32, world: bool| -> Result<Pos2, String> {
            if !world {
                return Ok(egui::pos2(x, y) / ctx.pixels_per_point());
            }
            if app.view != ViewMode::TwoD {
                return Err("world coordinates need the 2D view".into());
            }
            Ok(app.cam2d.world_to_screen(app.view_rect, DVec2::new(f64::from(x), f64::from(y))))
        };
        let button = |pos, button, pressed, modifiers| Event::PointerButton { pos, button, pressed, modifiers };
        match req {
            Request::Click { x, y, button: b, world, mods } => {
                let p = pos(x, y, world)?;
                self.click_at(p, b, mods);
            }
            Request::Drag { x1, y1, x2, y2, button: b, world, mods } => {
                let (p1, p2) = (pos(x1, y1, world)?, pos(x2, y2, world)?);
                self.mods = mods;
                self.frames.extend([vec![Event::PointerMoved(p1)], vec![button(p1, b, true, mods)]]);
                for i in 1..=DRAG_STEPS {
                    let t = f32::from(i) / f32::from(DRAG_STEPS);
                    self.frames.push_back(vec![Event::PointerMoved(p1 + (p2 - p1) * t)]);
                }
                self.frames.push_back(vec![button(p2, b, false, mods)]);
                self.release_mods(mods);
            }
            Request::Scroll { x, y, dy, world } => {
                let p = pos(x, y, world)?;
                self.mods = Modifiers::NONE;
                self.frames.extend([
                    vec![Event::PointerMoved(p)],
                    vec![Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, dy),
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Wheel,
                        modifiers: Modifiers::NONE,
                    }],
                ]);
            }
            Request::Key { key, mods } => {
                self.key_frames(key, mods);
            }
            Request::Text(t) => {
                self.mods = Modifiers::NONE;
                self.frames.push_back(vec![Event::Text(t)]);
            }
            _ => unreachable!("handled in poll"),
        }
        Ok(())
    }
}

fn encode_png(img: &egui::ColorImage) -> Result<Vec<u8>, String> {
    let [w, h] = img.size;
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w as u32, h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    let data: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    writer.write_image_data(&data).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

fn summary_fields(app: &CadApp) -> serde_json::Map<String, Value> {
    let Value::Object(m) = json!({
        "tool": format!("{:?}", app.tool),
        "view": format!("{:?}", app.view),
        "status": app.status,
        "entities": app.doc.len(),
        "selection": app.selection.iter().collect::<Vec<_>>(),
        "captures": app.captures.iter().map(Vec::len).collect::<Vec<_>>(),
        "dirty": app.is_dirty(),
        "section": app.section.on.then(|| app.section.clip()),
        "face_sel": app.face_sel.map(|(b, f)| [b, u64::from(f)]),
        "edge_sel": app.edge_sel.as_ref().map(|(b, _)| b),
        "component_sel": app.component_sel,
        "component_face": app.component_face.map(|(c, f)| [c, u64::from(f)]),
    }) else {
        unreachable!()
    };
    m
}

fn summary(app: &CadApp) -> Value {
    Value::Object(summary_fields(app))
}

fn insert_model(app: &CadApp, m: &mut serde_json::Map<String, Value>) {
    use cad_document::FeatureState;
    let model = app.model.as_ref().map(|(_, r)| r);
    let body = model.and_then(crate::model_panel::totals).map(|t| {
        let c = t.centroid;
        json!({ "volume": t.volume, "area": t.area, "centroid": [c.x, c.y, c.z], "triangles": t.triangles, "faces": t.faces })
    });
    let bodies: Vec<Value> = model.into_iter().flat_map(|r| &r.bodies).map(|(id, b)| json!({ "id": id, "volume": b.volume(), "edges": b.edges().len() })).collect();
    m.insert("bodies".into(), json!(bodies));
    let walls: Vec<Value> = model.into_iter().flat_map(|r| &r.walls).map(|(id, b)| {
        let (lo, hi) = b.bbox();
        json!({ "id": id, "volume": b.volume(), "zmax": hi.z, "zmin": lo.z })
    }).collect();
    m.insert("walls".into(), json!(walls));
    let components: Vec<Value> = model.into_iter().flat_map(|r| &r.components).map(|(id, placed)| {
        let c = app.doc.component(*id);
        let (min, max, volume, error) = match placed {
            Ok(b) => {
                let (lo, hi) = b.bbox();
                (Some([lo.x, lo.y, lo.z]), Some([hi.x, hi.y, hi.z]), Some(b.volume()), None)
            }
            Err(e) => (None, None, None, Some(e.clone())),
        };
        json!({ "id": id, "body": c.map(|c| c.body), "name": c.map(|c| c.name.clone()), "min": min, "max": max, "volume": volume, "error": error })
    }).collect();
    m.insert("components".into(), json!(components));
    let bom: Vec<Value> = model.map(|r| app.doc.bom(r)).unwrap_or_default().into_iter().map(|r| {
        json!({ "item": r.item, "name": r.name, "designation": r.designation, "material": r.material, "part_number": r.part_number, "quantity": r.quantity, "mass": r.mass })
    }).collect();
    m.insert("bom".into(), json!(bom));
    // Inertia as [Ixx, Iyy, Izz, Ixy, Iyz, Izx] (tensor entries, products with the minus sign), g·mm².
    let row = |r: &cad_document::MassRow| {
        let (p, i) = (&r.props, r.props.inertia);
        let (moments, axes) = p.principal();
        json!({
            "accuracy": r.accuracy.to_string(), "assumed": r.assumed, "density": r.density, "mass": p.mass, "volume": p.volume,
            "com": [p.com.x, p.com.y, p.com.z],
            "inertia": [i.x_axis.x, i.y_axis.y, i.z_axis.z, i.y_axis.x, i.z_axis.y, i.x_axis.z],
            "principal": moments, "axes": axes.map(|a| [a.x, a.y, a.z]),
        })
    };
    let mass = app.mass.as_ref().map(|rep| {
        let with = |key: &str, id: u64, r: &cad_document::MassRow| {
            let mut v = row(r);
            v[key] = json!(id);
            v
        };
        json!({
            "assembly": rep.assembly.as_ref().map(row),
            "bodies": rep.bodies.iter().map(|(id, r)| with("body", *id, r)).collect::<Vec<_>>(),
            "components": rep.components.iter().map(|(id, r)| with("component", *id, r)).collect::<Vec<_>>(),
        })
    });
    m.insert("mass".into(), json!(mass));
    let joints: Vec<Value> = app.doc.joints().map(|j| {
        let error = model.and_then(|r| r.components.iter().find(|(i, _)| *i == j.child)).and_then(|(_, r)| r.as_ref().err().cloned());
        let mut row = json!({ "id": j.id, "kind": format!("{:?}", j.kind), "parent": j.parent, "child": j.child, "value": j.value, "min": j.min, "max": j.max, "error": error });
        // Only on loop joints, so rows of plain joints stay as before.
        if j.closes_loop {
            row["closes_loop"] = json!(true);
        }
        if j.free {
            row["free"] = json!(true);
        }
        if let Some(v) = model.and_then(|r| r.joint_values.get(&j.id)) {
            row["solved"] = json!(if j.kind == cad_document::JointKind::Revolute { v.to_degrees() } else { *v });
        }
        row
    }).collect();
    m.insert("joints".into(), json!(joints));
    m.insert("grounded".into(), json!(app.doc.components().filter(|c| c.grounded).map(|c| c.id).collect::<Vec<_>>()));
    let factor = app.explode_factor();
    let offsets: serde_json::Map<String, Value> =
        app.doc.explode_offsets(factor).into_iter().map(|(id, v)| (id.to_string(), json!([v.x, v.y, v.z]))).collect();
    m.insert("explode".into(), json!({ "factor": factor, "offsets": offsets }));
    let check = app.check_result().map(|c| {
        let meetings: Vec<Value> = c.meetings.iter().map(|m| {
            let (kind, volume, error) = match &m.kind {
                Ok(cad_document::Overlap::Interference { volume, .. }) => ("interference", Some(*volume), None),
                Ok(cad_document::Overlap::Contact) => ("contact", None, None),
                Err(e) => ("error", None, Some(e.clone())),
            };
            json!({ "a": m.a, "b": m.b, "kind": kind, "volume": volume, "error": error })
        }).collect();
        let clearances: Vec<Value> = c.clearances.iter().map(|(a, b, k)| {
            let points = k.points.map(|(p, q)| [[p.x, p.y, p.z], [q.x, q.y, q.z]]);
            json!({ "a": a, "b": b, "distance": k.distance, "points": points })
        }).collect();
        json!({ "meetings": meetings, "clearances": clearances })
    });
    m.insert("check".into(), json!(check));
    // The last analysis mesh: numbers in mm, mm³, degrees; `volume_error` is a fraction; only `error` when it failed.
    let fea_mesh = app.mesh_result().map(|run| match &run.result {
        Ok(rep) => json!({
            "accuracy": rep.accuracy.to_string(), "body": rep.body, "body_volume": rep.body_volume, "elements": rep.stats.elements,
            "error": Value::Null, "h": rep.h, "inverted": rep.stats.inverted, "mesh_ms": run.ms, "min_dihedral": rep.stats.min_dihedral,
            "min_ratio": rep.stats.min_ratio, "nodes": rep.stats.nodes, "unsnapped": rep.mesh.unsnapped, "volume": rep.stats.volume,
            "volume_error": rep.volume_error(),
        }),
        Err(err) => json!({ "error": err }),
    });
    m.insert("fea_mesh".into(), json!(fea_mesh));
    // Values in degrees (revolute) or mm; `point` and `value` belong to the shown sample `step`.
    let motion = app.motion_result().map(|run| {
        let mo = &run.motion;
        let k = app.motion_step.min(mo.values.len() - 1);
        let shown = |v: f64| if mo.angle { v.to_degrees() } else { v };
        let (collision, error) = match &run.collision {
            Some(Ok(Some(c))) => (json!({ "moving": c.moving, "other": c.other, "sample": c.sample, "volume": c.volume }), Value::Null),
            Some(Err(err)) => (Value::Null, json!(err)),
            _ => (Value::Null, Value::Null),
        };
        let p = mo.trace[k];
        json!({
            "accuracy": mo.accuracy.to_string(),
            "collision_accuracy": cad_document::Accuracy::Approximate(cad_document::SAMPLED).to_string(),
            "check_ms": run.check_ms, "checked": run.collision.is_some(), "collision": collision, "error": error,
            "drive_ms": run.drive_ms, "joint": mo.joint, "playing": app.motion_play, "point": [p.x, p.y, p.z],
            "step": k, "steps": mo.values.len() - 1, "trace": mo.trace.iter().map(|p| [p.x, p.y, p.z]).collect::<Vec<_>>(),
            "unit": if mo.angle { "deg" } else { "mm" }, "value": shown(mo.values[k]),
            "values": mo.values.iter().map(|v| shown(*v)).collect::<Vec<_>>(),
        })
    });
    m.insert("motion".into(), json!(motion));
    let rooms: Vec<Value> =
        model.into_iter().flat_map(|r| &r.rooms).map(|(id, g)| json!({ "id": id, "area": g.area, "perimeter": g.perimeter })).collect();
    m.insert("rooms".into(), json!(rooms));
    let states: serde_json::Map<String, Value> = model.into_iter().flat_map(|r| &r.states).map(|(id, s)| {
        let text = match s {
            FeatureState::Ok => "Ok".to_string(),
            FeatureState::Suppressed => "Suppressed".to_string(),
            FeatureState::Error(e) => format!("Error: {e}"),
        };
        (id.to_string(), json!(text))
    }).collect();
    let values: serde_json::Map<String, Value> = app.doc.params().evaluate().into_iter().map(|(n, v)| {
        (n, v.map_or_else(|e| json!(format!("Error: {e}")), |v| json!(v.v)))
    }).collect();
    m.insert("body".into(), json!(body));
    m.insert("feature_states".into(), Value::Object(states));
    let depends: serde_json::Map<String, Value> = model.into_iter().flat_map(|r| &r.depends).map(|(id, d)| (id.to_string(), json!(d))).collect();
    m.insert("depends".into(), Value::Object(depends));
    m.insert("recomputed".into(), json!(model.map(|r| r.recomputed)));
    m.insert("param_values".into(), Value::Object(values));
    m.insert("side_tab".into(), json!(format!("{:?}", app.side_tab)));
}

fn state_json(app: &CadApp, ctx: &egui::Context) -> Value {
    let ppp = ctx.pixels_per_point();
    let win = ctx.content_rect().size() * ppp;
    let r = app.view_rect;
    let mut m = summary_fields(app);
    m.insert("path".into(), json!(app.path.as_ref().map(|p| p.display().to_string())));
    m.insert("current_layer".into(), json!(app.layer));
    m.insert("sketch".into(), json!(app.sketch));
    m.insert("rebuilding".into(), json!(!app.model_current()));
    m.insert("pick_face".into(), json!(app.pick_face != crate::app::PickFace::Off));
    m.insert("pick_edges".into(), json!(app.pick_edges.as_ref().map(|p| p.points.len())));
    m.insert("cursor_world".into(), json!([app.cursor_world.x, app.cursor_world.y]));
    m.insert("pixels_per_point".into(), json!(ppp));
    m.insert("window".into(), json!([win.x, win.y]));
    m.insert(
        "view2d".into(),
        json!({
            "center": [app.cam2d.center.x, app.cam2d.center.y],
            "zoom": app.cam2d.zoom,
            "rect": [r.left() * ppp, r.top() * ppp, r.width() * ppp, r.height() * ppp],
        }),
    );
    insert_model(app, &mut m);
    let cap_area: Option<f64> = app.section.plane().map(|(axis, d)| {
        app.model.iter().flat_map(|(_, m)| &m.bodies).flat_map(|(_, b)| b.section_cap(axis, d))
            .map(|[a, b, c]| 0.5 * (b - a).cross(c - a).length()).sum()
    });
    m.insert("gizmo".into(), json!(app.gizmo_screen.map(|g| g.map(|p| [(p.x * ppp).round(), (p.y * ppp).round()]))));
    m.insert("section_cap_area".into(), json!(cap_area));
    m.insert("constraint_status".into(), json!(crate::constraints_panel::status_text(&app.doc)));
    m.insert("document".into(), serde_json::to_value(&app.doc).unwrap_or(Value::Null));
    Value::Object(m)
}
