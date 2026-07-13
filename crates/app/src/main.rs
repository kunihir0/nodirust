#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
mod config;
mod daemon;
pub mod facepunch;
mod notify;
mod ui;

use eframe::egui;
use std::thread;
use tracing_subscriber::EnvFilter;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    #[cfg(target_os = "macos")]
    if let Err(e) = notify_rust::set_application("com.apple.Terminal") {
        tracing::error!("Failed to set macOS notification application: {}", e);
    }

    tracing::info!("Starting NODIrust");

    // Route to Auth Webview if spawned as a subprocess for login
    if std::env::args().any(|a| a == "--auth") {
        crate::ui::auth::spawn_auth_webview();
        std::process::exit(0);
    }

    // 1. Spawn tokio runtime on a dedicated background thread.
    // This is explicitly done to prevent main-thread event loop collisions on macOS.
    let (tx, rx) = std::sync::mpsc::channel();

    // Create state watch channels
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

    let app_state = crate::app_state::AppState::new(
        fcm_status_rx,
        steam_status_rx,
        steam_status_tx,
        servers_rx,
        servers_tx,
        devices_rx,
        devices_tx,
        pending_pair_rx,
        pending_pair_tx.clone()
    );

    let app_state_bg = app_state.clone();

    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime");

        rt.block_on(async {
            tracing::info!("Background tokio runtime started.");

            let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);

            let engine = crate::daemon::engine::DaemonEngine::new(event_tx, app_state_bg.servers_rx.clone());
            tokio::spawn(engine.run());

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
                            let title = format!("Team Chat ({})", server_ip);
                            let body = format!("{}: {}", sender, message);
                            crate::notify::Notifier::push(&title, &body);
                        }
                        crate::daemon::events::DaemonEvent::PairingRequest(server) => {
                            tracing::info!("Received Server Pairing Request for {}:{}", server.ip, server.port);
                            let _ = pending_pair_tx.send(Some(server.clone()));
                            
                            if !app_state_clone.is_ui_visible.load(std::sync::atomic::Ordering::SeqCst) {
                                let title = "Rust+ Server Pairing".to_string();
                                let body = format!("New pairing request for {}:{}. Open dashboard to accept.", server.ip, server.port);
                                
                                if let Some(handle) = crate::notify::Notifier::push(&title, &body) {
                                    let ui_ctx_arc = app_state_clone.ui_context.clone();
                                    let is_ui_visible = app_state_clone.is_ui_visible.clone();
                                    
                                    tokio::task::spawn_blocking(move || {
                                        handle.wait_for_action(|_action| {
                                            tracing::info!("Notification clicked!");
                                            let ctx_lock = ui_ctx_arc.lock().unwrap();
                                            if let Some(ctx) = &*ctx_lock {
                                                is_ui_visible.store(true, std::sync::atomic::Ordering::SeqCst);
                                                ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(true));
                                                ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Focus);
                                            }
                                        });
                                    });
                                }
                            }
                        }
                        crate::daemon::events::DaemonEvent::EntityPairingRequest(device) => {
                            tracing::info!("Received Entity Pairing Request for {}", device.entity_name);
                            // Auto-accept device pairings and save them
                            let mut devices = crate::config::store::Store::get_devices();
                            if !devices.iter().any(|d| d.entity_id == device.entity_id && d.server_ip == device.server_ip) {
                                let name = device.entity_name.clone();
                                devices.push(device);
                                let _ = crate::config::store::Store::set_devices(devices.clone());
                                let _ = app_state_clone.devices_tx.send(devices);
                                
                                let title = "Rust+ Device Paired".to_string();
                                let body = format!("Successfully paired with {}!", name);
                                crate::notify::Notifier::push(&title, &body);
                                
                                // Refresh UI
                                let ctx_lock = app_state_clone.ui_context.lock().unwrap();
                                if let Some(ctx) = &*ctx_lock {
                                    ctx.request_repaint();
                                }
                            }
                        }
                        crate::daemon::events::DaemonEvent::ConnectionStatusChanged(connected) => {
                            tracing::info!("FCM Connection status changed: {}", connected);
                            let _ = fcm_status_tx.send(connected);
                        }
                    }
                }
            });

            // Signal main thread that runtime is up
            let _ = tx.send(());

            // Block this thread indefinitely to keep the background tasks alive
            std::future::pending::<()>().await;
        });
    });

    // Wait for the background thread to confirm the runtime is up
    if rx.recv().is_err() {
        tracing::error!("Failed to start background tokio runtime.");
        std::process::exit(1);
    }

    // 2. Start the eframe/winit event loop on the main thread.
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([550.0, 500.0])
            .with_title("NODIrust")
            .with_decorations(false)
            .with_transparent(true),
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
