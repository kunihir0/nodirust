use crate::config::store::{DeviceConfig, ServerConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Reconnecting,
    AuthenticationFailed,
    Unreachable,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackLevel {
    Success,
    Error,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct UiFeedback {
    pub level: FeedbackLevel,
    pub message: String,
}

impl UiFeedback {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            level: FeedbackLevel::Success,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: FeedbackLevel::Error,
            message: message.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LastEvent {
    pub title: String,
    pub detail: String,
    pub received_at_epoch_seconds: u64,
}

impl LastEvent {
    pub fn new(title: impl Into<String>, detail: impl Into<String>) -> Self {
        let received_at_epoch_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            title: title.into(),
            detail: detail.into(),
            received_at_epoch_seconds,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FullState {
    pub push_status: ConnectionStatus,
    pub steam_logged_in: bool,
    pub servers: Vec<ServerConfig>,
    pub devices: Vec<DeviceConfig>,
    pub server_statuses: HashMap<String, ConnectionStatus>,
    pub pending_pair: Option<ServerConfig>,
    pub last_event: Option<LastEvent>,
    pub feedback: Option<UiFeedback>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum IpcEvent {
    FullState(FullState),
    PushStatusChanged(ConnectionStatus),
    SteamStatusChanged(bool),
    ServerStatusChanged {
        server_ip_port: String,
        status: ConnectionStatus,
    },
    PendingPairChanged(Option<ServerConfig>),
    DevicesChanged(Vec<DeviceConfig>),
    ServersChanged(Vec<ServerConfig>),
    ServerStatusesUpdated(HashMap<String, ConnectionStatus>),
    LastEventChanged(Option<LastEvent>),
    FeedbackChanged(Option<UiFeedback>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum IpcCommand {
    UpdateServers(Vec<ServerConfig>),
    UpdateDevices(Vec<DeviceConfig>),
    RefreshSteamStatus,
    UnlinkSteam,
    DeclinePairing,
    AcceptPairing(ServerConfig),
    SendTestNotification,
}
