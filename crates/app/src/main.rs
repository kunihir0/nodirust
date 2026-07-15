#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::multiple_crate_versions)]

mod app_state;
mod config;
mod daemon;
pub mod facepunch;
mod ipc;
mod notify;
mod ui;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use eframe::egui;
use std::thread;
use tracing_subscriber::EnvFilter;

#[allow(clippy::too_many_lines)]
fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    #[cfg(target_os = "macos")]
    if let Err(e) = notify_rust::set_application("com.apple.Terminal") {
        tracing::error!("Failed to set macOS notification application: {}", e);
    }

    if std::env::args().any(|a| a == "--auth") {
        crate::ui::auth::spawn_auth_webview();
        std::process::exit(0);
    }

    if std::env::args().any(|a| a == "--ui") {
        return run_ui_process();
    }

    run_daemon_process();
    Ok(())
}

fn create_app_state() -> (crate::app_state::AppState, Option<tokio::sync::mpsc::Receiver<crate::ipc::IpcCommand>>) {
    let (fcm_status_tx, fcm_status_rx) = tokio::sync::watch::channel(false);
    let initial_steam_login = crate::config::store::Store::get_steam_token().is_ok();
    let (steam_status_tx, steam_status_rx) = tokio::sync::watch::channel(initial_steam_login);

    let (servers_tx, servers_rx) = tokio::sync::watch::channel(
        crate::config::store::Store::get_servers()
    );

    let (devices_tx, devices_rx) = tokio::sync::watch::channel(
        crate::config::store::Store::get_devices()
    );
    
    let (pending_pair_tx, pending_pair_rx) = tokio::sync::watch::channel::<Option<crate::config::store::ServerConfig>>(None);

    let (server_statuses_tx, server_statuses_rx) = tokio::sync::watch::channel(std::collections::HashMap::new());

    let (command_tx, command_rx) = tokio::sync::mpsc::channel(100);

    let app_state = crate::app_state::AppState::new(
        fcm_status_rx,
        fcm_status_tx,
        steam_status_rx,
        steam_status_tx,
        servers_rx,
        servers_tx,
        devices_rx,
        devices_tx,
        pending_pair_rx,
        pending_pair_tx,
        server_statuses_rx,
        server_statuses_tx,
        Some(command_tx),
    );

    (app_state, Some(command_rx))
}

fn run_ui_process() -> eframe::Result {
    tracing::info!("Starting NODIrust UI Subprocess");

    let (app_state, command_rx) = create_app_state();
    let app_state_bg = app_state.clone();

    // Spawn Tokio runtime for IPC Client
    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime");

        rt.block_on(async {
            crate::ui::ipc_client::run_ipc_client(app_state_bg, command_rx.unwrap()).await;
        });
    });

    let icon_data = include_bytes!("../assets/app.ico");
    let image = image::load_from_memory_with_format(icon_data, image::ImageFormat::Ico)
        .expect("Failed to parse app.ico")
        .into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    let icon = std::sync::Arc::new(egui::IconData {
        rgba,
        width,
        height,
    });

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([750.0, 500.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title("NODIrust")
            .with_decorations(false)
            .with_transparent(true)
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "NODIrust",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(crate::ui::settings::SettingsWindow::new(
                cc,
                app_state.clone(),
            )))
        }),
    )
}

