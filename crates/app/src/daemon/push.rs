use crate::config::store::{DeviceConfig, FcmCredentials, ServerConfig, Store};
use crate::daemon::events::DaemonEvent;
use crate::ipc::ConnectionStatus;
use push_receiver::{Notification, PushReceiver};
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

const SENDER_ID: &str = "976529667804";
const API_KEY: &str = "AIzaSyB5y2y-Tzqb4-I4Qnlsh_9naYv_TD8pCvY";
const PROJECT_ID: &str = "rust-companion-app";
const GMS_APP_ID: &str = "1:976529667804:android:d6f1ddeb4403b338fea619";
const PACKAGE_NAME: &str = "com.facepunch.rust.companion";
const PACKAGE_CERT: &str = "38918a453d07199354f8b19af05ec6562ced5788";
const MAX_BACKOFF: Duration = Duration::from_mins(1);

pub async fn run(
    event_tx: mpsc::Sender<DaemonEvent>,
    mut steam_rx: tokio::sync::watch::Receiver<bool>,
    mut restart_rx: tokio::sync::watch::Receiver<u64>,
) {
    let mut backoff = Duration::from_secs(1);
    let mut first_attempt = true;
    loop {
        if !wait_until_logged_in(&event_tx, &mut steam_rx).await {
            return;
        }

        let status = if first_attempt {
            ConnectionStatus::Connecting
        } else {
            ConnectionStatus::Reconnecting
        };
        emit_status(&event_tx, status);
        first_attempt = false;

        let connection_result = tokio::select! {
            result = run_connection(&event_tx) => Some(result),
            changed = steam_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                None
            }
            changed = restart_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                tracing::info!(
                    generation = *restart_rx.borrow(),
                    "Push reconnect requested"
                );
                None
            }
        };

        match connection_result {
            None => {
                backoff = Duration::from_secs(1);
                first_attempt = true;
            }
            Some(Ok(())) => {
                backoff = Duration::from_secs(1);
                tracing::warn!("Push stream ended; reconnecting");
                emit_status(&event_tx, ConnectionStatus::Reconnecting);
            }
            Some(Err(error)) => {
                tracing::error!(%error, ?backoff, "Push connection failed");
                emit_status(&event_tx, ConnectionStatus::Unreachable);
                tokio::select! {
                    () = sleep(backoff) => {}
                    changed = steam_rx.changed() => {
                        if changed.is_err() {
                            return;
                        }
                    }
                }
                backoff = std::cmp::min(backoff * 2, MAX_BACKOFF);
            }
        }
    }
}

async fn wait_until_logged_in(
    event_tx: &mpsc::Sender<DaemonEvent>,
    steam_rx: &mut tokio::sync::watch::Receiver<bool>,
) -> bool {
    if *steam_rx.borrow() {
        return true;
    }

    emit_status(event_tx, ConnectionStatus::SignedOut);
    loop {
        if steam_rx.changed().await.is_err() {
            return false;
        }
        if *steam_rx.borrow() {
            return true;
        }
    }
}

async fn run_connection(event_tx: &mpsc::Sender<DaemonEvent>) -> Result<(), String> {
    let credentials = load_or_register_credentials().await?;
    register_with_facepunch(&credentials).await?;
    let mut builder = PushReceiver::builder(SENDER_ID);
    let persistent_ids = Store::get_config().fcm_persistent_ids;
    if !persistent_ids.is_empty() {
        builder = builder.persistent_ids(persistent_ids);
    }

    let (_receiver, mut stream) =
        builder.listen(credentials.android_id, credentials.security_token);
    emit_status(event_tx, ConnectionStatus::Connected);
    while let Some(notification) = stream.recv().await {
        process_notification(&notification, event_tx).await;
    }
    Ok(())
}

async fn load_or_register_credentials() -> Result<FcmCredentials, String> {
    if let Some(credentials) = Store::get_config().fcm_credentials {
        return Ok(credentials);
    }

    tracing::info!("Registering push notification device");
    let http = reqwest::Client::new();
    let registration = push_receiver::AndroidFcm::register(
        &http,
        API_KEY,
        PROJECT_ID,
        SENDER_ID,
        GMS_APP_ID,
        PACKAGE_NAME,
        PACKAGE_CERT,
    )
    .await
    .map_err(|error| format!("FCM registration failed: {error}"))?;
    let device_id = uuid::Uuid::new_v4().to_string();
    let expo_push_token =
        crate::daemon::expo::get_expo_push_token(&http, &registration.fcm.token, &device_id)
            .await
            .map_err(|error| format!("Expo token exchange failed: {error}"))?;

    let credentials = FcmCredentials {
        android_id: registration.gcm.android_id,
        security_token: registration.gcm.security_token,
        fcm_token: registration.fcm.token,
        expo_push_token,
    };
    Store::update_config(|config| config.fcm_credentials = Some(credentials.clone()))
        .map_err(|error| format!("Could not save push credentials: {error}"))?;
    Ok(credentials)
}

