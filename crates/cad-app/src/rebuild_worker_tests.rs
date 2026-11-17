use super::*;
use cad_document::{BodyOp, Command, Entity, Feature, FeatureKind, Target};
use cad_geometry::{DVec2, Shape};
use std::time::{Duration, Instant};

/// A 40 x 20 rectangle extruded by `distance`.
fn doc_with_extrude(distance: &str) -> Document {
    let mut d = Document::new("t");
    let shape = Shape::rect(DVec2::new(0.0, 0.0), DVec2::new(40.0, 20.0));
    d.apply(Command::Add(Entity { id: 1, name: "r".into(), shape, layer: 0, sketch: 0, construction: false, group: None, meta: Default::default() })).unwrap();
    let kind = FeatureKind::Extrude { profile: vec![1], distance: distance.into(), offset: "0".into(), taper: "0 deg".into(), op: BodyOp::Add, flip: false, target: Target::Auto };
    d.apply(Command::AddFeature { index: 0, feature: Feature { id: 1, name: "e".into(), suppressed: false, kind } }).unwrap();
    d
}

/// Polls until a result for `revision` arrives; every result seen on the way is returned.
fn wait_for(w: &RebuildWorker, revision: u64) -> Vec<(u64, RebuildResult)> {
    let end = Instant::now() + Duration::from_secs(20);
    let mut seen = Vec::new();
    while Instant::now() < end {
        if let Some(r) = w.poll() {
            let done = r.0 == revision;
            seen.push(r);
            if done {
                return seen;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("no result for revision {revision} within 20 s");
}

#[test]
fn rebuilds_off_thread() {
    let w = RebuildWorker::spawn(egui::Context::default());
    let d = doc_with_extrude("10 mm");
    w.request(d.revision(), d.clone());
    let seen = wait_for(&w, d.revision());
    let (rev, r) = seen.last().unwrap();
    assert_eq!(*rev, d.revision());
    assert!((r.total_volume() - 8000.0).abs() < 1e-6 * 8000.0);
}

#[test]
fn newest_request_wins() {
    let w = RebuildWorker::spawn(egui::Context::default());
    let mut d = doc_with_extrude("10 mm");
    w.request(d.revision(), d.clone());
    let mut f = d.feature(1).unwrap().clone();
    if let FeatureKind::Extrude { distance, .. } = &mut f.kind {
        *distance = "20 mm".into();
    }
    d.apply(Command::SetFeature(f)).unwrap();
    w.request(d.revision(), d.clone());
    let seen = wait_for(&w, d.revision());
    let (_, r) = seen.last().unwrap();
    assert!((r.total_volume() - 16000.0).abs() < 1e-6 * 16000.0);
}

#[test]
fn reset_then_request_still_answers() {
    let w = RebuildWorker::spawn(egui::Context::default());
    let d = doc_with_extrude("10 mm");
    w.request(d.revision(), d.clone());
    w.reset();
    // A new document starts counting revisions again; the old result must not be taken for it.
    let d2 = doc_with_extrude("5 mm");
    w.request(d2.revision(), d2.clone());
    let seen = wait_for(&w, d2.revision());
    let (_, r) = seen.last().unwrap();
    assert!((r.total_volume() - 4000.0).abs() < 1e-6 * 4000.0);
}

#[test]
fn many_requests_end_on_the_last() {
    let w = RebuildWorker::spawn(egui::Context::default());
    let mut d = doc_with_extrude("1 mm");
    w.request(d.revision(), d.clone());
    for n in 2..=20 {
        let mut f = d.feature(1).unwrap().clone();
        if let FeatureKind::Extrude { distance, .. } = &mut f.kind {
            *distance = format!("{n} mm");
        }
        d.apply(Command::SetFeature(f)).unwrap();
        w.request(d.revision(), d.clone());
    }
    let seen = wait_for(&w, d.revision());
    let (_, r) = seen.last().unwrap();
    assert!((r.total_volume() - 16000.0).abs() < 1e-6 * 16000.0);
}
