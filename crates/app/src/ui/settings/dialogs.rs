use super::ServerKey;
use crate::app_state::AppState;
use eframe::egui;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Confirmation {
    RemoveServer {
        server: ServerKey,
        name: String,
    },
    RemoveDevice {
        server: ServerKey,
        entity_id: u32,
        name: String,
    },
    UnlinkSteam,
}

pub fn show_confirmation_modal(
    ctx: &egui::Context,
    app_state: &AppState,
    confirmation: &Confirmation,
    open_confirmation: &mut Option<Confirmation>,
) {
    draw_modal_backdrop(ctx, "confirmation_backdrop");
    let content = confirmation_content(confirmation);
    egui::Area::new(egui::Id::new("confirmation_modal"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            super::theme::card_frame().show(ui, |ui| {
                draw_confirmation_content(ui, &content);
                draw_confirmation_actions(
                    ui,
                    app_state,
                    confirmation,
                    content.action,
                    open_confirmation,
                );
            });
        });
}

struct ConfirmationContent {
    title: &'static str,
    description: String,
    action: &'static str,
}

fn confirmation_content(confirmation: &Confirmation) -> ConfirmationContent {
    match confirmation {
        Confirmation::RemoveServer { server, name } => ConfirmationContent {
            title: "Remove server?",
            description: format!(
                "{name}\n{}\n\nIts saved pairing and device view will be removed from this app.",
                server.label()
            ),
            action: "Remove server",
        },
        Confirmation::RemoveDevice { name, .. } => ConfirmationContent {
            title: "Remove smart device?",
            description: format!(
                "{name}\n\nYou will stop receiving alerts associated with this device."
            ),
            action: "Remove device",
        },
        Confirmation::UnlinkSteam => ConfirmationContent {
            title: "Unlink Steam account?",
            description:
                "New Rust+ pairing requests cannot be registered until Steam is linked again."
                    .to_string(),
            action: "Unlink Steam",
        },
    }
}

fn draw_confirmation_content(ui: &mut egui::Ui, content: &ConfirmationContent) {
    ui.set_width(360.0);
    ui.heading(egui::RichText::new(content.title).color(egui::Color32::WHITE));
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(&content.description)
            .color(egui::Color32::from_rgb(175, 175, 175))
            .size(13.0),
    );
    ui.add_space(20.0);
}

fn draw_confirmation_actions(
    ui: &mut egui::Ui,
    app_state: &AppState,
    confirmation: &Confirmation,
    action: &str,
    open_confirmation: &mut Option<Confirmation>,
) {
    ui.horizontal(|ui| {
        if ui.button("Cancel").clicked() {
            *open_confirmation = None;
        }
        let destructive =
            egui::Button::new(egui::RichText::new(action).color(egui::Color32::WHITE))
                .fill(egui::Color32::from_rgb(153, 27, 27));
        if ui.add(destructive).clicked() {
            execute_confirmation(app_state, confirmation);
            *open_confirmation = None;
        }
    });
}

fn execute_confirmation(app_state: &AppState, confirmation: &Confirmation) {
    match confirmation {
        Confirmation::RemoveServer { server, .. } => remove_server(app_state, server),
        Confirmation::RemoveDevice {
            server, entity_id, ..
        } => remove_device(app_state, server, *entity_id),
        Confirmation::UnlinkSteam => {
            app_state.send_command(crate::ipc::IpcCommand::UnlinkSteam);
        }
    }
}

fn remove_server(app_state: &AppState, server: &ServerKey) {
    let mut servers = app_state.servers_rx.borrow().clone();
    servers.retain(|candidate| candidate.ip != server.ip || candidate.port != server.port);
    app_state.send_command(crate::ipc::IpcCommand::UpdateServers(servers));
}

fn remove_device(app_state: &AppState, server: &ServerKey, entity_id: u32) {
    let mut devices = app_state.devices_rx.borrow().clone();
    devices.retain(|candidate| {
        candidate.entity_id != entity_id
            || candidate.server_ip != server.ip
            || candidate.server_port != server.port
    });
    app_state.send_command(crate::ipc::IpcCommand::UpdateDevices(devices));
}

pub fn draw_modal_backdrop(ctx: &egui::Context, id: &str) {
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_black_alpha(190));
        });
}
