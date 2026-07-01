//! Forward engine FrontendEvents to the Tauri window as native events

use crate::state::TrayState;
use crate::tray;
use feiq_core::engine::events::FrontendEvent;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

/// Spawn a task that continuously forwards engine events to the Tauri window.
/// Also tracks unread message count and updates the tray / macOS dock badge.
pub fn start_event_forwarder(app_handle: AppHandle, state: &crate::state::AppState) {
    let event_rx = state.event_rx.clone();
    let unread_count = state.unread_count.clone();

    // Clone the tray icon handle so the spawned task can update badge
    let tray = app_handle.state::<TrayState>().tray.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            let event = event_rx.lock().await.recv().await;

            match event {
                Some(FrontendEvent::ContactUpdate { fellow }) => {
                    let _ = app_handle.emit("contact-update", &fellow);
                }
                Some(FrontendEvent::NewMessage { from_ip, from_name, contents, timestamp }) => {
                    // Update unread count and tray badge
                    let count = unread_count.fetch_add(1, Ordering::Relaxed) + 1;
                    tray::update_tray_badge(&tray, &app_handle, count);

                    let _ = app_handle.emit("new-message", serde_json::json!({
                        "fromIp": from_ip,
                        "fromName": from_name,
                        "contents": contents,
                        "timestamp": timestamp,
                    }));
                }
                Some(FrontendEvent::FileProgress { task_id, progress, total }) => {
                    let _ = app_handle.emit("file-progress", serde_json::json!({
                        "taskId": task_id,
                        "progress": progress,
                        "total": total,
                    }));
                }
                Some(FrontendEvent::FileStateChanged { task_id, state, message }) => {
                    let _ = app_handle.emit("file-state-changed", serde_json::json!({
                        "taskId": task_id,
                        "state": state,
                        "message": message,
                    }));
                }
                Some(FrontendEvent::FolderProgress {
                    task_id,
                    overall_progress,
                    overall_total,
                    current_file,
                    current_file_progress,
                    current_file_total,
                    files_completed,
                    total_files,
                }) => {
                    let _ = app_handle.emit("folder-progress", serde_json::json!({
                        "taskId": task_id,
                        "overallProgress": overall_progress,
                        "overallTotal": overall_total,
                        "currentFile": current_file,
                        "currentFileProgress": current_file_progress,
                        "currentFileTotal": current_file_total,
                        "filesCompleted": files_completed,
                        "totalFiles": total_files,
                    }));
                }
                Some(FrontendEvent::FolderStateChanged { task_id, state, message }) => {
                    let _ = app_handle.emit("folder-state-changed", serde_json::json!({
                        "taskId": task_id,
                        "state": state,
                        "message": message,
                    }));
                }
                Some(FrontendEvent::SendTimeout { to_ip, content }) => {
                    let _ = app_handle.emit("send-timeout", serde_json::json!({
                        "toIp": to_ip,
                        "content": content,
                    }));
                }
                Some(FrontendEvent::Error(msg)) => {
                    let _ = app_handle.emit("engine-error", &msg);
                }
                None => break,
            }
        }
    });
}
