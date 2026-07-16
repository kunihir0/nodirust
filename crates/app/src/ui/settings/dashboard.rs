use super::Confirmation;
use super::theme::{card_frame, connection_status_display, status_badge};
use crate::app_state::AppState;
use crate::ipc::ConnectionStatus;
use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn show_dashboard(
    app_state: &AppState,
    auth_in_progress: &std::sync::Arc<AtomicBool>,
    confirmation: &mut Option<Confirmation>,
    ui: &mut egui::Ui,
) {
    ui.heading(egui::RichText::new("Overview").color(egui::Color32::WHITE));
    ui.label(
        egui::RichText::new("The services that keep Rust+ alerts reaching this computer.")
            .color(egui::Color32::from_rgb(165, 165, 165))
            .size(13.0),
    );
    ui.add_space(20.0);

    show_summary(app_state, ui);
    ui.add_space(16.0);
    show_service_health(app_state, auth_in_progress, confirmation, ui);
    ui.add_space(16.0);
    show_last_event(app_state, ui);
}

fn show_summary(app_state: &AppState, ui: &mut egui::Ui) {
    let servers = app_state.servers_rx.borrow();
    let devices = app_state.devices_rx.borrow();
    let statuses = app_state.server_statuses_rx.borrow();
    let connected = statuses
        .values()
        .filter(|status| **status == ConnectionStatus::Connected)
        .count();

    ui.columns(3, |columns| {
        summary_card(&mut columns[0], "Paired servers", servers.len());
        summary_card(&mut columns[1], "Connected now", connected);
        summary_card(&mut columns[2], "Smart devices", devices.len());
    });
}

fn summary_card(ui: &mut egui::Ui, label: &str, value: usize) {
    card_frame().show(ui, |ui| {
        ui.set_min_height(58.0);
        ui.label(
            egui::RichText::new(value.to_string())
                .color(egui::Color32::WHITE)
                .size(22.0)
                .strong(),
        );
        ui.label(
            egui::RichText::new(label)
                .color(egui::Color32::from_rgb(165, 165, 165))
                .size(12.0),
        );
    });
}

fn show_service_health(
    app_state: &AppState,
    auth_in_progress: &std::sync::Arc<AtomicBool>,
    confirmation: &mut Option<Confirmation>,
    ui: &mut egui::Ui,
) {
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            egui::RichText::new("Service health")
                .color(egui::Color32::WHITE)
                .size(15.0)
                .strong(),
        );
        ui.add_space(12.0);

        service_row(
            ui,
            "Push connection",
            "Receives alarm and pairing events from Rust+.",
            |ui| {
                let (label, color) = connection_status_display(*app_state.push_status.borrow());
                status_badge(ui, label, color);
            },
        );
        ui.separator();

        service_row(
            ui,
            "System notifications",
            "Delivery permission is managed by Windows or macOS.",
            |ui| {
                if ui.button("Send test").clicked() {
                    app_state.send_command(crate::ipc::IpcCommand::SendTestNotification);
                }
            },
        );
        ui.separator();

        let is_logged_in = *app_state.steam_logged_in.borrow();
        service_row(
            ui,
            "Steam account",
            if is_logged_in {
                "Linked locally and ready to register new pairings."
            } else {
                "Link Steam before initiating a Rust+ pairing request."
            },
            |ui| {
                if is_logged_in {
                    if ui.button("Sign out").clicked() {
                        *confirmation = Some(Confirmation::SignOut);
                    }
                } else {
                    let in_progress = auth_in_progress.load(Ordering::Acquire);
                    let text = if in_progress {
                        "Login open…"
                    } else {
                        "Link Steam"
                    };
                    if ui
                        .add_enabled(!in_progress, egui::Button::new(text))
                        .clicked()
                    {
                        launch_auth(app_state, auth_in_progress, ui.ctx());
                    }
                }
            },
        );
    });
}

fn service_row(ui: &mut egui::Ui, title: &str, detail: &str, trailing: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(title)
                    .color(egui::Color32::WHITE)
                    .size(13.0)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(detail)
                    .color(egui::Color32::from_rgb(165, 165, 165))
                    .size(12.0),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), trailing);
    });
}

fn show_last_event(app_state: &AppState, ui: &mut egui::Ui) {
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            egui::RichText::new("Most recent event")
                .color(egui::Color32::WHITE)
                .size(15.0)
                .strong(),
        );
        ui.add_space(8.0);

        if let Some(event) = app_state.last_event_rx.borrow().as_ref() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&event.title)
                        .color(egui::Color32::from_rgb(225, 225, 225))
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(relative_time(event.received_at_epoch_seconds))
                        .color(egui::Color32::from_rgb(150, 150, 150))
                        .size(11.0),
                );
            });
            ui.label(
                egui::RichText::new(&event.detail)
                    .color(egui::Color32::from_rgb(170, 170, 170))
                    .size(12.0),
            );
        } else {
            ui.label(
                egui::RichText::new("No events received during this daemon session.")
                    .color(egui::Color32::from_rgb(165, 165, 165))
                    .size(12.0),
            );
        }
    });
}

fn launch_auth(
    app_state: &AppState,
    auth_in_progress: &std::sync::Arc<AtomicBool>,
    ctx: &egui::Context,
) {
    auth_in_progress.store(true, Ordering::Release);
    let Ok(executable) = std::env::current_exe() else {
        auth_in_progress.store(false, Ordering::Release);
        let _ = app_state
            .feedback_tx
            .send(Some(crate::ipc::UiFeedback::error(
                "Could not locate the login application.",
            )));
        return;
    };

    match std::process::Command::new(executable).arg("--auth").spawn() {
        Ok(mut child) => {
            let state = app_state.clone();
            let progress = std::sync::Arc::clone(auth_in_progress);
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let _ = child.wait();
                if crate::config::store::Store::get_steam_token().is_ok() {
                    state.send_command(crate::ipc::IpcCommand::RefreshSteamStatus);
                } else {
                    let _ = state.feedback_tx.send(Some(crate::ipc::UiFeedback::error(
                        "Steam login was canceled before the account was linked.",
                    )));
                }
                progress.store(false, Ordering::Release);
                ctx.request_repaint();
            });
        }
        Err(error) => {
            tracing::error!(%error, "Failed to open Steam login");
            auth_in_progress.store(false, Ordering::Release);
            let _ = app_state
                .feedback_tx
                .send(Some(crate::ipc::UiFeedback::error(
                    "Could not open the Steam login window.",
                )));
        }
    }
}

fn relative_time(received_at: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let age = now.saturating_sub(received_at);

    match age {
        0..=59 => "just now".to_string(),
        60..=3_599 => format!("{}m ago", age / 60),
        3_600..=86_399 => format!("{}h ago", age / 3_600),
        _ => format!("{}d ago", age / 86_400),
    }
}
