use crate::app_state::AppState;
use crate::config::store::Store;
use crate::ipc::{FullState, IpcCommand, IpcEvent};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(target_os = "macos")]
use tokio::net::UnixListener;
#[cfg(target_os = "windows")]
use tokio::net::windows::named_pipe::ServerOptions;

pub async fn run_ipc_server(app_state: AppState) {
    #[cfg(target_os = "macos")]
    {
        let socket_path = crate::config::store::Store::get_ipc_socket_path();
        let _ = std::fs::remove_file(&socket_path); // Ensure clean bind
        match UnixListener::bind(&socket_path) {
            Ok(listener) => {
                tracing::info!("IPC Server listening on {}", socket_path.display());
                while let Ok((stream, _)) = listener.accept().await {
                    let state = app_state.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, state).await {
                            tracing::warn!("IPC client error: {}", e);
                        }
                    });
                }
            }
            Err(e) => {
                tracing::error!("Failed to bind IPC socket: {}", e);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        const PIPE_NAME: &str = r"\\.\pipe\nodirust_ipc";
        match ServerOptions::new()
            .first_pipe_instance(true)
            .create(PIPE_NAME)
        {
            Ok(mut server) => {
                tracing::info!("IPC Server listening on {}", PIPE_NAME);
                loop {
                    if let Err(e) = server.connect().await {
                        tracing::error!("Named pipe connect error: {}", e);
                        break;
                    }
                    let state = app_state.clone();
                    let stream = server;

                    // Create the next server instance to accept future connections
                    server = match ServerOptions::new().create(PIPE_NAME) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::error!("Failed to recreate IPC pipe: {}", e);
                            break;
                        }
                    };

                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, state).await {
                            tracing::warn!("IPC client error: {}", e);
                        }
                    });
                }
            }
            Err(e) => {
                tracing::error!("Failed to bind IPC pipe: {}", e);
            }
        }
    }
}

async fn handle_client<S>(stream: S, app_state: AppState) -> std::io::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    tracing::info!("UI Client connected via IPC");

    // Split into read/write half
    let (read_half, mut write_half) = tokio::io::split(stream);

    // 1. Send Handshake
    let handshake = IpcEvent::FullState(FullState {
        push_status: *app_state.push_status.borrow(),
        steam_logged_in: *app_state.steam_logged_in.borrow(),
        servers: app_state.servers_rx.borrow().clone(),
        devices: app_state.devices_rx.borrow().clone(),
        server_statuses: app_state.server_statuses_rx.borrow().clone(),
        pending_pair: app_state.pending_pair_rx.borrow().clone(),
        last_event: app_state.last_event_rx.borrow().clone(),
        feedback: app_state.feedback_rx.borrow().clone(),
    });

    let mut serialized = serde_json::to_string(&handshake).unwrap();
    serialized.push('\n');
    write_half.write_all(serialized.as_bytes()).await?;

    // 2. Set up multiplexing: Read from pipe vs Read from watch channels
    let mut reader = BufReader::new(read_half).lines();

    let mut push_rx = app_state.push_status.clone();
    let mut steam_rx = app_state.steam_logged_in.clone();
    let mut servers_rx = app_state.servers_rx.clone();
    let mut devices_rx = app_state.devices_rx.clone();
    let mut statuses_rx = app_state.server_statuses_rx.clone();
    let mut pending_rx = app_state.pending_pair_rx.clone();
    let mut last_event_rx = app_state.last_event_rx.clone();
    let mut feedback_rx = app_state.feedback_rx.clone();

    loop {
        tokio::select! {
            line = reader.next_line() => {
                if let Ok(Some(text)) = line {
                    if let Ok(cmd) = serde_json::from_str::<IpcCommand>(&text) {
                        handle_command(cmd, &app_state);
                    }
                } else {
                    tracing::info!("UI Client disconnected.");
                    break;
                }
            }
            Ok(()) = push_rx.changed() => {
                let evt = IpcEvent::PushStatusChanged(*push_rx.borrow());
                send_event(&mut write_half, evt).await?;
            }
            Ok(()) = steam_rx.changed() => {
                let evt = IpcEvent::SteamStatusChanged(*steam_rx.borrow());
                send_event(&mut write_half, evt).await?;
            }
            Ok(()) = servers_rx.changed() => {
                let evt = IpcEvent::ServersChanged(servers_rx.borrow().clone());
                send_event(&mut write_half, evt).await?;
            }
            Ok(()) = devices_rx.changed() => {
                let evt = IpcEvent::DevicesChanged(devices_rx.borrow().clone());
                send_event(&mut write_half, evt).await?;
            }
            Ok(()) = pending_rx.changed() => {
                let evt = IpcEvent::PendingPairChanged(pending_rx.borrow().clone());
                send_event(&mut write_half, evt).await?;
            }
            Ok(()) = statuses_rx.changed() => {
                let evt = IpcEvent::ServerStatusesUpdated(statuses_rx.borrow().clone());
                send_event(&mut write_half, evt).await?;
            }
            Ok(()) = last_event_rx.changed() => {
                let evt = IpcEvent::LastEventChanged(last_event_rx.borrow().clone());
                send_event(&mut write_half, evt).await?;
            }
            Ok(()) = feedback_rx.changed() => {
                let evt = IpcEvent::FeedbackChanged(feedback_rx.borrow().clone());
                send_event(&mut write_half, evt).await?;
            }
        }
    }

    Ok(())
}