#[allow(clippy::too_many_lines)]
fn run_daemon_process() {
    tracing::info!("Starting NODIrust Background Daemon");

    let (app_state, _command_rx) = create_app_state();
    let app_state_bg = app_state.clone();

    // 1. Spawn tokio runtime on a dedicated background thread.
    let (tx, rx) = std::sync::mpsc::channel();

    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime");

        rt.block_on(async {
            tracing::info!("Background tokio runtime started.");

            let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);

            // Run DaemonEngine
            let engine = crate::daemon::engine::DaemonEngine::new(event_tx, app_state_bg.servers_rx.clone());
            tokio::spawn(engine.run());

            // Run IPC Server
            tokio::spawn(crate::daemon::ipc_server::run_ipc_server(app_state_bg.clone()));

            let app_state_clone = app_state_bg;
            // Handle DaemonEvents (e.g. notifications)
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    match event {
                        crate::daemon::events::DaemonEvent::PushNotificationReceived {
                            title,
                            body,
                        } => {
                            crate::notify::Notifier::push(&title, &body);
                        }
                        crate::daemon::events::DaemonEvent::ServerChatReceived {
                            server_ip,
                            sender,
                            message,
                        } => {
                            let title = format!("Team Chat ({server_ip})");
                            let body = format!("{sender}: {message}");
                            crate::notify::Notifier::push(&title, &body);
                        }
                        crate::daemon::events::DaemonEvent::PairingRequest(server) => {
                            tracing::info!("Received Server Pairing Request for {}:{}", server.ip, server.port);
                            let _ = app_state_clone.pending_pair_tx.send(Some(server.clone()));
                            
                            let title = "Rust+ Server Pairing".to_string();
                            let body = format!("New pairing request for {}:{}. Open dashboard to accept.", server.ip, server.port);
                            crate::notify::Notifier::push(&title, &body);
                        }
                        crate::daemon::events::DaemonEvent::EntityPairingRequest(device) => {
                            tracing::info!("Received Entity Pairing Request for {}", device.entity_name);
                            let mut devices = crate::config::store::Store::get_devices();
                            if !devices.iter().any(|d| d.entity_id == device.entity_id && d.server_ip == device.server_ip) {
                                let name = device.entity_name.clone();
                                devices.push(device);
                                let _ = crate::config::store::Store::set_devices(devices.clone());
                                let _ = app_state_clone.devices_tx.send(devices);
                                
                                let title = "Rust+ Device Paired".to_string();
                                let body = format!("Successfully paired with {name}!");
                                crate::notify::Notifier::push(&title, &body);
                            }
                        }
                        crate::daemon::events::DaemonEvent::ConnectionStatusChanged(connected) => {
                            tracing::info!("FCM Connection status changed: {}", connected);
                            let _ = app_state_clone.fcm_tx.send(connected);
                        }
                        crate::daemon::events::DaemonEvent::ServerConnectionStatusChanged { server_ip_port, connected } => {
                            let mut statuses = app_state_clone.server_statuses_rx.borrow().clone();
                            statuses.insert(server_ip_port, connected);
                            let _ = app_state_clone.server_statuses_tx.send(statuses);
                        }
                        crate::daemon::events::DaemonEvent::ServerNameDiscovered { ip, port, name } => {
                            let mut servers = crate::config::store::Store::get_servers();
                            let mut changed = false;
                            for s in servers.iter_mut() {
                                if s.ip == ip && s.port == port && s.name.as_deref() != Some(name.as_str()) {
                                    s.name = Some(name.clone());
                                    changed = true;
                                }
                            }
                            if changed {
                                let mut config = crate::config::store::Store::get_config();
                                config.servers = servers.clone();
                                let _ = crate::config::store::Store::save_config(&config);
                                let _ = app_state_clone.servers_tx.send(servers);
                            }
                        }
                    }
                }
            });

            let _ = tx.send(());
            std::future::pending::<()>().await;
        });
    });

    if rx.recv().is_err() {
        tracing::error!("Failed to start background tokio runtime.");
        std::process::exit(1);
    }

    // Initialize System Tray on the main thread using winit
    let (_tray_icon, tray_menu) = crate::ui::tray::setup_tray();
    
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let dash_id = tray_menu.dashboard_id.clone();
    let quit_id = tray_menu.quit_id.clone();

    let mut ui_process: Option<std::process::Child> = None;

    #[allow(deprecated)]
    event_loop.run(move |_event, target| {
        target.set_control_flow(winit::event_loop::ControlFlow::Wait);
        
        if let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if event.id.0 == quit_id {
                if let Some(mut child) = ui_process.take() {
                    let _ = child.kill();
                }
                target.exit();
            } else if event.id.0 == dash_id {
                let should_spawn = match &mut ui_process {
                    Some(child) => child.try_wait().map_or(true, |s| s.is_some()),
                    None => true,
                };
                if should_spawn {
                    if let Ok(exe) = std::env::current_exe() {
                        ui_process = std::process::Command::new(exe).arg("--ui").spawn().ok();
                    }
                }
            }
        }

        if let Ok(tray_icon::TrayIconEvent::Click { button: tray_icon::MouseButton::Left, button_state: tray_icon::MouseButtonState::Up, .. }) = tray_icon::TrayIconEvent::receiver().try_recv() {
            let should_spawn = match &mut ui_process {
                Some(child) => child.try_wait().map_or(true, |s| s.is_some()),
                None => true,
            };
            if should_spawn {
                if let Ok(exe) = std::env::current_exe() {
                    ui_process = std::process::Command::new(exe).arg("--ui").spawn().ok();
                }
            }
        }
    }).unwrap();


}
