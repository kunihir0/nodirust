use eframe::egui;
use crate::app_state::AppState;
use super::theme::custom_toggle;

pub struct ServersState<'a> {
    pub selected_server: &'a mut Option<String>,
    pub confirm_unpair_server: &'a mut Option<usize>,
    pub confirm_unpair_device: &'a mut Option<usize>,
}

pub fn show_servers_pane(
    app_state: &AppState,
    state: &mut ServersState<'_>,
    ui: &mut egui::Ui,
) {
    ui.heading(egui::RichText::new("Servers").color(egui::Color32::WHITE));
    ui.add_space(8.0);

    let servers = app_state.servers_rx.borrow().clone();

    if servers.is_empty() {
        show_empty_state(ui);
        return;
    }

    // Auto-select first server if none is selected
    if state.selected_server.is_none() && !servers.is_empty() {
        *state.selected_server = Some(format!("{}:{}", servers[0].ip, servers[0].port));
    }

    let statuses = app_state.server_statuses_rx.borrow().clone();
    let mut server_to_remove = None;

    egui::ScrollArea::vertical().id_salt("servers_scroll").auto_shrink([false, false]).show(ui, |ui| {
        for (idx, server) in servers.iter().enumerate() {
            let ip_port = format!("{}:{}", server.ip, server.port);
            let is_connected = statuses.get(&ip_port).copied().unwrap_or(false);
            let is_selected = Some(&ip_port) == state.selected_server.as_ref();

            let bg_color = if is_selected {
                egui::Color32::from_rgb(26, 26, 26) // Highlighted
            } else {
                egui::Color32::from_rgb(10, 10, 10)
            };

            let card_frame = egui::Frame::none()
                .fill(bg_color)
                .stroke(egui::Stroke::new(1.0_f32, if is_selected { egui::Color32::from_rgb(60, 60, 60) } else { egui::Color32::from_rgb(31, 31, 31) }))
                .rounding(6.0)
                .inner_margin(12.0);

            let response = card_frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        let name_display = server.name.as_deref().unwrap_or("Unknown Server");
                        ui.label(egui::RichText::new(name_display).color(egui::Color32::WHITE).size(14.0).strong());
                        ui.add_space(2.0);
                        
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&ip_port).color(egui::Color32::from_rgb(150, 150, 150)).size(11.0));
                            let btn = egui::Button::new(egui::RichText::new("📋").size(10.0)).fill(egui::Color32::TRANSPARENT);
                            let response = ui.add(btn).on_hover_text("Copy connect command");
                            if response.clicked() {
                                ui.output_mut(|o| o.copied_text = format!("client.connect {}", ip_port));
                            }
                        });

                        let status_color = if is_connected { egui::Color32::from_rgb(34, 197, 94) } else { egui::Color32::from_rgb(239, 68, 68) };
                        let status_text = if is_connected { "Connected" } else { "Reconnecting" };
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let (rect, _resp) = ui.allocate_exact_size(egui::vec2(6.0, 6.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 3.0, status_color);
                            ui.label(egui::RichText::new(status_text).color(status_color).size(11.0));
                        });
                    });

                    // Unpair action
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if *state.confirm_unpair_server == Some(idx) {
                            let btn = egui::Button::new(egui::RichText::new("Sure?").color(egui::Color32::WHITE))
                                .fill(egui::Color32::from_rgb(153, 27, 27));
                            if ui.add_sized([45.0, 24.0], btn).clicked() {
                                server_to_remove = Some(idx);
                                *state.confirm_unpair_server = None;
                            }
                        } else {
                            let btn = egui::Button::new(egui::RichText::new("✕").color(egui::Color32::from_rgb(150, 150, 150)))
                                .fill(egui::Color32::TRANSPARENT);
                            if ui.add(btn).clicked() {
                                *state.confirm_unpair_server = Some(idx);
                            }
                        }
                    });
                });
            }).response;

            // Click anywhere on the card to select it (except the unpair button which consumes clicks)
            let interact = ui.interact(response.rect, ui.id().with(idx), egui::Sense::click());
            if interact.clicked() {
                *state.selected_server = Some(ip_port);
                *state.confirm_unpair_server = None;
            }

            ui.add_space(8.0);
        }
    });

    if let Some(idx) = server_to_remove {
        let mut current_servers = app_state.servers_rx.borrow().clone();
        let removed_ip_port = format!("{}:{}", current_servers[idx].ip, current_servers[idx].port);
        current_servers.remove(idx);
        if let Some(tx) = &app_state.command_tx {
            let _ = tx.try_send(crate::ipc::IpcCommand::UpdateServers(current_servers));
        }
        if *state.selected_server == Some(removed_ip_port) {
            *state.selected_server = None;
        }
    }
}

