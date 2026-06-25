//! File task state machine with progress throttling.
//! Mirrors FileTask from original feiq.

use crate::protocol::types::*;
use std::sync::{Arc, Mutex};

/// Progress notification throttle: at least 100KB or 1% change
const MIN_PROGRESS_BYTES: i64 = 102_400;
const MIN_PROGRESS_PCT_DIVISOR: i64 = 100;

/// A file transfer task with async-safe state management
pub struct FileTaskHandle {
    inner: Arc<Mutex<FileTask>>,
    last_notified_progress: Arc<Mutex<i64>>,
}

impl FileTaskHandle {
    /// Create a new file task
    pub fn new(
        id: u64,
        fellow_ip: String,
        fellow_name: String,
        content: FileContent,
        task_type: FileTaskType,
    ) -> Self {
        let total = content.size;
        Self {
            inner: Arc::new(Mutex::new(FileTask {
                id,
                fellow_ip,
                fellow_name,
                content,
                task_type,
                state: FileTaskState::NotStart,
                progress: 0,
                total,
                cancel_pending: false,
            })),
            last_notified_progress: Arc::new(Mutex::new(0)),
        }
    }

    /// Get the current task state (snapshot)
    pub fn snapshot(&self) -> FileTask {
        self.inner.lock().unwrap().clone()
    }

    /// Set the task to Running state
    pub fn set_running(&self) {
        let mut task = self.inner.lock().unwrap();
        task.state = FileTaskState::Running;
    }

    /// Update progress. Returns true if should notify frontend (throttled).
    pub fn update_progress(&self, progress: i64) -> bool {
        let mut task = self.inner.lock().unwrap();
        task.progress = progress;

        let mut last = self.last_notified_progress.lock().unwrap();
        let delta = progress - *last;
        let pct_delta = progress / std::cmp::max(task.total / MIN_PROGRESS_PCT_DIVISOR, 1)
            - *last / std::cmp::max(task.total / MIN_PROGRESS_PCT_DIVISOR, 1);

        if delta >= MIN_PROGRESS_BYTES || pct_delta >= 1 || progress >= task.total {
            *last = progress;
            true
        } else {
            false
        }
    }

    /// Mark task as finished
    pub fn set_finish(&self) {
        let mut task = self.inner.lock().unwrap();
        task.progress = task.total;
        task.state = FileTaskState::Finish;
    }

    /// Mark task as error
    pub fn set_error(&self, msg: String) {
        let mut task = self.inner.lock().unwrap();
        task.state = FileTaskState::Error(msg);
    }

    /// Mark task as canceled
    pub fn set_canceled(&self) {
        let mut task = self.inner.lock().unwrap();
        task.state = FileTaskState::Canceled;
    }

    /// Request cancellation (async-safe flag)
    pub fn request_cancel(&self) {
        let mut task = self.inner.lock().unwrap();
        task.cancel_pending = true;
    }

    /// Check if cancellation was requested
    pub fn is_cancel_pending(&self) -> bool {
        self.inner.lock().unwrap().cancel_pending
    }
}
