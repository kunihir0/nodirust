//! Egui settings and status dashboard.

mod chrome;
pub mod dashboard;
mod dialogs;
pub mod pair;
pub mod servers;
pub mod theme;

use crate::app_state::AppState;
use eframe::egui;
use std::sync::atomic::AtomicBool;
use theme::apply_theme;

pub(super) use dialogs::Confirmation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Servers,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerKey {
    pub ip: String,
    pub port: u16,
}

impl ServerKey {
    fn label(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
}

pub struct SettingsWindow {
    app_state: AppState,
    active_tab: Tab,
    selected_server: Option<ServerKey>,
    compact_show_detail: bool,
    confirmation: Option<Confirmation>,
    pair_action_pending: bool,
    auth_in_progress: std::sync::Arc<AtomicBool>,
}

impl SettingsWindow {
    pub fn new(cc: &eframe::CreationContext<'_>, app_state: AppState) -> Self {
        apply_theme(&cc.egui_ctx);

        Self {
            app_state,
            active_tab: Tab::Dashboard,
            selected_server: None,
            compact_show_detail: false,
            confirmation: None,
            pair_action_pending: false,
            auth_in_progress: std::sync::Arc::new(AtomicBool::new(false)),
        }
    }

    fn draw_sidebar(&mut self, ctx: &egui::Context) {
        let sidebar_frame = egui::Frame::none()
            .fill(egui::Color32::BLACK)
            .inner_margin(egui::Margin::same(16.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(31, 31, 31)));

        egui::SidePanel::left("nav_panel")
            .resizable(false)
            .exact_width(172.0)
            .frame(sidebar_frame)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("NODI")
                            .strong()
                            .size(16.0)
                            .color(egui::Color32::WHITE),
                    );
                    ui.label(
                        egui::RichText::new("rust")
                            .size(16.0)
                            .color(egui::Color32::from_rgb(150, 150, 150)),
                    );
                });
                ui.add_space(24.0);

                if nav_button(ui, "Overview", self.active_tab == Tab::Dashboard).clicked() {
                    self.active_tab = Tab::Dashboard;
                }
                if nav_button(ui, "Servers & devices", self.active_tab == Tab::Servers).clicked() {
                    self.active_tab = Tab::Servers;
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(
                        egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .color(egui::Color32::from_rgb(145, 145, 145))
                            .size(11.0),
                    );
                });
            });
    }

    fn draw_feedback(&self, ctx: &egui::Context) {
        let feedback = self.app_state.feedback_rx.borrow().clone();
        let Some(feedback) = feedback else {
            return;
        };

        let (color, icon) = match feedback.level {
            crate::ipc::FeedbackLevel::Success => {
                (egui::Color32::from_rgb(74, 222, 128), "Success")
            }
            crate::ipc::FeedbackLevel::Error => (egui::Color32::from_rgb(248, 113, 113), "Error"),
        };

        egui::TopBottomPanel::top("feedback_bar")
            .frame(
                egui::Frame::none()
                    .fill(color.gamma_multiply(0.10))
                    .inner_margin(egui::Margin::symmetric(16.0, 8.0))
                    .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.35))),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon).color(color).strong());
                    ui.label(
                        egui::RichText::new(feedback.message)
                            .color(egui::Color32::from_rgb(225, 225, 225)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("Dismiss")
                            .on_hover_text("Dismiss message")
                            .clicked()
                        {
                            let _ = self.app_state.feedback_tx.send(None);
                        }
                    });
                });
            });
    }

    fn draw_main_content(&mut self, ctx: &egui::Context) {
        let central_frame = egui::Frame::none()
            .fill(egui::Color32::from_rgb(5, 5, 5))
            .inner_margin(egui::Margin::same(24.0));

        match self.active_tab {
            Tab::Dashboard => {
                egui::CentralPanel::default()
                    .frame(central_frame)
                    .show(ctx, |ui| {
                        dashboard::show_dashboard(
                            &self.app_state,
                            &self.auth_in_progress,
                            &mut self.confirmation,
                            ui,
                        );
                        chrome::draw_resize_grip(ctx, ui);
                    });
            }
            Tab::Servers => self.draw_servers(ctx, central_frame),
        }
    }

    fn draw_servers(&mut self, ctx: &egui::Context, central_frame: egui::Frame) {
        let compact = ctx.screen_rect().width() < 900.0;
        if !compact {
            self.compact_show_detail = false;
        }
        let mut state = servers::ServersState {
            selected_server: &mut self.selected_server,
            confirmation: &mut self.confirmation,
            compact_show_detail: &mut self.compact_show_detail,
            active_tab: &mut self.active_tab,
        };

        if compact {
            egui::CentralPanel::default()
                .frame(central_frame)
                .show(ctx, |ui| {
                    if *state.compact_show_detail && state.selected_server.is_some() {
                        if ui.button("< All servers").clicked() {
                            *state.compact_show_detail = false;
                        }
                        ui.add_space(12.0);
                        servers::show_devices_pane(&self.app_state, &mut state, ui);
                    } else {
                        servers::show_servers_pane(&self.app_state, &mut state, ui);
                    }
                    chrome::draw_resize_grip(ctx, ui);
                });
            return;
        }

        egui::SidePanel::left("servers_list_panel")
            .resizable(true)
            .min_width(220.0)
            .max_width(340.0)
            .default_width(280.0)
            .frame(central_frame)
            .show(ctx, |ui| {
                servers::show_servers_pane(&self.app_state, &mut state, ui);
            });

        egui::CentralPanel::default()
            .frame(central_frame)
            .show(ctx, |ui| {
                servers::show_devices_pane(&self.app_state, &mut state, ui);
                chrome::draw_resize_grip(ctx, ui);
            });
    }

    fn show_overlays(&mut self, ctx: &egui::Context) {
        let pending_pair = self.app_state.pending_pair_rx.borrow().clone();
        if self.pair_action_pending && pending_pair.is_none() {
            self.pair_action_pending = false;
            self.active_tab = Tab::Servers;
        }

        if let Some(server) = pending_pair {
            pair::show_pair_modal(ctx, &self.app_state, &server, &mut self.pair_action_pending);
        } else if let Some(confirmation) = self.confirmation.clone() {
            dialogs::show_confirmation_modal(
                ctx,
                &self.app_state,
                &confirmation,
                &mut self.confirmation,
            );
        }
    }
}

fn nav_button(ui: &mut egui::Ui, text: &str, active: bool) -> egui::Response {
    let text = egui::RichText::new(text).size(13.0).color(if active {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_rgb(160, 160, 160)
    });
    let button = egui::Button::new(text)
        .fill(if active {
            egui::Color32::from_rgb(24, 24, 24)
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(if active {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 48, 48))
        } else {
            egui::Stroke::NONE
        });
    ui.add_sized([ui.available_width(), 38.0], button)
}

impl eframe::App for SettingsWindow {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut ctx_lock = self.app_state.ui_context.lock().unwrap();
        if ctx_lock.is_none() {
            *ctx_lock = Some(ctx.clone());
        }
        drop(ctx_lock);

        if ctx.input(|input| input.viewport().close_requested()) {
            std::process::exit(0);
        }

        ctx.layer_painter(egui::LayerId::background()).rect_filled(
            ctx.screen_rect(),
            egui::Rounding::same(8.0),
            egui::Color32::from_rgb(5, 5, 5),
        );

        chrome::draw_title_bar(ctx);
        self.draw_feedback(ctx);
        self.draw_sidebar(ctx);
        self.draw_main_content(ctx);
        self.show_overlays(ctx);
    }
}
