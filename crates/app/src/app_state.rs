//! Shared application state.
//! Uses granular locking and watch channels per the Decision Log.

use tokio::sync::watch;

#[derive(Clone)]
pub struct AppState {
    pub fcm_connected: watch::Receiver<bool>,
    pub steam_logged_in: watch::Receiver<bool>,
    pub steam_tx: std::sync::Arc<watch::Sender<bool>>,
    pub servers_rx: watch::Receiver<Vec<crate::config::store::ServerConfig>>,
    pub servers_tx: std::sync::Arc<watch::Sender<Vec<crate::config::store::ServerConfig>>>,
    pub devices_rx: watch::Receiver<Vec<crate::config::store::DeviceConfig>>,
    pub devices_tx: std::sync::Arc<watch::Sender<Vec<crate::config::store::DeviceConfig>>>,
    pub pending_pair_rx: watch::Receiver<Option<crate::config::store::ServerConfig>>,
    pub pending_pair_tx: std::sync::Arc<watch::Sender<Option<crate::config::store::ServerConfig>>>,
    pub ui_context: std::sync::Arc<std::sync::Mutex<Option<eframe::egui::Context>>>,
    pub is_ui_visible: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl AppState {
    pub fn new(
        fcm_connected: watch::Receiver<bool>,
        steam_logged_in: watch::Receiver<bool>,
        steam_tx: watch::Sender<bool>,
        servers_rx: watch::Receiver<Vec<crate::config::store::ServerConfig>>,
        servers_tx: watch::Sender<Vec<crate::config::store::ServerConfig>>,
        devices_rx: watch::Receiver<Vec<crate::config::store::DeviceConfig>>,
        devices_tx: watch::Sender<Vec<crate::config::store::DeviceConfig>>,
        pending_pair_rx: watch::Receiver<Option<crate::config::store::ServerConfig>>,
        pending_pair_tx: watch::Sender<Option<crate::config::store::ServerConfig>>,
    ) -> Self {
        Self {
            fcm_connected,
            steam_logged_in,
            steam_tx: std::sync::Arc::new(steam_tx),
            servers_rx,
            servers_tx: std::sync::Arc::new(servers_tx),
            devices_rx,
            devices_tx: std::sync::Arc::new(devices_tx),
            pending_pair_rx,
            pending_pair_tx: std::sync::Arc::new(pending_pair_tx),
            ui_context: std::sync::Arc::new(std::sync::Mutex::new(None)),
            is_ui_visible: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }
}
