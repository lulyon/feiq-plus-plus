//! Feiq++ Tauri application entry point

mod commands;
mod events;
mod state;
mod tray;

use state::AppState;
use std::path::PathBuf;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("feiq=info".parse().unwrap()),
        )
        .init();

    // Load config from ~/.feiq_setting.ini (or create default)
    let config_path = home_dir().join(".feiq_setting.ini");
    let config = feiq_core::storage::settings::AppConfig::load(&config_path)
        .unwrap_or_default();

    let app_state = AppState::new(config);

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .manage(app_state)
        .setup(|app| {
            let handle = app.handle().clone();
            let _ = tray::init_tray(&handle);

            // Start event forwarding
            let state = handle.state::<AppState>();
            events::start_event_forwarder(handle.clone(), &*state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_engine,
            commands::stop_engine,
            commands::get_contacts,
            commands::search_contacts,
            commands::add_contact,
            commands::get_settings,
            commands::update_settings,
            commands::get_emoji_list,
            commands::send_knock,
        ])
        .run(tauri::generate_context!())
        .expect("error while running feiq++");
}

/// Get the user's home directory (~)
fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}
