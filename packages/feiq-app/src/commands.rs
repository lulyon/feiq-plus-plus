//! Tauri IPC commands — called from frontend to interact with the engine

use crate::state::AppState;
use feiq_core::engine::events::FrontendEvent;
use feiq_core::protocol::types::{FileTaskState, FileTaskType, Fellow};
use feiq_core::storage::history::MessageRecord;
use feiq_core::storage::settings::AppConfig;
use std::path::PathBuf;
use tauri::State;
use tracing;

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
    let engine = state.engine.lock().await;
    let json = engine.export_history().map_err(|e| e.to_string())?;
    std::fs::write(&path, &json).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_history(state: State<'_, AppState>, path: String) -> Result<usize, String> {
    let engine = state.engine.lock().await;
    let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
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

// ─── Screenshot (macOS) ─────────────────────────────────────────

#[tauri::command]
pub async fn capture_screenshot() -> Result<String, String> {
    let path = format!(
        "/tmp/feiq_screenshot_{}.png",
        std::process::id()
    );
    let path_clone = path.clone();

    #[cfg(target_os = "macos")]
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        let _output = std::process::Command::new("screencapture")
            .args(["-i", &path_clone])
            .output()?;
        if std::path::Path::new(&path_clone).exists() {
            Ok(path_clone)
        } else {
            Ok("FALLBACK".to_string()) // User canceled
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    #[cfg(not(target_os = "macos"))]
    let result: anyhow::Result<String> = Ok("FALLBACK".to_string());

    result.map_err(|e| e.to_string())
}

// ─── File Transfer ────────────────────────────────────────────

#[tauri::command]
pub async fn download_file(
    state: State<'_, AppState>,
    task_id: u64,
    save_path: String,
) -> Result<(), String> {
    // Phase 1: gather task info while holding engine lock
    let (task, task_info, event_tx, network) = {
        let engine = state.engine.lock().await;

        let task = engine.get_file_task(task_id).ok_or("Task not found")?;
        let snap = task.snapshot();

        if snap.task_type != FileTaskType::Download {
            return Err("Not a download task".into());
        }

        task.set_running();
        let _ = engine.event_tx().send(FrontendEvent::FileStateChanged {
            task_id,
            state: FileTaskState::Running,
            message: format!("Downloading: {}", snap.content.filename),
        });

        let task_info = (
            snap.fellow_ip.clone(),
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

    let (peer_ip, packet_no, file_id, total, filename) = task_info;
    let peer_port = 2425;

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
    let engine = state.engine.lock().await;
    engine
        .send_file_to(&ip, &file_path)
        .await
        .map_err(|e| e.to_string())
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
