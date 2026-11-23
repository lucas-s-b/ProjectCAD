//! The Assembly tab's motion study: drive one joint over a range, play it back in 3D, find the first collision.
use crate::analysis_worker::{JobKind, JobOutput};
use crate::app::CadApp;
use crate::assembly_joints::name;
use crate::widgets as w;
use cad_document::{Accuracy, Collision, ComponentId, JointId, JointKind, MAX_STEPS, Motion, SAMPLED};
use cad_geometry::DAffine3;
use eframe::egui::{self, Color32, RichText, TextEdit};
use std::collections::BTreeMap;
use std::time::Instant;

/// Samples shown per second while playing.
const PLAY_RATE: f64 = 30.0;

/// One run: the samples, the collision check (None when not asked, Err when a boolean failed) and the time each took.
pub struct StudyRun {
    pub motion: Motion,
    pub collision: Option<Result<Option<Collision>, String>>,
    pub drive_ms: f64,
    pub check_ms: Option<f64>,
}

/// The accuracy label as a small tagged line (so the MCP `widgets` list shows it).
pub(crate) fn accuracy(ui: &mut egui::Ui, a: Accuracy) {
    let text = a.to_string();
    let r = ui.small(&text);
    w::tag(ui, &text, r);
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// "Motion study: 36 steps in 2 ms; collision check 840 ms".
fn summary(run: &StudyRun) -> String {
    let mut s = format!("Motion study: {} steps in {:.0} ms", run.motion.values.len() - 1, run.drive_ms);
    if let Some(c) = run.check_ms {
        s += &format!("; collision check {c:.0} ms");
    }
    s
}

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    ui.separator();
    ui.heading("Motion study");
    let joints: Vec<(JointId, String)> =
        app.doc.joints().filter(|j| j.kind != JointKind::Fixed && !j.closes_loop).map(|j| (j.id, format!("Drive: Joint {}", j.id))).collect();
    let Some(first) = joints.first().map(|(id, _)| *id) else {
        ui.small("Add a revolute or slider joint to drive it.");
        return;
    };
    if !joints.iter().any(|(id, _)| *id == app.motion_study.joint) {
        app.motion_study.joint = first;
    }
    if app.doc.component(app.motion_study.trace).is_none() {
        app.motion_study.trace = app.doc.joint(app.motion_study.joint).map_or(0, |j| j.child);
    }
    let traces: Vec<(ComponentId, String)> = app.doc.components().map(|c| (c.id, format!("Trace: {}", c.name))).collect();
    let text = |list: &[(u64, String)], id: u64| list.iter().find(|(i, _)| *i == id).map_or_else(String::new, |(_, t)| t.clone());
    let (joint_text, trace_text) = (text(&joints, app.motion_study.joint), text(&traces, app.motion_study.trace));
    let current = app.model_current();
    let study = &mut app.motion_study;
    ui.horizontal_wrapped(|ui| {
        w::combo(ui, "Motion joint", egui::ComboBox::from_id_salt("motion_joint").selected_text(joint_text), |ui| {
            for (id, t) in &joints {
                w::selectable_value(ui, &mut study.joint, *id, t);
            }
        });
        ui.label("from");
        w::add(ui, "Motion start", TextEdit::singleline(&mut study.start).desired_width(60.0));
        ui.label("to");
        w::add(ui, "Motion end", TextEdit::singleline(&mut study.end).desired_width(60.0));
    });
    ui.horizontal_wrapped(|ui| {
        w::add(ui, "Motion steps", egui::DragValue::new(&mut study.steps).range(1..=MAX_STEPS));
        ui.label("steps");
    });
    ui.horizontal_wrapped(|ui| {
        w::combo(ui, "Trace component", egui::ComboBox::from_id_salt("motion_trace").selected_text(trace_text), |ui| {
            for (id, t) in &traces {
                w::selectable_value(ui, &mut study.trace, *id, t);
            }
        });
        for (axis, t) in ["X", "Y", "Z"].into_iter().zip(study.point.iter_mut()) {
            w::add(ui, &format!("Trace {axis}"), TextEdit::singleline(t).desired_width(50.0));
        }
    });
    let mut run = false;
    ui.horizontal_wrapped(|ui| {
        w::checkbox(ui, &mut app.motion_check, "Check collisions");
        run = w::add_enabled(ui, current && app.jobs.running().is_none(), "Run study", egui::Button::new("Run study")).clicked();
    });
    if run {
        run_study(app, ui.ctx());
    }
    results(app, ui);
}

/// Starts `Document::drive` and, when asked, `Document::first_collision` on the current rebuild on the analysis worker;
/// `finish` takes the result.
fn run_study(app: &mut CadApp, ctx: &egui::Context) {
    let Some(result) = app.model_result().cloned() else { return };
    let (doc, study, check) = (app.doc.clone(), app.motion_study.clone(), app.motion_check);
    app.jobs.start(ctx, JobKind::Motion, Some(app.doc.revision()), move || {
        let t = Instant::now();
        let out = doc.drive(&result, &study).map(|motion| {
            let drive_ms = ms(t);
            let t = Instant::now();
            let collision = check.then(|| doc.first_collision(&result, &motion));
            let check_ms = collision.as_ref().map(|_| ms(t));
            StudyRun { motion, collision, drive_ms, check_ms }
        });
        JobOutput::Motion(out)
    });
}

