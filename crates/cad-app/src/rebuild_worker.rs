//! Runs the solid rebuild on its own thread so a slow model never blocks the UI.
use cad_document::{Document, RebuildResult, Rebuilder};
use eframe::egui;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};

enum Msg {
    Rebuild { epoch: u64, revision: u64, seq: u64, doc: Box<Document> },
    Reset,
}

type Finished = (u64, u64, RebuildResult);

pub struct RebuildWorker {
    tx: Sender<Msg>,
    rx: Receiver<Finished>,
    /// Bumped by `reset`; results of an older epoch belong to a replaced document and are dropped.
    epoch: AtomicU64,
    /// Number of the newest request or reset; a running rebuild with an older number stops.
    latest: Arc<AtomicU64>,
    _thread: std::thread::JoinHandle<()>,
}

impl RebuildWorker {
    pub fn spawn(ctx: egui::Context) -> Self {
        let (tx, jobs) = mpsc::channel::<Msg>();
        let (done, rx) = mpsc::channel::<Finished>();
        let latest = Arc::new(AtomicU64::new(0));
        let seen = latest.clone();
        let thread = std::thread::spawn(move || run(&ctx, &jobs, &done, &seen));
        Self { tx, rx, epoch: AtomicU64::new(0), latest, _thread: thread }
    }

    pub fn request(&self, revision: u64, doc: Document) {
        let epoch = self.epoch.load(Ordering::Relaxed);
        let seq = self.latest.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.tx.send(Msg::Rebuild { epoch, revision, seq, doc: Box::new(doc) });
    }

    /// The newest finished result, if any.
    pub fn poll(&self) -> Option<(u64, RebuildResult)> {
        let epoch = self.epoch.load(Ordering::Relaxed);
        self.rx.try_iter().filter(|(e, ..)| *e == epoch).last().map(|(_, rev, r)| (rev, r))
    }

    /// Drops the cache; call when the document is replaced.
    pub fn reset(&self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
        self.latest.fetch_add(1, Ordering::SeqCst);
        let _ = self.tx.send(Msg::Reset);
    }
}

fn run(ctx: &egui::Context, jobs: &Receiver<Msg>, done: &Sender<Finished>, latest: &AtomicU64) {
    let mut rebuilder = Rebuilder::default();
    // Blocks on the channel; ends when the sender is dropped.
    while let Ok(first) = jobs.recv() {
        let mut newest = None;
        for msg in std::iter::once(first).chain(jobs.try_iter()) {
            match msg {
                // Older queued requests are stale.
                Msg::Rebuild { epoch, revision, seq, doc } => newest = Some((epoch, revision, seq, doc)),
                Msg::Reset => {
                    rebuilder = Rebuilder::default();
                    newest = None;
                }
            }
        }
        if let Some((epoch, revision, seq, doc)) = newest {
            let stop = || latest.load(Ordering::SeqCst) != seq;
            let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rebuilder.rebuild_until(&doc, &stop))) {
                Ok(Some(r)) => r,
                // A newer request is already queued; it is picked up by the next `recv`.
                Ok(None) => continue,
                Err(p) => {
                    // The cache may be half written: start clean and report every feature as failed.
                    rebuilder = Rebuilder::default();
                    let msg = format!("internal error during rebuild: {}", cad_document::panic_message(p.as_ref()));
                    let states = doc.features().iter().map(|f| (f.id, cad_document::FeatureState::Error(msg.clone()))).collect();
                    RebuildResult { bodies: Vec::new(), states, recomputed: 0, depends: Default::default(), planes: Default::default(), walls: Vec::new(), rooms: Vec::new(), components: Vec::new(), joint_values: Default::default() }
                }
            };
            if done.send((epoch, revision, result)).is_err() {
                return;
            }
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
#[path = "rebuild_worker_tests.rs"]
mod tests;
