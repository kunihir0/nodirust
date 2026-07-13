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

    let dashboard_id = dashboard_item.id().0.to_string();
    let quit_id = quit_item.id().0.to_string();

    let _ = tray_menu.append_items(&[
        &dashboard_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ]);

    // Create a simple 2x2 solid color icon for the tray
    let rgba = vec![
        255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
    ];
    let icon = Icon::from_rgba(rgba, 2, 2).expect("Failed to build tray icon");

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Rust+ Companion")
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
