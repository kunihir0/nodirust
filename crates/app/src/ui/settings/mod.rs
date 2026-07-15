//! Egui settings and status dashboard.

pub mod theme;
pub mod dashboard;
pub mod servers;
pub mod pair;

use eframe::egui;
use crate::app_state::AppState;
use theme::apply_theme;

#[derive(PartialEq)]
pub enum Tab {
    Dashboard,
    Servers,
}

pub struct SettingsWindow {
    app_state: AppState,
    active_tab: Tab,
    selected_server: Option<String>,
    confirm_unpair_server: Option<usize>,
    confirm_unpair_device: Option<usize>,
}

impl SettingsWindow {
    pub fn new(cc: &eframe::CreationContext<'_>, app_state: AppState) -> Self {
        apply_theme(&cc.egui_ctx);
        
        Self {
            app_state,
            active_tab: Tab::Dashboard,
            selected_server: None,
            confirm_unpair_server: None,
            confirm_unpair_device: None,
        }
    }

    fn draw_title_bar(&mut self, ctx: &egui::Context) {
        let title_frame = egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(8.0, 4.0));

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
    }

    fn draw_sidebar(&mut self, ctx: &egui::Context) {
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
            if nav_button(ui, "Servers & Devices", self.active_tab == Tab::Servers).clicked() {
                self.active_tab = Tab::Servers;
            }
        });
    }

    fn draw_main_content(&mut self, ctx: &egui::Context) {
        let central_frame = egui::Frame::none()
            .fill(egui::Color32::from_rgb(5, 5, 5)) // #050505
            .inner_margin(egui::Margin::same(32.0));

        let pending_pair = self.app_state.pending_pair_rx.borrow().clone();

        if pending_pair.is_none() && self.active_tab == Tab::Servers {
            let mut state = servers::ServersState {
                selected_server: &mut self.selected_server,
                confirm_unpair_server: &mut self.confirm_unpair_server,
                confirm_unpair_device: &mut self.confirm_unpair_device,
            };
            
            egui::SidePanel::left("servers_list_panel")
                .resizable(true)
                .min_width(200.0)
                .default_width(280.0)
                .frame(central_frame)
                .show(ctx, |ui| {
                    servers::show_servers_pane(&self.app_state, &mut state, ui);
                });
                
            egui::CentralPanel::default().frame(central_frame).show(ctx, |ui| {
                servers::show_devices_pane(&self.app_state, &mut state, ui);
                draw_resize_grip(ctx, ui);
            });
            return;
        }

        egui::CentralPanel::default().frame(central_frame).show(ctx, |ui| {
            if let Some(server) = pending_pair {
                pair::show_pair_screen(&self.app_state, ui, &server, &mut self.active_tab);
            } else {
                match self.active_tab {
                    Tab::Dashboard => dashboard::show_dashboard(&self.app_state, ui),
                    Tab::Servers => unreachable!(),
                }
            }
            draw_resize_grip(ctx, ui);
        });
    }
}

fn draw_resize_grip(ctx: &egui::Context, ui: &mut egui::Ui) {
        let grip_size = 16.0;
        let rect = egui::Rect::from_min_size(
            ui.max_rect().max - egui::vec2(grip_size, grip_size),
            egui::vec2(grip_size, grip_size),
        );
        let response = ui.interact(rect, ui.id().with("resize_grip"), egui::Sense::drag());
        if response.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(egui::ResizeDirection::SouthEast));
        }
        if response.hovered() || response.dragged() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeNwSe);
        }
    }

impl eframe::App for SettingsWindow {
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

        self.draw_title_bar(ctx);
        self.draw_sidebar(ctx);
        self.draw_main_content(ctx);
    }
}
