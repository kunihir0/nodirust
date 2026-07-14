#![allow(clippy::collapsible_if)] // UI layouts often benefit from nested ifs for readability

//! Egui settings and status dashboard.

use eframe::egui;

use crate::app_state::AppState;

#[derive(PartialEq)]
enum Tab {
    Dashboard,
    Servers,
    Devices,
}

pub struct SettingsWindow {
    app_state: AppState,
    active_tab: Tab,
}

pub fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // --- 1. Spacing & Layout ---
    style.spacing.item_spacing = egui::vec2(16.0, 16.0);
    style.spacing.button_padding = egui::vec2(16.0, 8.0);

    // --- 2. Visuals (Colors) ---
    let mut visuals = egui::Visuals::dark();

    let app_bg = egui::Color32::from_rgb(5, 5, 5); // #050505
    let border_color = egui::Color32::from_rgb(31, 31, 31); // #1f1f1f
    let btn_bg = egui::Color32::from_rgb(20, 20, 20); // #141414
    let primary_text = egui::Color32::from_rgb(240, 240, 240);
    let accent_white = egui::Color32::WHITE;

    visuals.window_fill = app_bg;
    visuals.panel_fill = app_bg; 

    visuals.override_text_color = Some(primary_text);
    
    visuals.selection.bg_fill = egui::Color32::from_rgb(17, 17, 17); // #111111 (active nav)
    visuals.selection.stroke = egui::Stroke::NONE;

    // --- 3. Widget States ---
    visuals.widgets.inactive.bg_fill = btn_bg;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, border_color);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, primary_text);
    visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);

    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(26, 26, 26); // #1a1a1a
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, border_color);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, accent_white);
    visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);

    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(40, 40, 40);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, border_color);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, accent_white);
    visuals.widgets.active.rounding = egui::Rounding::same(6.0);

    style.visuals = visuals;
    ctx.set_style(style);

    // Fonts
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Nunito".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../../fonts/Nunito-Regular.ttf"))),
    );
    fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "Nunito".to_owned());
    ctx.set_fonts(fonts);
}

impl SettingsWindow {
    pub fn new(cc: &eframe::CreationContext<'_>, app_state: AppState) -> Self {
        apply_theme(&cc.egui_ctx);
        
        Self {
            app_state,
            active_tab: Tab::Dashboard,
        }
    }
}

impl eframe::App for SettingsWindow {
    #[allow(clippy::too_many_lines)] // UI rendering requires monolithic layout flow
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Save ctx for background threads
        let mut ctx_lock = self.app_state.ui_context.lock().unwrap();
        if ctx_lock.is_none() {
            *ctx_lock = Some(ctx.clone());
        }
        drop(ctx_lock);

        // In the new architecture, closing the window simply exits the UI process
        if ctx.input(|i| i.viewport().close_requested()) {
            std::process::exit(0);
        }

        // Force background fill to prevent any transparent gaps between panels
        ctx.layer_painter(egui::LayerId::background()).rect_filled(
            ctx.screen_rect(),
            egui::Rounding::same(8.0), // Rounded corners for the whole app
            egui::Color32::from_rgb(5, 5, 5)
        );

