#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

fn create_app_state() -> (
    crate::app_state::AppState,
    tokio::sync::mpsc::Receiver<crate::ipc::IpcCommand>,
) {
    let initial_steam_login = crate::config::store::Store::get_steam_token().is_ok();
    crate::app_state::AppState::new(
        initial_steam_login,
        crate::config::store::Store::get_servers(),
        crate::config::store::Store::get_devices(),
    )
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
            crate::ui::ipc_client::run_ipc_client(app_state_bg, command_rx).await;
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

fn run_daemon_process() {
    tracing::info!("Starting NODIrust Background Daemon");
    let (app_state, _command_rx) = create_app_state();
    let startup = crate::daemon::runtime::start(app_state);
    if startup.recv().is_err() {
        tracing::error!("Failed to start background tokio runtime.");
        std::process::exit(1);
    }
    crate::ui::tray::run_event_loop();
}
