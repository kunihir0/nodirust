use crate::daemon::events::DaemonEvent;
use crate::ipc::ConnectionStatus;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::{AbortHandle, JoinSet};
use tokio::time::sleep;

pub struct DaemonEngine {
    event_tx: mpsc::Sender<DaemonEvent>,
    servers_rx: tokio::sync::watch::Receiver<Vec<crate::config::store::ServerConfig>>,
    steam_rx: tokio::sync::watch::Receiver<bool>,
}

impl DaemonEngine {
    pub fn new(
        event_tx: mpsc::Sender<DaemonEvent>,
        servers_rx: tokio::sync::watch::Receiver<Vec<crate::config::store::ServerConfig>>,
        steam_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        Self {
            event_tx,
            servers_rx,
            steam_rx,
        }
    }

    pub async fn run(mut self) {
        let mut tasks = JoinSet::new();
        let push_events = self.event_tx.clone();
        let steam_rx = self.steam_rx.clone();
        tasks.spawn(async move {
            crate::daemon::push::run(push_events, steam_rx).await;
        });

        let initial_servers = crate::config::store::Store::get_servers();
        let mut server_tasks = HashMap::new();
        start_new_servers(
            &initial_servers,
            &mut server_tasks,
            &mut tasks,
            &self.event_tx,
        );

        loop {
            tokio::select! {
                result = tasks.join_next() => {
                    if !handle_task_result(result) {
                        break;
                    }
                }
                Ok(()) = self.servers_rx.changed() => {
                    let servers = self.servers_rx.borrow().clone();
                    sync_server_tasks(
                        &servers,
                        &mut server_tasks,
                        &mut tasks,
                        &self.event_tx,
                    );
                }
            }
        }
    }
}

fn handle_task_result(result: Option<Result<(), tokio::task::JoinError>>) -> bool {
    let Some(result) = result else {
        return false;
    };
    if let Err(error) = result
        && !error.is_cancelled()
    {
        tracing::error!(%error, "Background task failed");
    }
    true
}

fn sync_server_tasks(
    servers: &[crate::config::store::ServerConfig],
    server_tasks: &mut HashMap<String, AbortHandle>,
    tasks: &mut JoinSet<()>,
    event_tx: &mpsc::Sender<DaemonEvent>,
) {
    stop_removed_servers(servers, server_tasks);
    start_new_servers(servers, server_tasks, tasks, event_tx);
}

fn stop_removed_servers(
    servers: &[crate::config::store::ServerConfig],
    server_tasks: &mut HashMap<String, AbortHandle>,
) {
    let active_keys: HashSet<_> = servers.iter().map(server_key).collect();
    server_tasks.retain(|key, handle| {
        if active_keys.contains(key) {
            true
        } else {
            tracing::info!(server = key, "Stopping removed server connection");
            handle.abort();
            false
        }
    });
}

fn start_new_servers(
    servers: &[crate::config::store::ServerConfig],
    server_tasks: &mut HashMap<String, AbortHandle>,
    tasks: &mut JoinSet<()>,
    event_tx: &mpsc::Sender<DaemonEvent>,
) {
    for server in servers {
        let key = server_key(server);
        if let std::collections::hash_map::Entry::Vacant(entry) = server_tasks.entry(key.clone()) {
            tracing::info!(server = key, "Starting server connection");
            let server = server.clone();
            let events = event_tx.clone();
            let handle = tasks.spawn(async move {
                rustplus_loop(server, events).await;
            });
            entry.insert(handle);
        }
    }
}

fn server_key(server: &crate::config::store::ServerConfig) -> String {
    format!("{}:{}", server.ip, server.port)
}

async fn rustplus_loop(
    server: crate::config::store::ServerConfig,
    event_tx: mpsc::Sender<DaemonEvent>,
) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);
    let key = server_key(&server);
    let mut first_attempt = true;

    loop {
        let status = if first_attempt {
            ConnectionStatus::Connecting
        } else {
            ConnectionStatus::Reconnecting
        };
        emit_server_status(&event_tx, &key, status);
        first_attempt = false;

        match connect_and_monitor_server(&server, &event_tx, &key).await {
            Ok(()) => {
                backoff = Duration::from_secs(1);
                tracing::warn!(server = key, "Rust+ connection dropped; reconnecting");
                emit_server_status(&event_tx, &key, ConnectionStatus::Reconnecting);
            }
            Err(error) => {
                tracing::error!(%error, ?backoff, server = key, "Rust+ connection failed");
                emit_server_status(&event_tx, &key, status_for_error(&error));
                sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, max_backoff);
            }
        }
    }
}

async fn connect_and_monitor_server(
    server: &crate::config::store::ServerConfig,
    event_tx: &mpsc::Sender<DaemonEvent>,
    key: &str,
) -> rustplus::Result<()> {
    let mut client = rustplus::RustPlusClient::new(
        server.ip.clone(),
        server.port,
        server.player_id,
        server.player_token,
        false,
    );
    client.connect().await?;
    let info = client
        .get_info()
        .await?
        .response
        .and_then(|response| response.info);

    tracing::info!(server = key, "Connected to Rust+ server");
    emit_server_status(event_tx, key, ConnectionStatus::Connected);
    emit_discovered_name(server, info, event_tx);
    if let Some(broadcasts) = client.take_broadcast_receiver() {
        listen_for_server_broadcasts(broadcasts, &server.ip, event_tx).await;
    }
    Ok(())
}

fn emit_discovered_name(
    server: &crate::config::store::ServerConfig,
    info: Option<rustplus::proto::AppInfo>,
    event_tx: &mpsc::Sender<DaemonEvent>,
) {
    if server.name.is_some() {
        return;
    }
    if let Some(info) = info {
        let _ = event_tx.try_send(DaemonEvent::ServerNameDiscovered {
            ip: server.ip.clone(),
            port: server.port,
            name: info.name,
        });
    }
}

async fn listen_for_server_broadcasts(
    mut broadcasts: tokio::sync::broadcast::Receiver<rustplus::proto::AppMessage>,
    server_ip: &str,
    event_tx: &mpsc::Sender<DaemonEvent>,
) {
    while let Ok(message) = broadcasts.recv().await {
        let Some(broadcast) = message.broadcast else {
            continue;
        };
        if let Some(team_message) = broadcast.team_message {
            let message = team_message.message;
            let _ = event_tx.try_send(DaemonEvent::ServerChatReceived {
                server_ip: server_ip.to_string(),
                sender: message.name,
                message: message.message,
            });
        }
        if let Some(entity) = broadcast.entity_changed {
            tracing::info!(
                entity_id = entity.entity_id,
                payload = ?entity.payload,
                "Rust+ entity changed"
            );
        }
    }
}

fn emit_server_status(event_tx: &mpsc::Sender<DaemonEvent>, key: &str, status: ConnectionStatus) {
    let _ = event_tx.try_send(DaemonEvent::ServerConnectionStatusChanged {
        server_ip_port: key.to_string(),
        status,
    });
}

fn status_for_error(error: &rustplus::Error) -> ConnectionStatus {
    if matches!(error, rustplus::Error::Api(_)) {
        ConnectionStatus::AuthenticationFailed
    } else {
        ConnectionStatus::Unreachable
    }
}
