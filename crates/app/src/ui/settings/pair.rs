use super::Tab;
use crate::app_state::AppState;
use eframe::egui;

pub fn show_pair_screen(
    app_state: &AppState,
    ui: &mut egui::Ui,
    server: &crate::config::store::ServerConfig,
    active_tab: &mut Tab,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.heading(
            egui::RichText::new("New Server Pairing")
                .color(egui::Color32::WHITE)
                .size(20.0)
                .strong(),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("A new Rust server has requested to pair with your desktop.")
                .color(egui::Color32::from_rgb(150, 150, 150))
                .size(14.0),
        );

        ui.add_space(30.0);

        let card_frame = egui::Frame::none()
            .fill(egui::Color32::from_rgb(15, 15, 15)) // slightly lighter background for emphasis
            .stroke(egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgb(40, 40, 40),
            ))
            .rounding(8.0)
            .inner_margin(24.0);

        card_frame.show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("{}:{}", server.ip, server.port))
                    .color(egui::Color32::WHITE)
                    .size(18.0)
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format!("Player ID: {}", server.player_id))
                    .color(egui::Color32::from_rgb(115, 115, 115))
                    .size(13.0),
            );
        });

        ui.add_space(40.0);

        ui.horizontal_centered(|ui| {
            let btn_decline = egui::Button::new(
                egui::RichText::new("Decline").color(egui::Color32::from_rgb(240, 240, 240)),
            )
            .fill(egui::Color32::from_rgb(26, 26, 26))
            .stroke(egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgb(40, 40, 40),
            ));

            if ui.add_sized([120.0, 40.0], btn_decline).clicked() {
                if let Some(tx) = &app_state.command_tx {
                    let _ = tx.try_send(crate::ipc::IpcCommand::DeclinePairing);
                }
            }

            ui.add_space(20.0);

            let btn_accept =
                egui::Button::new(egui::RichText::new("Pair Server").color(egui::Color32::BLACK))
                    .fill(egui::Color32::WHITE)
                    .stroke(egui::Stroke::NONE);

            if ui.add_sized([120.0, 40.0], btn_accept).clicked() {
                if let Some(tx) = &app_state.command_tx {
                    let _ = tx.try_send(crate::ipc::IpcCommand::AcceptPairing(server.clone()));
                }
                *active_tab = Tab::Servers;
            }
        });
    });
}
