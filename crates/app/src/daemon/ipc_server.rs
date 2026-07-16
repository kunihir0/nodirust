use crate::app_state::AppState;
use crate::config::store::Store;
use crate::ipc::{FullState, IpcCommand, IpcEvent};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(target_os = "windows")]
use tokio::net::windows::named_pipe::ServerOptions;
#[cfg(target_os = "macos")]
use tokio::net::UnixListener;

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
        fcm_connected: *app_state.fcm_connected.borrow(),
        steam_logged_in: *app_state.steam_logged_in.borrow(),
        servers: app_state.servers_rx.borrow().clone(),
        devices: app_state.devices_rx.borrow().clone(),
        server_statuses: app_state.server_statuses_rx.borrow().clone(),
        pending_pair: app_state.pending_pair_rx.borrow().clone(),
    });

    let mut serialized = serde_json::to_string(&handshake).unwrap();
    serialized.push('\n');
    write_half.write_all(serialized.as_bytes()).await?;

    // 2. Set up multiplexing: Read from pipe vs Read from watch channels
    let mut reader = BufReader::new(read_half).lines();

    let mut fcm_rx = app_state.fcm_connected.clone();
    let mut steam_rx = app_state.steam_logged_in.clone();
    let mut servers_rx = app_state.servers_rx.clone();
    let mut devices_rx = app_state.devices_rx.clone();
    let mut statuses_rx = app_state.server_statuses_rx.clone();
    let mut pending_rx = app_state.pending_pair_rx.clone();

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
            Ok(()) = fcm_rx.changed() => {
                let evt = IpcEvent::FcmStatusChanged(*fcm_rx.borrow());
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
        IpcCommand::UpdateServers(servers) => {
            let mut config = Store::get_config();
            config.servers.clone_from(&servers);
            let _ = Store::save_config(&config);
            let _ = app_state.servers_tx.send(servers);
        }
        IpcCommand::UpdateDevices(devices) => {
            let mut config = Store::get_config();
            config.devices.clone_from(&devices);
            let _ = Store::save_config(&config);
            let _ = app_state.devices_tx.send(devices);
        }
        IpcCommand::SetSteamToken(token_opt) => {
            if let Some(_token) = token_opt {
                // Actually the token is saved directly by auth.rs.
                // The UI process can just tell the daemon to refresh steam status.
            } else {
                let _ = Store::delete_steam_token();
            }
            let _ = app_state.steam_tx.send(Store::get_steam_token().is_ok());
        }
        IpcCommand::DeclinePairing => {
            let _ = app_state.pending_pair_tx.send(None);
        }
        IpcCommand::AcceptPairing(server) => {
            let mut config = Store::get_config();
            config.servers.push(server);
            let _ = Store::save_config(&config);
            let _ = app_state.servers_tx.send(config.servers);
            let _ = app_state.pending_pair_tx.send(None);
        }
    }
}
