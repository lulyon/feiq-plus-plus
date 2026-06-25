//! Tauri managed application state

use feiq_core::engine::engine::Engine;
use feiq_core::engine::events::FrontendEvent;
use feiq_core::storage::settings::AppConfig;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Application state managed by Tauri
pub struct AppState {
    pub engine: Arc<Mutex<Engine>>,
    pub config: Arc<Mutex<AppConfig>>,
    /// Channel for sending events from engine to frontend
    pub event_rx: Arc<Mutex<mpsc::UnboundedReceiver<FrontendEvent>>>,
    /// Channel for sending events from engine to frontend (sender side, kept for engine)
    pub event_tx: mpsc::UnboundedSender<FrontendEvent>,
    /// Engine start/stop state
    pub running: Arc<Mutex<bool>>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel::<FrontendEvent>();
        let engine = Engine::new(config.clone(), event_tx.clone());

        Self {
            engine: Arc::new(Mutex::new(engine)),
            config: Arc::new(Mutex::new(config)),
            event_rx: Arc::new(Mutex::new(event_rx)),
            event_tx,
            running: Arc::new(Mutex::new(false)),
        }
    }
}
