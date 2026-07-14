#![allow(clippy::collapsible_if)] // Nested ifs are cleaner for deep JSON optional unwrapping
#![allow(clippy::cast_possible_truncation)] // Entity type truncation from 64 to 32 is acceptable here
#![allow(clippy::map_entry)] // Entry API would require cloning the key for the log statement
#![allow(clippy::duration_suboptimal_units)] // from_secs(60) is fine

use crate::config::store::Store;
use crate::daemon::events::DaemonEvent;
use push_receiver::PushReceiver;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::sleep;

pub struct DaemonEngine {
    event_tx: mpsc::Sender<DaemonEvent>,
    servers_rx: tokio::sync::watch::Receiver<Vec<crate::config::store::ServerConfig>>,
}

impl DaemonEngine {
    pub fn new(event_tx: mpsc::Sender<DaemonEvent>, servers_rx: tokio::sync::watch::Receiver<Vec<crate::config::store::ServerConfig>>) -> Self {
        Self { event_tx, servers_rx }
    }

    pub async fn run(mut self) {
        let mut set = JoinSet::new();

        let fcm_tx = self.event_tx.clone();
        set.spawn(async move {
            Self::fcm_loop(fcm_tx).await;
        });

        // We no longer fetch servers on startup since there is no centralized Facepunch API for it.
        // Paired servers are received over FCM push notifications and saved to config locally.
        // Load config to spawn rustplus websocket loops
        let initial_servers = Store::get_config().servers;
        let mut server_tasks: std::collections::HashMap<String, tokio::task::AbortHandle> = std::collections::HashMap::new();

        for server in initial_servers {
            let r_tx = self.event_tx.clone();
            let ip_port = format!("{}:{}", server.ip, server.port);
            let abort_handle = set.spawn(async move {
                Self::rustplus_loop(server, r_tx).await;
            });
            server_tasks.insert(ip_port, abort_handle);
        }

        loop {
            tokio::select! {
                res = set.join_next() => {
                    if let Some(res) = res {
                        if let Err(e) = res {
                            tracing::error!("Background task panicked: {}", e);
                        }
                    } else {
                        break;
                    }
                }
                Ok(()) = self.servers_rx.changed() => {
                    let new_servers = self.servers_rx.borrow().clone();
                    
                    // Stop removed servers
                    let mut new_ip_ports = std::collections::HashSet::new();
                    for s in &new_servers {
                        new_ip_ports.insert(format!("{}:{}", s.ip, s.port));
                    }
                    
                    let mut to_remove = Vec::new();
                    for ip_port in server_tasks.keys() {
                        if !new_ip_ports.contains(ip_port) {
                            to_remove.push(ip_port.clone());
                        }
                    }
                    
                    for ip_port in to_remove {
                        if let Some(handle) = server_tasks.remove(&ip_port) {
                            tracing::info!("Stopping connection to removed server {}", ip_port);
                            handle.abort();
                        }
                    }
                    
                    // Start new servers
                    for server in &new_servers {
                        let ip_port = format!("{}:{}", server.ip, server.port);
                        if !server_tasks.contains_key(&ip_port) {
                            tracing::info!("Starting connection to new server {}", ip_port);
                            let r_tx = self.event_tx.clone();
                            let srv = server.clone();
                            let abort_handle = set.spawn(async move {
                                Self::rustplus_loop(srv, r_tx).await;
                            });
                            server_tasks.insert(ip_port, abort_handle);
                        }
                    }
                    
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)] // FCM loop is monolithic by design
    async fn fcm_loop(event_tx: mpsc::Sender<DaemonEvent>) {
        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(60);

        loop {
            tracing::info!("Starting FCM lifecycle...");

            let mut config = Store::get_config();
            let creds = if let Some(c) = config.fcm_credentials.clone() {
                c
            } else {
                tracing::info!("No FCM credentials found. Registering new device...");
                let http = reqwest::Client::new();
                
                // Magic values from CLI prototype
                let api_key = "AIzaSyB5y2y-Tzqb4-I4Qnlsh_9naYv_TD8pCvY";
                let project_id = "rust-companion-app";
                let gcm_sender_id = "976529667804";
                let gms_app_id = "1:976529667804:android:d6f1ddeb4403b338fea619";
                let pkg_name = "com.facepunch.rust.companion";
                let pkg_cert = "38918a453d07199354f8b19af05ec6562ced5788";

                match push_receiver::AndroidFcm::register(
                    &http,
                    api_key,
                    project_id,
                    gcm_sender_id,
                    gms_app_id,
                    pkg_name,
                    pkg_cert,
                ).await {
                    Ok(reg) => {
                        tracing::info!("Swapping FCM token for Expo token...");
                        let device_id = uuid::Uuid::new_v4().to_string();

                        match crate::daemon::expo::get_expo_push_token(&http, &reg.fcm.token, &device_id).await {
                            Ok(expo_token) => {
                                let c = crate::config::store::FcmCredentials {
                                    android_id: reg.gcm.android_id,
                                    security_token: reg.gcm.security_token,
                                    fcm_token: reg.fcm.token,
                                    expo_push_token: expo_token,
                                };
                                config.fcm_credentials = Some(c.clone());
                                let _ = Store::save_config(&config);
                                tracing::info!("FCM & Expo registration successful.");
                                c
                            }
                            Err(e) => {
                                tracing::error!("Failed to get Expo token: {e}");
                                sleep(backoff).await;
                                backoff = std::cmp::min(backoff * 2, max_backoff);
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("FCM registration failed: {e}");
                        sleep(backoff).await;
                        backoff = std::cmp::min(backoff * 2, max_backoff);
                        continue;
                    }
                }
            };

            // Register with Facepunch to route notifications to this Expo token
            if let Ok(steam_token) = Store::get_steam_token() {
                let fp = crate::facepunch::FacepunchClient::new(steam_token);
                if let Err(e) = fp.register_push(&creds.expo_push_token).await {
                    tracing::error!("Failed to register Expo token with Facepunch: {}", e);
                } else {
                    tracing::info!("Successfully registered Expo token with Facepunch!");
                }
            }

            tracing::info!("Connecting to MCS stream...");
            // Now we listen using the raw credentials
            let mut builder = PushReceiver::builder("976529667804");
            let pids = Store::get_config().fcm_persistent_ids;
            if !pids.is_empty() {
                builder = builder.persistent_ids(pids);
            }
            match builder.listen(creds.android_id, creds.security_token).await {
                Ok((_receiver, mut message_stream)) => {
                    tracing::info!("Connected to MCS.");
                    backoff = Duration::from_secs(1);
                    let _ = event_tx.try_send(DaemonEvent::ConnectionStatusChanged(true));

                    // Listen for incoming push notifications
                    while let Some(notification) = message_stream.recv().await {
                        if let Some(pid) = notification.persistent_id.clone() {
                            let mut cfg = Store::get_config();
                            if !cfg.fcm_persistent_ids.contains(&pid) {
                                cfg.fcm_persistent_ids.push(pid);
                                // Keep only last 100 to avoid unbounded growth
                                if cfg.fcm_persistent_ids.len() > 100 {
                                    cfg.fcm_persistent_ids.remove(0);
                                }
                                let _ = Store::save_config(&cfg);
                            }
                        }

                        // Ignore old notifications (e.g., older than 5 minutes)
                        if let Some(sent) = notification.sent {
                            let now_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as i64;
                            // If `sent` is smaller than 20 billion, it's likely in seconds, so multiply by 1000
                            let sent_ms = if sent < 20_000_000_000 { sent * 1000 } else { sent };
                            let age_ms = now_ms.saturating_sub(sent_ms);
                            
                            // 5 minutes in ms = 300_000
                            if age_ms > 300_000 {
                                tracing::info!("Ignoring old notification ({} ms old)", age_ms);
                                continue;
                            }
                        }

                        let mut title = "Rust+".to_string();
                        let mut body = "Unknown event".to_string();

                        let mut json_payload = None;
                        
                        // 1. Try parsing decrypted payload (used for smart alarms)
                        if let Ok(text) = String::from_utf8(notification.decrypted.clone()) {
                            tracing::info!("Received Decrypted Push Payload: {}", text);
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                json_payload = Some(json);
                            } else if !text.trim().is_empty() {
                                body = text;
                            }
                        }

                        // 2. Try parsing unencrypted FCM data payload (used for server pairing)
                        if json_payload.is_none() {
                            for app_data in &notification.app_data {
                                if app_data.key == "body" {
                                    tracing::info!("Received Unencrypted AppData Body: {}", app_data.value);
                                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&app_data.value) {
                                        json_payload = Some(json);
                                    }
                                }
                            }
                        }

                        // 2. Try parsing unencrypted FCM data payload
                        if json_payload.is_none() {
                            for app_data in &notification.app_data {
                                if app_data.key == "body" {
                                    tracing::info!("Received Unencrypted AppData Body: {}", app_data.value);
                                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&app_data.value) {
                                        json_payload = Some(json);
                                    }
                                }
                            }
                        }

                        let mut is_pairing = false;
                        let mut req_type = String::new();
                        let mut server_ip = String::new();
                        let mut server_port = 0;

                        if let Some(json) = &json_payload {
                            req_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            
                            // Infer type if missing
                            if req_type.is_empty() {
                                if json.get("entityId").is_some() && json.get("ip").is_some() {
                                    req_type = "entity".to_string();
                                } else if json.get("playerId").is_some() && json.get("playerToken").is_some() && json.get("ip").is_some() {
                                    req_type = "server".to_string();
                                }
                            }
                            
                            if let Some(ip) = json.get("ip").and_then(|v| v.as_str()) {
                                server_ip = ip.to_string();
                                if let Some(port) = json.get("port").and_then(|v| v.as_u64().map(|u| u as u16).or_else(|| v.as_str().and_then(|s| s.parse().ok()))) {
                                    server_port = port;
                                    if let Some(player_id) = json.get("playerId").and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))) {
                                        if let Some(player_token) = json.get("playerToken").and_then(|v| v.as_i64().map(|i| i as i32).or_else(|| v.as_str().and_then(|s| s.parse().ok()))) {
                                            if req_type == "server" {
                                                let server = crate::config::store::ServerConfig {
                                                    ip: ip.to_string(),
                                                    port,
                                                    player_id,
                                                    player_token,
                                                };
                                                let _ = event_tx.try_send(DaemonEvent::PairingRequest(server));
                                                is_pairing = true;
                                            }
                                        }
                                    }
                                    
                                    // Entity pairing doesn't necessarily need playerToken in the same way, but it usually comes with ip/port
                                    if req_type == "entity" {
                                        if let Some(entity_id) = json.get("entityId").and_then(|v| v.as_u64().map(|u| u as u32).or_else(|| v.as_str().and_then(|s| s.parse().ok()))) {
                                            if let Some(entity_type) = json.get("entityType").and_then(|v| v.as_u64().map(|u| u as u32).or_else(|| v.as_str().and_then(|s| s.parse().ok()))) {
                                                let entity_name = json.get("entityName").and_then(|v| v.as_str()).unwrap_or("Smart Device").to_string();
                                                let device = crate::config::store::DeviceConfig {
                                                    entity_id,
                                                    entity_name,
                                                    entity_type,
                                                    server_ip: ip.to_string(),
                                                    server_port: port,
                                                    enabled: true,
                                                };
                                                let _ = event_tx.try_send(DaemonEvent::EntityPairingRequest(device));
                                                is_pairing = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if is_pairing {
                            continue;
                        }

                        // For normal notifications (like 'alarm'), we want to extract the display text.
                        // Often FCM sends `title` and `message` in `app_data`.
                        for data in &notification.app_data {
                            if data.key == "title" {
                                title.clone_from(&data.value);
                            } else if data.key == "message" {
                                body.clone_from(&data.value);
                            } else if data.key == "body" && json_payload.is_none() {
                                // Only use 'body' as the notification body if it's not JSON
                                body.clone_from(&data.value);
                            }
                        }

                        // Fallback to JSON properties if app_data didn't have title/message
                        if let Some(json) = &json_payload {
                            if title == "Rust+" || title.is_empty() {
                                if let Some(t) = json.get("title").or(json.get("name")).and_then(|v| v.as_str()) {
                                    title = t.to_string();
                                }
                            }
                            if body == "Unknown event" || body.is_empty() {
                                if let Some(b) = json.get("message").and_then(|v| v.as_str()) {
                                    body = b.to_string();
                                } else if req_type == "alarm" {
                                    body = "Smart Alarm Triggered!".to_string();
                                }
                            }
                        }
                        
                        // Check if we should suppress the alarm based on user toggles
                        if req_type == "alarm" && !server_ip.is_empty() {
                            let config = Store::get_config();
                            let server_devices: Vec<_> = config.devices.iter().filter(|d| d.server_ip == server_ip && d.server_port == server_port).collect();
                            
                            // If we have paired devices for this server, and ALL of them are disabled, suppress the notification
                            if !server_devices.is_empty() && server_devices.iter().all(|d| !d.enabled) {
                                tracing::info!("Suppressing alarm for {}:{} because all devices are disabled.", server_ip, server_port);
                                continue;
                            }
                        }

                        let _ = event_tx
                            .try_send(DaemonEvent::PushNotificationReceived { title, body });
                    }

                    tracing::warn!("MCS stream ended. Reconnecting...");
                    let _ = event_tx.try_send(DaemonEvent::ConnectionStatusChanged(false));
                }
                Err(e) => {
                    tracing::error!("MCS connect failed: {}. Retrying in {:?}...", e, backoff);
                    let _ = event_tx.try_send(DaemonEvent::ConnectionStatusChanged(false));
                    sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, max_backoff);
                }
            }
        }
    }

    async fn rustplus_loop(
        server: crate::config::store::ServerConfig,
        event_tx: mpsc::Sender<DaemonEvent>,
    ) {
        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(30);
        let ip_port = format!("{}:{}", server.ip, server.port);

        loop {
            tracing::info!("Connecting to Rust+ Server: {}", ip_port);

            let mut client = rustplus::RustPlusClient::new(
                server.ip.clone(),
                server.port,
                server.player_id,
                server.player_token,
                false, // use_facepunch_proxy
            );

            match client.connect().await {
                Ok(()) => {
                    tracing::info!("Connected to Rust+ Server {}", ip_port);
                    let _ = event_tx.try_send(DaemonEvent::ServerConnectionStatusChanged {
                        server_ip_port: ip_port.clone(),
                        connected: true,
                    });
                    backoff = Duration::from_secs(1);

                    if let Some(mut broadcast_rx) = client.take_broadcast_receiver() {
                        while let Ok(msg) = broadcast_rx.recv().await {
                            if let Some(broadcast) = msg.broadcast {
                                if let Some(team_msg) = broadcast.team_message {
                                    let msg_data = team_msg.message;
                                    let _ = event_tx.try_send(DaemonEvent::ServerChatReceived {
                                        server_ip: server.ip.clone(),
                                        sender: msg_data.name,
                                        message: msg_data.message,
                                    });
                                }
                                if let Some(entity_changed) = broadcast.entity_changed {
                                    tracing::info!(
                                        "Entity {} changed payload: {:?}",
                                        entity_changed.entity_id,
                                        entity_changed.payload
                                    );
                                    // TODO: Update state or notify if this corresponds to a known smart switch or alarm
                                }
                            }
                        }
                    }

                    tracing::warn!(
                        "Rust+ connection dropped for {}. Reconnecting...",
                        server.ip
                    );
                    let _ = event_tx.try_send(DaemonEvent::ServerConnectionStatusChanged {
                        server_ip_port: ip_port.clone(),
                        connected: false,
                    });
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to connect to Rust+: {}. Retrying in {:?}...",
                        e,
                        backoff
                    );
                    let _ = event_tx.try_send(DaemonEvent::ServerConnectionStatusChanged {
                        server_ip_port: ip_port.clone(),
                        connected: false,
                    });
                    sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, max_backoff);
                }
            }
        }
    }
}
