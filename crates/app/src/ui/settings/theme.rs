use eframe::egui;

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

// Custom iOS style toggle
pub fn custom_toggle(ui: &mut egui::Ui, on: bool) -> egui::Response {
    let desired_size = egui::vec2(36.0, 20.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let how_on = if on { 1.0 } else { 0.0 };
        let radius = 10.0;

        let bg_color = if on {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_rgb(38, 38, 38)
        };
        ui.painter()
            .rect(rect, radius, bg_color, egui::Stroke::NONE);

        let circle_x = egui::lerp(
            (rect.left() + radius + 2.0)..=(rect.right() - radius - 2.0),
            how_on,
        );
        let center = egui::pos2(circle_x, rect.center().y);

        let dot_color = if on {
            egui::Color32::BLACK
        } else {
            egui::Color32::from_rgb(156, 163, 175)
        };
        ui.painter()
            .circle(center, radius - 4.0, dot_color, egui::Stroke::NONE);
    }
    response
}
