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

    // Parse CLI: feiq++ --name Alice --port 2426
    let args: Vec<String> = std::env::args().collect();
    let mut cli_port: Option<u16> = None;
    let mut cli_name: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => { if i+1 < args.len() { cli_port = args[i+1].parse().ok(); i+=1; } }
            "--name" => { if i+1 < args.len() { cli_name = Some(args[i+1].clone()); i+=1; } }
            _ => {}
        }
        i += 1;
    }

    // Load config from ~/.feiq_setting.ini (or create default)
    let config_path = home_dir().join(".feiq_setting.ini");
    let mut config = feiq_core::storage::settings::AppConfig::load(&config_path)
        .unwrap_or_default();

    // Env var overrides (FEIQ_PORT=2426 FEIQ_NAME=Bob)
    if cli_port.is_none() { if let Ok(port) = std::env::var("FEIQ_PORT") { if let Ok(p) = port.parse() { config.port = p; } } }
    if cli_name.is_none() { if let Ok(name) = std::env::var("FEIQ_NAME") { config.name = name; } }
    // CLI overrides (highest priority)
    if let Some(port) = cli_port { config.port = port; }
    if let Some(name) = cli_name { config.name = name; }

    tracing::info!("Starting feiq++ as '{}' on port {}", config.name, config.port);

    let history_db_path = home_dir().join(".feiq_history.sqlite3");
    let app_state = AppState::new(config, history_db_path);

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .manage(app_state)
        .setup(|app| {
            let handle = app.handle().clone();

            // Init tray and store in managed state
            let tray_icon = tray::init_tray(&handle).map_err(|e| {
                Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            })?;
            handle.manage(state::TrayState { tray: tray_icon.clone() });

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
            commands::get_chat_history,
            commands::search_chat_history,
            commands::get_emoji_list,
            commands::send_knock,
            commands::send_text,
            commands::set_alias,
            commands::set_contact_group,
            commands::create_group,
            commands::get_groups,
            commands::send_group_text,
            commands::capture_screenshot,
            commands::export_history,
            commands::import_history,
            commands::add_to_blacklist,
            commands::remove_from_blacklist,
            commands::get_blacklist,
            commands::reset_unread_count,
            commands::download_file,
            commands::cancel_file_task,
            commands::send_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running feiq++");
}

/// Get the user's home directory (~)
fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}
