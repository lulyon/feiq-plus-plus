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

    /// Mark task as finished (no-op if already in a terminal state)
    pub fn set_finish(&self) {
        let mut task = self.inner.lock().unwrap();
        if matches!(
            task.state,
            FileTaskState::Finish | FileTaskState::Canceled | FileTaskState::Error(_)
        ) {
            return;
        }
        task.progress = task.total;
        task.state = FileTaskState::Finish;
    }

    /// Mark task as error (no-op if already in a terminal state)
    pub fn set_error(&self, msg: String) {
        let mut task = self.inner.lock().unwrap();
        if matches!(
            task.state,
            FileTaskState::Finish | FileTaskState::Canceled | FileTaskState::Error(_)
        ) {
            return;
        }
        task.state = FileTaskState::Error(msg);
    }

    /// Mark task as canceled (no-op if already in Finish or Error state)
    pub fn set_canceled(&self) {
        let mut task = self.inner.lock().unwrap();
        if matches!(task.state, FileTaskState::Finish | FileTaskState::Error(_)) {
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_handle(id: u64, total: i64) -> FileTaskHandle {
        FileTaskHandle::new(
            id,
            "192.168.1.1".into(),
            "TestUser".into(),
            FileContent {
                file_id: id,
                filename: format!("file_{}.bin", id),
                path: String::new(),
                size: total,
                modify_time: 0,
                file_type: 0,
                packet_no: 0,
                local_task_id: None,
            },
            FileTaskType::Download,
        )
    }

    // --- new task ---

    #[test]
    fn test_new_task_not_start_state() {
        let handle = make_handle(1, 1_000_000);
        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::NotStart,
            "new task must be in NotStart state");
    }

    #[test]
    fn test_new_task_total_from_content() {
        let handle = make_handle(2, 5_242_880);
        let snap = handle.snapshot();
        assert_eq!(snap.total, 5_242_880,
            "total must equal content.size");
    }

    #[test]
    fn test_new_task_zero_total() {
        // Edge case: zero-byte file
        let handle = make_handle(3, 0);
        let snap = handle.snapshot();
        assert_eq!(snap.total, 0);
        assert_eq!(snap.state, FileTaskState::NotStart);
    }

    // --- set_running ---

    #[test]
    fn test_set_running() {
        let handle = make_handle(10, 1_000_000);
        handle.set_running();
        assert_eq!(handle.snapshot().state, FileTaskState::Running);
    }

    // --- update_progress ---

    #[test]
    fn test_update_progress_throttled() {
        // File large enough that 1% exceeds 100KB.
        // total = 20MB = 20_971_520 bytes, 1% = 209_715 bytes.
        let total = 20_971_520i64;
        let handle = make_handle(20, total);

        // First call always notifies (last_notified_progress starts at 0,
        // and delta >= 0 implicitly). Reset last_notified by calling once
        // with a value that triggers.
        assert!(handle.update_progress(209_715), "first call notifies");
        // Now last = 209_715.

        // Advance by 50_000 bytes (below both 100KB and 1%).
        assert!(!handle.update_progress(259_715), "should be throttled: delta=50_000 < 100KB, pct < 1%");
    }

    #[test]
    fn test_update_progress_byte_threshold() {
        // Use a huge file so 1% >> 100KB, isolating the byte threshold.
        let total = 1_000_000_000i64; // 1 GB — 1% = 10 MB
        let handle = make_handle(30, total);

        // last_notified_progress starts at 0, so delta = 102_400 >= 102_400.
        assert!(handle.update_progress(102_400), "byte threshold must trigger notification");
    }

    #[test]
    fn test_update_progress_pct_threshold() {
        // Use a file where 1% is 500 bytes, well below 100KB.
        let total = 50_000i64; // 1% = 500
        let handle = make_handle(40, total);

        // last_notified_progress starts at 0.
        // delta = 500 (< 102400), pct_delta = 500/500 - 0/500 = 1 >= 1.
        assert!(handle.update_progress(500), "1% pct threshold must trigger notification");
        // last = 500.

        // Advance by 490 bytes (0% bucket change, byte delta << 100KB).
        // 990 / 500 = 1 bucket, 500 / 500 = 1 bucket: pct_delta = 1 - 1 = 0.
        // delta = 490 < 102400.
        assert!(!handle.update_progress(990), "should be throttled: pct bucket unchanged, byte delta < 100KB");
    }

    #[test]
    fn test_update_progress_at_completion() {
        let total = 10_000_000i64;
        let handle = make_handle(50, total);

        // Set progress somewhere in the middle.
        assert!(handle.update_progress(1_000_000), "first call notifies");
        // Even if delta is tiny, reaching total must notify.
        assert!(handle.update_progress(total),
            "progress == total must always notify");
    }

    #[test]
    fn test_update_progress_small_file() {
        // A file small enough that even its final update would be below
        // the byte threshold — but reaching total must still notify.
        let total = 128i64;
        let handle = make_handle(60, total);

        // Partial update that doesn't hit completion.
        // total/100 = 1, max(1,1) = 1.
        // pct_delta = 50/1 - 0/1 = 50 >= 1 → notifies.
        assert!(handle.update_progress(50), "small file partial progress notifies via pct");

        // Jump straight to total — must notify.
        assert!(handle.update_progress(total),
            "small file at total must notify");
    }

    // --- set_finish ---

    #[test]
    fn test_set_finish() {
        let handle = make_handle(70, 500_000);
        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::NotStart);

        handle.set_finish();
        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::Finish,
            "set_finish must transition to Finish state");
        assert_eq!(snap.progress, snap.total,
            "set_finish must set progress == total");
    }

    // --- set_error ---

    #[test]
    fn test_set_error() {
        let handle = make_handle(80, 200_000);
        let msg = "disk full".to_string();
        handle.set_error(msg.clone());

        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::Error(msg),
            "set_error must preserve the error message");
    }

    #[test]
    fn test_set_error_empty_message() {
        let handle = make_handle(81, 100);
        handle.set_error(String::new());
        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::Error(String::new()));
    }

    // --- set_canceled ---

    #[test]
    fn test_set_canceled() {
        let handle = make_handle(90, 300_000);
        handle.set_canceled();

        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::Canceled);
    }

    // --- request_cancel ---

    #[test]
    fn test_request_cancel_and_is_cancel_pending() {
        let handle = make_handle(100, 1_000_000);

        // Initially false.
        assert!(!handle.is_cancel_pending(),
            "fresh task must not have cancel_pending");

        handle.request_cancel();
        assert!(handle.is_cancel_pending(),
            "after request_cancel, is_cancel_pending must be true");

        // Other state is unchanged.
        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::NotStart,
            "request_cancel must not change task state");
    }

    // --- terminal state guards (no overwrite from in-flight I/O) ---

    #[test]
    fn test_terminal_guards_cancel_then_finish() {
        // Canceled is terminal — set_finish must be a no-op.
        let handle = make_handle(110, 1_000_000);
        handle.set_canceled();
        handle.set_finish();
        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::Canceled,
            "set_finish after set_canceled must be a no-op");
        assert_eq!(snap.progress, 0,
            "set_finish (no-op) must not snap progress to total");
    }

    #[test]
    fn test_terminal_guards_finish_then_cancel() {
        // Finish is terminal — set_canceled must be a no-op.
        let handle = make_handle(111, 1_000_000);
        handle.set_finish();
        handle.set_canceled();
        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::Finish,
            "set_canceled after set_finish must be a no-op");
        assert_eq!(snap.progress, snap.total,
            "finish state still has correct progress");
    }

    #[test]
    fn test_terminal_guards_error_then_cancel() {
        // Error is terminal — set_canceled must be a no-op.
        let handle = make_handle(120, 1_000_000);
        handle.set_error("connection lost".into());
        handle.set_canceled();
        assert_eq!(handle.snapshot().state, FileTaskState::Error("connection lost".into()),
            "set_canceled after set_error must be a no-op");
    }

    #[test]
    fn test_terminal_guards_cancel_then_error() {
        // Canceled is terminal — set_error must be a no-op.
        let handle = make_handle(121, 1_000_000);
        handle.set_canceled();
        handle.set_error("timeout".into());
        assert_eq!(handle.snapshot().state, FileTaskState::Canceled,
            "set_error after set_canceled must be a no-op");
    }

    #[test]
    fn test_terminal_guards_finish_then_error() {
        // Finish is terminal — set_error must be a no-op.
        let handle = make_handle(122, 1_000_000);
        handle.set_finish();
        handle.set_error("oops".into());
        assert_eq!(handle.snapshot().state, FileTaskState::Finish,
            "set_error after set_finish must be a no-op");
    }

    #[test]
    fn test_terminal_guards_error_then_finish() {
        // Error is terminal — set_finish must be a no-op.
        let handle = make_handle(123, 1_000_000);
        handle.set_error("disk full".into());
        handle.set_finish();
        assert_eq!(handle.snapshot().state, FileTaskState::Error("disk full".into()),
            "set_finish after set_error must be a no-op");
    }
}
