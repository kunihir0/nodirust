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
        IpcCommand::UpdateServers(servers) => update_servers(servers, app_state),
        IpcCommand::UpdateDevices(devices) => update_devices(devices, app_state),
        IpcCommand::RefreshSteamStatus => refresh_steam_status(app_state),
        IpcCommand::UnlinkSteam => unlink_steam(app_state),
        IpcCommand::DeclinePairing => decline_pairing(app_state),
        IpcCommand::AcceptPairing(server) => accept_pairing(server, app_state),
        IpcCommand::SendTestNotification => send_test_notification(app_state),
    }
}

fn update_servers(servers: Vec<crate::config::store::ServerConfig>, app_state: &AppState) {
    let mut config = Store::get_config();
    config.servers.clone_from(&servers);
    match Store::save_config(&config) {
        Ok(()) => {
            let _ = app_state.servers_tx.send(servers);
            send_success(app_state, "Server removed.");
        }
        Err(error) => {
            tracing::error!(%error, "Failed to save server changes");
            send_error(app_state, "Could not save the server change.");
        }
    }
}

fn update_devices(devices: Vec<crate::config::store::DeviceConfig>, app_state: &AppState) {
    let mut config = Store::get_config();
    config.devices.clone_from(&devices);
    match Store::save_config(&config) {
        Ok(()) => {
            let _ = app_state.devices_tx.send(devices);
            send_success(app_state, "Alert preference saved.");
        }
        Err(error) => {
            tracing::error!(%error, "Failed to save device changes");
            send_error(app_state, "Could not save the alert preference.");
        }
    }
}

fn refresh_steam_status(app_state: &AppState) {
    let logged_in = Store::get_steam_token().is_ok();
    let _ = app_state.steam_tx.send(logged_in);
    if logged_in {
        tokio::spawn(crate::daemon::push::register_with_facepunch_current());
        send_success(app_state, "Steam account linked.");
    } else {
        send_error(
            app_state,
            "Steam login did not produce a valid account token.",
        );
    }
}

fn unlink_steam(app_state: &AppState) {
    if let Err(error) = Store::delete_steam_token() {
        tracing::error!(%error, "Failed to delete Steam token");
        send_error(app_state, "Could not unlink the Steam account.");
        return;
    }
    let _ = app_state.steam_tx.send(false);
    send_success(app_state, "Steam account unlinked.");
}

fn decline_pairing(app_state: &AppState) {
    let _ = app_state.pending_pair_tx.send(None);
}

fn accept_pairing(server: crate::config::store::ServerConfig, app_state: &AppState) {
    let mut config = Store::get_config();
    upsert_server(&mut config.servers, server);
    match Store::save_config(&config) {
        Ok(()) => {
            let _ = app_state.servers_tx.send(config.servers);
            let _ = app_state.pending_pair_tx.send(None);
            send_success(app_state, "Server paired successfully.");
        }
        Err(error) => {
            tracing::error!(%error, "Failed to save paired server");
            send_error(app_state, "Could not save the paired server.");
        }
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
    let result = crate::notify::Notifier::push(
        "NODIrust test",
        "Desktop notifications are reaching this device.",
    );
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
