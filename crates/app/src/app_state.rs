//! Shared application state.
//! Uses granular locking and watch channels per the Decision Log.

use tokio::sync::watch;

#[derive(Clone)]
pub struct AppState {
    pub push_status: watch::Receiver<crate::ipc::ConnectionStatus>,
    pub push_tx: std::sync::Arc<watch::Sender<crate::ipc::ConnectionStatus>>,
    pub steam_logged_in: watch::Receiver<bool>,
    pub steam_tx: std::sync::Arc<watch::Sender<bool>>,
    pub servers_rx: watch::Receiver<Vec<crate::config::store::ServerConfig>>,
    pub servers_tx: std::sync::Arc<watch::Sender<Vec<crate::config::store::ServerConfig>>>,
    pub devices_rx: watch::Receiver<Vec<crate::config::store::DeviceConfig>>,
    pub devices_tx: std::sync::Arc<watch::Sender<Vec<crate::config::store::DeviceConfig>>>,
    pub pending_pair_rx: watch::Receiver<Option<crate::config::store::ServerConfig>>,
    pub pending_pair_tx: std::sync::Arc<watch::Sender<Option<crate::config::store::ServerConfig>>>,
    pub server_statuses_rx:
        watch::Receiver<std::collections::HashMap<String, crate::ipc::ConnectionStatus>>,
    pub server_statuses_tx: std::sync::Arc<
        watch::Sender<std::collections::HashMap<String, crate::ipc::ConnectionStatus>>,
    >,
    pub last_event_rx: watch::Receiver<Option<crate::ipc::LastEvent>>,
    pub last_event_tx: std::sync::Arc<watch::Sender<Option<crate::ipc::LastEvent>>>,
    pub feedback_rx: watch::Receiver<Option<crate::ipc::UiFeedback>>,
    pub feedback_tx: std::sync::Arc<watch::Sender<Option<crate::ipc::UiFeedback>>>,
    pub ui_context: std::sync::Arc<std::sync::Mutex<Option<eframe::egui::Context>>>,
    command_tx: tokio::sync::mpsc::Sender<crate::ipc::IpcCommand>,
}

impl AppState {
    pub fn new(
        steam_logged_in: bool,
        servers: Vec<crate::config::store::ServerConfig>,
        devices: Vec<crate::config::store::DeviceConfig>,
    ) -> (Self, tokio::sync::mpsc::Receiver<crate::ipc::IpcCommand>) {
        let initial_push_status = if steam_logged_in {
            crate::ipc::ConnectionStatus::Connecting
        } else {
            crate::ipc::ConnectionStatus::SignedOut
        };
        let (push_tx, push_status) = watch::channel(initial_push_status);
        let (steam_tx, steam_logged_in) = watch::channel(steam_logged_in);
        let (servers_tx, servers_rx) = watch::channel(servers);
        let (devices_tx, devices_rx) = watch::channel(devices);
        let (pending_pair_tx, pending_pair_rx) = watch::channel(None);
        let (server_statuses_tx, server_statuses_rx) =
            watch::channel(std::collections::HashMap::new());
        let (last_event_tx, last_event_rx) = watch::channel(None);
        let (feedback_tx, feedback_rx) = watch::channel(None);
        let (command_tx, command_rx) = tokio::sync::mpsc::channel(100);

        let state = Self {
            push_status,
            push_tx: std::sync::Arc::new(push_tx),
            steam_logged_in,
            steam_tx: std::sync::Arc::new(steam_tx),
            servers_rx,
            servers_tx: std::sync::Arc::new(servers_tx),
            devices_rx,
            devices_tx: std::sync::Arc::new(devices_tx),
            pending_pair_rx,
            pending_pair_tx: std::sync::Arc::new(pending_pair_tx),
            server_statuses_rx,
            server_statuses_tx: std::sync::Arc::new(server_statuses_tx),
            last_event_rx,
            last_event_tx: std::sync::Arc::new(last_event_tx),
            feedback_rx,
            feedback_tx: std::sync::Arc::new(feedback_tx),
            ui_context: std::sync::Arc::new(std::sync::Mutex::new(None)),
            command_tx,
        };
        (state, command_rx)
    }

    pub fn send_command(&self, command: crate::ipc::IpcCommand) -> bool {
        let result = self.command_tx.try_send(command).is_ok();

        if !result {
            let _ = self.feedback_tx.send(Some(crate::ipc::UiFeedback::error(
                "The background service is busy. Please try again.",
            )));
        }

        result
    }
}
