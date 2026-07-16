use super::theme::{card_frame, connection_status_display, custom_toggle, status_badge};
use super::{Confirmation, ServerKey, Tab};
use crate::app_state::AppState;
use crate::ipc::ConnectionStatus;
use eframe::egui;

pub struct ServersState<'a> {
    pub selected_server: &'a mut Option<ServerKey>,
    pub confirmation: &'a mut Option<Confirmation>,
    pub compact_show_detail: &'a mut bool,
    pub active_tab: &'a mut Tab,
}

pub fn show_servers_pane(app_state: &AppState, state: &mut ServersState<'_>, ui: &mut egui::Ui) {
    show_servers_header(ui);
    let servers = app_state.servers_rx.borrow().clone();
    if servers.is_empty() {
        *state.selected_server = None;
        *state.compact_show_detail = false;
        show_empty_state(app_state, state, ui);
        return;
    }

    ensure_server_selected(&servers, state.selected_server);
    let statuses = app_state.server_statuses_rx.borrow().clone();
    egui::ScrollArea::vertical()
        .id_salt("servers_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for server in &servers {
                show_server_card(server, &statuses, state, ui);
                ui.add_space(10.0);
            }
        });
}

fn show_servers_header(ui: &mut egui::Ui) {
    ui.heading(egui::RichText::new("Servers").color(egui::Color32::WHITE));
    ui.label(
        egui::RichText::new("Paired Rust+ servers and their live connection state.")
            .color(egui::Color32::from_rgb(165, 165, 165))
            .size(12.0),
    );
    ui.add_space(16.0);
}

fn ensure_server_selected(
    servers: &[crate::config::store::ServerConfig],
    selected_server: &mut Option<ServerKey>,
) {
    let selection_exists = selected_server.as_ref().is_some_and(|selected| {
        servers
            .iter()
            .any(|server| server.ip == selected.ip && server.port == selected.port)
    });
    if !selection_exists {
        *selected_server = Some(ServerKey {
            ip: servers[0].ip.clone(),
            port: servers[0].port,
        });
    }
}

fn show_server_card(
    server: &crate::config::store::ServerConfig,
    statuses: &std::collections::HashMap<String, ConnectionStatus>,
    state: &mut ServersState<'_>,
    ui: &mut egui::Ui,
) {
    let key = ServerKey {
        ip: server.ip.clone(),
        port: server.port,
    };
    let status_key = format!("{}:{}", server.ip, server.port);
    let status = statuses
        .get(&status_key)
        .copied()
        .unwrap_or(ConnectionStatus::Connecting);
    let selected = state.selected_server.as_ref() == Some(&key);

    egui::Frame::none()
        .fill(if selected {
            egui::Color32::from_rgb(24, 24, 24)
        } else {
            egui::Color32::from_rgb(11, 11, 11)
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected {
                egui::Color32::from_rgb(64, 64, 64)
            } else {
                egui::Color32::from_rgb(38, 38, 38)
            },
        ))
        .rounding(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            show_server_identity(server, &status_key, &key, state, ui);
            ui.add_space(8.0);
            show_server_actions(server, &status_key, status, &key, state, ui);
        });
}

fn show_server_identity(
    server: &crate::config::store::ServerConfig,
    status_key: &str,
    key: &ServerKey,
    state: &mut ServersState<'_>,
    ui: &mut egui::Ui,
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(server.name.as_deref().unwrap_or("Unnamed Rust server"))
                    .color(egui::Color32::WHITE)
                    .size(14.0)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(status_key)
                    .color(egui::Color32::from_rgb(170, 170, 170))
                    .size(11.0),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("View devices").clicked() {
                *state.selected_server = Some(key.clone());
                *state.compact_show_detail = true;
            }
        });
    });
}

fn show_server_actions(
    server: &crate::config::store::ServerConfig,
    status_key: &str,
    status: ConnectionStatus,
    key: &ServerKey,
    state: &mut ServersState<'_>,
    ui: &mut egui::Ui,
) {
    ui.horizontal(|ui| {
        let (label, color) = connection_status_display(status);
        status_badge(ui, label, color);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("Remove…")
                .on_hover_text("Remove this saved server pairing")
                .clicked()
            {
                *state.confirmation = Some(Confirmation::RemoveServer {
                    server: key.clone(),
                    name: server
                        .name
                        .clone()
                        .unwrap_or_else(|| status_key.to_string()),
                });
            }
            if ui
                .small_button("Copy address")
                .on_hover_text("Copy the Rust client.connect command")
                .clicked()
            {
                ui.output_mut(|output| {
                    output.copied_text = format!("client.connect {status_key}");
                });
            }
        });
    });
}

pub fn show_devices_pane(app_state: &AppState, state: &mut ServersState<'_>, ui: &mut egui::Ui) {
    let Some(selected) = state.selected_server.clone() else {
        ui.heading(egui::RichText::new("Smart devices").color(egui::Color32::WHITE));
        ui.add_space(16.0);
        ui.label(
            egui::RichText::new("Select a server to view its paired devices.")
                .color(egui::Color32::from_rgb(165, 165, 165))
                .size(13.0),
        );
        return;
    };

    show_devices_header(app_state, &selected, ui);

    let mut devices = app_state.devices_rx.borrow().clone();
    let (has_devices, changes) = show_device_list(&mut devices, &selected, state, ui);
    if !has_devices {
        show_no_devices(ui);
    }

    for (entity_id, enabled) in changes {
        app_state.send_command(crate::ipc::IpcCommand::SetDeviceEnabled {
            server_ip: selected.ip.clone(),
            server_port: selected.port,
            entity_id,
            enabled,
        });
    }
}