async fn register_with_facepunch(credentials: &FcmCredentials) -> Result<(), String> {
    let steam_token = Store::get_steam_token()?;
    let client = crate::facepunch::FacepunchClient::new(steam_token);
    client
        .register_push(&credentials.expo_push_token)
        .await
        .map_err(|error| format!("Facepunch push registration failed: {error}"))?;
    tracing::info!("Registered push token with Facepunch");
    Ok(())
}

async fn process_notification(notification: &Notification, event_tx: &mpsc::Sender<DaemonEvent>) {
    let app_data_keys: Vec<_> = notification
        .app_data
        .iter()
        .map(|entry| entry.key.as_str())
        .collect();
    tracing::debug!(
        payload_bytes = notification.decrypted.len(),
        persistent_id_present = notification.persistent_id.is_some(),
        sent = notification.sent,
        app_data_keys = ?app_data_keys,
        "Received push notification from MCS"
    );
    remember_persistent_id(notification.persistent_id.as_deref());
    if is_stale(notification.sent) {
        tracing::warn!(sent = notification.sent, "Ignoring stale push notification");
        return;
    }

    let (json, fallback_body) = extract_payload(notification);
    let kind = json.as_ref().map_or("", infer_kind);
    if let Some(payload) = json.as_ref() {
        let payload_keys: Vec<_> = payload
            .as_object()
            .map(|object| object.keys().map(String::as_str).collect())
            .unwrap_or_default();
        tracing::debug!(kind, payload_keys = ?payload_keys, "Decoded push JSON payload");
        match parse_pairing_event(payload, kind) {
            Ok(Some(event)) => {
                tracing::info!(kind, "Decoded Rust+ pairing notification");
                emit_event(event_tx, event).await;
                return;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(kind, %error, "Rejected malformed Rust+ pairing notification");
                return;
            }
        }
    } else {
        tracing::warn!(
            payload_bytes = notification.decrypted.len(),
            app_data_keys = ?app_data_keys,
            "Push notification did not contain a JSON payload"
        );
    }

    let endpoint = json.as_ref().and_then(parse_endpoint);
    let entity_id = json
        .as_ref()
        .and_then(|payload| payload.get("entityId"))
        .and_then(value_as_u32);
    if kind == "alarm"
        && endpoint.as_ref().is_some_and(|(ip, port)| {
            should_suppress_alarm(&Store::get_devices(), ip, *port, entity_id)
        })
    {
        tracing::info!("Suppressing disabled alarm notification");
        return;
    }

    let (title, body) = notification_text(notification, json.as_ref(), kind, fallback_body);
    emit_event(
        event_tx,
        DaemonEvent::PushNotificationReceived { title, body },
    )
    .await;
}

async fn emit_event(event_tx: &mpsc::Sender<DaemonEvent>, event: DaemonEvent) {
    if event_tx.send(event).await.is_err() {
        tracing::error!("Daemon event consumer stopped");
    }
}

fn remember_persistent_id(persistent_id: Option<&str>) {
    let Some(persistent_id) = persistent_id else {
        return;
    };
    if let Err(error) = Store::update_config(|config| {
        if config
            .fcm_persistent_ids
            .iter()
            .all(|known| known != persistent_id)
        {
            config.fcm_persistent_ids.push(persistent_id.to_string());
            let excess = config.fcm_persistent_ids.len().saturating_sub(100);
            config.fcm_persistent_ids.drain(..excess);
        }
    }) {
        tracing::error!(%error, "Failed to save push message identifier");
    }
}

fn is_stale(sent: Option<i64>) -> bool {
    let Some(sent) = sent else {
        return false;
    };
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(i64::MAX, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        });
    let sent_ms = if sent < 20_000_000_000 {
        sent.saturating_mul(1_000)
    } else {
        sent
    };
    now_ms.saturating_sub(sent_ms) > 300_000
}

fn extract_payload(notification: &Notification) -> (Option<Value>, Option<String>) {
    let decrypted = String::from_utf8(notification.decrypted.clone()).ok();
    if let Some(text) = decrypted.as_deref()
        && let Ok(json) = serde_json::from_str(text)
    {
        return (Some(normalize_payload(json)), None);
    }

    let json = app_data_value(notification, "body")
        .and_then(|body| serde_json::from_str(body).ok())
        .map(normalize_payload);
    let fallback = decrypted.filter(|text| !text.trim().is_empty());
    (json, fallback)
}

