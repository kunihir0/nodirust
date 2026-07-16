//! Persistent storage for configuration and secure keychain access for tokens.

use serde::{Deserialize, Serialize};
use std::fmt;

static CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Configuration stored in the application's data directory.
#[derive(Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub servers: Vec<ServerConfig>,
    #[serde(default)]
    pub devices: Vec<DeviceConfig>,
    #[serde(default)]
    pub fcm_credentials: Option<FcmCredentials>,
    #[serde(default)]
    pub steam_token: Option<String>,
    #[serde(default)]
    pub fcm_persistent_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FcmCredentials {
    pub android_id: u64,
    pub security_token: u64,
    pub fcm_token: String,
    pub expo_push_token: String,
}

impl fmt::Debug for FcmCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FcmCredentials")
            .field("android_id", &self.android_id)
            .field("security_token", &"<redacted>")
            .field("fcm_token", &"<redacted>")
            .field("expo_push_token", &"<redacted>")
            .finish()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceConfig {
    pub entity_id: u32,
    pub entity_name: String,
    pub entity_type: u32,
    pub server_ip: String,
    pub server_port: u16,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    pub ip: String,
    pub port: u16,
    pub player_id: u64,
    // Sensitive token used to pair with the specific Rust server
    pub player_token: i32,
    #[serde(default)]
    pub name: Option<String>,
}

// Implement a custom Debug trait to redact the sensitive token, satisfying AGENTS.md Section 12.
impl fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerConfig")
            .field("ip", &self.ip)
            .field("port", &self.port)
            .field("player_id", &self.player_id)
            .field("player_token", &"<redacted>")
            .field("name", &self.name)
            .finish()
    }
}

pub struct Store;

impl Store {
    /// Retrieves the Steam authentication token from disk.
    pub fn get_steam_token() -> Result<String, String> {
        let config = Self::get_config();
        config
            .steam_token
            .ok_or_else(|| "No token found".to_string())
    }

    /// Persists the Steam authentication token to disk.
    pub fn set_steam_token(token: &str) -> Result<(), String> {
        Self::update_config(|config| config.steam_token = Some(token.to_string()))
    }

    /// Removes all account-bound state so the next login starts a fresh session.
    pub fn sign_out() -> Result<(), String> {
        Self::update_config(|config| {
            config.steam_token = None;
            config.fcm_credentials = None;
            config.fcm_persistent_ids.clear();
            config.servers.clear();
            config.devices.clear();
        })
    }

    pub fn get_servers() -> Vec<ServerConfig> {
        let config = Self::get_config();
        config.servers
    }

    pub fn get_devices() -> Vec<DeviceConfig> {
        let config = Self::get_config();
        config.devices
    }

    pub fn get_config() -> AppConfig {
        let _guard = config_guard();
        Self::read_config()
    }

    pub fn update_config<T>(update: impl FnOnce(&mut AppConfig) -> T) -> Result<T, String> {
        let _guard = config_guard();
        let mut config = Self::read_config();
        let result = update(&mut config);
        Self::write_config(&config)
            .map(|()| result)
            .map_err(|error| error.to_string())
    }

    fn read_config() -> AppConfig {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "nodirust", "nodirust") {
            let config_dir = proj_dirs.config_dir();
            let config_file = config_dir.join("config.json");
            if let Ok(contents) = std::fs::read_to_string(&config_file)
                && let Ok(config) = serde_json::from_str(&contents)
            {
                return config;
            }
        }
        AppConfig::default()
    }

    fn write_config(config: &AppConfig) -> Result<(), std::io::Error> {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "nodirust", "nodirust") {
            let config_dir = proj_dirs.config_dir();
            std::fs::create_dir_all(config_dir)?;
            let config_file = config_dir.join("config.json");
            let json = serde_json::to_string_pretty(config).map_err(std::io::Error::other)?;
            std::fs::write(&config_file, json)?;
        }
        Ok(())
    }

    pub fn get_ipc_socket_path() -> std::path::PathBuf {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "nodirust", "nodirust") {
            // Use runtime_dir if available (e.g. /run/user/1000 on Linux, missing on macOS)
            // fallback to config_dir (e.g. ~/Library/Application Support/com.nodirust.nodirust)
            let dir = proj_dirs
                .runtime_dir()
                .unwrap_or_else(|| proj_dirs.config_dir());
            let _ = std::fs::create_dir_all(dir);
            dir.join("nodirust.sock")
        } else {
            std::path::PathBuf::from("/tmp/nodirust.sock")
        }
    }
}

fn config_guard() -> std::sync::MutexGuard<'static, ()> {
    match CONFIG_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
