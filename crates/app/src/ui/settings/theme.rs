use crate::ipc::ConnectionStatus;
use eframe::egui;

pub fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // --- 1. Spacing & Layout ---
    style.spacing.item_spacing = egui::vec2(12.0, 12.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);

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
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../../fonts/Nunito-Regular.ttf"
        ))),
    );
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "Nunito".to_owned());
    ctx.set_fonts(fonts);
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(12, 12, 12))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgb(38, 38, 38),
        ))
        .rounding(8.0)
        .inner_margin(16.0)
}

pub fn status_badge(ui: &mut egui::Ui, label: &str, color: egui::Color32) {
    egui::Frame::none()
        .fill(color.gamma_multiply(0.14))
        .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.45)))
        .rounding(12.0)
        .inner_margin(egui::Margin::symmetric(10.0, 4.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 3.5, color);
                ui.label(egui::RichText::new(label).color(color).size(12.0).strong());
            });
        });
}

pub fn connection_status_display(status: ConnectionStatus) -> (&'static str, egui::Color32) {
    match status {
        ConnectionStatus::SignedOut => ("Signed out", egui::Color32::from_rgb(156, 163, 175)),
        ConnectionStatus::Connecting => ("Connecting", egui::Color32::from_rgb(250, 204, 21)),
        ConnectionStatus::Connected => ("Connected", egui::Color32::from_rgb(74, 222, 128)),
        ConnectionStatus::Reconnecting => ("Reconnecting", egui::Color32::from_rgb(250, 204, 21)),
        ConnectionStatus::AuthenticationFailed => (
            "Authentication failed",
            egui::Color32::from_rgb(248, 113, 113),
        ),
        ConnectionStatus::Unreachable => ("Unavailable", egui::Color32::from_rgb(248, 113, 113)),
    }
}

/// Draws an accessible, keyboard-operable toggle and updates `on` when activated.
pub fn custom_toggle(ui: &mut egui::Ui, on: &mut bool, label: &str) -> egui::Response {
    let desired_size = egui::vec2(44.0, 24.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, label)
    });

    let keyboard_activated = response.has_focus()
        && ui.input(|input| {
            input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::Space)
        });
    if response.clicked() || keyboard_activated {
        *on = !*on;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *on);
        let radius = rect.height() / 2.0;

        let bg_color = if *on {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_rgb(52, 52, 52)
        };
        let stroke = if response.has_focus() {
            egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(125, 180, 255))
        } else {
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(90, 90, 90))
        };
        ui.painter().rect(rect, radius, bg_color, stroke);

        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);

        let dot_color = if *on {
            egui::Color32::BLACK
        } else {
            egui::Color32::from_rgb(210, 210, 210)
        };
        ui.painter()
            .circle(center, radius - 4.0, dot_color, egui::Stroke::NONE);
    }
    response
}
