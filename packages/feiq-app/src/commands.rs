//! Tauri IPC commands — called from frontend to interact with the engine

use crate::state::AppState;
use feiq_core::engine::events::FrontendEvent;
use feiq_core::protocol::types::{FileTaskState, FileTaskType, Fellow};
use feiq_core::storage::history::MessageRecord;
use feiq_core::storage::settings::AppConfig;
use std::path::PathBuf;
use tauri::State;
use tracing;

/// Validate a file path for security (prevent path traversal and access to system files).
pub fn validate_path(path: &str) -> Result<(), String> {
    // Reject empty paths
    if path.is_empty() || path.trim().is_empty() {
        return Err("Path is empty".into());
    }
    // Reject null byte injection
    if path.contains('\0') {
        return Err("Path contains null byte".into());
    }
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
    tracing::info!("send_text to {ip}: contact_port={port}");
    engine.send_text_to(&ip, port, &text).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_read_receipt(
    state: State<'_, AppState>,
    ip: String,
    packet_id: String,
) -> Result<(), String> {
    let engine = state.engine.lock().await;
    let port = engine.find_contact(&ip).map(|f| f.port).unwrap_or(2425);
    tracing::info!("send_read_receipt to {ip}: packet_id={packet_id}");
    engine
        .send_readmsg(&ip, port, &packet_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_typing(
    state: State<'_, AppState>,
    ip: String,
    is_typing: bool,
) -> Result<(), String> {
    let engine = state.engine.lock().await;
    let port = engine.find_contact(&ip).map(|f| f.port).unwrap_or(2425);
    engine
        .send_typing_to(&ip, port, is_typing)
        .await
        .map_err(|e| e.to_string())
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
pub async fn send_group_file(
    state: State<'_, AppState>,
    group_name: String,
    file_path: String,
) -> Result<(), String> {
    let engine = state.engine.lock().await;
    tracing::info!("send_group_file to group={group_name}: {file_path}");
    engine
        .send_file_to_group(&group_name, &file_path)
        .await
        .map(|results| {
            tracing::info!("Group file sent: {} members, {} succeeded",
                results.len(),
                results.iter().filter(|(_, tid)| *tid > 0).count());
        })
        .map_err(|e| e.to_string())
}

// ─── Group Management ──────────────────────────────────────────

#[tauri::command]
pub async fn delete_group_cmd(
    state: State<'_, AppState>,
    group_name: String,
) -> Result<(), String> {
    let engine = state.engine.lock().await;
    engine.delete_group(&group_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_group_announcement(
    state: State<'_, AppState>,
    group_name: String,
    content: String,
) -> Result<(), String> {
    let engine = state.engine.lock().await;
    engine
        .send_announcement_to_group(&group_name, &content)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_group_announcements(
    state: State<'_, AppState>,
    group_name: String,
    limit: usize,
    offset: usize,
) -> Result<Vec<(i64, String, String, i64)>, String> {
    let engine = state.engine.lock().await;
    engine
        .get_announcements(&group_name, limit, offset)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_group_text(
    state: State<'_, AppState>,
    group_name: String,
    text: String,
) -> Result<(), String> {
    let engine = state.engine.lock().await;
    tracing::info!("send_group_text to group={group_name}");
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

    // Relay peers: send GETFILEDATA request via relay and receive via binary chunks
    if peer_ip.starts_with("relay:") {
        let relay_peer_id = peer_ip
            .strip_prefix("relay:")
            .unwrap_or(&peer_ip)
            .to_string();

        // Send GETFILEDATA request via relay
        {
            let engine = state.engine.lock().await;
            if let Some(ref relay) = engine.relay_client() {
                let getfile_data = feiq_core::engine::engine::build_get_file_data(packet_no, file_id, 0);
                if let Err(e) = relay
                    .send_to(&relay_peer_id, feiq_core::protocol::constants::IPMSG_GETFILEDATA, &getfile_data)
                    .await
                {
                    task.set_error(e.to_string());
                    let _ = event_tx.send(FrontendEvent::FileStateChanged {
                        task_id,
                        state: FileTaskState::Error(e.to_string()),
                        message: format!("Relay file request failed: {}", e),
                    });
                    return Err(format!("Relay file request failed: {}", e));
                }
            } else {
                task.set_error("Relay not connected".into());
                return Err("Relay not connected".into());
            }
        }

        task.set_running();
        let _ = event_tx.send(FrontendEvent::FileStateChanged {
            task_id,
            state: FileTaskState::Running,
            message: format!("Downloading via relay: {}", filename),
        });

        // The relay file data will arrive as FileChunk events handled by the engine,
        // which will update the file task progress. Return success — the frontend
        // will monitor FileProgress events.
        return Ok(());
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

    // Receive file with progress callbacks and cancellation support
    let recv_result = {
        let task_clone = task.clone();
        let tx_clone = event_tx.clone();
        let download_limit: Option<u64> = {
            let engine = state.engine.lock().await;
            engine
                .get_config()
                .download_speed_limit_kbps
                .checked_mul(1024)
                .map(|v| v as u64)
                .filter(|&v| v > 0)
        };
        ft.recv_file(
            &save_path, total,
            move |progress, total_size| {
                let should_notify = task_clone.update_progress(progress);
                if should_notify {
                    let _ = tx_clone.send(FrontendEvent::FileProgress {
                        task_id,
                        progress,
                        total: total_size,
                    });
                }
            },
            Some(&*task.cancel_flag),
            download_limit,
        )
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
            // Only emit Error event if task is not already in a terminal state
            // (e.g., user canceled while I/O was in-flight)
            let snap = task.snapshot();
            if snap.state != FileTaskState::Canceled && snap.state != FileTaskState::Finish {
                let _ = event_tx.send(FrontendEvent::FileStateChanged {
                    task_id,
                    state: FileTaskState::Error(e.to_string()),
                    message: format!("Download failed: {}", e),
                });
            }
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

    // Resolve non-absolute paths: macOS dialog may return a bare name
    // when selecting directories from the sidebar. Try home dir first.
    let resolved = if std::fs::metadata(&file_path).is_err() && !file_path.starts_with('/') {
        if let Ok(home) = std::env::var("HOME") {
            let candidate = format!("{}/{}", home, file_path);
            if std::fs::metadata(&candidate).is_ok() {
                candidate
            } else {
                file_path
            }
        } else {
            file_path
        }
    } else {
        file_path
    };

    let engine = state.engine.lock().await;

    // Reject directories — folder transfer is not supported
    if std::fs::metadata(&resolved).map(|m| m.is_dir()).unwrap_or(false) {
        return Err("Cannot send a directory. Please select individual files.".into());
    }

    engine
        .send_file_to(&ip, &resolved)
        .await
        .map_err(|e| e.to_string())
}


// ─── Stealth Mode ──────────────────────────────────────────────

#[tauri::command]
pub async fn set_stealth_mode(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let mut engine = state.engine.lock().await;
    engine.set_stealth_mode(enabled);
    Ok(())
}

// ─── Avatar ────────────────────────────────────────────────────

#[tauri::command]
pub async fn set_avatar(
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    validate_path(&path)?;
    // Validate: must be PNG or JPEG, max 100KB
    let meta = std::fs::metadata(&path).map_err(|e| format!("Cannot read file: {e}"))?;
    if meta.len() > 102400 {
        return Err("Avatar image must be ≤ 100KB".into());
    }
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "png" && ext != "jpg" && ext != "jpeg" {
        return Err("Avatar must be PNG or JPEG".into());
    }

    let mut engine = state.engine.lock().await;
    let mut config = engine.get_config();
    config.avatar_path = path;
    engine.update_config(config);
    Ok(())
}

#[tauri::command]
pub async fn get_avatar(
    state: State<'_, AppState>,
    ip: String,
) -> Result<Option<String>, String> {
    let engine = state.engine.lock().await;
    let contact = engine.find_contact(&ip);
    match contact {
        Some(fellow) if !fellow.avatar_hash.is_empty() => {
            Ok(Some(fellow.avatar_hash.clone()))
        }
        _ => Ok(None),
    }
}

// ─── File Share (Browse remote shared directories) ────────────

#[tauri::command]
pub async fn browse_shared_folder(
    state: State<'_, AppState>,
    ip: String,
    password: Option<String>,
) -> Result<(), String> {
    let engine = state.engine.lock().await;

    let peer = engine
        .find_contact(&ip)
        .ok_or_else(|| format!("Contact not found for IP: {}", ip))?;

    let port = peer.port;
    let data = feiq_core::engine::engine::build_get_dir_files(0, password.as_deref());

    let network = engine
        .network()
        .ok_or("Network not available")?
        .clone();

    // Release engine lock before async send
    drop(engine);

    network
        .send_to(&ip, port, &data)
        .await
        .map_err(|e| format!("Failed to send GETDIRFILES: {}", e))
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

    #[test]
    fn test_validate_path_rejects_null_byte() {
        let cases = [
            "/tmp/foo\0.txt",
            "/tmp/../../etc/passwd\0suffix",
            "safe.txt\0../../etc/passwd",
            "\0/tmp/bar.txt",
            "/tmp/evil.exe\0",
        ];
        for path in &cases {
            let result = validate_path(path);
            assert!(result.is_err(), "Expected '{}' to be rejected for null byte", path);
            let err = result.unwrap_err();
            assert!(
                err.contains("null byte"),
                "error should mention 'null byte', got: {err}"
            );
        }
    }

    #[test]
    fn test_validate_path_rejects_empty_path() {
        assert!(validate_path("").is_err(), "empty path should be rejected");
    }

    #[test]
    fn test_validate_path_rejects_whitespace_only_path() {
        assert!(validate_path("   ").is_err(), "whitespace-only path should be rejected");
        assert!(validate_path("\t").is_err(), "tab-only path should be rejected");
        assert!(validate_path("\n").is_err(), "newline-only path should be rejected");
    }

    #[test]
    fn test_validate_path_rejects_null_byte_with_traversal_disguise() {
        // Combined attack: null byte terminates string at OS level, bypassing traversal check
        let path = "safe_folder/safe_file.txt\0../../etc/passwd";
        let result = validate_path(path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("null byte"), "error should mention 'null byte', got: {err}");
    }
}
