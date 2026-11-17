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
        if matches!(req, Request::State | Request::Screenshot | Request::Export(_)) && !app.model_current() {
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
