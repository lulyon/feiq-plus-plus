//! Tauri managed application state

use feiq_core::engine::engine::Engine;
use feiq_core::engine::events::FrontendEvent;
use feiq_core::storage::settings::AppConfig;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Application state managed by Tauri
pub struct AppState {
    pub engine: Arc<Mutex<Engine>>,
    pub config: Arc<Mutex<AppConfig>>,
    pub event_rx: Arc<Mutex<mpsc::UnboundedReceiver<FrontendEvent>>>,
    pub event_tx: mpsc::UnboundedSender<FrontendEvent>,
    pub running: Arc<Mutex<bool>>,
}

impl AppState {
    pub fn new(config: AppConfig, history_db_path: PathBuf) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel::<FrontendEvent>();
        let engine = Engine::new(config.clone(), event_tx.clone(), Some(history_db_path));

        Self {
            engine: Arc::new(Mutex::new(engine)),
            config: Arc::new(Mutex::new(config)),
            event_rx: Arc::new(Mutex::new(event_rx)),
            event_tx,
            running: Arc::new(Mutex::new(false)),
        }
    }
}