async fn send_event<W>(write_half: &mut W, evt: IpcEvent) -> std::io::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut serialized = serde_json::to_string(&evt).unwrap();
    serialized.push('\n');
    write_half.write_all(serialized.as_bytes()).await
}

fn handle_command(cmd: IpcCommand, app_state: &AppState) {
    match cmd {
        IpcCommand::RemoveServer { ip, port } => remove_server(&ip, port, app_state),
        IpcCommand::RemoveDevice {
            server_ip,
            server_port,
            entity_id,
        } => remove_device(&server_ip, server_port, entity_id, app_state),
        IpcCommand::SetDeviceEnabled {
            server_ip,
            server_port,
            entity_id,
            enabled,
        } => set_device_enabled(&server_ip, server_port, entity_id, enabled, app_state),
        IpcCommand::RefreshSteamStatus => refresh_steam_status(app_state),
        IpcCommand::DismissFeedback => dismiss_feedback(app_state),
        IpcCommand::ReconnectPush => reconnect_push(app_state),
        IpcCommand::SignOut => sign_out(app_state),
        IpcCommand::DeclinePairing => decline_pairing(app_state),
        IpcCommand::AcceptPairing(server) => accept_pairing(server, app_state),
        IpcCommand::SendTestNotification => send_test_notification(app_state),
    }
}

fn dismiss_feedback(app_state: &AppState) {
    let _ = app_state.feedback_tx.send(None);
}

fn reconnect_push(app_state: &AppState) {
    let next_generation = app_state.push_restart_rx.borrow().saturating_add(1);
    let _ = app_state.push_restart_tx.send(next_generation);
    send_success(app_state, "Reconnecting the Rust+ push service.");
}

fn remove_server(ip: &str, port: u16, app_state: &AppState) {
    let result = Store::update_config(|config| {
        remove_server_pairing(config, ip, port);
        (config.servers.clone(), config.devices.clone())
    });
    let (servers, devices) = match result {
        Ok(updated) => updated,
        Err(error) => {
            tracing::error!(%error, "Failed to remove server");
            send_error(app_state, "Could not remove the server.");
            return;
        }
    };

    let _ = app_state.servers_tx.send(servers);
    let _ = app_state.devices_tx.send(devices);
    let mut statuses = app_state.server_statuses_rx.borrow().clone();
    statuses.remove(&format!("{ip}:{port}"));
    let _ = app_state.server_statuses_tx.send(statuses);
    clear_matching_pending_pair(app_state, ip, port);
    send_success(app_state, "Server and its devices removed.");
}

fn remove_device(server_ip: &str, server_port: u16, entity_id: u32, app_state: &AppState) {
    let result = Store::update_config(|config| {
        config.devices.retain(|device| {
            device.entity_id != entity_id
                || device.server_ip != server_ip
                || device.server_port != server_port
        });
        config.devices.clone()
    });
    match result {
        Ok(devices) => {
            let _ = app_state.devices_tx.send(devices);
            send_success(app_state, "Smart device removed.");
        }
        Err(error) => {
            tracing::error!(%error, "Failed to remove smart device");
            send_error(app_state, "Could not remove the smart device.");
        }
    }
}

fn set_device_enabled(
    server_ip: &str,
    server_port: u16,
    entity_id: u32,
    enabled: bool,
    app_state: &AppState,
) {
    let result = Store::update_config(|config| {
        let mut found = false;
        for device in &mut config.devices {
            if device.entity_id == entity_id
                && device.server_ip == server_ip
                && device.server_port == server_port
            {
                device.enabled = enabled;
                found = true;
                break;
            }
        }
        (found, config.devices.clone())
    });
    match result {
        Ok((true, devices)) => {
            let _ = app_state.devices_tx.send(devices);
            send_success(app_state, "Alert preference saved.");
        }
        Ok((false, _)) => send_error(app_state, "That smart device is no longer paired."),
        Err(error) => {
            tracing::error!(%error, "Failed to save device alert preference");
            send_error(app_state, "Could not save the alert preference.");
        }
    }
}

fn refresh_steam_status(app_state: &AppState) {
    let logged_in = Store::get_steam_token().is_ok();
    let _ = app_state.steam_tx.send(logged_in);
    if logged_in {
        send_success(app_state, "Steam account linked.");
    } else {
        send_error(
            app_state,
            "Steam login did not produce a valid account token.",
        );
    }
}

fn sign_out(app_state: &AppState) {
    if let Err(error) = Store::sign_out() {
        tracing::error!(%error, "Failed to clear account state");
        send_error(app_state, "Could not sign out.");
        return;
    }
    let _ = app_state.steam_tx.send(false);
    let _ = app_state.servers_tx.send(Vec::new());
    let _ = app_state.devices_tx.send(Vec::new());
    let _ = app_state
        .server_statuses_tx
        .send(std::collections::HashMap::new());
    let _ = app_state.pending_pair_tx.send(None);
    let _ = app_state.last_event_tx.send(None);
    send_success(
        app_state,
        "Signed out. Local pairings and push credentials were removed.",
    );
}