        let title_frame = egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(8.0, 4.0)); // Removed explicit fill to let background show, or we can use the same color

        egui::TopBottomPanel::top("title_bar")
            .frame(title_frame)
            .exact_height(32.0)
            .show(ctx, |ui| {
                let title_bar_rect = ui.max_rect();
                let title_bar_response = ui.interact(title_bar_rect, ui.id(), egui::Sense::click_and_drag());
                if title_bar_response.is_pointer_button_down_on() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                ui.horizontal_centered(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let close_btn = egui::Button::new(egui::RichText::new("X").size(14.0).strong().color(egui::Color32::from_rgb(150, 150, 150)))
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                            .rounding(12.0);
                        
                        if ui.add_sized([24.0, 24.0], close_btn).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            std::process::exit(0);
                        }
                        
                        let min_btn = egui::Button::new(egui::RichText::new("-").size(16.0).strong().color(egui::Color32::from_rgb(150, 150, 150)))
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                            .rounding(12.0);

                        if ui.add_sized([24.0, 24.0], min_btn).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                    });
                });
            });

        let sidebar_frame = egui::Frame::none()
            .fill(egui::Color32::BLACK) // #000000
            .inner_margin(egui::Margin::same(16.0))
            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(31, 31, 31)));

        egui::SidePanel::left("nav_panel").resizable(false).frame(sidebar_frame).show(ctx, |ui| {
            ui.add_space(10.0);
            
            // Header / Logo
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("NODI").strong().size(16.0).color(egui::Color32::WHITE));
                ui.label(egui::RichText::new("rust").size(16.0).color(egui::Color32::from_rgb(115, 115, 115)));
            });
            
            ui.add_space(20.0);

            let nav_button = |ui: &mut egui::Ui, text: &str, active: bool| -> egui::Response {
                let text_rt = egui::RichText::new(text).size(13.0).color(
                    if active { egui::Color32::WHITE } else { egui::Color32::from_rgb(115, 115, 115) }
                );
                
                let btn = egui::Button::new(text_rt)
                    .fill(if active { egui::Color32::from_rgb(17, 17, 17) } else { egui::Color32::TRANSPARENT })
                    .stroke(egui::Stroke::NONE);
                    
                ui.add_sized([ui.available_width(), 36.0], btn)
            };

            if nav_button(ui, "Dashboard", self.active_tab == Tab::Dashboard).clicked() {
                self.active_tab = Tab::Dashboard;
            }
            if nav_button(ui, "Paired Servers", self.active_tab == Tab::Servers).clicked() {
                self.active_tab = Tab::Servers;
            }
            if nav_button(ui, "Devices", self.active_tab == Tab::Devices).clicked() {
                self.active_tab = Tab::Devices;
            }
        });

        let central_frame = egui::Frame::none()
            .fill(egui::Color32::from_rgb(5, 5, 5)) // #050505
            .inner_margin(egui::Margin::same(32.0));

        egui::CentralPanel::default().frame(central_frame).show(ctx, |ui| {
            let pending_pair = self.app_state.pending_pair_rx.borrow().clone();
            if let Some(server) = pending_pair {
                self.show_pair_screen(ui, ctx, &server);
            } else {
                match self.active_tab {
                    Tab::Dashboard => self.show_dashboard(ui),
                    Tab::Servers => self.show_servers(ui),
                    Tab::Devices => self.show_devices(ui),
                }
            }
        });

        // eframe will now sleep when nothing is happening instead of spinning at 10Hz
    }
}