pub fn show_devices_pane(
    app_state: &AppState,
    state: &mut ServersState<'_>,
    ui: &mut egui::Ui,
) {
    ui.heading(egui::RichText::new("Smart Devices").color(egui::Color32::WHITE));
    ui.add_space(8.0);

    let Some(selected_ip_port) = state.selected_server.as_ref() else {
        ui.add_space(20.0);
        ui.label(egui::RichText::new("Select a server to view its devices.").color(egui::Color32::from_rgb(115, 115, 115)).size(13.0));
        return;
    };

    let mut parts = selected_ip_port.split(':');
    let ip = parts.next().unwrap_or("");
    let port_str = parts.next().unwrap_or("");
    
    let mut devices = app_state.devices_rx.borrow().clone();
    let mut changed = false;
    let mut device_to_remove = None;

    egui::ScrollArea::vertical().id_salt("devices_scroll").auto_shrink([false, false]).show(ui, |ui| {
        let server_devices = devices.iter_mut().enumerate().filter(|(_, d)| d.server_ip == ip && d.server_port.to_string() == port_str);
        let mut has_devices = false;

        for (idx, device) in server_devices {
            has_devices = true;
            let card_frame = egui::Frame::none()
                .fill(egui::Color32::from_rgb(12, 12, 12))
                .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(31, 31, 31)))
                .rounding(6.0)
                .inner_margin(12.0);

            card_frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&device.entity_name)
                            .color(egui::Color32::WHITE)
                            .size(14.0)
                            .strong());
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Alerts:").color(egui::Color32::from_rgb(115, 115, 115)).size(11.0));
                            if custom_toggle(ui, device.enabled).clicked() {
                                device.enabled = !device.enabled;
                                changed = true;
                            }
                        });
                    });
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if *state.confirm_unpair_device == Some(idx) {
                            let btn = egui::Button::new(egui::RichText::new("Sure?").color(egui::Color32::WHITE))
                                .fill(egui::Color32::from_rgb(153, 27, 27));
                            if ui.add_sized([45.0, 24.0], btn).clicked() {
                                device_to_remove = Some(idx);
                                *state.confirm_unpair_device = None;
                            }
                        } else {
                            let btn = egui::Button::new(egui::RichText::new("Unpair").color(egui::Color32::from_rgb(239, 68, 68)))
                                .fill(egui::Color32::from_rgb(20, 20, 20))
                                .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(127, 29, 29)));
                            if ui.add(btn).clicked() {
                                *state.confirm_unpair_device = Some(idx);
                            }
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }

        if !has_devices {
            ui.add_space(20.0);
            ui.label(egui::RichText::new("No devices paired on this server.").color(egui::Color32::from_rgb(115, 115, 115)).size(13.0));
        }
    });

    if let Some(idx) = device_to_remove {
        devices.remove(idx);
        changed = true;
    }

    if changed {
        if let Some(tx) = &app_state.command_tx {
            let _ = tx.try_send(crate::ipc::IpcCommand::UpdateDevices(devices));
        }
    }
}

fn show_empty_state(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(egui::RichText::new("No servers paired yet.").color(egui::Color32::WHITE).size(15.0).strong());
        ui.add_space(8.0);
        ui.label(egui::RichText::new("To pair a server, first pair it in the official Rust+ mobile app.\nThen, ensure your Steam account is linked here to sync them automatically.")
            .color(egui::Color32::from_rgb(115, 115, 115))
            .size(13.0));
    });
}