fn decline_pairing(app_state: &AppState) {
    let _ = app_state.pending_pair_tx.send(None);
}

fn accept_pairing(server: crate::config::store::ServerConfig, app_state: &AppState) {
    match Store::update_config(|config| {
        upsert_server(&mut config.servers, server);
        config.servers.clone()
    }) {
        Ok(servers) => {
            let _ = app_state.servers_tx.send(servers);
            let _ = app_state.pending_pair_tx.send(None);
            send_success(app_state, "Server paired successfully.");
        }
        Err(error) => {
            tracing::error!(%error, "Failed to save paired server");
            send_error(app_state, "Could not save the paired server.");
        }
    }
}

fn remove_server_pairing(config: &mut crate::config::store::AppConfig, ip: &str, port: u16) {
    config
        .servers
        .retain(|server| server.ip != ip || server.port != port);
    config
        .devices
        .retain(|device| device.server_ip != ip || device.server_port != port);
}

fn clear_matching_pending_pair(app_state: &AppState, ip: &str, port: u16) {
    let should_clear = app_state
        .pending_pair_rx
        .borrow()
        .as_ref()
        .is_some_and(|server| server.ip == ip && server.port == port);
    if should_clear {
        let _ = app_state.pending_pair_tx.send(None);
    }
}

fn upsert_server(
    servers: &mut Vec<crate::config::store::ServerConfig>,
    server: crate::config::store::ServerConfig,
) {
    if let Some(existing) = servers
        .iter_mut()
        .find(|existing| existing.ip == server.ip && existing.port == server.port)
    {
        existing.player_id = server.player_id;
        existing.player_token = server.player_token;
        if server.name.is_some() {
            existing.name = server.name;
        }
    } else {
        servers.push(server);
    }
}

fn send_test_notification(app_state: &AppState) {
    let result = crate::notify::Notifier::push("Smart Alarm", "Your base is under attack!");
    match result {
        Ok(()) => send_success(
            app_state,
            "Notification submitted. If no banner appears, enable NODIrust in system settings.",
        ),
        Err(error) => {
            tracing::error!(%error, "Failed to send test notification");
            send_error(
                app_state,
                &format!("Could not send test notification: {error}"),
            );
        }
    }
}

fn send_success(app_state: &AppState, message: &str) {
    let _ = app_state
        .feedback_tx
        .send(Some(crate::ipc::UiFeedback::success(message)));
}

fn send_error(app_state: &AppState, message: &str) {
    let _ = app_state
        .feedback_tx
        .send(Some(crate::ipc::UiFeedback::error(message)));
}

#[cfg(test)]
mod tests {
    use super::{dismiss_feedback, reconnect_push, remove_server_pairing, upsert_server};
    use crate::app_state::AppState;
    use crate::config::store::{AppConfig, DeviceConfig, ServerConfig};

    fn server(ip: &str, port: u16, player_token: i32) -> ServerConfig {
        ServerConfig {
            ip: ip.to_string(),
            port,
            player_id: 42,
            player_token,
            name: None,
        }
    }

    fn device(ip: &str, port: u16, entity_id: u32) -> DeviceConfig {
        DeviceConfig {
            entity_id,
            entity_name: format!("Alarm {entity_id}"),
            entity_type: 2,
            server_ip: ip.to_string(),
            server_port: port,
            enabled: true,
        }
    }

    #[test]
    fn removing_server_cascades_to_its_devices_only() {
        let mut config = AppConfig {
            servers: vec![
                server("one.example", 28_082, 1),
                server("two.example", 28_083, 2),
            ],
            devices: vec![
                device("one.example", 28_082, 10),
                device("two.example", 28_083, 20),
            ],
            ..AppConfig::default()
        };

        remove_server_pairing(&mut config, "one.example", 28_082);

        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].ip, "two.example");
        assert_eq!(config.devices.len(), 1);
        assert_eq!(config.devices[0].entity_id, 20);
    }

    #[test]
    fn repairing_server_after_removal_uses_new_credentials() {
        let mut servers = vec![server("rust.example", 28_082, 1)];
        servers.clear();

        upsert_server(&mut servers, server("rust.example", 28_082, 99));

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].player_token, 99);
    }

    #[test]
    fn dismissing_feedback_clears_daemon_owned_state() {
        let (state, _commands) = AppState::new(false, Vec::new(), Vec::new());
        let _ = state
            .feedback_tx
            .send(Some(crate::ipc::UiFeedback::success("Saved")));

        dismiss_feedback(&state);

        assert!(state.feedback_rx.borrow().is_none());
    }

    #[test]
    fn reconnecting_push_advances_restart_generation() {
        let (state, _commands) = AppState::new(true, Vec::new(), Vec::new());

        reconnect_push(&state);

        assert_eq!(*state.push_restart_rx.borrow(), 1);
    }
}
