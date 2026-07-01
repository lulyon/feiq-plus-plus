//! Tauri IPC commands — called from frontend to interact with the engine

use crate::state::AppState;
use feiq_core::engine::events::FrontendEvent;
use feiq_core::protocol::types::{FileTaskState, FileTaskType, Fellow};
use feiq_core::storage::history::MessageRecord;
use feiq_core::storage::settings::AppConfig;
use std::io::Write as IoWrite;
use std::path::PathBuf;
use tauri::State;
use tracing;

/// Validate a file path for security (prevent path traversal and access to system files).
pub fn validate_path(path: &str) -> Result<(), String> {
    let normalized = path.replace('\\', "/");
    if normalized.contains("..") {
        return Err("Path traversal detected: '..' is not allowed".into());
    }
    let system_dirs = ["/etc", "/var", "/sys", "/proc", "/dev", "/bin", "/sbin", "/usr", "/boot", "/lib", "/opt", "/root"];
    for dir in &system_dirs {
        if normalized == *dir || normalized.starts_with(&format!("{}/", dir)) {
            return Err(format!("Path points to a system directory: {}", dir));
        }
    }
    let win_lower = normalized.to_lowercase();
    let win_system_prefixes = ["c:/windows", "c:/program files", "c:/programdata", "c:/system volume information"];
    for prefix in &win_system_prefixes {
        if win_lower.starts_with(prefix) {
            return Err("Path points to a system directory".into());
        }
    }
    Ok(())
}

// ─── Engine Lifecycle ────────────────────────────────────────

#[tauri::command]
pub async fn start_engine(state: State<'_, AppState>) -> Result<String, String> {
    let mut running = state.running.lock().await;
    if *running {
        return Ok("Engine already running".into());
    }

    let mut engine = state.engine.lock().await;
    engine.start().await.map_err(|e| e.to_string())?;
    *running = true;
    Ok("Engine started".into())
}

#[tauri::command]
pub async fn stop_engine(state: State<'_, AppState>) -> Result<String, String> {
    let mut running = state.running.lock().await;
    if !*running {
        return Ok("Engine not running".into());
    }
    let mut engine = state.engine.lock().await;
    engine.stop().await;
    *running = false;
    tracing::info!("Engine stopped by user request");
    Ok("Engine stopped".into())
}

// ─── Contacts ────────────────────────────────────────────────

#[tauri::command]
pub async fn get_contacts(state: State<'_, AppState>) -> Result<Vec<Fellow>, String> {
    let engine = state.engine.lock().await;
    Ok(engine.get_contacts())
}

#[tauri::command]
pub async fn search_contacts(state: State<'_, AppState>, query: String) -> Result<Vec<Fellow>, String> {
    let engine = state.engine.lock().await;
    Ok(engine.search_contacts(&query))
}

#[tauri::command]
pub async fn add_contact(state: State<'_, AppState>, ip: String) -> Result<Fellow, String> {
    let mut engine = state.engine.lock().await;
    Ok(engine.add_contact(&ip))
}

// ─── Settings ────────────────────────────────────────────────

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state.config.lock().await;
    Ok(config.clone())
}

#[tauri::command]
pub async fn update_settings(state: State<'_, AppState>, config: AppConfig) -> Result<String, String> {
    let config_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".feiq_setting.ini");

    // Persist to file
    config.save(&config_path).map_err(|e| e.to_string())?;

    // Update in-memory state
    let mut current = state.config.lock().await;
    *current = config.clone();

    // Update running engine (takes effect immediately for new messages,
    // but periodic broadcast continues with old name until restart)
    let mut engine = state.engine.lock().await;
    engine.update_config(config);

    tracing::info!("Settings saved to {:?}", config_path);
    Ok("Settings saved. Restart recommended for full effect.".into())
}

// ─── Chat History ────────────────────────────────────────────

#[tauri::command]
pub async fn get_chat_history(
    state: State<'_, AppState>,
    ip: String,
    offset: i64,
    limit: i64,
) -> Result<Vec<MessageRecord>, String> {
    let engine = state.engine.lock().await;
    engine
        .get_chat_history(&ip, offset, limit)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_chat_history(
    state: State<'_, AppState>,
    query: String,
    limit: i64,
) -> Result<Vec<MessageRecord>, String> {
    let engine = state.engine.lock().await;
    engine
        .search_chat_history(&query, limit)
        .map_err(|e| e.to_string())
}

// ─── Emoji ────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_emoji_list() -> Result<Vec<serde_json::Value>, String> {
    use feiq_core::protocol::emoji;
    let list: Vec<serde_json::Value> = (0..emoji::EMOJI_LEN)
        .map(|i| {
            serde_json::json!({
                "code": emoji::EMOJI_CODES[i],
                "name": emoji::EMOJI_NAMES[i],
                "image": format!("emojis/{}.gif", i + 1),
            })
        })
        .collect();
    Ok(list)
}