// Custom iOS style toggle
fn custom_toggle(ui: &mut egui::Ui, on: bool) -> egui::Response {
    let desired_size = egui::vec2(36.0, 20.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    
    if ui.is_rect_visible(rect) {
        let how_on = if on { 1.0 } else { 0.0 }; 
        let radius = 10.0;
        
        let bg_color = if on { egui::Color32::WHITE } else { egui::Color32::from_rgb(38, 38, 38) };
        ui.painter().rect(rect, radius, bg_color, egui::Stroke::NONE);

        let circle_x = egui::lerp((rect.left() + radius + 2.0)..=(rect.right() - radius - 2.0), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        
        let dot_color = if on { egui::Color32::BLACK } else { egui::Color32::from_rgb(156, 163, 175) };
        ui.painter().circle(center, radius - 4.0, dot_color, egui::Stroke::NONE);
    }
    response
}

impl SettingsWindow {
    fn show_dashboard(&mut self, ui: &mut egui::Ui) {
        ui.heading(egui::RichText::new("Dashboard").color(egui::Color32::WHITE));
        ui.add_space(30.0);

        let is_fcm_connected = *self.app_state.fcm_connected.borrow();
        let is_steam_logged_in = *self.app_state.steam_logged_in.borrow();

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
                        if let Some(tx) = &self.app_state.command_tx {
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
                                    let command_tx = self.app_state.command_tx.clone();
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

    fn show_servers(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("Paired Servers").color(egui::Color32::WHITE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let btn = egui::Button::new(egui::RichText::new("Clear All").color(egui::Color32::WHITE))
                    .fill(egui::Color32::from_rgb(153, 27, 27)) // Destructive dark red
                    .stroke(egui::Stroke::NONE);
                if ui.add_sized([90.0, 32.0], btn).clicked() {
                    if let Some(tx) = &self.app_state.command_tx {
                        let _ = tx.try_send(crate::ipc::IpcCommand::UpdateServers(vec![]));
                    }
                }
            });
        });
        
        ui.add_space(20.0);

        let servers = self.app_state.servers_rx.borrow().clone();
        let statuses = self.app_state.server_statuses_rx.borrow().clone();
        
        if servers.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(egui::RichText::new("No servers paired yet.").color(egui::Color32::WHITE).size(15.0).strong());
                ui.add_space(8.0);
                ui.label(egui::RichText::new("To pair a server, first pair it in the official Rust+ mobile app.\nThen, ensure your Steam account is linked here to sync them automatically.")
                    .color(egui::Color32::from_rgb(115, 115, 115))
                    .size(13.0));
            });
        } else {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut server_to_remove = None;
                for (idx, server) in servers.iter().enumerate() {
                    let ip_port = format!("{}:{}", server.ip, server.port);
                    let is_connected = statuses.get(&ip_port).copied().unwrap_or(false);
                    
                    // Draw Card
                    let card_frame = egui::Frame::none()
                        .fill(egui::Color32::from_rgb(10, 10, 10)) // #0a0a0a
                        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(31, 31, 31))) // #1f1f1f
                        .rounding(8.0)
                        .inner_margin(16.0);

                    card_frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&ip_port)
                                        .color(egui::Color32::WHITE)
                                        .size(15.0)
                                        .strong());
                                        
                                    // Status Indicator
                                    let status_color = if is_connected {
                                        egui::Color32::from_rgb(34, 197, 94) // Green
                                    } else {
                                        egui::Color32::from_rgb(239, 68, 68) // Red
                                    };
                                    let status_text = if is_connected { "Connected" } else { "Reconnecting..." };
                                    
                                    ui.add_space(8.0);
                                    let (rect, _resp) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 4.0, status_color);
                                    ui.label(egui::RichText::new(status_text).color(status_color).size(12.0));
                                });
                                
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new(format!("Player ID: {}", server.player_id))
                                    .color(egui::Color32::from_rgb(115, 115, 115))
                                    .size(12.0));
                            });
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let btn = egui::Button::new(egui::RichText::new("Unpair").color(egui::Color32::from_rgb(239, 68, 68)))
                                    .fill(egui::Color32::from_rgb(20, 20, 20)) // #141414
                                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(127, 29, 29)));
                                
                                if ui.add_sized([80.0, 32.0], btn).clicked() {
                                    server_to_remove = Some(idx);
                                }
                            });
                        });
                    });
                    ui.add_space(12.0);
                }

                if let Some(idx) = server_to_remove {
                    let mut servers = self.app_state.servers_rx.borrow().clone();
                    servers.remove(idx);
                    if let Some(tx) = &self.app_state.command_tx {
                        let _ = tx.try_send(crate::ipc::IpcCommand::UpdateServers(servers));
                    }
                }
            });
        }
    }

    fn show_pair_screen(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context, server: &crate::config::store::ServerConfig) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.heading(egui::RichText::new("New Server Pairing").color(egui::Color32::WHITE).size(20.0).strong());
            ui.add_space(8.0);
            ui.label(egui::RichText::new("A new Rust server has requested to pair with your desktop.")
                .color(egui::Color32::from_rgb(150, 150, 150))
                .size(14.0));
            
            ui.add_space(30.0);
            
            let card_frame = egui::Frame::none()
                .fill(egui::Color32::from_rgb(15, 15, 15)) // slightly lighter background for emphasis
                .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(40, 40, 40)))
                .rounding(8.0)
                .inner_margin(24.0);

            card_frame.show(ui, |ui| {
                ui.label(egui::RichText::new(format!("{}:{}", server.ip, server.port))
                    .color(egui::Color32::WHITE)
                    .size(18.0)
                    .strong());
                ui.add_space(8.0);
                ui.label(egui::RichText::new(format!("Player ID: {}", server.player_id))
                    .color(egui::Color32::from_rgb(115, 115, 115))
                    .size(13.0));
            });

            ui.add_space(40.0);
            
            ui.horizontal_centered(|ui| {
                let btn_decline = egui::Button::new(egui::RichText::new("Decline").color(egui::Color32::from_rgb(240, 240, 240)))
                    .fill(egui::Color32::from_rgb(26, 26, 26))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(40, 40, 40)));
                    
                if ui.add_sized([120.0, 40.0], btn_decline).clicked() {
                    if let Some(tx) = &self.app_state.command_tx {
                        let _ = tx.try_send(crate::ipc::IpcCommand::DeclinePairing);
                    }
                }
                
                ui.add_space(20.0);

                let btn_accept = egui::Button::new(egui::RichText::new("Pair Server").color(egui::Color32::BLACK))
                    .fill(egui::Color32::WHITE)
                    .stroke(egui::Stroke::NONE);
                    
                if ui.add_sized([120.0, 40.0], btn_accept).clicked() {
                    if let Some(tx) = &self.app_state.command_tx {
                        let _ = tx.try_send(crate::ipc::IpcCommand::AcceptPairing(server.clone()));
                    }
                    self.active_tab = Tab::Servers;
                }
            });
        });
    }

    fn show_devices(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("Smart Devices").color(egui::Color32::WHITE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let btn = egui::Button::new(egui::RichText::new("Clear All").color(egui::Color32::WHITE))
                    .fill(egui::Color32::from_rgb(153, 27, 27))
                    .stroke(egui::Stroke::NONE);
                if ui.add_sized([90.0, 32.0], btn).clicked() {
                    if let Some(tx) = &self.app_state.command_tx {
                        let _ = tx.try_send(crate::ipc::IpcCommand::UpdateDevices(vec![]));
                    }
                }
            });
        });
        
        ui.add_space(20.0);

        let mut devices = self.app_state.devices_rx.borrow().clone();
        
        if devices.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(egui::RichText::new("No devices paired yet.").color(egui::Color32::WHITE).size(15.0).strong());
                ui.add_space(8.0);
                ui.label(egui::RichText::new("To pair a smart device, go in-game and pair it through the Rust+ menu.")
                    .color(egui::Color32::from_rgb(115, 115, 115))
                    .size(13.0));
            });
        } else {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut changed = false;
                let mut device_to_remove = None;

                for (idx, device) in devices.iter_mut().enumerate() {
                    let card_frame = egui::Frame::none()
                        .fill(egui::Color32::from_rgb(10, 10, 10))
                        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(31, 31, 31)))
                        .rounding(8.0)
                        .inner_margin(16.0);

                    card_frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(&device.entity_name)
                                    .color(egui::Color32::WHITE)
                                    .size(15.0)
                                    .strong());
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new(format!("Server: {}:{}", device.server_ip, device.server_port))
                                    .color(egui::Color32::from_rgb(115, 115, 115))
                                    .size(12.0));
                            });
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let btn = egui::Button::new(egui::RichText::new("Unpair").color(egui::Color32::from_rgb(239, 68, 68)))
                                    .fill(egui::Color32::from_rgb(20, 20, 20))
                                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(127, 29, 29)));
                                
                                if ui.add_sized([80.0, 32.0], btn).clicked() {
                                    device_to_remove = Some(idx);
                                }

                                ui.add_space(12.0);
                                
                                ui.vertical_centered(|ui| {
                                    ui.add_space(6.0);
                                    if custom_toggle(ui, device.enabled).clicked() {
                                        device.enabled = !device.enabled;
                                        changed = true;
                                    }
                                });
                                ui.label(egui::RichText::new("Notifications").color(egui::Color32::from_rgb(150, 150, 150)).size(12.0));
                            });
                        });
                    });
                    ui.add_space(12.0);
                }

                if let Some(idx) = device_to_remove {
                    devices.remove(idx);
                    changed = true;
                }

                if changed {
                    if let Some(tx) = &self.app_state.command_tx {
                        let _ = tx.try_send(crate::ipc::IpcCommand::UpdateDevices(devices));
                    }
                }
            });
        }
    }
}
