use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub struct TrayMenu {
    pub dashboard_id: String,
    pub quit_id: String,
}

pub fn setup_tray() -> (TrayIcon, TrayMenu) {
    let tray_menu = Menu::new();
    let dashboard_item = MenuItem::new("Open Dashboard", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let dashboard_id = dashboard_item.id().0.clone();
    let quit_id = quit_item.id().0.clone();

    let _ = tray_menu.append_items(&[
        &dashboard_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ]);

    // Load the actual app icon
    let icon_data = include_bytes!("../../assets/app.ico");
    let image = image::load_from_memory_with_format(icon_data, image::ImageFormat::Ico)
        .expect("Failed to parse app.ico")
        .into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    let icon = Icon::from_rgba(rgba, width, height).expect("Failed to build tray icon");

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Rust+ Companion")
        .with_icon_as_template(true)
        .with_icon(icon)
        .build()
        .expect("Failed to build system tray");

    (
        tray,
        TrayMenu {
            dashboard_id,
            quit_id,
        },
    )
}
