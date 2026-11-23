use super::*;
use std::time::Duration;

/// Polls every 5 ms for at most 10 s.
fn wait(jobs: &mut Jobs) -> Option<(JobKind, Outcome)> {
    for _ in 0..2000 {
        if let Some(done) = jobs.poll() {
            return Some(done);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

fn output() -> JobOutput {
    JobOutput::Validation(1.5, Ok(Vec::new()))
}

#[test]
fn a_job_hands_back_its_output() {
    let ctx = egui::Context::default();
    let mut jobs = Jobs::default();
    jobs.start(&ctx, JobKind::Validation, Some(7), output);
    assert!(matches!(jobs.running(), Some((JobKind::Validation, _))));
    let (kind, out) = wait(&mut jobs).expect("the job finishes");
    assert_eq!(kind, JobKind::Validation);
    assert!(matches!(out, Ok(JobOutput::Validation(ms, Ok(rows))) if ms == 1.5 && rows.is_empty()));
    assert_eq!((jobs.finished, jobs.cancelled), (1, 0));
    assert!(jobs.running().is_none());
    assert!(jobs.poll().is_none());
}

#[test]
fn an_edit_drops_the_job() {
    let ctx = egui::Context::default();
    let mut jobs = Jobs::default();
    let (go, gate) = mpsc::channel::<()>();
    jobs.start(&ctx, JobKind::Validation, Some(7), move || {
        gate.recv().ok();
        output()
    });
    jobs.drop_stale(7);
    assert!(jobs.running().is_some());
    jobs.drop_stale(8);
    assert_eq!(jobs.cancelled, 1);
    assert!(jobs.running().is_none());
    go.send(()).ok();
    std::thread::sleep(Duration::from_millis(50));
    assert!(jobs.poll().is_none());
    assert_eq!(jobs.finished, 0);
}

#[test]
fn document_independent_jobs_survive_edits() {
    let ctx = egui::Context::default();
    let mut jobs = Jobs::default();
    let (go, gate) = mpsc::channel::<()>();
    jobs.start(&ctx, JobKind::Validation, None, move || {
        gate.recv().ok();
        output()
    });
    jobs.drop_stale(8);
    assert!(jobs.running().is_some());
    go.send(()).ok();
    assert!(matches!(wait(&mut jobs), Some((JobKind::Validation, Ok(_)))));
}

#[test]
fn a_new_job_replaces_the_running_one() {
    let ctx = egui::Context::default();
    let mut jobs = Jobs::default();
    let (go, gate) = mpsc::channel::<()>();
    jobs.start(&ctx, JobKind::Validation, None, move || {
        gate.recv().ok();
        JobOutput::Validation(1.0, Ok(Vec::new()))
    });
    jobs.start(&ctx, JobKind::Validation, None, || JobOutput::Validation(2.0, Ok(Vec::new())));
    assert_eq!(jobs.cancelled, 1);
    go.send(()).ok();
    assert!(matches!(wait(&mut jobs), Some((_, Ok(JobOutput::Validation(ms, _)))) if ms == 2.0));
}

#[test]
fn a_panicking_job_reports_its_message() {
    let ctx = egui::Context::default();
    let mut jobs = Jobs::default();
    jobs.start(&ctx, JobKind::Validation, None, || panic!("boom"));
    let (_, out) = wait(&mut jobs).expect("the job ends");
    assert!(matches!(out, Err(msg) if msg.contains("boom")));
}