/// Shows a finished run (the status line gives both times). Playback starts at the first colliding sample, else at 0.
pub fn finish(app: &mut CadApp, out: Result<StudyRun, String>) {
    app.motion_play = false;
    // The scene shows the result (see the stamp in `view3d::show`).
    app.sel_rev += 1;
    match out {
        Ok(run) => {
            app.status = summary(&run);
            app.motion_step = match &run.collision {
                Some(Ok(Some(c))) => c.sample,
                _ => 0,
            };
            app.motion = Some((app.doc.revision(), run));
        }
        Err(err) => {
            app.status = format!("Motion study: {err}");
            app.motion = None;
        }
    }
}

/// Step slider, play/pause, the joint value and traced point of the shown sample, the first collision, the labels,
/// the times and the CSV export.
fn results(app: &mut CadApp, ui: &mut egui::Ui) {
    let s = *app.doc.settings();
    let Some(run) = app.motion_result() else {
        ui.small("The study runs on demand; any edit clears the result.");
        return;
    };
    let n = run.motion.values.len() - 1;
    let k = app.motion_step.min(n);
    let value = |v: f64| if run.motion.angle { format!("{:.2} deg", v.to_degrees()) } else { format!("{} {}", s.format_length(v), s.units.suffix()) };
    let at = format!("Step {k} of {n}: {}", value(run.motion.values[k]));
    let point = format!("Trace point {}", crate::mass_panel::point(&s, run.motion.trace[k]));
    let collision = run.collision.as_ref().map(|c| match c {
        Ok(Some(c)) => {
            let pair = format!("{} / {}", name(app, c.moving), name(app, c.other));
            (format!("First collision at step {} ({}): {pair}, interference {}", c.sample, value(run.motion.values[c.sample]), s.format_volume(c.volume)), true)
        }
        Ok(None) => ("No collision at the steps".to_string(), false),
        Err(err) => (err.clone(), true),
    });
    let times = summary(run);
    let positions = run.motion.accuracy;
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        changed |= w::add(ui, "Motion slider", egui::Slider::new(&mut app.motion_step, 0..=n).show_value(false)).changed();
        changed |= w::add(ui, "Motion step", egui::DragValue::new(&mut app.motion_step).range(0..=n)).changed();
        if w::button(ui, if app.motion_play { "Pause" } else { "Play" }).clicked() {
            app.motion_play = !app.motion_play;
            app.motion_clock = ui.input(|i| i.time);
        }
    });
    if changed {
        app.motion_play = false;
        app.sel_rev += 1;
    }
    ui.label(at);
    ui.label(point);
    accuracy(ui, positions);
    if let Some((text, red)) = collision {
        ui.label(if red { RichText::new(text).color(Color32::RED) } else { RichText::new(text) });
        accuracy(ui, Accuracy::Approximate(SAMPLED));
    }
    ui.small(times);
    if w::button(ui, "Export motion CSV").clicked() {
        app.export_motion_dialog();
    }
}

/// Advances a playing study by one sample every 1/PLAY_RATE s (looping) and keeps frames coming while it plays.
pub fn tick(app: &mut CadApp, ctx: &egui::Context) {
    let Some(len) = app.motion_result().map(|r| r.motion.values.len()) else {
        app.motion_play = false;
        return;
    };
    if !app.motion_play {
        return;
    }
    let now = ctx.input(|i| i.time);
    if now - app.motion_clock >= 1.0 / PLAY_RATE {
        app.motion_clock = now;
        app.motion_step = (app.motion_step + 1) % len;
        app.sel_rev += 1;
    }
    ctx.request_repaint();
}

/// The motions of the moving components: of a joint value being dragged, else of the shown study sample; None while
/// exploded or with neither.
pub fn shown_moves(app: &CadApp) -> Option<&BTreeMap<ComponentId, DAffine3>> {
    if app.explode_factor() != 0.0 {
        return None;
    }
    if let Some(d) = app.joint_drag.as_ref().filter(|d| d.rev == app.doc.revision() && app.joint_sel == Some(d.joint)) {
        return d.moves.as_ref().ok();
    }
    let run = app.motion_result()?;
    run.motion.moves.get(app.motion_step.min(run.motion.moves.len() - 1))
}

/// The first collision while its sample is shown.
pub fn shown_collision(app: &CadApp) -> Option<&Collision> {
    let c = app.motion_result().filter(|_| app.explode_factor() == 0.0)?.collision.as_ref()?.as_ref().ok()?.as_ref()?;
    (c.sample == app.motion_step).then_some(c)
}