fn show_devices_header(app_state: &AppState, selected: &ServerKey, ui: &mut egui::Ui) {
    let servers = app_state.servers_rx.borrow();
    let server_name = servers
        .iter()
        .find(|server| server.ip == selected.ip && server.port == selected.port)
        .and_then(|server| server.name.as_deref())
        .unwrap_or("Selected server");
    ui.heading(egui::RichText::new("Smart devices").color(egui::Color32::WHITE));
    ui.label(
        egui::RichText::new(format!("{server_name} · {}:{}", selected.ip, selected.port))
            .color(egui::Color32::from_rgb(165, 165, 165))
            .size(12.0),
    );
    ui.add_space(16.0);
}

fn show_device_list(
    devices: &mut [crate::config::store::DeviceConfig],
    selected: &ServerKey,
    state: &mut ServersState<'_>,
    ui: &mut egui::Ui,
) -> (bool, Vec<(u32, bool)>) {
    let mut has_devices = false;
    let mut changes = Vec::new();
    egui::ScrollArea::vertical()
        .id_salt("devices_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for device in devices.iter_mut().filter(|device| {
                device.server_ip == selected.ip && device.server_port == selected.port
            }) {
                has_devices = true;
                if show_device_card(device, selected, state, ui) {
                    changes.push((device.entity_id, device.enabled));
                }
                ui.add_space(10.0);
            }
        });
    (has_devices, changes)
}

fn show_device_card(
    device: &mut crate::config::store::DeviceConfig,
    selected: &ServerKey,
    state: &mut ServersState<'_>,
    ui: &mut egui::Ui,
) -> bool {
    let mut changed = false;
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            show_device_identity(device, ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                changed = show_device_toggle(device, ui);
            });
        });
        ui.add_space(10.0);
        show_device_actions(device, selected, state, ui);
    });
    changed
}

fn show_device_identity(device: &crate::config::store::DeviceConfig, ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(&device.entity_name)
                .color(egui::Color32::WHITE)
                .size(14.0)
                .strong(),
        );
        ui.label(
            egui::RichText::new(format!("Entity ID {}", device.entity_id))
                .color(egui::Color32::from_rgb(165, 165, 165))
                .size(11.0),
        );
    });
}

fn show_device_toggle(device: &mut crate::config::store::DeviceConfig, ui: &mut egui::Ui) -> bool {
    let response = custom_toggle(
        ui,
        &mut device.enabled,
        &format!("Alerts for {}", device.entity_name),
    )
    .on_hover_text(if device.enabled {
        "Disable alerts for this device"
    } else {
        "Enable alerts for this device"
    });
    ui.label(
        egui::RichText::new(if device.enabled {
            "Alerts on"
        } else {
            "Alerts off"
        })
        .color(egui::Color32::from_rgb(190, 190, 190))
        .size(12.0),
    );
    response.changed()
}

fn show_device_actions(
    device: &crate::config::store::DeviceConfig,
    selected: &ServerKey,
    state: &mut ServersState<'_>,
    ui: &mut egui::Ui,
) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Only notifications tagged with this entity are affected.")
                .color(egui::Color32::from_rgb(150, 150, 150))
                .size(11.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Remove device…").clicked() {
                *state.confirmation = Some(Confirmation::RemoveDevice {
                    server: selected.clone(),
                    entity_id: device.entity_id,
                    name: device.entity_name.clone(),
                });
            }
        });
    });
}

fn show_no_devices(ui: &mut egui::Ui) {
    card_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("No smart devices are paired with this server yet.")
                .color(egui::Color32::from_rgb(180, 180, 180))
                .size(13.0),
        );
        ui.label(
            egui::RichText::new(
                "Pair a Smart Alarm in Rust+ while NODIrust is running and connected.",
            )
            .color(egui::Color32::from_rgb(155, 155, 155))
            .size(12.0),
        );
    });
}

fn show_empty_state(app_state: &AppState, state: &mut ServersState<'_>, ui: &mut egui::Ui) {
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            egui::RichText::new("Pair your first server")
                .color(egui::Color32::WHITE)
                .size(16.0)
                .strong(),
        );
        ui.add_space(8.0);

        let steam_ready = *app_state.steam_logged_in.borrow();
        let push_ready = *app_state.push_status.borrow() == ConnectionStatus::Connected;
        setup_step(ui, 1, "Link Steam in Overview", steam_ready);
        setup_step(ui, 2, "Wait for the push connection", push_ready);
        setup_step(
            ui,
            3,
            "Initiate server pairing in the official Rust+ app",
            false,
        );
        ui.add_space(14.0);
        if ui.button("Open Overview").clicked() {
            *state.active_tab = Tab::Dashboard;
        }
    });
}

fn setup_step(ui: &mut egui::Ui, number: usize, label: &str, complete: bool) {
    ui.horizontal(|ui| {
        let marker = if complete {
            "Complete".to_string()
        } else {
            format!("Step {number}")
        };
        let color = if complete {
            egui::Color32::from_rgb(74, 222, 128)
        } else {
            egui::Color32::from_rgb(190, 190, 190)
        };
        ui.label(egui::RichText::new(marker).color(color).size(11.0).strong());
        ui.label(
            egui::RichText::new(label)
                .color(egui::Color32::from_rgb(190, 190, 190))
                .size(12.0),
        );
    });
}