fn normalize_payload(mut value: Value) -> Value {
    loop {
        if let Some(data) = value.get("data").filter(|data| data.is_object()) {
            value = data.clone();
            continue;
        }
        let nested_body = value
            .get("body")
            .and_then(Value::as_str)
            .and_then(|body| serde_json::from_str(body).ok());
        if let Some(body) = nested_body {
            value = body;
            continue;
        }
        return value;
    }
}

fn infer_kind(json: &Value) -> &str {
    if let Some(kind) = json.get("type").and_then(Value::as_str) {
        return kind;
    }
    if json.get("entityId").is_some() && json.get("ip").is_some() {
        "entity"
    } else if json.get("playerId").is_some()
        && json.get("playerToken").is_some()
        && json.get("ip").is_some()
    {
        "server"
    } else {
        ""
    }
}

fn parse_pairing_event(json: &Value, kind: &str) -> Result<Option<DaemonEvent>, PairingParseError> {
    if !matches!(kind, "server" | "entity") {
        return Ok(None);
    }
    let (ip, port) = parse_pairing_endpoint(json)?;
    match kind {
        "server" => Ok(Some(DaemonEvent::PairingRequest(ServerConfig {
            ip,
            port,
            player_id: required_value(json, "playerId", value_as_u64)?,
            player_token: required_value(json, "playerToken", value_as_i32)?,
            name: json
                .get("name")
                .and_then(Value::as_str)
                .map(std::string::ToString::to_string),
        }))),
        "entity" => Ok(Some(DaemonEvent::EntityPairingRequest(DeviceConfig {
            entity_id: required_value(json, "entityId", value_as_u32)?,
            entity_name: json
                .get("entityName")
                .and_then(Value::as_str)
                .unwrap_or("Smart Device")
                .to_string(),
            entity_type: required_value(json, "entityType", value_as_u32)?,
            server_ip: ip,
            server_port: port,
            enabled: true,
        }))),
        _ => Ok(None),
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
enum PairingParseError {
    #[error("missing pairing field `{0}`")]
    MissingField(&'static str),
    #[error("invalid pairing field `{0}`")]
    InvalidField(&'static str),
}

fn required_value<T>(
    json: &Value,
    field: &'static str,
    parse: impl FnOnce(&Value) -> Option<T>,
) -> Result<T, PairingParseError> {
    let value = json
        .get(field)
        .ok_or(PairingParseError::MissingField(field))?;
    parse(value).ok_or(PairingParseError::InvalidField(field))
}

fn parse_pairing_endpoint(json: &Value) -> Result<(String, u16), PairingParseError> {
    let ip = required_value(json, "ip", |value| {
        value
            .as_str()
            .filter(|address| !address.trim().is_empty())
            .map(str::to_string)
    })?;
    let port = required_value(json, "port", value_as_u16)?;
    Ok((ip, port))
}

fn parse_endpoint(json: &Value) -> Option<(String, u16)> {
    let ip = json.get("ip")?.as_str()?.to_string();
    let port = json.get("port").and_then(value_as_u16)?;
    Some((ip, port))
}

fn notification_text(
    notification: &Notification,
    json: Option<&Value>,
    kind: &str,
    fallback_body: Option<String>,
) -> (String, String) {
    let title = app_data_value(notification, "title")
        .map(std::string::ToString::to_string)
        .or_else(|| json.and_then(json_title))
        .unwrap_or_else(|| "Rust+".to_string());
    let body = app_data_value(notification, "message")
        .map(std::string::ToString::to_string)
        .or(fallback_body)
        .or_else(|| json.and_then(json_message))
        .unwrap_or_else(|| {
            if kind == "alarm" {
                "Smart Alarm Triggered!".to_string()
            } else {
                "Unknown event".to_string()
            }
        });
    (title, body)
}

fn app_data_value<'a>(notification: &'a Notification, key: &str) -> Option<&'a str> {
    notification
        .app_data
        .iter()
        .find(|data| data.key == key)
        .map(|data| data.value.as_str())
}

fn json_title(json: &Value) -> Option<String> {
    json.get("title")
        .or_else(|| json.get("name"))
        .and_then(Value::as_str)
        .map(std::string::ToString::to_string)
}

fn json_message(json: &Value) -> Option<String> {
    json.get("message")
        .and_then(Value::as_str)
        .map(std::string::ToString::to_string)
}

fn value_as_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn value_as_u32(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn value_as_u16(value: &Value) -> Option<u16> {
    value
        .as_u64()
        .and_then(|number| u16::try_from(number).ok())
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn value_as_i32(value: &Value) -> Option<i32> {
    value
        .as_i64()
        .and_then(|number| i32::try_from(number).ok())
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn should_suppress_alarm(
    devices: &[DeviceConfig],
    server_ip: &str,
    server_port: u16,
    entity_id: Option<u32>,
) -> bool {
    let server_devices: Vec<_> = devices
        .iter()
        .filter(|device| device.server_ip == server_ip && device.server_port == server_port)
        .collect();
    if let Some(entity_id) = entity_id {
        return server_devices
            .iter()
            .find(|device| device.entity_id == entity_id)
            .is_some_and(|device| !device.enabled);
    }
    !server_devices.is_empty() && server_devices.iter().all(|device| !device.enabled)
}

fn emit_status(event_tx: &mpsc::Sender<DaemonEvent>, status: ConnectionStatus) {
    if let Err(error) = event_tx.try_send(DaemonEvent::ConnectionStatusChanged(status)) {
        tracing::error!(%error, "Could not publish push connection status");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PairingParseError, normalize_payload, parse_pairing_event, process_notification,
        should_suppress_alarm,
    };
    use crate::config::store::DeviceConfig;
    use crate::daemon::events::DaemonEvent;
    use push_receiver::Notification;
    use serde_json::json;

    fn device(entity_id: u32, enabled: bool) -> DeviceConfig {
        DeviceConfig {
            entity_id,
            entity_name: format!("Alarm {entity_id}"),
            entity_type: 2,
            server_ip: "127.0.0.1".to_string(),
            server_port: 28_082,
            enabled,
        }
    }

    #[test]
    fn suppresses_only_the_disabled_matching_device() {
        let devices = vec![device(1, false), device(2, true)];
        assert!(should_suppress_alarm(
            &devices,
            "127.0.0.1",
            28_082,
            Some(1)
        ));
        assert!(!should_suppress_alarm(
            &devices,
            "127.0.0.1",
            28_082,
            Some(2)
        ));
    }

    #[test]
    fn suppresses_unidentified_alarm_only_when_all_server_devices_are_disabled() {
        let devices = vec![device(1, false), device(2, false)];
        assert!(should_suppress_alarm(&devices, "127.0.0.1", 28_082, None));
    }

    #[tokio::test]
    async fn forwards_smart_alarm_to_daemon_event_queue() {
        let notification = Notification {
            decrypted:
                br#"{"type":"alarm","title":"Smart Alarm","message":"Your base is under attack!"}"#
                    .to_vec(),
            persistent_id: None,
            app_data: Vec::new(),
            sent: None,
        };
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(1);

        process_notification(&notification, &event_tx).await;

        let event = event_rx.recv().await;
        assert!(matches!(
            event,
            Some(DaemonEvent::PushNotificationReceived { title, body })
                if title == "Smart Alarm" && body == "Your base is under attack!"
        ));
    }

    #[tokio::test]
    async fn forwards_app_data_only_server_pairing() {
        let notification = Notification {
            decrypted: Vec::new(),
            persistent_id: Some("message-id".to_string()),
            app_data: vec![push_receiver::proto::AppData {
                key: "body".to_string(),
                value: r#"{"type":"server","ip":"127.0.0.1","port":"28082","playerId":"76561190000000000","playerToken":"-42","name":"Test Server"}"#.to_string(),
            }],
            sent: None,
        };
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(1);

        process_notification(&notification, &event_tx).await;

        let event = event_rx.recv().await;
        assert!(matches!(
            event,
            Some(DaemonEvent::PairingRequest(server))
                if server.ip == "127.0.0.1"
                    && server.port == 28_082
                    && server.player_token == -42
        ));
    }

    #[test]
    fn unwraps_nested_fcm_data_and_body_payload() {
        let payload = json!({
            "data": {
                "body": r#"{"type":"server","ip":"127.0.0.1"}"#
            }
        });

        let normalized = normalize_payload(payload);

        assert_eq!(
            normalized.get("type").and_then(serde_json::Value::as_str),
            Some("server")
        );
        assert_eq!(
            normalized.get("ip").and_then(serde_json::Value::as_str),
            Some("127.0.0.1")
        );
    }

    #[test]
    fn explains_missing_pairing_fields() {
        let payload = json!({"type": "server", "ip": "127.0.0.1", "port": 28082});

        let result = parse_pairing_event(&payload, "server");

        assert!(matches!(
            result,
            Err(PairingParseError::MissingField("playerId"))
        ));
    }
}
