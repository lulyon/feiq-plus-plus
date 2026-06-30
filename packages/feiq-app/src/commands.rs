//! Tauri IPC commands — called from frontend to interact with the engine

use crate::state::AppState;
use feiq_core::protocol::types::Fellow;
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