// ─── Messaging ────────────────────────────────────────────────

#[tauri::command]
pub async fn send_knock(state: State<'_, AppState>, ip: String) -> Result<(), String> {
    let engine = state.engine.lock().await;
    let port = engine.find_contact(&ip).map(|f| f.port).unwrap_or(2425);
    tracing::info!("send_knock to {ip}: contact_port={port}");
    engine.send_knock_to(&ip, port).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_text(state: State<'_, AppState>, ip: String, text: String) -> Result<(), String> {
    let engine = state.engine.lock().await;
    let port = engine.find_contact(&ip).map(|f| f.port).unwrap_or(2425);
    tracing::info!("send_text to {ip}: contact_port={port}, text={text}");
    engine.send_text_to(&ip, port, &text).await.map_err(|e| e.to_string())
}

// ─── Alias & Contact Meta ─────────────────────────────────────

#[tauri::command]
pub async fn set_alias(state: State<'_, AppState>, ip: String, alias: String) -> Result<(), String> {
    let engine = state.engine.lock().await;
    engine
        .set_contact_alias(&ip, &alias)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_contact_group(
    state: State<'_, AppState>,
    ip: String,
    group_name: String,
) -> Result<(), String> {
    let engine = state.engine.lock().await;
    engine
        .set_contact_group(&ip, &group_name)
        .map_err(|e| e.to_string())
}

// ─── Groups ────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_group(
    state: State<'_, AppState>,
    name: String,
    member_ips: Vec<String>,
) -> Result<(), String> {
    let engine = state.engine.lock().await;
    engine
        .create_group(&name, &member_ips)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_groups(
    state: State<'_, AppState>,
) -> Result<Vec<(String, Vec<String>)>, String> {
    let engine = state.engine.lock().await;
    engine.get_groups().map_err(|e| e.to_string())
}

// ─── History Export / Import ──────────────────────────────────

#[tauri::command]
pub async fn export_history(state: State<'_, AppState>, path: String) -> Result<(), String> {
    validate_path(&path)?;
    let engine = state.engine.lock().await;
    let json = engine.export_history().map_err(|e| e.to_string())?;
    tokio::fs::write(&path, &json).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_history(state: State<'_, AppState>, path: String) -> Result<usize, String> {
    validate_path(&path)?;
    let engine = state.engine.lock().await;
    let json = tokio::fs::read_to_string(&path).await.map_err(|e| e.to_string())?;
    engine.import_history(&json).map_err(|e| e.to_string())
}

// ─── Blacklist ────────────────────────────────────────────────

#[tauri::command]
pub async fn add_to_blacklist(state: State<'_, AppState>, ip: String) -> Result<(), String> {
    let engine = state.engine.lock().await;
    engine.add_to_blacklist(&ip);
    Ok(())
}

#[tauri::command]
pub async fn remove_from_blacklist(state: State<'_, AppState>, ip: String) -> Result<(), String> {
    let engine = state.engine.lock().await;
    engine.remove_from_blacklist(&ip);
    Ok(())
}

#[tauri::command]
pub async fn get_blacklist(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let engine = state.engine.lock().await;
    Ok(engine.get_blacklist())
}

// ─── Group Chat ──────────────────────────────────────────────────

#[tauri::command]
pub async fn send_group_text(
    state: State<'_, AppState>,
    group_name: String,
    text: String,
) -> Result<(), String> {
    let engine = state.engine.lock().await;
    tracing::info!("send_group_text to group={group_name}, text={text}");
    engine
        .send_text_to_group(&group_name, &text)
        .await
        .map_err(|e| e.to_string())
}

// ─── File Transfer ────────────────────────────────────────────

#[tauri::command]
pub async fn download_file(
    state: State<'_, AppState>,
    task_id: u64,
    save_path: String,
) -> Result<(), String> {
    validate_path(&save_path)?;
    // Phase 1: gather task info while holding engine lock
    let (task, task_info, event_tx, network) = {
        let engine = state.engine.lock().await;

        let task = engine.get_file_task(task_id).ok_or("Task not found")?;
        let snap = task.snapshot();

        if snap.task_type != FileTaskType::Download {
            return Err("Not a download task".into());
        }

        // Check cancel before starting any I/O
        if task.is_cancel_pending() {
            task.set_canceled();
            let _ = engine.event_tx().send(FrontendEvent::FileStateChanged {
                task_id,
                state: FileTaskState::Canceled,
                message: format!("Download canceled for: {}", snap.content.filename),
            });
            return Err("Download canceled by user".into());
        }

        task.set_running();
        let _ = engine.event_tx().send(FrontendEvent::FileStateChanged {
            task_id,
            state: FileTaskState::Running,
            message: format!("Downloading: {}", snap.content.filename),
        });

        // Look up the contact port while holding the engine lock to avoid
        // racing with contact updates (e.g. a newly-arrived BR_BROADCAST
        // that changes the fellow's port between Phase 1 and Phase 2).
        let peer_port = engine
            .find_contact(&snap.fellow_ip)
            .map(|f| f.port)
            .unwrap_or(2425);

        let task_info = (
            snap.fellow_ip.clone(),
            peer_port,
            snap.content.packet_no,
            snap.content.file_id,
            snap.content.size,
            snap.content.filename.clone(),
        );
        let event_tx = engine.event_tx().clone();
        let network = engine
            .network()
            .ok_or("Network not available")?
            .clone();

        (task, task_info, event_tx, network)
    };

    let (peer_ip, peer_port, packet_no, file_id, total, filename) = task_info;

    // Guard: relay peers use WebSocket, not direct TCP — file transfer not yet supported
    if peer_ip.starts_with("relay:") {
        task.set_error("File download not supported for relay peers".into());
        let _ = event_tx.send(FrontendEvent::FileStateChanged {
            task_id,
            state: FileTaskState::Error("File download not supported for relay peers".into()),
            message: format!("Cannot download from relay peer: {}", filename),
        });
        return Err("File download not supported for relay peers. Use direct LAN connection.".into());
    }

    // Phase 2: TCP transfer (engine lock released)
    let mut ft = network
        .connect_for_file(&peer_ip, peer_port)
        .await
        .map_err(|e| {
            task.set_error(e.to_string());
            let _ = event_tx.send(FrontendEvent::FileStateChanged {
                task_id,
                state: FileTaskState::Error(e.to_string()),
                message: format!("Connection failed: {}", e),
            });
            e.to_string()
        })?;

    // Send GETFILEDATA request over TCP
    let request = format!("{}:{}:0:", packet_no, file_id);
    ft.send(request.as_bytes()).await.map_err(|e| {
        task.set_error(e.to_string());
        let _ = event_tx.send(FrontendEvent::FileStateChanged {
            task_id,
            state: FileTaskState::Error(e.to_string()),
            message: format!("Send GETFILEDATA failed: {}", e),
        });
        e.to_string()
    })?;

    // Receive file with progress callbacks
    let recv_result = {
        let task_clone = task.clone();
        let tx_clone = event_tx.clone();
        ft.recv_file(&save_path, total, move |progress, total_size| {
            let should_notify = task_clone.update_progress(progress);
            if should_notify {
                let _ = tx_clone.send(FrontendEvent::FileProgress {
                    task_id,
                    progress,
                    total: total_size,
                });
            }
        })
        .await
    };

    match recv_result {
        Ok(_) => {
            task.set_finish();
            let _ = event_tx.send(FrontendEvent::FileStateChanged {
                task_id,
                state: FileTaskState::Finish,
                message: format!("Downloaded: {}", filename),
            });
            Ok(())
        }
        Err(e) => {
            task.set_error(e.to_string());
            let _ = event_tx.send(FrontendEvent::FileStateChanged {
                task_id,
                state: FileTaskState::Error(e.to_string()),
                message: format!("Download failed: {}", e),
            });
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn cancel_file_task(state: State<'_, AppState>, task_id: u64) -> Result<(), String> {
    let engine = state.engine.lock().await;
    engine.cancel_file_task(task_id);
    Ok(())
}

#[tauri::command]
pub async fn send_file(
    state: State<'_, AppState>,
    ip: String,
    file_path: String,
) -> Result<u64, String> {
    validate_path(&file_path)?;
    let engine = state.engine.lock().await;

    // Auto-detect: if path is a directory, route to folder transfer
    let is_dir = std::fs::metadata(&file_path)
        .map(|m| m.is_dir())
        .unwrap_or(false);

    if is_dir {
        engine
            .send_folder_to(&ip, &file_path)
            .await
            .map_err(|e| e.to_string())
    } else {
        engine
            .send_file_to(&ip, &file_path)
            .await
            .map_err(|e| e.to_string())
    }
}

/// Send a folder notification to a peer (feiq++ only).
/// Returns the task_id for tracking.
#[tauri::command]
pub async fn send_folder(
    state: State<'_, AppState>,
    ip: String,
    folder_path: String,
) -> Result<u64, String> {
    validate_path(&folder_path)?;
    let engine = state.engine.lock().await;
    engine
        .send_folder_to(&ip, &folder_path)
        .await
        .map_err(|e| e.to_string())
}

/// Download a folder from a peer after receiving a folder notification.
/// Connects via TCP, receives manifest, and downloads all files.
#[tauri::command]
pub async fn download_folder(
    state: State<'_, AppState>,
    task_id: u64,
    save_path: String,
) -> Result<(), String> {
    use feiq_core::protocol::constants::{
        FOLDER_FILE_ACK, FOLDER_MANIFEST_REQUEST, FOLDER_TRANSFER_CANCEL,
        FOLDER_TRANSFER_COMPLETE,
    };
    use feiq_core::protocol::types::FileTaskType;
    use std::path::Path;

    validate_path(&save_path)?;

    // Phase 1: gather task info while holding engine lock
    let (task, task_info, event_tx, network) = {
        let engine = state.engine.lock().await;
        let task = engine.get_file_task(task_id).ok_or("Task not found")?;
        let snap = task.snapshot();

        if snap.task_type != FileTaskType::Download {
            return Err("Not a download task".into());
        }
        if task.is_cancel_pending() {
            task.set_canceled();
            let _ = engine.event_tx().send(FrontendEvent::FolderStateChanged {
                task_id,
                state: FileTaskState::Canceled,
                message: "Download canceled by user".into(),
            });
            return Err("Download canceled by user".into());
        }

        // Parse folder metadata from the content filename
        let folder_meta = snap.content.filename
            .strip_prefix("__FOLDER__")
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

        let folder_name = folder_meta
            .as_ref()
            .and_then(|m| m["name"].as_str())
            .unwrap_or(&snap.content.filename)
            .to_string();
        let transfer_id = folder_meta
            .as_ref()
            .and_then(|m| m["tid"].as_u64())
            .unwrap_or(snap.content.packet_no);

        // Look up peer port
        let peer_port = engine
            .find_contact(&snap.fellow_ip)
            .map(|f| f.port)
            .unwrap_or(2425);

        let task_info = (
            snap.fellow_ip.clone(),
            peer_port,
            transfer_id,
            folder_name,
        );
        let event_tx = engine.event_tx().clone();
        let network = engine.network().ok_or("Network not available")?.clone();

        (task, task_info, event_tx, network)
    };

    let (peer_ip, peer_port, transfer_id, folder_name) = task_info;

    // Set task to running
    task.set_running();
    let _ = event_tx.send(FrontendEvent::FolderStateChanged {
        task_id,
        state: FileTaskState::Running,
        message: format!("Downloading folder: {folder_name}"),
    });

    // Connect TCP
    let mut ft = network
        .connect_for_file(&peer_ip, peer_port)
        .await
        .map_err(|e| {
            task.set_error(e.to_string());
            let _ = event_tx.send(FrontendEvent::FolderStateChanged {
                task_id,
                state: FileTaskState::Error(e.to_string()),
                message: format!("Connection failed: {e}"),
            });
            e.to_string()
        })?;

    // Send FOLDER_MANIFEST_REQUEST + transfer_id
    let mut request = FOLDER_MANIFEST_REQUEST.to_vec();
    write!(&mut request, "{}\n", transfer_id).unwrap();
    ft.send(&request).await.map_err(|e| {
        task.set_error(e.to_string());
        e.to_string()
    })?;

    // Receive manifest
    let manifest = ft.recv_folder_manifest().await.map_err(|e| {
        task.set_error(format!("Failed to receive manifest: {e}"));
        let _ = event_tx.send(FrontendEvent::FolderStateChanged {
            task_id,
            state: FileTaskState::Error(format!("Failed to receive manifest: {e}")),
            message: format!("Manifest error: {e}"),
        });
        format!("{e}")
    })?;

    let total_files = manifest.total_files;
    let total_bytes = manifest.total_bytes as i64;

    // Create the base directory
    let base_dir = Path::new(&save_path);
    std::fs::create_dir_all(base_dir).map_err(|e| {
        task.set_error(e.to_string());
        e.to_string()
    })?;

    // Download files one by one
    let mut completed: u32 = 0;
    let mut overall_bytes: i64 = 0;

    for entry in &manifest.files {
        if task.is_cancel_pending() {
            task.set_canceled();
            let _ = ft.send_marker(FOLDER_TRANSFER_CANCEL).await;
            let _ = event_tx.send(FrontendEvent::FolderStateChanged {
                task_id,
                state: FileTaskState::Canceled,
                message: "Folder download canceled".into(),
            });
            return Err("Download canceled".into());
        }

        let file_path = base_dir.join(&entry.relative_path);

        // Create parent directories
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                task.set_error(format!("Failed to create directory: {e}"));
                format!("{e}")
            })?;
        }

        let file_path_str = file_path.to_string_lossy().to_string();

        // Receive file header (validates we're in sync)
        let (rel_path, file_size) = ft.recv_folder_file_header().await.map_err(|e| {
            task.set_error(format!("Failed to receive file header: {e}"));
            format!("{e}")
        })?;

        if rel_path != entry.relative_path || file_size != entry.size {
            task.set_error("Folder transfer protocol mismatch".into());
            return Err("Protocol mismatch".into());
        }

        // Receive file content with progress
        let recv_result = {
            let tx_clone = event_tx.clone();
            let rel_clone = entry.relative_path.clone();
            ft.recv_file(&file_path_str, entry.size as i64, move |progress, total| {
                let current_overall = overall_bytes + progress;
                let _ = tx_clone.send(FrontendEvent::FolderProgress {
                    task_id,
                    overall_progress: current_overall,
                    overall_total: total_bytes,
                    current_file: rel_clone.clone(),
                    current_file_progress: progress,
                    current_file_total: total,
                    files_completed: completed + 1,
                    total_files,
                });
            })
            .await
        };

        match recv_result {
            Ok(_) => {}
            Err(e) => {
                task.set_error(format!("Failed to receive {}: {}", entry.relative_path, e));
                return Err(format!("{e}"));
            }
        }

        overall_bytes += entry.size as i64;
        completed += 1;

        // Send ACK to signal ready for next file
        let _ = ft.send_marker(FOLDER_FILE_ACK).await;

        let _ = event_tx.send(FrontendEvent::FolderProgress {
            task_id,
            overall_progress: overall_bytes,
            overall_total: total_bytes,
            current_file: entry.relative_path.clone(),
            current_file_progress: entry.size as i64,
            current_file_total: entry.size as i64,
            files_completed: completed,
            total_files,
        });
    }

    // Receive completion marker
    match ft.expect_marker(FOLDER_TRANSFER_COMPLETE).await {
        Ok(true) => {
            task.set_finish();
            let _ = event_tx.send(FrontendEvent::FolderStateChanged {
                task_id,
                state: FileTaskState::Finish,
                message: format!("Downloaded folder: {folder_name} ({} files)", completed),
            });
            Ok(())
        }
        _ => {
            task.set_error("Transfer incomplete".into());
            Err("Transfer incomplete".into())
        }
    }
}

// ─── Unread Badge ─────────────────────────────────────────────

#[tauri::command]
pub async fn reset_unread_count(
    state: State<'_, AppState>,
    tray_state: State<'_, crate::state::TrayState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    state.unread_count.store(0, Ordering::Relaxed);
    crate::tray::update_tray_badge(&tray_state.tray, &app_handle, 0);
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_rejects_dotdot_traversal() {
        for path in &["../etc/passwd", "foo/../../etc/passwd", "..", "a/.."] {
            let result = validate_path(path);
            assert!(result.is_err(), "Expected '{}' to be rejected", path);
            assert!(result.unwrap_err().contains(".."), "should mention '..': {}", path);
        }
    }

    #[test]
    fn test_validate_path_rejects_system_paths() {
        for path in &["/etc/passwd", "/etc/", "/var/log", "/sys/kernel", "/proc/self/environ", "/dev/null", "/bin/sh", "/sbin/init", "/usr/bin"] {
            let result = validate_path(path);
            assert!(result.is_err(), "Expected '{}' to be rejected", path);
        }
    }

    #[test]
    fn test_validate_path_rejects_windows_system_paths() {
        for path in &["c:\\windows\\system32\\cmd.exe", "C:\\Windows\\System32", "c:\\Program Files\\SomeApp", "c:\\ProgramData\\SomeData"] {
            let result = validate_path(path);
            assert!(result.is_err(), "Expected '{}' to be rejected", path);
        }
    }

    #[test]
    fn test_validate_path_allows_normal_paths() {
        for path in &["/tmp/test.txt", "/home/user/documents/report.pdf", "/Users/alice/Desktop", "relative/path/to/file.txt", "plain_filename.txt", "/data/backup.sqlite", "./local_file.csv"] {
            let result = validate_path(path);
            assert!(result.is_ok(), "Expected '{}' to be allowed: {:?}", path, result.err());
        }
    }
}
