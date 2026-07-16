use crate::app_state::AppState;
use crate::ipc::{IpcCommand, IpcEvent};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(target_os = "macos")]
use tokio::net::UnixStream;
#[cfg(target_os = "windows")]
use tokio::net::windows::named_pipe::ClientOptions;

pub async fn run_ipc_client(
    app_state: AppState,
    command_rx: tokio::sync::mpsc::Receiver<IpcCommand>,
) {
    #[cfg(target_os = "macos")]
    {
        let socket_path = crate::config::store::Store::get_ipc_socket_path();
        match UnixStream::connect(&socket_path).await {
            Ok(stream) => {
                tracing::info!("Connected to Daemon IPC.");
                let (read_half, mut write_half) = tokio::io::split(stream);
                run_client_loop(read_half, &mut write_half, app_state, command_rx).await;
            }
            Err(e) => {
                tracing::error!("Failed to connect to Daemon: {}", e);
                std::process::exit(1);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        const PIPE_NAME: &str = r"\\.\pipe\nodirust_ipc";
        match ClientOptions::new().open(PIPE_NAME) {
            Ok(stream) => {
                tracing::info!("Connected to Daemon IPC.");
                let (read_half, mut write_half) = tokio::io::split(stream);
                run_client_loop(read_half, &mut write_half, app_state, command_rx).await;
            }
            Err(e) => {
                tracing::error!("Failed to connect to Daemon: {}", e);
                std::process::exit(1);
            }
        }
    }
}

async fn run_client_loop<R, W>(
    read_half: R,
    write_half: &mut W,
    app_state: AppState,
    mut command_rx: tokio::sync::mpsc::Receiver<IpcCommand>,
) where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut reader = BufReader::new(read_half).lines();

    loop {
        tokio::select! {
            line = reader.next_line() => {
                if let Ok(Some(text)) = line {
                    if let Ok(evt) = serde_json::from_str::<IpcEvent>(&text) {
                        handle_event(evt, &app_state);
                    }
                } else {
                    tracing::error!("Daemon disconnected. Exiting.");
                    std::process::exit(1);
                }
            }
            cmd = command_rx.recv() => {
                if let Some(cmd) = cmd {
                    let mut serialized = serde_json::to_string(&cmd).unwrap();
                    serialized.push('\n');
                    let _ = write_half.write_all(serialized.as_bytes()).await;
                }
            }
        }
    }
}

fn handle_event(evt: IpcEvent, app_state: &AppState) {
    match evt {
        IpcEvent::FullState(full) => {
            let _ = app_state.fcm_tx.send(full.fcm_connected);
            let _ = app_state.steam_tx.send(full.steam_logged_in);
            let _ = app_state.servers_tx.send(full.servers);
            let _ = app_state.devices_tx.send(full.devices);
            let _ = app_state.server_statuses_tx.send(full.server_statuses);
            let _ = app_state.pending_pair_tx.send(full.pending_pair);
        }
        IpcEvent::FcmStatusChanged(status) => {
            let _ = app_state.fcm_tx.send(status);
        }
        IpcEvent::SteamStatusChanged(status) => {
            let _ = app_state.steam_tx.send(status);
        }
        IpcEvent::ServersChanged(servers) => {
            let _ = app_state.servers_tx.send(servers);
        }
        IpcEvent::DevicesChanged(devices) => {
            let _ = app_state.devices_tx.send(devices);
        }
        IpcEvent::PendingPairChanged(pending) => {
            let _ = app_state.pending_pair_tx.send(pending);
        }
        IpcEvent::ServerStatusesUpdated(statuses) => {
            let _ = app_state.server_statuses_tx.send(statuses);
        }
        IpcEvent::ServerStatusChanged {
            server_ip_port,
            connected,
        } => {
            let mut current = app_state.server_statuses_rx.borrow().clone();
            current.insert(server_ip_port, connected);
            let _ = app_state.server_statuses_tx.send(current);
        }
    }

    // Refresh UI
    let ctx_lock = app_state.ui_context.lock().unwrap();
    if let Some(ctx) = &*ctx_lock {
        ctx.request_repaint();
    }
}
