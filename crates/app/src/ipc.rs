use crate::config::store::{DeviceConfig, ServerConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FullState {
    pub fcm_connected: bool,
    pub steam_logged_in: bool,
    pub servers: Vec<ServerConfig>,
    pub devices: Vec<DeviceConfig>,
    pub server_statuses: HashMap<String, bool>,
    pub pending_pair: Option<ServerConfig>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum IpcEvent {
    FullState(FullState),
    FcmStatusChanged(bool),
    SteamStatusChanged(bool),
    ServerStatusChanged {
        server_ip_port: String,
        connected: bool,
    },
    PendingPairChanged(Option<ServerConfig>),
    DevicesChanged(Vec<DeviceConfig>),
    ServersChanged(Vec<ServerConfig>),
    ServerStatusesUpdated(HashMap<String, bool>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum IpcCommand {
    UpdateServers(Vec<ServerConfig>),
    UpdateDevices(Vec<DeviceConfig>),
    SetSteamToken(Option<String>),
    DeclinePairing,
    AcceptPairing(ServerConfig),
}
