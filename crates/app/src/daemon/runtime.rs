use crate::app_state::AppState;
use crate::daemon::events::DaemonEvent;

pub fn start(app_state: AppState) -> std::sync::mpsc::Receiver<()> {
    let (startup_tx, startup_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime");
        runtime.block_on(run(app_state, startup_tx));
    });
    startup_rx
}

async fn run(app_state: AppState, startup_tx: std::sync::mpsc::Sender<()>) {
    tracing::info!("Background tokio runtime started.");
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(100);
    let engine = crate::daemon::engine::DaemonEngine::new(event_tx, app_state.servers_rx.clone());
    tokio::spawn(engine.run());
    tokio::spawn(crate::daemon::ipc_server::run_ipc_server(app_state.clone()));
    tokio::spawn(handle_events(event_rx, app_state));
    let _ = startup_tx.send(());
    std::future::pending::<()>().await;
}

async fn handle_events(mut events: tokio::sync::mpsc::Receiver<DaemonEvent>, state: AppState) {
    while let Some(event) = events.recv().await {
        handle_event(event, &state);
    }
}

fn handle_event(event: DaemonEvent, state: &AppState) {
    match event {
        DaemonEvent::PushNotificationReceived { title, body } => {
            notify_and_record(state, &title, &body);
        }
        DaemonEvent::ServerChatReceived {
            server_ip,
            sender,
            message,
        } => {
            let title = format!("Team Chat ({server_ip})");
            let body = format!("{sender}: {message}");
            notify_and_record(state, &title, &body);
        }
        DaemonEvent::PairingRequest(server) => handle_server_pairing(state, &server),
        DaemonEvent::EntityPairingRequest(device) => handle_device_pairing(state, device),
        DaemonEvent::ConnectionStatusChanged(status) => {
            tracing::info!(?status, "Push connection status changed");
            let _ = state.push_tx.send(status);
        }
        DaemonEvent::ServerConnectionStatusChanged {
            server_ip_port,
            status,
        } => {
            let mut statuses = state.server_statuses_rx.borrow().clone();
            statuses.insert(server_ip_port, status);
            let _ = state.server_statuses_tx.send(statuses);
        }
        DaemonEvent::ServerNameDiscovered { ip, port, name } => {
            update_server_name(state, &ip, port, &name);
        }
    }
}

fn notify_and_record(state: &AppState, title: &str, body: &str) {
    let _ = state
        .last_event_tx
        .send(Some(crate::ipc::LastEvent::new(title, body)));
    if let Err(error) = crate::notify::Notifier::push(title, body) {
        tracing::error!(%error, "Failed to send system notification");
    }
}

fn handle_server_pairing(state: &AppState, server: &crate::config::store::ServerConfig) {
    tracing::info!(server_ip = %server.ip, server_port = server.port, "Received server pairing request");
    let _ = state.pending_pair_tx.send(Some(server.clone()));
    let body = format!(
        "New pairing request for {}:{}. Open settings to accept.",
        server.ip, server.port
    );
    notify_and_record(state, "Rust+ Server Pairing", &body);
}

fn handle_device_pairing(state: &AppState, device: crate::config::store::DeviceConfig) {
    tracing::info!(device_name = %device.entity_name, "Received entity pairing request");
    let mut devices = crate::config::store::Store::get_devices();
    let already_paired = devices.iter().any(|existing| {
        existing.entity_id == device.entity_id && existing.server_ip == device.server_ip
    });
    if already_paired {
        return;
    }

    let name = device.entity_name.clone();
    devices.push(device);
    if let Err(error) = crate::config::store::Store::set_devices(devices.clone()) {
        tracing::error!(%error, "Failed to save paired device");
        return;
    }
    let _ = state.devices_tx.send(devices);
    notify_and_record(
        state,
        "Rust+ Device Paired",
        &format!("Successfully paired with {name}!"),
    );
}

fn update_server_name(state: &AppState, ip: &str, port: u16, name: &str) {
    let mut config = crate::config::store::Store::get_config();
    let Some(server) = config
        .servers
        .iter_mut()
        .find(|server| server.ip == ip && server.port == port)
    else {
        return;
    };
    if server.name.as_deref() == Some(name) {
        return;
    }

    server.name = Some(name.to_string());
    if let Err(error) = crate::config::store::Store::save_config(&config) {
        tracing::error!(%error, "Failed to save discovered server name");
        return;
    }
    let _ = state.servers_tx.send(config.servers);
}
