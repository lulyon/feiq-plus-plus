//! File task state machine with progress throttling.
//! Mirrors FileTask from original feiq.

use crate::protocol::types::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Progress notification throttle: at least 100KB or 1% change
const MIN_PROGRESS_BYTES: i64 = 102_400;
const MIN_PROGRESS_PCT_DIVISOR: i64 = 100;

/// A file transfer task with async-safe state management
pub struct FileTaskHandle {
    inner: Arc<Mutex<FileTask>>,
    last_notified_progress: Arc<Mutex<i64>>,
    /// Atomic flag for cancelling in-flight I/O (checked by send_file/recv_file loops)
    pub cancel_flag: Arc<AtomicBool>,
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
                terminal_at: None,
            })),
            last_notified_progress: Arc::new(Mutex::new(0)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the current task state (snapshot)
    pub fn snapshot(&self) -> FileTask {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Set the task to Running state (no-op if already in a terminal state)
    pub fn set_running(&self) {
        let mut task = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(
            task.state,
            FileTaskState::Finish | FileTaskState::Canceled | FileTaskState::Error(_)
        ) {
            return;
        }
        task.state = FileTaskState::Running;
    }

    /// Update progress. Returns true if should notify frontend (throttled).
    pub fn update_progress(&self, progress: i64) -> bool {
        let mut task = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        task.progress = progress;

        let mut last = self.last_notified_progress.lock().unwrap_or_else(|e| e.into_inner());
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

    /// Record current time as the terminal timestamp (for cleanup).
    fn set_terminal_at(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let mut task = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        task.terminal_at = Some(now);
    }

    /// Mark task as finished (no-op if already in a terminal state)
    pub fn set_finish(&self) {
        {
            let mut task = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if matches!(
                task.state,
                FileTaskState::Finish | FileTaskState::Canceled | FileTaskState::Error(_)
            ) {
                return;
            }
            task.progress = task.total;
            task.state = FileTaskState::Finish;
        } // drop lock before calling set_terminal_at
        self.set_terminal_at();
    }

    /// Mark task as error (no-op if already in a terminal state)
    pub fn set_error(&self, msg: String) {
        {
            let mut task = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if matches!(
                task.state,
                FileTaskState::Finish | FileTaskState::Canceled | FileTaskState::Error(_)
            ) {
                return;
            }
            task.state = FileTaskState::Error(msg);
        } // drop lock before calling set_terminal_at
        self.set_terminal_at();
    }

    /// Mark task as canceled (no-op if already in a terminal state)
    pub fn set_canceled(&self) {
        {
            let mut task = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if matches!(
                task.state,
                FileTaskState::Finish
                    | FileTaskState::Canceled
                    | FileTaskState::Error(_)
            ) {
                return;
            }
            task.state = FileTaskState::Canceled;
        } // drop lock before calling set_terminal_at
        self.set_terminal_at();
    }

    /// Set terminal_at to a specific Unix timestamp (test-only, for cleanup verification).
    #[cfg(test)]
    pub fn set_terminal_at_for_test(&self, ts: i64) {
        let mut task = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        task.terminal_at = Some(ts);
    }

    /// Request cancellation (async-safe flag).
    /// Only sets the flag — the transfer loop transitions state via set_canceled().
    pub fn request_cancel(&self) {
        let mut task = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        task.cancel_pending = true;
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Check if cancellation was requested
    pub fn is_cancel_pending(&self) -> bool {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).cancel_pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

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

    // --- progress event emission at byte boundaries ---

    #[test]
    fn test_update_progress_zero_delta() {
        // Progress == last_notified (both 0). No threshold met.
        let total = 1_000_000i64;
        let handle = make_handle(1000, total);
        assert!(
            !handle.update_progress(0),
            "zero delta must not notify"
        );
    }

    #[test]
    fn test_update_progress_byte_ceiling() {
        // MIN_PROGRESS_BYTES = 102400.
        // Set last=102400, then delta=102399 is below threshold, delta=102400 triggers.
        let total = 1_000_000_000i64; // 1% = 10MB >> 100KB, isolates byte check
        let handle = make_handle(1001, total);
        assert!(handle.update_progress(102_400), "initial byte threshold notifies");
        assert!(
            !handle.update_progress(204_799),
            "delta=102399 < 102400 must not notify"
        );
        assert!(
            handle.update_progress(204_800),
            "delta=102400 == MIN_PROGRESS_BYTES must notify"
        );
    }

    #[test]
    fn test_update_progress_byte_overflow() {
        // Jump from 0 to i64::MAX. delta >> 102400, must notify.
        let total = i64::MAX;
        let handle = make_handle(1002, total);
        assert!(
            handle.update_progress(total),
            "huge delta must notify via byte threshold"
        );
    }

    #[test]
    fn test_update_progress_byte_repeated_102400() {
        // Consecutive calls each advancing exactly 102400 — each must notify.
        let total = 1_073_741_824i64; // 1 GiB
        let handle = make_handle(1003, total);
        assert!(handle.update_progress(102_400), "first 102400 notifies");
        assert!(handle.update_progress(204_800), "second 102400 notifies");
        assert!(handle.update_progress(307_200), "third 102400 notifies");
    }

    // --- progress event emission at percentage boundaries ---

    #[test]
    fn test_update_progress_pct_boundary_exact() {
        // 1% of total = 500 bytes. Crossing bucket N to N+1 must notify.
        let total = 50_000i64;
        let handle = make_handle(1010, total);
        // last=0, progress=500: pct_delta=500/500-0/500=1 >= 1, notifies
        assert!(handle.update_progress(500), "first 1% boundary notifies");
        // last=500, progress=999: pct_delta=999/500-500/500=1-1=0, delta=499
        assert!(!handle.update_progress(999), "within same bucket must not notify");
        // progress=1000: pct_delta=1000/500-500/500=2-1=1, delta=1
        assert!(handle.update_progress(1000), "next 1% boundary notifies");
    }

    #[test]
    fn test_update_progress_pct_back_to_back() {
        // Two consecutive 1% jumps from properly set last, both trigger.
        let total = 50_000i64;
        let handle = make_handle(1011, total);
        // last=0, progress=1000: pct_delta=1000/500-0/500=2 >= 1, notifies
        assert!(handle.update_progress(1000), "2% crosses notify");
        // last=1000, progress=1500: pct_delta=1500/500-1000/500=3-2=1 >= 1, notifies
        assert!(handle.update_progress(1500), "3% crosses notify");
    }

    #[test]
    fn test_update_progress_pct_edge_at_99_pct() {
        // Crossing from 99% bucket into 100% bucket must notify.
        let total = 10_000_000i64;
        let _divisor = total / 100; // 100_000
        let handle = make_handle(1012, total);
        // last=0, set last to bucket 99 entry: 99 * 100_000 = 9_900_000
        assert!(handle.update_progress(9_900_000), "99% entry notifies");
        // last=9_900_000 in bucket 99: 9_900_000/100_000=99
        // progress=9_999_999: pct_delta=99-99=0, delta=99_999 < 102400
        assert!(!handle.update_progress(9_999_999), "within 99% bucket throttled");
        // progress=10_000_000: pct_delta=100-99=1 >= 1, delta=1
        assert!(handle.update_progress(10_000_000),
            "crossing 99% to 100% bucket must notify via pct");
    }

    #[test]
    fn test_update_progress_pct_small_bucket_many_steps() {
        // With small total, percent bucket is 1 byte (divisor clamped to 1).
        // Every progress change triggers via pct: pct_delta = progress - last.
        // progress=1 reaches total (completion check), progress=2 exceeds total.
        let total = 1i64;
        let handle = make_handle(1013, total);
        // divisor = max(0, 1) = 1
        // progress=1: delta=1, pct_delta=1-0=1 >= 1, notifies
        assert!(handle.update_progress(1), "progress=1 reaches total, notifies");
        // last=1, progress=2: delta=1, pct_delta=2-1=1 >= 1, notifies
        assert!(handle.update_progress(2), "progress=2 exceeds total, notifies via pct");
    }

    // --- completion always fires ---

    #[test]
    fn test_update_progress_completion_from_zero() {
        // progress == total == 0. progress >= total is true.
        let handle = make_handle(1020, 0);
        assert!(
            handle.update_progress(0),
            "completion with zero total must notify"
        );
    }

    #[test]
    fn test_update_progress_completion_via_total_check() {
        // Only `progress >= task.total` triggers: delta < 102400 and pct_delta == 0.
        // total = 10_485_760 (10 MiB), total/100 = 104857.
        // Bucket 100 starts at 100 * 104857 = 10_485_700.
        // Set last = 10_485_700. Then progress = total = 10_485_760.
        // delta = 60 < 102400.  pct_delta = 100 - 100 = 0.  progress >= total -> true.
        let total = 10_485_760i64;
        let handle = make_handle(1021, total);
        assert!(handle.update_progress(10_485_700), "entry to bucket 100 notifies");
        assert!(
            handle.update_progress(total),
            "completion must notify when delta and pct_delta are both below thresholds"
        );
    }

    #[test]
    fn test_update_progress_completion_above_total() {
        // progress > total must also notify.
        let total = 1_000_000i64;
        let handle = make_handle(1022, total);
        assert!(handle.update_progress(1_500_000),
            "progress exceeding total must notify");
    }

    #[test]
    fn test_update_progress_completion_after_throttled() {
        // Completion fires even after a progress update that was throttled
        // (last_notified unchanged, so the delta from last_notified to total is large).
        let total = 10_000_000i64;
        let handle = make_handle(1023, total);
        // Set last=102400
        assert!(handle.update_progress(102_400), "initial notifies");
        // Small advance that does NOT update last_notified
        assert!(!handle.update_progress(152_399), "throttled (delta=49999, same pct bucket)");
        // Jump to total. last is still 102400, delta=9_847_600 >= 102400
        assert!(handle.update_progress(total),
            "completion after throttled step must notify");
        let snap = handle.snapshot();
        assert_eq!(snap.progress, total, "progress must be total after completion");
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

    // ─── Cancel during mid-transfer scenarios ───────────────────

    #[test]
    fn test_cancel_during_send_file_mid_chunk() {
        // Simulate: send_file is mid-chunk (Running, partial progress),
        // cancel is requested. request_cancel only sets the flag;
        // the transfer loop's next is_cancel_pending check triggers set_canceled.
        let handle = make_handle(1100, 1_000_000);
        handle.set_running();
        handle.update_progress(250_000); // mid-chunk

        handle.request_cancel();

        // Flag set, state still Running (no state change from request_cancel alone)
        assert!(handle.is_cancel_pending());
        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::Running,
            "request_cancel must not change state");
        assert_eq!(snap.progress, 250_000,
            "progress unchanged by request_cancel");

        // The transfer loop checks the flag and transitions
        if handle.is_cancel_pending() {
            handle.set_canceled();
        }
        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::Canceled,
            "flag→set_canceled must transition to Canceled");
        assert_eq!(snap.progress, 250_000,
            "Canceled mid-chunk preserves bytes-sent-so-far");
    }

    #[test]
    fn test_cancel_during_recv_file_mid_chunk() {
        // Same pattern as send_file mid-chunk but with Download type
        let handle = FileTaskHandle::new(
            1101,
            "192.168.1.2".into(),
            "RemoteUser".into(),
            FileContent {
                file_id: 1101,
                filename: "download.bin".into(),
                path: "/tmp/download.bin".into(),
                size: 2_000_000,
                modify_time: 0,
                file_type: 0,
                packet_no: 0,
                local_task_id: None,
            },
            FileTaskType::Download,
        );
        handle.set_running();
        handle.update_progress(750_000);

        handle.request_cancel();
        assert!(handle.is_cancel_pending());
        assert_eq!(handle.snapshot().state, FileTaskState::Running);

        // Transfer loop discovers cancel
        handle.set_canceled();
        assert_eq!(handle.snapshot().state, FileTaskState::Canceled);
        assert_eq!(handle.snapshot().progress, 750_000,
            "partial download preserved on cancel");
    }

    #[test]
    fn test_cancel_during_folder_transfer_between_files() {
        // Folder transfer: one file just finished, cancel is requested
        // before the next file starts. The transfer loop checks the flag
        // between files.
        let handle = make_handle(1102, 5_000_000);
        handle.set_running();

        // No cancel flag before file 2
        assert!(!handle.is_cancel_pending(), "between-file check: no cancel yet");

        // Cancel arrives between files
        handle.request_cancel();
        assert!(handle.is_cancel_pending(),
            "between-file check: cancel flag now set");

        // Transfer loop discovers flag and cancels (never starts file 2)
        handle.set_canceled();
        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::Canceled,
            "cancel between folder files must transition to Canceled");
    }

    #[test]
    fn test_cancel_before_transfer_starts() {
        // User cancels before the transfer loop even sets_running
        let handle = make_handle(1103, 1_000_000);
        assert_eq!(handle.snapshot().state, FileTaskState::NotStart);

        handle.request_cancel();
        assert!(handle.is_cancel_pending());

        // The transfer loop should check is_cancel_pending before calling
        // set_running, and go directly to set_canceled.
        if handle.is_cancel_pending() {
            handle.set_canceled();
        }
        assert_eq!(handle.snapshot().state, FileTaskState::Canceled,
            "cancel before start → Canceled directly");

        // set_running on a canceled task is a no-op (terminal guard)
        handle.set_running();
        assert_eq!(handle.snapshot().state, FileTaskState::Canceled,
            "set_running after cancel is a no-op");
    }

    #[test]
    fn test_double_cancel() {
        // Double cancel: first call transitions, second is a no-op
        let handle = make_handle(1104, 1_000_000);

        // First cancel (full path: request + set)
        handle.request_cancel();
        assert!(handle.is_cancel_pending());
        handle.set_canceled();
        assert_eq!(handle.snapshot().state, FileTaskState::Canceled);
        let first_terminal = handle.snapshot().terminal_at;

        // Second cancel: request_cancel is idempotent (flag stays true),
        // set_canceled is a no-op (state already Canceled)
        handle.request_cancel();
        assert!(handle.is_cancel_pending(), "double request_cancel still true");
        handle.set_canceled();
        assert_eq!(handle.snapshot().state, FileTaskState::Canceled,
            "double cancel must stay in Canceled");
        assert_eq!(handle.snapshot().terminal_at, first_terminal,
            "terminal_at must not change on second no-op cancel");
    }

    #[test]
    fn test_double_cancel_only_request() {
        // Call request_cancel twice without set_canceled in between
        let handle = make_handle(1105, 1_000_000);

        handle.request_cancel();
        assert!(handle.is_cancel_pending());

        handle.request_cancel(); // second request
        assert!(handle.is_cancel_pending(), "double request_cancel still true");
        assert_eq!(handle.snapshot().state, FileTaskState::NotStart,
            "state unchanged by double request_cancel");
    }

    #[test]
    fn test_cancel_with_progress_after_flag() {
        // Progress can still advance after request_cancel (the flag is advisory;
        // in-flight I/O continues until the loop checks the flag).
        let handle = make_handle(1106, 1_000_000);
        handle.set_running();

        handle.request_cancel();
        assert!(handle.is_cancel_pending());

        // More progress arrives after the cancel flag
        assert!(handle.update_progress(500_000),
            "progress updates must still work after cancel flag");
        let snap = handle.snapshot();
        assert_eq!(snap.progress, 500_000,
            "progress advances even after cancel flag");
        assert_eq!(snap.state, FileTaskState::Running,
            "state stays Running until transfer loop acts");

        // Eventually the loop checks
        handle.set_canceled();
        assert_eq!(handle.snapshot().state, FileTaskState::Canceled);
        assert_eq!(handle.snapshot().progress, 500_000);
    }

    #[test]
    fn test_cancel_during_concurrent_progress_updates() {
        // Two threads: one updating progress, one requesting cancel concurrently.
        // No panics, no corruption, correct final state after discovery.
        use std::sync::Arc;
        let total = 10_000_000i64;
        let handle = Arc::new(make_handle(1107, total));
        handle.set_running();

        let h1 = Arc::clone(&handle);
        let t1 = std::thread::spawn(move || {
            for i in 0..100 {
                h1.update_progress(i * 100_000);
                std::thread::sleep(std::time::Duration::from_micros(10));
            }
        });

        let h2 = Arc::clone(&handle);
        let t2 = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(5));
            h2.request_cancel();
        });

        t1.join().expect("progress thread panicked");
        t2.join().expect("cancel thread panicked");

        // After concurrent access: flag set, state still Running
        assert!(handle.is_cancel_pending(),
            "concurrent cancel must set flag");
        assert_eq!(handle.snapshot().state, FileTaskState::Running,
            "concurrent cancel leaves state as Running (set_canceled not called)");

        // Transfer loop discovers flag
        handle.set_canceled();
        assert_eq!(handle.snapshot().state, FileTaskState::Canceled);
        assert!(handle.snapshot().progress >= 0, "progress non-negative after concurrent cancel");
    }

    #[test]
    fn test_cancel_after_finish() {
        // Cancel after finish is a no-op (Finish is terminal)
        let handle = make_handle(1108, 1_000_000);
        handle.set_running();
        handle.update_progress(900_000);
        handle.set_finish();

        // Frontend calls cancel_file_task which does request + set
        handle.request_cancel();
        handle.set_canceled();

        let snap = handle.snapshot();
        assert_eq!(snap.state, FileTaskState::Finish,
            "cancel after finish must be a no-op");
        assert_eq!(snap.progress, snap.total,
            "finish progress preserved after no-op cancel");
        assert!(snap.terminal_at.is_some());
    }

    #[test]
    fn test_cancel_after_error() {
        // Cancel after error is a no-op (Error is terminal)
        let handle = make_handle(1109, 1_000_000);
        handle.set_running();
        handle.set_error("disk full".into());

        handle.request_cancel();
        handle.set_canceled();

        assert_eq!(handle.snapshot().state, FileTaskState::Error("disk full".into()),
            "cancel after error must be a no-op");
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

    // --- set_running on already-terminal task ---

    #[test]
    fn test_set_running_on_terminal_finish() {
        let h = make_handle(400, 1000);
        h.set_finish();
        h.set_running();
        assert_eq!(h.snapshot().state, FileTaskState::Finish,
            "set_running after set_finish must be a no-op");
        assert_eq!(h.snapshot().progress, 1000,
            "set_running after set_finish must not alter progress");
    }

    #[test]
    fn test_set_running_on_terminal_error() {
        let h = make_handle(401, 1000);
        h.set_error("disk full".into());
        h.set_running();
        assert_eq!(h.snapshot().state, FileTaskState::Error("disk full".into()),
            "set_running after set_error must be a no-op");
    }

    #[test]
    fn test_set_running_on_terminal_canceled() {
        let h = make_handle(402, 1000);
        h.set_canceled();
        h.set_running();
        assert_eq!(h.snapshot().state, FileTaskState::Canceled,
            "set_running after set_canceled must be a no-op");
    }

    #[test]
    fn test_set_running_not_start() {
        let h = make_handle(403, 1000);
        h.set_running();
        assert_eq!(h.snapshot().state, FileTaskState::Running,
            "set_running on NotStart must transition to Running");
    }

    #[test]
    fn test_set_running_already_running() {
        let h = make_handle(404, 1000);
        h.set_running();
        h.set_running();
        assert_eq!(h.snapshot().state, FileTaskState::Running,
            "set_running on Running must stay Running");
    }

    // --- update_progress with very large file sizes ---

    #[test]
    fn test_update_progress_large_file_no_overflow() {
        // total = i64::MAX ensures division in pct check does not overflow
        let total = i64::MAX;
        let h = make_handle(410, total);
        // Call with nonzero progress so delta triggers
        assert!(h.update_progress(102_400), "byte threshold (102400) notifies");
        // Tiny advance — throttled (delta=50, pct bucket unchanged at this scale)
        assert!(!h.update_progress(102_450), "tiny delta with huge file is throttled");
        // Jump to total always notifies
        assert!(h.update_progress(total), "progress == total must always notify");
        let snap = h.snapshot();
        assert_eq!(snap.progress, i64::MAX);
        assert_eq!(snap.total, i64::MAX);
    }

    #[test]
    fn test_update_progress_large_file_pct_triggers() {
        // Use a total large enough that 1% >> 100KB, isolating the pct path
        let total = 100_000_000_000i64; // 100 GB — 1% = 1 GB
        let h = make_handle(411, total);

        // First nonzero call notifies via byte threshold (delta >= 102400)
        assert!(h.update_progress(102_400), "byte threshold notifies");
        // Advance by 1% worth — must notify via pct threshold
        let one_pct = total / 100; // 1 GB
        assert!(h.update_progress(one_pct), "1% progress jump must notify via pct threshold");
        // Advance by 50KB — should be throttled (delta=51200 < 102400, pct bucket unchanged)
        assert!(!h.update_progress(one_pct + 51_200), "50KB advance with 100GB file is throttled");
    }

    #[test]
    fn test_update_progress_tiny_total_large_progress() {
        // total = 1, divisor = max(0, 1) = 1, pct calc = progress / 1
        let total = 1i64;
        let h = make_handle(412, total);
        // pct_delta = 50_000/1 - 0/1 = 50_000 >= 1 -> notifies
        assert!(h.update_progress(50_000), "large progress on tiny file must notify via pct");
        let snap = h.snapshot();
        assert_eq!(snap.progress, 50_000);
        // progress > total is allowed by the progress field (it's just bytes received)
        assert!(snap.progress > snap.total, "progress can exceed total for malformed files");
    }

    #[test]
    fn test_update_progress_negative_progress() {
        let total = 1_000_000i64;
        let h = make_handle(413, total);
        // Negative progress should not panic (delta = -0 = 0, pct_delta = 0)
        // This is a boundary test — no contract that negative is good, just must not panic.
        h.update_progress(-100);
        let snap = h.snapshot();
        assert_eq!(snap.progress, -100, "negative progress stored as-is");
    }

    // --- concurrent progress updates from multiple threads ---

    #[test]
    fn test_concurrent_progress_updates() {
        let total = 1_000_000_000i64;
        let handle = Arc::new(make_handle(500, total));
        let thread_count = 10;
        let updates_per_thread = 100;
        let mut threads = vec![];

        for i in 0..thread_count {
            let h = Arc::clone(&handle);
            threads.push(std::thread::spawn(move || {
                for j in 0..updates_per_thread {
                    let progress = i as i64 * 100_000_000 + j as i64 * 1_000_000;
                    if progress <= total {
                        h.update_progress(progress);
                    }
                }
            }));
        }

        for t in threads {
            t.join().expect("thread panicked");
        }

        let snap = handle.snapshot();
        assert!(snap.progress >= 0, "progress must not be corrupted");
        assert!(snap.progress <= total, "progress must not exceed total");
        // Each thread sets values in expected ranges; no lock corruption
        assert_eq!(snap.id, 500, "id must remain unchanged");
        assert_eq!(snap.total, total, "total must remain unchanged");
    }

    #[test]
    fn test_concurrent_progress_and_state_mutations() {
        // Mix update_progress, set_running, set_finish, set_error from many threads
        let total = 10_000_000i64;
        let handle = Arc::new(make_handle(510, total));
        let mut threads = vec![];

        // Threads that just update progress
        for i in 0..8 {
            let h = Arc::clone(&handle);
            threads.push(std::thread::spawn(move || {
                for j in 0..50 {
                    h.update_progress(i as i64 * 1_250_000 + j as i64 * 25_000);
                }
            }));
        }

        // Thread that races to finish
        let h = Arc::clone(&handle);
        threads.push(std::thread::spawn(move || {
            h.set_running();
            h.update_progress(5_000_000);
            h.set_finish();
        }));

        // Thread that races to error
        let h = Arc::clone(&handle);
        threads.push(std::thread::spawn(move || {
            h.set_error("connection timeout".into());
        }));

        for t in threads {
            t.join().expect("thread panicked");
        }

        let snap = handle.snapshot();
        // State must be something; no panics, no corruption.
        // Note: Finish progress may not equal total because other updater threads
        // can overwrite progress after set_finish releases its lock.
        match snap.state {
            FileTaskState::Finish => {
                // set_finish snapped progress to total, but a concurrent update_progress
                // may have changed it afterward — accept any non-negative value
                assert!(snap.progress >= 0, "Finish progress must be >= 0");
            }
            FileTaskState::Error(_) => {
                // progress may be anything
            }
            FileTaskState::Running | FileTaskState::Canceled | FileTaskState::NotStart => {
                // These are possible if the mutation threads haven't run yet
                // (though threads join, so they have finished).
            }
        }
        assert!(snap.progress >= 0, "progress must be non-negative");
    }

    // --- task snapshot consistency under concurrent access ---

    #[test]
    fn test_snapshot_consistency_under_concurrent_access() {
        let total = 10_000_000i64;
        let handle = Arc::new(make_handle(600, total));
        let reader_count = 8;
        let writer_count = 4;
        let mut threads = vec![];

        // Readers: take repeated snapshots and verify invariants
        for _ in 0..reader_count {
            let h = Arc::clone(&handle);
            threads.push(std::thread::spawn(move || {
                for _ in 0..200 {
                    let snap = h.snapshot();
                    // Every snapshot must be internally consistent.
                    // progress may exceed total temporarily when writers
                    // pass values larger than total (no upstream clamping).
                    assert!(snap.progress >= 0, "snapshot progress must be non-negative");
                    assert_eq!(snap.id, 600, "snapshot id must not change");
                    assert_eq!(snap.total, total, "snapshot total must not change");
                    // state must be a valid variant (no corruption)
                    match snap.state {
                        FileTaskState::NotStart | FileTaskState::Running
                        | FileTaskState::Finish | FileTaskState::Canceled
                        | FileTaskState::Error(_) => {}
                    }
                    // fellow fields must not be corrupted
                    assert_eq!(snap.fellow_ip, "192.168.1.1", "fellow_ip must not change");
                    assert_eq!(snap.fellow_name, "TestUser", "fellow_name must not change");
                }
            }));
        }

        // Writers: mutate state concurrently
        for i in 0..writer_count {
            let h = Arc::clone(&handle);
            threads.push(std::thread::spawn(move || {
                for j in 0..30 {
                    h.update_progress(i as i64 * 2_500_000 + j as i64 * 80_000);
                    h.set_running();
                    h.update_progress(i as i64 * 2_500_000 + j as i64 * 160_000);
                }
            }));
        }

        for t in threads {
            t.join().expect("thread panicked");
        }

        let snap = handle.snapshot();
        assert!(snap.progress >= 0, "final progress must be non-negative");
    }

    #[test]
    fn test_snapshot_consistency_with_terminal_races() {
        // Intensive snapshots while threads race between terminal states
        let total = 1_000_000i64;
        let handle = Arc::new(make_handle(610, total));
        let mut threads = vec![];

        // Racer 1: finish
        let h = Arc::clone(&handle);
        threads.push(std::thread::spawn(move || {
            for _ in 0..50 {
                h.set_finish();
            }
        }));

        // Racer 2: error
        let h = Arc::clone(&handle);
        threads.push(std::thread::spawn(move || {
            for _ in 0..50 {
                h.set_error("disk full".into());
            }
        }));

        // Racer 3: cancel
        let h = Arc::clone(&handle);
        threads.push(std::thread::spawn(move || {
            for _ in 0..50 {
                h.set_canceled();
            }
        }));

        // Snapshotters
        for _ in 0..6 {
            let h = Arc::clone(&handle);
            threads.push(std::thread::spawn(move || {
                for _ in 0..100 {
                    let snap = h.snapshot();
                    // Must always be a valid state
                    match snap.state {
                        FileTaskState::NotStart | FileTaskState::Running
                        | FileTaskState::Finish | FileTaskState::Canceled
                        | FileTaskState::Error(_) => {}
                    }
                    assert!(snap.progress >= 0, "snapshot progress must be >= 0");
                }
            }));
        }

        for t in threads {
            t.join().expect("thread panicked");
        }

        let snap = handle.snapshot();
        let valid_terminal = matches!(snap.state,
            FileTaskState::Finish | FileTaskState::Canceled | FileTaskState::Error(_)
        );
        assert!(valid_terminal,
            "after concurrent terminal racers, state must be terminal: got {:?}", snap.state);
    }

    // --- race between set_finish and set_error ---

    #[test]
    fn test_race_set_finish_set_error() {
        for run in 0..50 {
            let handle = Arc::new(make_handle(700 + run, 1_000_000));
            let h1 = Arc::clone(&handle);
            let h2 = Arc::clone(&handle);

            let t1 = std::thread::spawn(move || {
                h1.set_finish();
            });
            let t2 = std::thread::spawn(move || {
                h2.set_error("timeout".into());
            });

            t1.join().expect("thread 1 panicked");
            t2.join().expect("thread 2 panicked");

            let snap = handle.snapshot();
            let is_terminal = matches!(snap.state,
                FileTaskState::Finish | FileTaskState::Error(_)
            );
            assert!(is_terminal, "must end in Finish or Error (run {})", run);
            if snap.state == FileTaskState::Finish {
                assert_eq!(snap.progress, snap.total,
                    "Finish must have progress == total (run {})", run);
            }
        }
    }

    #[test]
    fn test_race_set_finish_set_error_with_prior_running() {
        for run in 0..50 {
            let handle = Arc::new(make_handle(800 + run, 1_000_000));
            handle.set_running();

            let h1 = Arc::clone(&handle);
            let h2 = Arc::clone(&handle);

            let t1 = std::thread::spawn(move || {
                h1.set_finish();
            });
            let t2 = std::thread::spawn(move || {
                h2.set_error("disk failure".into());
            });

            t1.join().expect("thread 1 panicked");
            t2.join().expect("thread 2 panicked");

            let snap = handle.snapshot();
            let is_terminal = matches!(snap.state,
                FileTaskState::Finish | FileTaskState::Error(_)
            );
            assert!(is_terminal,
                "with prior Running, must end in Finish or Error (run {})", run);
            if snap.state == FileTaskState::Finish {
                assert_eq!(snap.progress, snap.total,
                    "Finish must have progress == total (run {})", run);
            }
        }
    }

    #[test]
    fn test_race_set_finish_set_canceled() {
        for run in 0..50 {
            let handle = Arc::new(make_handle(900 + run, 1_000_000));
            let h1 = Arc::clone(&handle);
            let h2 = Arc::clone(&handle);

            let t1 = std::thread::spawn(move || {
                h1.set_finish();
            });
            let t2 = std::thread::spawn(move || {
                h2.set_canceled();
            });

            t1.join().expect("thread 1 panicked");
            t2.join().expect("thread 2 panicked");

            let snap = handle.snapshot();
            let is_terminal = matches!(snap.state,
                FileTaskState::Finish | FileTaskState::Canceled
            );
            assert!(is_terminal, "must end in Finish or Canceled (run {})", run);
            if snap.state == FileTaskState::Finish {
                assert_eq!(snap.progress, snap.total,
                    "Finish must have progress == total (run {})", run);
            }
            if snap.state == FileTaskState::Canceled {
                assert_eq!(snap.progress, 0,
                    "Canceled must not snap progress to total (run {})", run);
            }
        }
    }

    #[test]
    fn test_race_set_error_set_canceled() {
        for run in 0..50 {
            let handle = Arc::new(make_handle(950 + run, 1_000_000));
            let h1 = Arc::clone(&handle);
            let h2 = Arc::clone(&handle);

            let t1 = std::thread::spawn(move || {
                h1.set_error("i/o error".into());
            });
            let t2 = std::thread::spawn(move || {
                h2.set_canceled();
            });

            t1.join().expect("thread 1 panicked");
            t2.join().expect("thread 2 panicked");

            let snap = handle.snapshot();
            let is_valid = matches!(snap.state,
                FileTaskState::Error(_) | FileTaskState::Canceled
            );
            assert!(is_valid, "must end in Error or Canceled (run {})", run);
        }
    }

    // --- terminal_at accuracy ---

    #[test]
    fn test_terminal_at_not_set_before_terminal() {
        let handle = make_handle(2000, 1_000_000);
        let snap = handle.snapshot();
        assert!(
            snap.terminal_at.is_none(),
            "new NotStart task must not have terminal_at set"
        );
    }

    #[test]
    fn test_terminal_at_not_set_while_running() {
        let handle = make_handle(2001, 1_000_000);
        handle.set_running();
        let snap = handle.snapshot();
        assert!(
            snap.terminal_at.is_none(),
            "Running task must not have terminal_at set"
        );
    }

    #[test]
    fn test_terminal_at_accuracy_finish() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let handle = make_handle(2002, 1_000_000);
        handle.set_finish();
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let snap = handle.snapshot();
        assert!(
            snap.terminal_at.is_some(),
            "finished task must have terminal_at set"
        );
        let ts = snap.terminal_at.unwrap();
        assert!(
            ts >= before && ts <= after,
            "terminal_at {} must be between {} and {}",
            ts,
            before,
            after
        );
    }

    #[test]
    fn test_terminal_at_accuracy_canceled() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let handle = make_handle(2003, 1_000_000);
        handle.set_canceled();
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let snap = handle.snapshot();
        assert!(snap.terminal_at.is_some());
        let ts = snap.terminal_at.unwrap();
        assert!(
            ts >= before && ts <= after,
            "terminal_at {} must be between {} and {}",
            ts,
            before,
            after
        );
    }

    #[test]
    fn test_terminal_at_accuracy_error() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let handle = make_handle(2004, 1_000_000);
        handle.set_error("test error".into());
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let snap = handle.snapshot();
        assert!(snap.terminal_at.is_some());
        let ts = snap.terminal_at.unwrap();
        assert!(
            ts >= before && ts <= after,
            "terminal_at {} must be between {} and {}",
            ts,
            before,
            after
        );
    }

    #[test]
    fn test_terminal_at_guard_no_overwrite_on_second_terminal() {
        // terminal_at must be set once and never overwritten if another
        // terminal transition is attempted (it's a no-op).
        let handle = make_handle(2005, 1_000_000);
        handle.set_finish();
        let first_ts = handle.snapshot().terminal_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        handle.set_error("no overwrite".into());
        // The second terminal call is a no-op, so terminal_at remains unchanged.
        assert_eq!(
            handle.snapshot().terminal_at,
            first_ts,
            "terminal_at must not change on second terminal transition (no-op)"
        );
    }

    #[test]
    fn test_terminal_at_assigns_reasonable_timestamp() {
        // Ensure terminal_at is a valid Unix timestamp (post-2020).
        let handle = make_handle(2006, 1_000_000);
        handle.set_finish();
        let ts = handle.snapshot().terminal_at.unwrap();
        assert!(
            ts > 1_577_836_800,
            "terminal_at must be a valid Unix timestamp (> 2020-01-01), got {}",
            ts
        );
    }

    #[test]
    fn test_terminal_at_finish_then_cancel_preserves_ts() {
        let handle = make_handle(2007, 1_000_000);
        handle.set_finish();
        let ts = handle.snapshot().terminal_at;
        assert!(ts.is_some(), "terminal_at must be set after finish");
        // cancel is a no-op after finish; ts must stay the same
        handle.set_canceled();
        assert_eq!(
            handle.snapshot().terminal_at,
            ts,
            "terminal_at must not change after no-op cancel"
        );
    }

    // --- test helper: set_terminal_at_for_test ---

    #[test]
    fn test_set_terminal_at_for_test_helper() {
        let handle = make_handle(2008, 1_000_000);
        assert!(handle.snapshot().terminal_at.is_none());
        handle.set_terminal_at_for_test(1_000_000);
        assert_eq!(handle.snapshot().terminal_at, Some(1_000_000));
    }

    #[test]
    fn test_race_three_terminal_mutations() {
        // All three terminal setters racing simultaneously with a Barrier
        for run in 0..50 {
            let handle = Arc::new(make_handle(980 + run, 1_000_000));
            let ba = Arc::new(Barrier::new(3));
            let mut threads = vec![];

            let h1 = Arc::clone(&handle);
            let b1 = Arc::clone(&ba);
            threads.push(std::thread::spawn(move || {
                b1.wait();
                h1.set_finish();
            }));

            let h2 = Arc::clone(&handle);
            let b2 = Arc::clone(&ba);
            threads.push(std::thread::spawn(move || {
                b2.wait();
                h2.set_error("timeout".into());
            }));

            let h3 = Arc::clone(&handle);
            let b3 = Arc::clone(&ba);
            threads.push(std::thread::spawn(move || {
                b3.wait();
                h3.set_canceled();
            }));

            for t in threads {
                t.join().expect("thread panicked");
            }

            let snap = handle.snapshot();
            let valid = matches!(snap.state,
                FileTaskState::Finish | FileTaskState::Error(_) | FileTaskState::Canceled
            );
            assert!(valid,
                "three-way race must end in a terminal state (run {}): got {:?}",
                run, snap.state);
            // terminal_at must be set
            assert!(snap.terminal_at.is_some(),
                "terminal_at must be set after terminal race (run {})", run);
        }
    }
}
