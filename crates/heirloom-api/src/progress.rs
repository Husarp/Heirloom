//! How far a long job (a backup .zip, the write-back) has got. The job runs under the command lock; the window reads
//! this without it (`job.progress`, `job.cancel`), so the busy dialog can count files and offer „Przerwij”.

use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct State {
    running: bool,
    label: String,
    done: u64,
    total: u64,
    cancel: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Progress(Arc<Mutex<State>>);

impl Progress {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn start(&self, label: &str, total: u64) {
        *self.state() = State { running: true, label: label.to_string(), done: 0, total, cancel: false };
    }

    /// Records progress; false when the user asked to stop.
    pub fn step(&self, done: u64) -> bool {
        let mut state = self.state();
        state.done = done;
        !state.cancel
    }

    pub fn finish(&self) {
        let mut state = self.state();
        state.running = false;
        state.cancel = false;
    }

    pub fn cancel(&self) {
        let mut state = self.state();
        if state.running {
            state.cancel = true;
        }
    }

    /// `job.progress` and `job.cancel`.
    pub fn call(&self, method: &str) -> Value {
        if method == "job.cancel" {
            self.cancel();
        }
        let state = self.state();
        json!({ "running": state.running, "label": state.label, "done": state.done, "total": state.total, "cancelling": state.cancel })
    }
}
