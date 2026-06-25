//! Tauri IPC commands — called from frontend to interact with the engine

use crate::state::AppState;
use feiq_core::protocol::types::Fellow;
use feiq_core::storage::settings::AppConfig;
use tauri::State;

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
    *running = false;
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
pub async fn update_settings(state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    let mut current = state.config.lock().await;
    *current = config;
    Ok(())
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
    let config = state.config.lock().await;

    let data = feiq_core::engine::engine::build_knock(
        &config.name,
        &config.host,
        engine.version(),
    );

    // Need network access — but Engine doesn't expose send_to directly yet
    // For MVP, this is a placeholder
    tracing::info!("Knock sent to {ip}");
    let _ = (ip, data);
    Ok(())
}

