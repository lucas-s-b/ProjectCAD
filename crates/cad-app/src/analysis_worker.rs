//! Runs one analysis at a time (static study, motion study, interference check, validation benchmarks) on a thread
//! of its own, so a long run never holds the UI thread. A job carries the document revision it read; the app drops a
//! job whose document has changed since (cancel on edit) and hands a finished one to its panel, which stores it with
//! that revision like the synchronous code did. The analysis mesh stays on the UI thread: batch B measured it far
//! under the 100 ms cut-off (docs/BENCHMARKS.md).
use cad_document::{Benchmark, Check, StudyReport};
use eframe::egui;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Instant;

/// What a job computes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Study,
    Motion,
    Check,
    Validation,
}

impl JobKind {
    /// "static study", "motion study", "interference check", "benchmarks": the progress line and MCP `jobs.running`.
    pub fn name(self) -> &'static str {
        match self {
            JobKind::Study => "static study",
            JobKind::Motion => "motion study",
            JobKind::Check => "interference check",
            JobKind::Validation => "benchmarks",
        }
    }
}

/// The output of a finished job: what the panel's synchronous code computed before.
pub enum JobOutput {
    Study(Result<StudyReport, String>),
    Motion(Result<crate::motion_panel::StudyRun, String>),
    Check(Result<Check, String>),
    /// Milliseconds and rows of `cad_document::benchmarks`.
    Validation(f64, Result<Vec<Benchmark>, String>),
}

/// A job's output, or the panic message when it panicked.
pub type Outcome = Result<JobOutput, String>;

struct Running {
    kind: JobKind,
    /// The document revision the job read; None for a job that does not read the document (validation).
    revision: Option<u64>,
    started: Instant,
    rx: Receiver<Outcome>,
}

/// The analysis job in progress, and counters for MCP `jobs`.
#[derive(Default)]
pub struct Jobs {
    running: Option<Running>,
    /// Jobs whose output was handed over.
    pub finished: u64,
    /// Jobs dropped before they finished (an edit, a new or opened document, "Cancel job", or a newer job).
    pub cancelled: u64,
}

impl Jobs {
    /// Runs `work` on a new thread; a running job is cancelled first. `revision` is the document revision `work`
    /// reads (None: independent of the document).
    pub fn start(&mut self, ctx: &egui::Context, kind: JobKind, revision: Option<u64>, work: impl FnOnce() -> JobOutput + Send + 'static) {
        self.cancel();
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).map_err(|p| cad_document::panic_message(p.as_ref()));
            // The receiver is gone when the job was cancelled.
            if tx.send(out).is_ok() {
                ctx.request_repaint();
            }
        });
        self.running = Some(Running { kind, revision, started: Instant::now(), rx });
    }

    /// The running job and the seconds since it started.
    pub fn running(&self) -> Option<(JobKind, f64)> {
        self.running.as_ref().map(|r| (r.kind, r.started.elapsed().as_secs_f64()))
    }

    /// Forgets the running job; its output is dropped when it arrives.
    // shortcut: the thread runs on to the end of its job (the mesher, solver and booleans have no stop points; MAX_CELLS, MAX_DOFS and MAX_STEPS bound it); pass a stop flag into them if cancelled runs ever cost too much.
    pub fn cancel(&mut self) {
        if self.running.take().is_some() {
            self.cancelled += 1;
        }
    }

    /// Cancels a job that read another document revision than `revision`.
    pub fn drop_stale(&mut self, revision: u64) {
        if self.running.as_ref().is_some_and(|r| r.revision.is_some_and(|rev| rev != revision)) {
            self.cancel();
        }
    }

    /// The finished job's kind and outcome, once.
    pub fn poll(&mut self) -> Option<(JobKind, Outcome)> {
        let r = self.running.as_ref()?;
        let out = match r.rx.try_recv() {
            Ok(out) => out,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("the job ended without a result".into()),
        };
        let kind = r.kind;
        self.running = None;
        self.finished += 1;
        Some((kind, out))
    }
}

#[cfg(test)]
#[path = "analysis_worker_tests.rs"]
mod tests;
