use eframe::egui;
use crate::app_state::AppState;
use super::theme::custom_toggle;

pub fn show_dashboard(app_state: &AppState, ui: &mut egui::Ui) {
    ui.heading(egui::RichText::new("Dashboard").color(egui::Color32::WHITE));
    ui.add_space(30.0);

    let is_fcm_connected = *app_state.fcm_connected.borrow();
    let is_steam_logged_in = *app_state.steam_logged_in.borrow();

    // Push Network Item
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Background Notifications").color(egui::Color32::WHITE).size(14.0).strong());
            ui.add_space(2.0);
            ui.label(egui::RichText::new("Receive real-time smart alarm triggers.")
                .color(egui::Color32::from_rgb(115, 115, 115)).size(12.0));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            custom_toggle(ui, is_fcm_connected);
        });
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(16.0);

    // Steam Account Item
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Steam Account").color(egui::Color32::WHITE).size(14.0).strong());
            ui.add_space(2.0);
            if is_steam_logged_in {
                ui.label(egui::RichText::new("Account is currently linked.")
                    .color(egui::Color32::from_rgb(115, 115, 115)).size(12.0));
            } else {
                ui.label(egui::RichText::new("Please link your Steam account.")
                    .color(egui::Color32::from_rgb(115, 115, 115)).size(12.0));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if is_steam_logged_in {
                let btn = egui::Button::new(egui::RichText::new("Logout").color(egui::Color32::BLACK))
                    .fill(egui::Color32::WHITE)
                    .stroke(egui::Stroke::NONE);
                if ui.add_sized([80.0, 32.0], btn).clicked() {
                    if let Some(tx) = &app_state.command_tx {
                        let _ = tx.try_send(crate::ipc::IpcCommand::SetSteamToken(None));
                    }
                }
            } else {
                let btn = egui::Button::new(egui::RichText::new("Login").color(egui::Color32::WHITE))
                    .fill(egui::Color32::from_rgb(20, 20, 20))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(31, 31, 31)));
                if ui.add_sized([80.0, 32.0], btn).clicked() {
                    if let Ok(exe) = std::env::current_exe() {
                        match std::process::Command::new(exe).arg("--auth").spawn() {
                            Ok(mut child) => {
                                let command_tx = app_state.command_tx.clone();
                                std::thread::spawn(move || {
                                    let _ = child.wait();
                                    if let Ok(token) = crate::config::store::Store::get_steam_token() {
                                        if let Some(tx) = &command_tx {
                                            let _ = tx.try_send(crate::ipc::IpcCommand::SetSteamToken(Some(token)));
                                        }
                                    }
                                });
                            }
                            Err(e) => tracing::error!("Failed to spawn auth window: {}", e),
                        }
                    }
                }
            }
        });
    });
}
