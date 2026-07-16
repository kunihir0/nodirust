use crate::app_state::AppState;
use eframe::egui;

pub fn show_pair_modal(
    ctx: &egui::Context,
    app_state: &AppState,
    server: &crate::config::store::ServerConfig,
    action_pending: &mut bool,
) {
    super::dialogs::draw_modal_backdrop(ctx, "pairing_backdrop");

    egui::Area::new(egui::Id::new("pairing_modal"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            super::theme::card_frame().show(ui, |ui| {
                ui.set_width(420.0);
                ui.heading(
                    egui::RichText::new("New server pairing request")
                        .color(egui::Color32::WHITE)
                        .size(20.0)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(
                        "Only accept this request if you just initiated pairing in Rust+.",
                    )
                    .color(egui::Color32::from_rgb(248, 190, 90))
                    .size(13.0)
                    .strong(),
                );
                ui.add_space(18.0);

                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(20, 20, 20))
                    .rounding(6.0)
                    .inner_margin(14.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            egui::RichText::new(
                                server.name.as_deref().unwrap_or("Unnamed Rust server"),
                            )
                            .color(egui::Color32::WHITE)
                            .size(16.0)
                            .strong(),
                        );
                        ui.label(
                            egui::RichText::new(format!("{}:{}", server.ip, server.port))
                                .color(egui::Color32::from_rgb(175, 175, 175))
                                .size(12.0),
                        );
                        ui.label(
                            egui::RichText::new(format!("Steam player ID: {}", server.player_id))
                                .color(egui::Color32::from_rgb(175, 175, 175))
                                .size(12.0),
                        );
                    });

                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    let decline = ui.add_enabled(!*action_pending, egui::Button::new("Decline"));
                    if decline.clicked()
                        && app_state.send_command(crate::ipc::IpcCommand::DeclinePairing)
                    {
                        *action_pending = true;
                    }

                    let label = if *action_pending {
                        "Saving…"
                    } else {
                        "Accept and pair"
                    };
                    let accept =
                        egui::Button::new(egui::RichText::new(label).color(egui::Color32::BLACK))
                            .fill(egui::Color32::WHITE)
                            .stroke(egui::Stroke::NONE);
                    if ui.add_enabled(!*action_pending, accept).clicked()
                        && app_state
                            .send_command(crate::ipc::IpcCommand::AcceptPairing(server.clone()))
                    {
                        *action_pending = true;
                    }
                });
            });
        });
}
