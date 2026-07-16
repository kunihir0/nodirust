use eframe::egui;

pub fn draw_title_bar(ctx: &egui::Context) {
    let frame = egui::Frame::none()
        .fill(egui::Color32::from_rgb(8, 8, 8))
        .inner_margin(egui::Margin::symmetric(12.0, 5.0))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgb(31, 31, 31),
        ));
    egui::TopBottomPanel::top("title_bar")
        .frame(frame)
        .exact_height(38.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                draw_title(ui);
                draw_drag_region(ctx, ui);
                draw_window_controls(ctx, ui);
            });
        });
}

fn draw_title(ui: &mut egui::Ui) {
    ui.label(
        egui::RichText::new("NODIrust")
            .color(egui::Color32::from_rgb(210, 210, 210))
            .size(12.0)
            .strong(),
    );
}

fn draw_drag_region(ctx: &egui::Context, ui: &mut egui::Ui) {
    let drag_width = (ui.available_width() - 100.0).max(0.0);
    let response = ui.allocate_response(
        egui::vec2(drag_width, ui.available_height()),
        egui::Sense::click_and_drag(),
    );
    if response.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if response.double_clicked() {
        let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
}

fn draw_window_controls(ctx: &egui::Context, ui: &mut egui::Ui) {
    let minimize = window_button("−", 17.0, "Minimize", ui);
    if minimize.clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
    let close = window_button("×", 18.0, "Close settings", ui);
    if close.clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

fn window_button(symbol: &str, size: f32, tooltip: &str, ui: &mut egui::Ui) -> egui::Response {
    let button = egui::Button::new(
        egui::RichText::new(symbol)
            .size(size)
            .color(egui::Color32::from_rgb(205, 205, 205)),
    )
    .fill(egui::Color32::TRANSPARENT)
    .stroke(egui::Stroke::NONE);
    ui.add_sized([32.0, 26.0], button).on_hover_text(tooltip)
}

pub fn draw_resize_grip(ctx: &egui::Context, ui: &mut egui::Ui) {
    let grip_size = 18.0;
    let rect = egui::Rect::from_min_size(
        ui.max_rect().max - egui::vec2(grip_size, grip_size),
        egui::vec2(grip_size, grip_size),
    );
    let response = ui.interact(rect, ui.id().with("resize_grip"), egui::Sense::drag());
    if response.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(
            egui::ResizeDirection::SouthEast,
        ));
    }
    if response.hovered() || response.dragged() {
        ctx.set_cursor_icon(egui::CursorIcon::ResizeNwSe);
    }
}
