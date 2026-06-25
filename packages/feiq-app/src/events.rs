//! Forward engine FrontendEvents to the Tauri window as native events

use feiq_core::engine::events::FrontendEvent;
use tauri::{AppHandle, Emitter};

/// Spawn a task that continuously forwards engine events to the Tauri window
pub fn start_event_forwarder(app_handle: AppHandle, state: &crate::state::AppState) {
    let event_rx = state.event_rx.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            let event = event_rx.lock().await.recv().await;

            match event {
                Some(FrontendEvent::ContactUpdate { fellow }) => {
                    let _ = app_handle.emit("contact-update", &fellow);
                }
                Some(FrontendEvent::NewMessage { from_ip, from_name, contents, timestamp }) => {
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
